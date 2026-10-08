use pokerforge::{parse_hand, process, scan};
const HAND: &str = "Winamax Poker - CashGame - HandId: #1-2-1781953322 - Holdem no limit (0.01€/0.02€) - 2026/06/20 11:02:02 UTC\nTable: 'Test' 5-max (real money) Seat #1 is the button\nSeat 1: A (2€)\nSeat 2: A B (2.50€)\nSeat 3: C (1€)\n*** ANTE/BLINDS ***\nA posts small blind 0.01€\nA B posts big blind 0.02€\n*** PRE-FLOP ***\nC raises 0.04€ to 0.06€\nA raises 0.14€ to 0.20€\nA B folds\nC calls 0.14€\n*** FLOP *** [4s Kd Qs]\nA checks\nC bets 0.80€ and is all-in\nA folds\nC collected 0.42€ from pot\n*** SUMMARY ***\nTotal pot 0.42€ | No rake\nBoard: [4s Kd Qs]\nSeat 3: C won 0.42€\n";
#[test]
fn facts_and_prefix_names() {
    let h = parse_hand(HAND.as_bytes()).unwrap();
    assert_eq!(
        (
            h.player_count,
            h.actions,
            h.small_blind,
            h.big_blind,
            h.button_seat,
            h.board_len
        ),
        (3, 7, 1, 2, 1, 3)
    );
    assert_eq!(h.players[1].stack, 250);
    assert!(h.players[0].three_bet);
    assert!(h.players[0].pfr);
    assert!(h.players[2].pfr);
    assert!(!h.players[2].three_bet);
    assert!(!h.players[1].vpip);
}
#[test]
fn line_endings_and_bom() {
    let a = process(HAND.as_bytes());
    assert_eq!(
        a,
        process(format!("\u{feff}{}", HAND.replace('\n', "\r\n")).as_bytes())
    );
    assert_eq!(a, process(HAND.trim_end().as_bytes()));
}
#[test]
fn reject_corruption() {
    for text in [
        HAND.replace("2.50€", "2.501€"),
        HAND.replace("[4s Kd Qs]", "[4s 4s Qs]"),
        HAND.replace("A B folds", "Nobody folds"),
        HAND.replace("C calls 0.14€", "C dances"),
        HAND.replace("*** PRE-FLOP ***", "*** TURN *** [As]"),
        HAND.replace("Total pot 0.42€ | No rake", ""),
        HAND.replace("Seat 3: C (1€)", "Seat 1: C (1€)"),
    ] {
        assert!(parse_hand(text.as_bytes()).is_err(), "{text}");
    }
}
#[test]
fn recovery() {
    let bad = HAND.split("*** SUMMARY ***").next().unwrap();
    let s = process(format!("{bad}{HAND}").as_bytes());
    assert_eq!((s.hands, s.errors), (1, 1));
}
#[test]
fn forced_blinds_are_not_vpip() {
    let text = HAND
        .replace("C raises 0.04€ to 0.06€", "C folds")
        .replace("A raises 0.14€ to 0.20€", "A calls 0.01€")
        .replace("C calls 0.14€", "A B checks");
    let h = parse_hand(text.as_bytes()).unwrap();
    assert!(h.players[0].vpip);
    assert!(!h.players[0].pfr);
    assert!(!h.players[1].vpip);
    assert!(!h.players[2].vpip);
}
#[test]
fn supplied_corpus_if_available() {
    let Ok(bytes) = std::fs::read("data/20260620_Aalen 12_real_holdem_no-limit.txt") else {
        return;
    };
    let s = process(&bytes);
    assert_eq!(
        (s.hands, s.players, s.actions, s.errors),
        (269, 1316, 2275, 0)
    );
    let mut first = None;
    scan(&bytes, |_, h| {
        if first.is_none() {
            first = Some(h.unwrap());
        }
    });
    let h = first.unwrap();
    assert_eq!(h.actions, 13);
    assert_eq!(h.players[0].stack, 267);
    assert!(h.players[0].vpip);
    assert!(h.players[4].vpip);
    assert!(!h.players[1].vpip);
    assert_eq!(h.board_len, 5);
}

#[test]
fn arbitrary_truncation_does_not_panic() {
    for end in 0..HAND.len() {
        let _ = process(&HAND.as_bytes()[..end]);
    }
    for index in 0..HAND.len() {
        let mut bytes = HAND.as_bytes().to_vec();
        bytes[index] = 0xff;
        let _ = process(&bytes);
    }
}

