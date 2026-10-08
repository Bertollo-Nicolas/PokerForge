use std::hint::black_box;

/// Borrowed, newline-inclusive slices; no allocation or UTF-8 validation.
fn lines(mut input: &[u8]) -> impl Iterator<Item = &[u8]> {
    std::iter::from_fn(move || {
        if input.is_empty() {
            return None;
        }
        let end = memchr::memchr(b'\n', input).map_or(input.len(), |i| i + 1);
        let (line, rest) = input.split_at(end);
        input = rest;
        Some(line)
    })
}
pub const HEADER: &[u8] = b"Winamax Poker - CashGame - HandId: #";
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerFacts {
    pub stack: u64,
    pub player_id: u8,
    pub seat: u8,
    pub vpip: bool,
    pub pfr: bool,
    pub three_bet: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HandFacts {
    pub hand_id: u64,
    pub timestamp: u64,
    pub small_blind: u64,
    pub big_blind: u64,
    pub actions: u32,
    pub player_count: u8,
    pub button_seat: u8,
    pub board: [u8; 5],
    pub board_len: u8,
    pub players: [PlayerFacts; 10],
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub hands: u64,
    pub players: u64,
    pub actions: u64,
    pub errors: u64,
    pub checksum: u64,
}
impl Stats {
    fn record(&mut self, result: Result<HandFacts, &'static str>) {
        match result {
            Ok(h) => {
                let h = black_box(h);
                self.hands += 1;
                self.players += h.player_count as u64;
                self.actions += h.actions as u64;
                self.checksum = self.checksum.wrapping_add(checksum(&h));
            }
            Err(_) => self.errors += 1,
        }
    }
    pub fn merge(&mut self, s: Self) {
        self.hands += s.hands;
        self.players += s.players;
        self.actions += s.actions;
        self.errors += s.errors;
        self.checksum = self.checksum.wrapping_add(s.checksum);
    }
}
fn split<'a>(s: &'a [u8], needle: &[u8]) -> Result<(&'a [u8], &'a [u8]), &'static str> {
    assert!(!needle.is_empty());
    let p = memchr::memmem::find(s, needle).ok_or("missing delimiter")?;
    Ok((&s[..p], &s[p + needle.len()..]))
}
fn number(s: &[u8]) -> Result<u64, &'static str> {
    if s.is_empty() {
        return Err("empty number");
    }
    s.iter().try_fold(0u64, |a, &b| {
        if !b.is_ascii_digit() {
            return Err("invalid number");
        }
        a.checked_mul(10)
            .and_then(|v| v.checked_add((b - b'0') as u64))
            .ok_or("overflow")
    })
}
fn money(s: &[u8]) -> Result<u64, &'static str> {
    let s = s.strip_suffix("€".as_bytes()).ok_or("expected EUR")?;
    let (whole, frac) = match s.iter().position(|&b| b == b'.') {
        Some(i) => {
            if i + 1 == s.len() {
                return Err("empty fraction");
            }
            (&s[..i], &s[i + 1..])
        }
        None => (s, &b""[..]),
    };
    let cents = match frac.len() {
        0 => 0,
        1 => number(frac)? * 10,
        2 => number(frac)?,
        _ => return Err("precision"),
    };
    number(whole)?
        .checked_mul(100)
        .and_then(|n| n.checked_add(cents))
        .ok_or("overflow")
}
fn hash(s: &[u8]) -> u64 {
    s.iter().fold(0xcbf29ce484222325u64, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    })
}
fn cards(s: &[u8]) -> Result<([u8; 5], u8), &'static str> {
    let mut out = [0; 5];
    let mut n = 0;
    for group in s.split(|&b| b == b'[').skip(1) {
        let (part, _) = split(group, b"]")?;
        for c in part.split(|&b| b == b' ') {
            if c.len() != 2 || n == 5 {
                return Err("invalid board");
            }
            let r = b"23456789TJQKA"
                .iter()
                .position(|&b| b == c[0])
                .ok_or("rank")?;
            let suit = b"cdhs".iter().position(|&b| b == c[1]).ok_or("suit")?;
            let id = (r * 4 + suit + 1) as u8;
            if out[..n].contains(&id) {
                return Err("duplicate board card");
            }
            out[n] = id;
            n += 1;
        }
    }
    Ok((out, n as u8))
}
// Used only when two seated names match the same line.
fn action_verb(action: &[u8]) -> bool {
    matches!(
        action.split(|&b| b == b' ').next().unwrap_or_default(),
        b"folds"
            | b"checks"
            | b"calls"
            | b"bets"
            | b"raises"
            | b"posts"
            | b"shows"
            | b"mucks"
            | b"collected"
    )
}
pub fn parse_hand(input: &[u8]) -> Result<HandFacts, &'static str> {
    let mut lines = lines(input).map(|l| l.trim_ascii());
    let line = lines.next().ok_or("header")?;
    let is_escape = line.starts_with(b"Winamax Poker - ESCAPE \"");
    let header = if let Some(header) = line.strip_prefix(HEADER) {
        header
    } else if let Some(escape) = line.strip_prefix(b"Winamax Poker - ESCAPE \"") {
        let (name, header) = split(escape, b"\" - HandId: #")?;
        if name.is_empty() {
            return Err("empty ESCAPE name");
        }
        header
    } else {
        return Err("unsupported format");
    };
    let (id, rest) = split(header, b" - Holdem no limit (")?;
    let mut ids = id.split(|&b| b == b'-');
    number(ids.next().ok_or("id")?)?;
    number(ids.next().ok_or("id")?)?;
    let timestamp = number(ids.next().ok_or("timestamp")?)?;
    if ids.next().is_some() {
        return Err("id");
    }
    let (blinds, date) = split(rest, b") - ")?;
    if !date.ends_with(b" UTC") {
        return Err("date");
    }
    let (sb, bb) = split(blinds, b"/")?;
    let mut h = HandFacts {
        hand_id: hash(id),
        timestamp,
        small_blind: money(sb)?,
        big_blind: money(bb)?,
        ..Default::default()
    };
    let table = lines.next().ok_or("table")?;
    if !table.starts_with(b"Table: '") {
        return Err("table");
    }
    let (_, button) = split(table, b" Seat #")?;
    h.button_seat = u8::try_from(number(
        button.strip_suffix(b" is the button").ok_or("button")?,
    )?)
    .map_err(|_| "button")?;
    let mut names: [&[u8]; 10] = [&[]; 10];
    let mut state = 0;
    let mut raises = 0;
    let mut total = false;
    let mut summary_board = false;
    for l in lines {
        if l.is_empty() {
            continue;
        }
        if l == b"*** ANTE/BLINDS ***" {
            if state != 0 || h.player_count < 2 {
                return Err("seats");
            }
            state = 1;
            continue;
        }
        if l == b"*** PRE-FLOP ***" {
            if state != 1 {
                return Err("preflop order");
            }
            state = 2;
            continue;
        }
        if l.starts_with(b"*** FLOP ***")
            || l.starts_with(b"*** TURN ***")
            || l.starts_with(b"*** RIVER ***")
        {
            let expected = if l.starts_with(b"*** FLOP") {
                3
            } else if l.starts_with(b"*** TURN") {
                4
            } else {
                5
            };
            if state != 2 && state != 3 {
                return Err("street order");
            }
            let (board, n) = cards(l)?;
            if n != expected
                || (n == 3 && h.board_len != 0)
                || (n > 3 && h.board_len != n - 1)
                || board[..h.board_len as usize] != h.board[..h.board_len as usize]
            {
                return Err("board progression");
            }
            h.board = board;
            h.board_len = n;
            state = 3;
            continue;
        }
        if l == b"*** SHOW DOWN ***" {
            if !(2..=3).contains(&state) {
                return Err("showdown order");
            }
            state = 4;
            continue;
        }
        if l == b"*** SUMMARY ***" {
            if !(2..=4).contains(&state) {
                return Err("summary order");
            }
            state = 5;
            continue;
        }
        if state == 0 && is_escape && l.starts_with(b"Escape to Pot: total ") {
            money(&l[b"Escape to Pot: total ".len()..])?;
            continue;
        }
        if state == 0 {
            let seat = l.strip_prefix(b"Seat ").ok_or("seat")?;
            let (seat, rest) = split(seat, b": ")?;
            let p = rest.windows(2).rposition(|w| w == b" (").ok_or("stack")?;
            let name = &rest[..p];
            let stack = money(rest[p + 2..].strip_suffix(b")").ok_or("stack")?)?;
            let seat = u8::try_from(number(seat)?).map_err(|_| "seat")?;
            let n = h.player_count as usize;
            if n == 10
                || seat == 0
                || seat > 10
                || name.is_empty()
                || h.players[..n].iter().any(|p| p.seat == seat)
                || names[..n].contains(&name)
            {
                return Err("duplicate/invalid seat");
            }
            names[n] = name;
            h.players[n] = PlayerFacts {
                stack,
                seat,
                player_id: seat,
                ..Default::default()
            };
            h.player_count += 1;
            continue;
        }
        if state == 5 {
            if let Some(pot) = l.strip_prefix(b"Total pot ") {
                let (amount, rake) = split(pot, b" | ")?;
                money(amount)?;
                if rake != b"No rake" {
                    money(rake.strip_prefix(b"Rake ").ok_or("rake")?)?;
                }
                if total {
                    return Err("duplicate total");
                }
                total = true;
            } else if l.starts_with(b"Board: ") {
                let (b, n) = cards(l)?;
                if summary_board || b != h.board || n != h.board_len {
                    return Err("summary board mismatch");
                }
                summary_board = true;
            } else if !l.starts_with(b"Seat ") {
                return Err("unknown summary line");
            }
            continue;
        }
        if state == 1 && l.starts_with(b"Dealt to ") {
            continue;
        }
        let mut actor: Option<(usize, &[u8])> = None;
        let mut longest = 0;
        for (i, name) in names[..h.player_count as usize].iter().enumerate() {
            let len = name.len();
            if l.get(len) == Some(&b' ') && l.starts_with(name) {
                let action = &l[len + 1..];
                if let Some((_, previous)) = actor
                    && (!action_verb(action) || (action_verb(previous) && len <= longest))
                {
                    continue;
                }
                actor = Some((i, action));
                longest = len;
            }
        }
        let (idx, a) = actor.ok_or("unknown actor/line")?;
        if let Some(a) = a.strip_prefix(b"posts ") {
            if state != 1 {
                return Err("post order");
            }
            let a = a
                .strip_prefix(b"small blind ")
                .or_else(|| a.strip_prefix(b"big blind "))
                .or_else(|| a.strip_prefix(b"ante "))
                .ok_or("post")?;
            let a = a.strip_suffix(b" out of position").unwrap_or(a);
            let a = a.strip_suffix(b" and is all-in").unwrap_or(a);
            money(a)?;
            continue;
        }
        if a.starts_with(b"collected ") {
            let (amount, _) = split(&a[10..], b" from ")?;
            money(amount)?;
            continue;
        }
        if a.starts_with(b"shows [") || a == b"mucks" {
            continue;
        }
        if state != 2 && state != 3 {
            return Err("action order");
        }
        let a = a.strip_suffix(b" and is all-in").unwrap_or(a);
        let p = &mut h.players[idx];
        if a == b"folds" || a == b"checks" {
        } else if let Some(amount) = a
            .strip_prefix(b"calls ")
            .or_else(|| a.strip_prefix(b"bets "))
        {
            money(amount)?;
            if state == 2 {
                p.vpip = true;
            }
        } else if let Some(amount) = a.strip_prefix(b"raises ") {
            let (by, to) = split(amount, b" to ")?;
            money(by)?;
            money(to)?;
            if state == 2 {
                p.vpip = true;
                p.pfr = true;
                raises += 1;
                if raises == 2 {
                    p.three_bet = true;
                }
            }
        } else {
            return Err("unknown action");
        }
        h.actions = h.actions.checked_add(1).ok_or("too many actions")?;
    }
    if state != 5
        || !total
        || (h.board_len > 0 && !summary_board)
        || h.button_seat == 0
        || h.button_seat > 10
    {
        return Err("incomplete hand");
    }
    Ok(h)
}
fn checksum(h: &HandFacts) -> u64 {
    let mut x = h.hand_id;
    let mut add = |n: u64| {
        x = (x ^ n).wrapping_mul(0x100000001b3);
    };
    for n in [
        h.timestamp,
        h.small_blind,
        h.big_blind,
        h.actions as u64,
        h.player_count as u64,
        h.button_seat as u64,
        h.board_len as u64,
    ] {
        add(n);
    }
    for &c in &h.board {
        add(c as u64);
    }
    for p in &h.players[..h.player_count as usize] {
        for n in [
            p.stack,
            p.player_id as u64,
            p.seat as u64,
            p.vpip as u64,
            p.pfr as u64,
            p.three_bet as u64,
        ] {
            add(n);
        }
    }
    x
}
/// Boundaries are recognized only at the start of a line. No per-hand allocations.
pub fn scan(input: &[u8], mut consume: impl FnMut(usize, Result<HandFacts, &'static str>)) {
    let input = input.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(input);
    let mut start = None;
    let mut offset = 0;
    for line in lines(input) {
        if line.starts_with(b"Winamax Poker -") {
            if let Some(s) = start {
                consume(s, parse_hand(&input[s..offset]));
            }
            start = Some(offset);
        } else if start.is_none() && !line.trim_ascii().is_empty() {
            consume(offset, Err("text outside hand"));
        }
        offset += line.len();
    }
    if let Some(s) = start {
        consume(s, parse_hand(&input[s..]));
    }
}
pub fn process(input: &[u8]) -> Stats {
    let mut s = Stats::default();
    scan(input, |_, result| s.record(result));
    s
}

pub const DEFAULT_MAX_HAND_BYTES: usize = 8 * 1024 * 1024;

/// Incremental input, with an 8 MiB limit on each raw hand/preamble.
/// Offsets follow scan(): relative to content after the initial UTF-8 BOM.
pub fn scan_reader(
    reader: impl std::io::Read,
    chunk_bytes: usize,
    consume: impl FnMut(u64, Result<HandFacts, &'static str>),
) -> std::io::Result<u64> {
    scan_reader_limited(reader, chunk_bytes, DEFAULT_MAX_HAND_BYTES, consume)
}

/// The limit includes raw whitespace/BOM. Oversize input is a fatal I/O error;
/// no hand is truncated or silently discarded. Callback results before it are partial.
pub fn scan_reader_limited(
    mut reader: impl std::io::Read,
    chunk_bytes: usize,
    max_hand_bytes: usize,
    mut consume: impl FnMut(u64, Result<HandFacts, &'static str>),
) -> std::io::Result<u64> {
    use std::io::{Error, ErrorKind};
    if chunk_bytes == 0 || max_hand_bytes == 0 {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "stream buffer and hand limit must be positive",
        ));
    }
    const BOUNDARY: &[u8] = b"\nWinamax Poker -";
    let capacity_limit = max_hand_bytes
        .checked_add(BOUNDARY.len())
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "hand limit overflow"))?;
    let oversized = |offset: u64| {
        Error::new(
            ErrorKind::InvalidData,
            format!("hand/preamble exceeds max-hand-bytes={max_hand_bytes} at raw byte {offset}"),
        )
    };
    let finder = memchr::memmem::Finder::new(BOUNDARY);
    let mut buffer = Vec::new();
    let mut used = 0usize;
    let mut consumed = 0u64;
    let mut bom = 0u64;
    loop {
        let read_size = chunk_bytes.min(capacity_limit - used);
        if read_size == 0 {
            return Err(oversized(consumed));
        }
        let end = used + read_size;
        if end > buffer.len() {
            buffer
                .try_reserve_exact(end - buffer.len())
                .map_err(|e| Error::other(e.to_string()))?;
        }
        buffer.resize(end, 0);
        let n = loop {
            match reader.read(&mut buffer[used..end]) {
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                result => break result?,
            }
        };
        if n == 0 {
            if used > max_hand_bytes {
                return Err(oversized(consumed));
            }
            if consumed == 0 && buffer[..used].starts_with(&[0xef, 0xbb, 0xbf]) {
                bom = 3;
            }
            scan(&buffer[..used], |offset, result| {
                consume(consumed.saturating_sub(bom) + offset as u64, result)
            });
            return Ok(consumed + used as u64);
        }
        let search_from = used.saturating_sub(BOUNDARY.len() - 1);
        used += n;
        // Scan only newly read bytes plus the boundary overlap, even for a huge hand.
        let mut split = 0;
        for boundary in finder.find_iter(&buffer[search_from..used]) {
            let next = search_from + boundary + 1;
            if next - split > max_hand_bytes {
                return Err(oversized(consumed + split as u64));
            }
            split = next;
        }
        if used - split >= capacity_limit {
            return Err(oversized(consumed + split as u64));
        }
        if split > 0 {
            if consumed == 0 && buffer[..used].starts_with(&[0xef, 0xbb, 0xbf]) {
                bom = 3;
            }
            scan(&buffer[..split], |offset, result| {
                consume(consumed.saturating_sub(bom) + offset as u64, result)
            });
            buffer.copy_within(split..used, 0);
            used -= split;
            consumed += split as u64;
        }
    }
}

