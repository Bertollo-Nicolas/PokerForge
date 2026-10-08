#[path = "support/reference.rs"]
mod reference;
use pokerforge::{HandFacts, PlayerFacts, parse_hand};

fn player(seat: u8, stack: u64, vpip: bool, pfr: bool, three_bet: bool) -> PlayerFacts {
    PlayerFacts {
        seat,
        player_id: seat,
        stack,
        vpip,
        pfr,
        three_bet,
    }
}
fn expected(actions: u32, button: u8, players: &[PlayerFacts], board: &[u8]) -> HandFacts {
    let mut facts = HandFacts {
        hand_id: 17063255705727781921,
        timestamp: 1781953322,
        small_blind: 1,
        big_blind: 2,
        actions,
        button_seat: button,
        player_count: players.len() as u8,
        board_len: board.len() as u8,
        ..Default::default()
    };
    facts.players[..players.len()].copy_from_slice(players);
    facts.board[..board.len()].copy_from_slice(board);
    facts
}
fn hand(seats: &str, posts: &str, actions: &str) -> String {
    format!(
        "Winamax Poker - CashGame - HandId: #1-2-1781953322 - Holdem no limit (0.01€/0.02€) - 2026/06/20 11:02:02 UTC\nTable: 'Oracle' 6-max (real money) Seat #2 is the button\n{seats}\n*** ANTE/BLINDS ***\n{posts}\n*** PRE-FLOP ***\n{actions}\n*** SUMMARY ***\nTotal pot 0.03€ | No rake\n"
    )
}
fn cases() -> Vec<(&'static str, String, HandFacts)> {
    let seats = "Seat 2: A (2€)\nSeat 6: B (2.50€)";
    let posts = "A posts small blind 0.01€\nB posts big blind 0.02€";
    let mut cases = vec![
        (
            "heads-up walk and empty seats",
            hand(seats, posts, "A folds\nB collected 0.03€ from pot"),
            expected(
                1,
                2,
                &[
                    player(2, 200, false, false, false),
                    player(6, 250, false, false, false),
                ],
                &[],
            ),
        ),
        (
            "limp and check",
            hand(seats, posts, "A calls 0.01€\nB checks"),
            expected(
                2,
                2,
                &[
                    player(2, 200, true, false, false),
                    player(6, 250, false, false, false),
                ],
                &[],
            ),
        ),
        (
            "short all-in second raise event",
            hand(
                "Seat 2: A (2€)\nSeat 6: B (0.07€)",
                posts,
                "A raises 0.04€ to 0.06€\nB raises 0.01€ to 0.07€ and is all-in\nA calls 0.01€",
            ),
            expected(
                3,
                2,
                &[
                    player(2, 200, true, true, false),
                    player(6, 7, true, true, true),
                ],
                &[],
            ),
        ),
        (
            "four-bet does not mark opener as three-bettor",
            hand(
                seats,
                posts,
                "A raises 0.04€ to 0.06€\nB raises 0.14€ to 0.20€\nA raises 0.40€ to 0.60€\nB folds",
            ),
            expected(
                4,
                2,
                &[
                    player(2, 200, true, true, false),
                    player(6, 250, true, true, true),
                ],
                &[],
            ),
        ),
        (
            "ambiguous action word in name",
            hand(
                "Seat 6: A raises (2.50€)\nSeat 2: A (2€)",
                "A posts small blind 0.01€\nA raises posts big blind 0.02€",
                "A raises 0.04€ to 0.06€\nA raises calls 0.04€",
            ),
            expected(
                2,
                2,
                &[
                    player(6, 250, true, false, false),
                    player(2, 200, true, true, false),
                ],
                &[],
            ),
        ),
        (
            "postflop aggression does not set preflop flags",
            hand(
                seats,
                posts,
                "A calls 0.01€\nB checks\n*** FLOP *** [2c 7h As]\nA checks\nB bets 0.02€\nA folds",
            )
            .replace(
                "Total pot 0.03€ | No rake",
                "Total pot 0.04€ | No rake\nBoard: [2c 7h As]",
            ),
            expected(
                5,
                2,
                &[
                    player(2, 200, true, false, false),
                    player(6, 250, false, false, false),
                ],
                &[1, 23, 52],
            ),
        ),
        (
            "full board and split pot",
            include_str!("fixtures/split-pot.txt").to_owned(),
            expected(
                12,
                3,
                &[
                    player(1, 200, true, false, false),
                    player(2, 200, false, false, false),
                    player(3, 200, false, false, false),
                ],
                &[52, 46, 41, 39, 36],
            ),
        ),
    ];
    let escape = hand(seats, posts, "A folds")
        .replace("CashGame", "ESCAPE \"Synthetic\"")
        .replace(
            "*** ANTE/BLINDS ***",
            "Escape to Pot: total 0.10€\n*** ANTE/BLINDS ***",
        );
    cases.push((
        "escape pot excluded from actions",
        escape,
        expected(
            1,
            2,
            &[
                player(2, 200, false, false, false),
                player(6, 250, false, false, false),
            ],
            &[],
        ),
    ));
    cases
}