#[test]
fn cli_parallel_equivalence() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("cli-test-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("nested")).unwrap();
    std::fs::write(dir.join("a.txt"), HAND).unwrap();
    std::fs::write(dir.join("nested/b.TXT"), HAND.replace("#1-2-", "#1-3-")).unwrap();
    std::fs::write(dir.join("ignored.csv"), "invalid").unwrap();
    let mut expected = None;
    for memory in [false, true] {
        for threads in [1, 2, 8] {
            let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_pokerforge"));
            cmd.arg(&dir)
                .args(["--repeat", "3", "--threads", &threads.to_string()]);
            if memory {
                cmd.arg("--memory");
            }
            let output = cmd.output().unwrap();
            assert!(output.status.success(), "{:?}", output);
            let text = String::from_utf8(output.stdout).unwrap();
            let fields: Vec<_> = text
                .lines()
                .filter(|l| {
                    [
                        "Files:",
                        "Bytes:",
                        "Hands:",
                        "Players:",
                        "Actions:",
                        "Parse errors:",
                        "Checksum:",
                    ]
                    .iter()
                    .any(|p| l.starts_with(p))
                })
                .map(str::to_owned)
                .collect();
            assert!(text.contains("Hands: 6\n"));
            if let Some(ref e) = expected {
                assert_eq!(e, &fields);
            } else {
                expected = Some(fields);
            }
        }
    }
    std::fs::write(dir.join("bad.txt"), "broken input").unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pokerforge"))
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("text outside hand"));
    // Delete only the exact files this test created; never a recursive directory deletion.
    for path in ["a.txt", "nested/b.TXT", "ignored.csv", "bad.txt"] {
        std::fs::remove_file(dir.join(path)).unwrap();
    }
    std::fs::remove_dir(dir.join("nested")).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn escape_header_same_facts() {
    let escape = HAND.replace(
        "Winamax Poker - CashGame -",
        "Winamax Poker - ESCAPE \"Colorado\" -",
    );
    assert_eq!(parse_hand(HAND.as_bytes()), parse_hand(escape.as_bytes()));
    assert!(parse_hand(HAND.replace("CashGame", "Tournament").as_bytes()).is_err());
    assert!(parse_hand(HAND.replace("CashGame", "ESCAPE \"\"").as_bytes()).is_err());
}

#[test]
fn escape_pot_is_not_an_action() {
    let escape = HAND.replace("CashGame", "ESCAPE \"Colorado\"");
    let with_pot = escape.replace(
        "*** ANTE/BLINDS ***",
        "Escape to Pot: total 0.10€\n*** ANTE/BLINDS ***",
    );
    assert_eq!(
        parse_hand(escape.as_bytes()),
        parse_hand(with_pot.as_bytes())
    );
    assert!(parse_hand(with_pot.replace("total 0.10€", "total bad").as_bytes()).is_err());
    assert!(
        parse_hand(
            with_pot
                .replace("ESCAPE \"Colorado\"", "CashGame")
                .as_bytes()
        )
        .is_err()
    );
}

#[test]
fn blocks_preserve_all_bytes_facts_and_errors() {
    use pokerforge::{hand_blocks, process_blocks};
    let truncated = HAND.split("*** SUMMARY ***").next().unwrap();
    for text in [
        HAND.repeat(7),
        format!("\u{feff}junk\n{HAND}{truncated}{HAND}"),
        HAND.replace('\n', "\r\n").repeat(3),
        format!("noise Winamax Poker -\n{HAND}"),
        String::new(),
        "junk\n\n".into(),
    ] {
        let input = text.as_bytes();
        for target in [1, HAND.len() - 1, HAND.len(), HAND.len() + 1, 100000] {
            let ranges = hand_blocks(input, target);
            let reconstructed: Vec<_> = ranges
                .iter()
                .flat_map(|r| input[r.clone()].iter().copied())
                .collect();
            assert_eq!(reconstructed, input);
            for r in ranges.iter().skip(1) {
                assert_eq!(input[r.start - 1], b'\n');
                assert!(input[r.clone()].starts_with(b"Winamax Poker -"));
            }
            let mut expected = pokerforge::Stats::default();
            for _ in 0..2 {
                expected.merge(process(input));
            }
            assert_eq!(process_blocks(input, target, 4, 2).0, expected);
        }
    }
}