pub fn process_reader(
    reader: impl std::io::Read,
    chunk_bytes: usize,
) -> std::io::Result<(Stats, u64)> {
    process_reader_limited(reader, chunk_bytes, DEFAULT_MAX_HAND_BYTES)
}

pub fn process_reader_limited(
    reader: impl std::io::Read,
    chunk_bytes: usize,
    max_hand_bytes: usize,
) -> std::io::Result<(Stats, u64)> {
    let mut stats = Stats::default();
    let bytes = scan_reader_limited(reader, chunk_bytes, max_hand_bytes, |_, result| {
        stats.record(result)
    })?;
    Ok((stats, bytes))
}

/// Partition without splitting hands. Ranges cover every byte, including junk/BOM.
/// A hand larger than the target remains intact; target is a soft lower bound.
pub fn hand_blocks(input: &[u8], target: usize) -> Vec<std::ops::Range<usize>> {
    assert!(target > 0);
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut offset = 0;
    for line in lines(input) {
        if offset - start >= target && line.starts_with(b"Winamax Poker -") {
            ranges.push(start..offset);
            start = offset;
        }
        offset += line.len();
    }
    if start < input.len() {
        ranges.push(start..input.len());
    }
    ranges
}

/// Scoped workers borrow the original buffer; only block ranges are allocated.
pub fn process_blocks(
    input: &[u8],
    target: usize,
    threads: usize,
    repeat: usize,
) -> (Stats, usize) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    assert!(threads > 0 && repeat > 0);
    let blocks = hand_blocks(input, target);
    let workers = threads.min(blocks.len()).max(1);
    if workers == 1 {
        let mut stats = Stats::default();
        for _ in 0..repeat {
            stats.merge(process(input));
        }
        return (stats, workers);
    }
    let next = AtomicUsize::new(0);
    let stats = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut stats = Stats::default();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(range) = blocks.get(i) else {
                            break;
                        };
                        for _ in 0..repeat {
                            stats.merge(process(&input[range.clone()]));
                        }
                    }
                    stats
                })
            })
            .collect();
        let mut stats = Stats::default();
        for h in handles {
            stats.merge(h.join().expect("block worker panicked"));
        }
        stats
    });
    (stats, workers)
}

