//! Deliberately allocating UTF-8 oracle. Shares only the public facts model.
//! Intended for valid supported histories and targeted invalid fixtures, not as
//! a second production parser or an oracle for arbitrary malformed byte input.
use pokerforge::{HandFacts, PlayerFacts};

fn euros(text: &str) -> Result<u64, String> {
    let value = text.strip_suffix('€').ok_or("currency")?;
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() > 2
        || parts
            .iter()
            .any(|s| s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err("decimal".into());
    }
    let fraction = match parts.get(1) {
        None => 0,
        Some(s) if s.len() == 1 => s.parse::<u64>().map_err(|_| "fraction")? * 10,
        Some(s) if s.len() == 2 => s.parse().map_err(|_| "fraction")?,
        _ => return Err("precision".into()),
    };
    parts[0]
        .parse::<u64>()
        .map_err(|_| "amount")?
        .checked_mul(100)
        .and_then(|n| n.checked_add(fraction))
        .ok_or("overflow".into())
}

fn board(text: &str) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    for group in text.split('[').skip(1) {
        let (cards, _) = group.split_once(']').ok_or("bracket")?;
        for card in cards.split_whitespace() {
            let mut chars = card.chars();
            let rank = chars.next().ok_or("rank")?;
            let suit = chars.next().ok_or("suit")?;
            if chars.next().is_some() {
                return Err("card".into());
            }
            let rank = "23456789TJQKA".find(rank).ok_or("rank")?;
            let suit = "cdhs".find(suit).ok_or("suit")?;
            let code = (1 + rank * 4 + suit) as u8;
            if result.contains(&code) {
                return Err("duplicate card".into());
            }
            result.push(code);
        }
    }
    if result.len() > 5 {
        return Err("board size".into());
    }
    Ok(result)
}

