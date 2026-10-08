//! Reproducible mutation fuzzing; complements the coverage-guided libFuzzer target.
use pokerforge::{process, process_reader, scan, scan_reader};
fn main() {
    let cases = std::env::args()
        .nth(1)
        .map(|s| s.parse::<usize>().expect("case count"))
        .unwrap_or(20_000);
    let seed = include_bytes!("../tests/fixtures/split-pot.txt");
    let mut state = 0x504f4b4552u64;
    let mut random = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        state
    };
    for case in 0..cases {
        let mut data = seed.to_vec();
        for _ in 0..1 + random() % 12 {
            let i = random() as usize % (data.len() + 1);
            match random() % 4 {
                0 if i < data.len() => data[i] = random() as u8,
                1 => data.truncate(i),
                2 => data.insert(i, random() as u8),
                _ => data.extend_from_slice(b"\nWinamax Poker -"),
            }
        }
        let size = 1 + (random() as usize % 257);
        let expected = process(&data);
        assert_eq!(
            process_reader(data.as_slice(), size).unwrap().0,
            expected,
            "case {case}"
        );
        let mut a = Vec::new();
        let mut b = Vec::new();
        scan(&data, |o, h| a.push((o as u64, h)));
        scan_reader(data.as_slice(), size, |o, h| b.push((o, h))).unwrap();
        assert_eq!(a, b, "case {case}");
    }
    println!("Mutation fuzzing: {cases} cases passed, fixed seed 0x504f4b4552");
}