#[cfg(test)]
mod line_tests {
    #[test]
    fn separators_match_scalar_reference() {
        let needles: &[&[u8]] = &[
            b"/",
            b"]",
            b": ",
            b" to ",
            b" | ",
            b" Seat #",
            b" - Holdem no limit (",
            b"aaaa",
            b"\xff\x00",
        ];
        let mut state = 123u64;
        for len in 0..256 {
            let mut input: Vec<u8> = (0..len)
                .map(|_| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    (state >> 32) as u8
                })
                .collect();
            for &needle in needles {
                for position in [0, input.len() / 2, input.len()] {
                    let mut present = input.clone();
                    present.splice(position..position, needle.iter().copied());
                    for bytes in [&input[..], &present[..], &needle[..needle.len() - 1]] {
                        let expected = bytes
                            .windows(needle.len())
                            .position(|w| w == needle)
                            .map(|i| (&bytes[..i], &bytes[i + needle.len()..]))
                            .ok_or("missing delimiter");
                        assert_eq!(super::split(bytes, needle), expected);
                    }
                }
            }
            input.fill(b'a');
            let expected = if len >= 4 {
                Ok((&input[..0], &input[4..]))
            } else {
                Err("missing delimiter")
            };
            assert_eq!(super::split(&input, b"aaaa"), expected);
        }
    }
    #[test]
    fn optimized_lines_match_standard_split() {
        let mut seed = 1u64;
        for len in 0..1024 {
            let bytes: Vec<u8> = (0..len)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    if seed >> 60 == 0 {
                        b'\n'
                    } else {
                        (seed >> 32) as u8
                    }
                })
                .collect();
            let expected: Vec<_> = bytes.split_inclusive(|&b| b == b'\n').collect();
            assert_eq!(super::lines(&bytes).collect::<Vec<_>>(), expected);
        }
        for bytes in [
            b"".as_slice(),
            b"\n",
            b"\n\n",
            b"abc",
            b"abc\r\n",
            b"abc\nlast",
        ] {
            assert_eq!(
                super::lines(bytes).collect::<Vec<_>>(),
                bytes.split_inclusive(|&b| b == b'\n').collect::<Vec<_>>()
            );
        }
    }
}