pub fn parse(text: &str) -> Result<HandFacts, String> {
    let rows: Vec<String> = text
        .lines()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    let header = rows.first().ok_or("header")?;
    if !header.starts_with("Winamax Poker - CashGame - ")
        && !header.starts_with("Winamax Poker - ESCAPE \"")
    {
        return Err("format".into());
    }
    let (_, tail) = header.split_once("HandId: #").ok_or("id")?;
    let (id, tail) = tail.split_once(" - Holdem no limit (").ok_or("game")?;
    let ids: Vec<u64> = id
        .split('-')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| "id")?;
    if ids.len() != 3 {
        return Err("id components".into());
    }
    let (blinds, date) = tail.split_once(") - ").ok_or("blinds")?;
    if !date.ends_with(" UTC") {
        return Err("date".into());
    }
    let (sb, bb) = blinds.split_once('/').ok_or("blinds")?;
    // FNV-1a is the public hand-id representation; no production helper reused.
    let mut digest = 14695981039346656037u64;
    for byte in id.bytes() {
        digest ^= u64::from(byte);
        digest = digest.wrapping_mul(1099511628211);
    }
    let table = rows.get(1).ok_or("table")?;
    let button = table
        .rsplit_once("Seat #")
        .ok_or("button")?
        .1
        .strip_suffix(" is the button")
        .ok_or("button")?
        .parse::<u8>()
        .map_err(|_| "button")?;
    if !(1..=10).contains(&button) {
        return Err("button range".into());
    }
    let mut facts = HandFacts {
        hand_id: digest,
        timestamp: ids[2],
        small_blind: euros(sb)?,
        big_blind: euros(bb)?,
        button_seat: button,
        ..Default::default()
    };
    let mut names = Vec::new();
    let mut current_board = Vec::new();
    let mut preflop = false;
    let mut seats_open = true;
    let mut started = false;
    let mut summary = false;
    let mut total_seen = false;
    let mut board_seen = false;
    let mut raises = 0;
    for line in rows.iter().skip(2) {
        if line == "*** ANTE/BLINDS ***" {
            seats_open = false;
            continue;
        }
        if line == "*** PRE-FLOP ***" {
            preflop = true;
            started = true;
            continue;
        }
        if line == "*** SUMMARY ***" {
            summary = true;
            preflop = false;
            continue;
        }
        if ["*** FLOP ***", "*** TURN ***", "*** RIVER ***"]
            .iter()
            .any(|p| line.starts_with(p))
        {
            let next = board(line)?;
            let count = if line.starts_with("*** FLOP") {
                3
            } else if line.starts_with("*** TURN") {
                4
            } else {
                5
            };
            if !started
                || next.len() != count
                || (count == 3 && !current_board.is_empty())
                || (count > 3 && current_board.len() != count - 1)
                || !next.starts_with(&current_board)
            {
                return Err("board progression".into());
            }
            current_board = next;
            preflop = false;
            continue;
        }
        if line == "*** SHOW DOWN ***" {
            preflop = false;
            continue;
        }
        if summary {
            if let Some(pot) = line.strip_prefix("Total pot ") {
                if total_seen {
                    return Err("duplicate total".into());
                }
                let (amount, rake) = pot.split_once(" | ").ok_or("total")?;
                euros(amount)?;
                if rake != "No rake" {
                    euros(rake.strip_prefix("Rake ").ok_or("rake")?)?;
                }
                total_seen = true;
            } else if line.starts_with("Board: ") {
                if board_seen || board(line)? != current_board {
                    return Err("summary board".into());
                }
                board_seen = true;
            } else if !line.starts_with("Seat ") {
                return Err("summary line".into());
            }
            continue;
        }
        if seats_open {
            if let Some(amount) = line.strip_prefix("Escape to Pot: total ") {
                euros(amount)?;
                continue;
            }
            let (seat, rest) = line
                .strip_prefix("Seat ")
                .ok_or("seat")?
                .split_once(": ")
                .ok_or("seat")?;
            let seat: u8 = seat.parse().map_err(|_| "seat")?;
            let (name, stack) = rest.rsplit_once(" (").ok_or("stack")?;
            let stack = euros(stack.strip_suffix(')').ok_or("stack")?)?;
            let n = names.len();
            if n == 10
                || !(1..=10).contains(&seat)
                || name.is_empty()
                || names.iter().any(|s| s == name)
                || facts.players[..n].iter().any(|p| p.seat == seat)
            {
                return Err("duplicate seat/name".into());
            }
            names.push(name.to_owned());
            facts.players[n] = PlayerFacts {
                player_id: seat,
                seat,
                stack,
                ..Default::default()
            };
            continue;
        }
        if line.starts_with("Dealt to ") {
            continue;
        }
        // Tokenize at every possible actor/action boundary, then select the
        // longest exact seated name with a recognized action token.
        let mut candidates = Vec::new();
        for (offset, _) in line.match_indices(' ') {
            if let Some(index) = names.iter().position(|n| n == &line[..offset]) {
                let action = &line[offset + 1..];
                let verb = action.split_whitespace().next().ok_or("verb")?;
                if [
                    "posts",
                    "collected",
                    "shows",
                    "mucks",
                    "folds",
                    "checks",
                    "calls",
                    "bets",
                    "raises",
                ]
                .contains(&verb)
                {
                    candidates.push((offset, index, action));
                }
            }
        }
        let (_, index, action) = candidates
            .into_iter()
            .max_by_key(|c| c.0)
            .ok_or("actor/action")?;
        let action = action.strip_suffix(" and is all-in").unwrap_or(action);
        let tokens: Vec<_> = action.split_whitespace().collect();
        match tokens[0] {
            "posts" | "collected" | "shows" | "mucks" => continue,
            "folds" | "checks" if tokens.len() == 1 => {}
            "calls" | "bets" if tokens.len() == 2 => {
                euros(tokens[1])?;
                if preflop {
                    facts.players[index].vpip = true;
                }
            }
            "raises" if tokens.len() == 4 && tokens[2] == "to" => {
                euros(tokens[1])?;
                euros(tokens[3])?;
                if preflop {
                    raises += 1;
                    let p = &mut facts.players[index];
                    p.vpip = true;
                    p.pfr = true;
                    p.three_bet |= raises == 2;
                }
            }
            _ => return Err("action".into()),
        }
        if !started {
            return Err("action before preflop".into());
        }
        facts.actions += 1;
    }
    if !started
        || !summary
        || !total_seen
        || names.len() < 2
        || (!current_board.is_empty() && !board_seen)
    {
        return Err("incomplete".into());
    }
    facts.player_count = names.len() as u8;
    facts.board_len = current_board.len() as u8;
    facts.board[..current_board.len()].copy_from_slice(&current_board);
    Ok(facts)
}

/// Independent line-based framing, without calling the production scanner.
pub fn hands(text: &str) -> Vec<&str> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut starts = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.starts_with("Winamax Poker -") {
            starts.push(offset);
        }
        offset += line.len();
    }
    starts.push(text.len());
    starts.windows(2).map(|w| &text[w[0]..w[1]]).collect()
}