#[test]
fn independently_specified_facts() {
    for (name, text, expected) in cases() {
        for input in [
            text.clone(),
            text.replace('\n', "\r\n"),
            text.trim_end().to_owned(),
        ] {
            assert_eq!(
                reference::parse(&input).unwrap(),
                expected,
                "oracle: {name}"
            );
            assert_eq!(
                parse_hand(input.as_bytes()).unwrap(),
                expected,
                "fast: {name}"
            );
        }
    }
}

#[test]
fn targeted_invalid_histories_are_rejected_by_both() {
    let text = include_str!("fixtures/split-pot.txt");
    let invalid = [
        text.split("*** SUMMARY ***").next().unwrap().to_owned(),
        text.replace("Total pot 0.06€ | No rake", ""),
        text.replace("Seat 2: Bob (2€)", "Seat 1: Bob (2€)"),
        text.replace("Alice calls 0.01€", "Nobody calls 0.01€"),
        text.replace("Alice calls 0.01€", "Alice calls 0.001€"),
        text.replace("Alice calls 0.01€", "Alice calls 2.€"),
        text.replace("Alice calls 0.01€", "Alice dances"),
        text.replace("[As Kd Qc]", "[As As Qc]"),
        text.replace("Board: [As Kd Qc Jh Ts]", "Board: [As Kd Qc Jh 9s]"),
        text.replace("*** TURN *** [As Kd Qc][Jh]", "*** TURN *** [As Kd Qc][Ts]"),
    ];
    for (index, input) in invalid.iter().enumerate() {
        assert!(
            reference::parse(input).is_err(),
            "oracle invalid case {index}"
        );
        assert!(
            parse_hand(input.as_bytes()).is_err(),
            "fast invalid case {index}"
        );
    }
}

#[test]
fn independent_framing_and_private_corpus_facts() {
    // Synthetic framing always runs, even on machines without private data.
    let joined = format!(
        "\u{feff}{}",
        cases()
            .into_iter()
            .map(|(_, text, _)| text)
            .collect::<String>()
    );
    compare("synthetic", &joined);
    let root = std::path::Path::new("data");
    if !root.exists() {
        return;
    }
    let mut dirs = vec![root.to_owned()];
    let mut files = Vec::new();
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("txt"))
            {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut count = 0;
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        count += compare(&path.display().to_string(), &text);
    }
    eprintln!(
        "Independent oracle compared {count} hands in {} files",
        files.len()
    );
}
fn compare(label: &str, text: &str) -> usize {
    let inputs = reference::hands(text);
    let mut fast = Vec::new();
    pokerforge::scan(text.as_bytes(), |_, facts| fast.push(facts));
    assert_eq!(inputs.len(), fast.len(), "framing: {label}");
    for (index, (input, actual)) in inputs.iter().zip(fast).enumerate() {
        let oracle = reference::parse(input)
            .unwrap_or_else(|error| panic!("oracle {label} hand {index}: {error}"));
        assert_eq!(actual.unwrap(), oracle, "facts: {label} hand {index}");
    }
    inputs.len()
}