#[test]
fn cli_single_file_uses_multiple_block_workers() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("block-test-{}.txt", std::process::id()));
    std::fs::write(&path, HAND.repeat(8)).unwrap();
    let mut expected = None;
    for memory in [false, true] {
        for blocks in [false, true] {
            let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_pokerforge"));
            cmd.arg(&path).args(["--threads", "4", "--repeat", "2"]);
            if memory {
                cmd.arg("--memory");
            }
            if blocks {
                cmd.args(["--block-bytes", "1"]);
            }
            let out = cmd.output().unwrap();
            assert!(out.status.success());
            let text = String::from_utf8(out.stdout).unwrap();
            if blocks {
                assert!(text.contains("Workers: 4\n"));
            }
            let fields: Vec<_> = text
                .lines()
                .filter(|l| {
                    [
                        "Files:",
                        "Bytes:",
                        "Hands:",
                        "Players:",
                        "Actions:",
                        "Parse errors:",
                        "Checksum:",
                    ]
                    .iter()
                    .any(|p| l.starts_with(p))
                })
                .map(str::to_owned)
                .collect();
            if let Some(ref e) = expected {
                assert_eq!(e, &fields);
            } else {
                expected = Some(fields);
            }
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn golden_split_pot_all_facts() {
    use pokerforge::{HandFacts, PlayerFacts};
    let mut players = [PlayerFacts::default(); 10];
    for (i, p) in players[..3].iter_mut().enumerate() {
        *p = PlayerFacts {
            stack: 200,
            player_id: (i + 1) as u8,
            seat: (i + 1) as u8,
            vpip: i == 0,
            ..Default::default()
        };
    }
    let expected = HandFacts {
        hand_id: 17063255705727781921,
        timestamp: 1781953322,
        small_blind: 1,
        big_blind: 2,
        actions: 12,
        player_count: 3,
        button_seat: 3,
        board: [52, 46, 41, 39, 36],
        board_len: 5,
        players,
    };
    assert_eq!(
        parse_hand(include_bytes!("fixtures/split-pot.txt")).unwrap(),
        expected
    );
}

#[test]
fn incomplete_all_in_raise_counts_as_second_raise_event() {
    let text = include_str!("fixtures/split-pot.txt");
    let pre = text
        .split("*** PRE-FLOP ***")
        .next()
        .unwrap()
        .replace("Alice (2€)", "Alice (0.07€)");
    let runout = "*** PRE-FLOP ***\nCharlie raises 0.04€ to 0.06€\nAlice raises 0.01€ to 0.07€ and is all-in\nBob folds\nCharlie calls 0.01€\n*** FLOP *** [As Kd Qc]\n*** TURN *** [As Kd Qc][Jh]\n*** RIVER *** [As Kd Qc Jh][Ts]\n*** SHOW DOWN ***\nAlice shows [2c 3c] (Straight Ace high)\nCharlie shows [6c 7c] (Straight Ace high)\nAlice collected 0.08€ from pot\nCharlie collected 0.08€ from pot\n*** SUMMARY ***\nTotal pot 0.16€ | No rake\nBoard: [As Kd Qc Jh Ts]\nSeat 1: Alice won 0.08€\nSeat 3: Charlie won 0.08€\n";
    let h = parse_hand(format!("{pre}{runout}").as_bytes()).unwrap();
    assert_eq!(h.actions, 4);
    assert_eq!(h.players[0].stack, 7);
    assert!(h.players[0].three_bet && h.players[0].pfr && h.players[0].vpip);
    assert!(h.players[2].pfr && h.players[2].vpip && !h.players[2].three_bet);
    assert!(!h.players[1].vpip && !h.players[1].pfr && !h.players[1].three_bet);
}

fn assert_parallel_facts(input: &[u8]) {
    let mut expected = Vec::new();
    scan(input, |_, h| expected.push(h));
    let blocks = pokerforge::hand_blocks(input, 4096);
    let actual = std::thread::scope(|scope| {
        let handles: Vec<_> = blocks
            .chunks(blocks.len().div_ceil(4).max(1))
            .map(|group| {
                scope.spawn(move || {
                    let mut facts = Vec::new();
                    for range in group {
                        scan(&input[range.clone()], |_, h| facts.push(h));
                    }
                    facts
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(expected, actual);
}

#[test]
fn full_facts_match_across_parallel_boundaries() {
    let fixture = include_str!("fixtures/split-pot.txt");
    assert_parallel_facts(
        format!(
            "\u{feff}noise\n{}{}",
            fixture.repeat(50),
            HAND.split("*** SUMMARY ***").next().unwrap()
        )
        .as_bytes(),
    );
}

#[test]
fn private_corpus_full_facts_if_available() {
    let Ok(entries) = std::fs::read_dir("data") else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("txt"))
        {
            assert_parallel_facts(&std::fs::read(path).unwrap());
        }
    }
}

#[test]
fn reject_empty_fraction_and_overflow() {
    for amount in ["2.€", "18446744073709551616€", "184467440737095516.16€"] {
        assert!(parse_hand(HAND.replace("2.50€", amount).as_bytes()).is_err());
    }
}

#[test]
fn actor_longest_prefix_is_independent_of_seat_declaration_order() {
    for name in ["A raises", "A folds", "A checks", "A élan"] {
        let text = HAND.replace("A B", name);
        let expected = parse_hand(HAND.as_bytes()).unwrap();
        assert_eq!(parse_hand(text.as_bytes()).unwrap(), expected);
        let short = "Seat 1: A (2€)";
        let long = format!("Seat 2: {name} (2.50€)");
        let text = text.replace(&format!("{short}\n{long}"), &format!("{long}\n{short}"));
        let mut actual = parse_hand(text.as_bytes()).unwrap();
        actual.players.swap(0, 1);
        assert_eq!(actual, expected);
    }
}

#[test]
fn stream_matches_complete_facts_offsets_and_errors() {
    let bad = HAND.split("*** SUMMARY ***").next().unwrap();
    for text in [
        String::new(),
        "\u{feff}".into(),
        "junk\n\njunk".into(),
        format!(
            "\u{feff}noise\r\n{}{}{}",
            HAND,
            HAND.replace('\n', "\r\n"),
            bad
        ),
        HAND.repeat(3),
        HAND.trim_end().into(),
        format!("{HAND}Winamax Poker -"),
        format!("noise Winamax Poker -\n{HAND}"),
    ] {
        let input = text.as_bytes();
        let mut expected = Vec::new();
        scan(input, |o, h| expected.push((o as u64, h)));
        for size in [1, 2, 3, 7, 14, 15, 16, 31, 64, 511, 4096] {
            let mut actual = Vec::new();
            let bytes = pokerforge::scan_reader(input, size, |o, h| actual.push((o, h))).unwrap();
            assert_eq!(bytes, input.len() as u64);
            assert_eq!(actual, expected, "buffer size {size}");
            assert_eq!(
                pokerforge::process_reader(input, size).unwrap(),
                (process(input), input.len() as u64)
            );
        }
    }
    assert!(pokerforge::process_reader(HAND.as_bytes(), 0).is_err());
}

#[test]
fn stream_handles_short_interrupted_and_failed_reads() {
    struct Reader {
        data: std::io::Cursor<Vec<u8>>,
        calls: usize,
        fail: bool,
    }
    impl std::io::Read for Reader {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            self.calls += 1;
            if self.calls == 2 {
                return Err(std::io::ErrorKind::Interrupted.into());
            }
            if self.fail && self.calls == 5 {
                return Err(std::io::Error::other("injected read error"));
            }
            let end = out.len().min(3);
            self.data.read(&mut out[..end])
        }
    }
    let reader = |fail| Reader {
        data: std::io::Cursor::new(HAND.repeat(3).into_bytes()),
        calls: 0,
        fail,
    };
    assert_eq!(
        pokerforge::process_reader(reader(false), 1024).unwrap().0,
        process(HAND.repeat(3).as_bytes())
    );
    assert!(pokerforge::process_reader(reader(true), 1024).is_err());
}

#[test]
fn streaming_private_corpus_if_available() {
    let Ok(entries) = std::fs::read_dir("data") else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("txt"))
        {
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(
                pokerforge::process_reader(std::fs::File::open(path).unwrap(), 4096).unwrap(),
                (process(&bytes), bytes.len() as u64)
            );
        }
    }
}

#[test]
fn cli_streaming_matches_whole_files_and_rejects_conflicting_flags() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("stream-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["a.txt", "b.txt"] {
        std::fs::write(dir.join(name), HAND.repeat(4)).unwrap();
    }
    let mut expected = None;
    for stream in [false, true] {
        for threads in [1, 4] {
            let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_pokerforge"));
            cmd.arg(&dir)
                .args(["--repeat", "2", "--threads", &threads.to_string()]);
            if stream {
                cmd.args(["--stream-bytes", "17"]);
            }
            let output = cmd.output().unwrap();
            assert!(output.status.success());
            let text = String::from_utf8(output.stdout).unwrap();
            let fields: Vec<_> = text
                .lines()
                .filter(|l| {
                    [
                        "Files:",
                        "Bytes:",
                        "Hands:",
                        "Players:",
                        "Actions:",
                        "Parse errors:",
                        "Checksum:",
                    ]
                    .iter()
                    .any(|p| l.starts_with(p))
                })
                .map(str::to_owned)
                .collect();
            if let Some(ref e) = expected {
                assert_eq!(e, &fields);
            } else {
                expected = Some(fields);
            }
        }
    }
    for flags in [
        vec!["--stream-bytes", "0"],
        vec!["--stream-bytes", "17", "--memory"],
        vec!["--stream-bytes", "17", "--block-bytes", "17"],
    ] {
        assert!(
            !std::process::Command::new(env!("CARGO_BIN_EXE_pokerforge"))
                .arg(&dir)
                .args(flags)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    std::fs::write(dir.join("bad.txt"), "\u{feff}junk\n").unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_pokerforge"))
        .arg(&dir)
        .args(["--stream-bytes", "1"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("byte 0: text outside hand"));
    for name in ["a.txt", "b.txt", "bad.txt"] {
        std::fs::remove_file(dir.join(name)).unwrap();
    }
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn stream_retains_hands_larger_than_buffer() {
    let huge = HAND.replace(
        "C collected",
        &format!("C shows [Ac Kc] ({})\nC collected", "x".repeat(100_000)),
    );
    let text = format!("{huge}{HAND}");
    assert_eq!(
        pokerforge::process_reader(text.as_bytes(), 1024).unwrap().0,
        process(text.as_bytes())
    );
}

#[test]
fn bounded_stream_limits_include_complete_and_partial_records() {
    use pokerforge::process_reader_limited;
    let len = HAND.len();
    for size in [1, 7, len / 2, len, 2 * len, usize::MAX] {
        assert_eq!(
            process_reader_limited(HAND.as_bytes(), size, len)
                .unwrap()
                .0,
            process(HAND.as_bytes())
        );
        assert_eq!(
            process_reader_limited(HAND.repeat(3).as_bytes(), size, len)
                .unwrap()
                .0,
            process(HAND.repeat(3).as_bytes())
        );
        for text in [HAND.to_string(), HAND.repeat(3), "x".repeat(len + 100)] {
            let err = process_reader_limited(text.as_bytes(), size, len - 1).unwrap_err();
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
            assert!(err.to_string().contains("max-hand-bytes"));
        }
    }
    assert!(process_reader_limited(HAND.as_bytes(), 1, usize::MAX).is_err());
    assert!(process_reader_limited(HAND.as_bytes(), 1, 0).is_err());
}

#[test]
fn bounded_stream_stops_reading_hostile_input() {
    struct Infinite {
        count: usize,
    }
    impl std::io::Read for Infinite {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            out.fill(b'x');
            self.count += out.len();
            Ok(out.len())
        }
    }
    let mut reader = Infinite { count: 0 };
    assert_eq!(
        pokerforge::process_reader_limited(&mut reader, usize::MAX, 1024)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    assert!(reader.count <= 1024 + 32);
}

#[test]
fn cli_hand_limit_is_explicit_and_fatal() {
    let path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/split-pot.txt");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_pokerforge"))
        .arg(&path)
        .args(["--stream-bytes", "1024", "--max-hand-bytes", "64"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("max-hand-bytes=64"));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("Hands:"));
    for flags in [
        vec!["--max-hand-bytes", "64"],
        vec!["--stream-bytes", "1024", "--max-hand-bytes", "0"],
    ] {
        assert!(
            !std::process::Command::new(env!("CARGO_BIN_EXE_pokerforge"))
                .arg(&path)
                .args(flags)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}
