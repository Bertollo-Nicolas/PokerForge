//! Phase benchmark, not a sampling CPU profiler. Setup and allocations are untimed.
use std::{fs, hint::black_box, path::Path, time::Instant};
struct Input {
    bytes: Vec<u8>,
    ranges: Vec<std::ops::Range<usize>>,
}
fn load(path: &Path, out: &mut Vec<Input>, skipped: &mut usize) -> std::io::Result<()> {
    if path.is_dir() {
        for e in fs::read_dir(path)? {
            let e = e?;
            if !e.file_type()?.is_symlink() {
                load(&e.path(), out, skipped)?;
            }
        }
    } else if path
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("txt"))
    {
        let raw = fs::read(path)?;
        let bytes = raw
            .strip_prefix(&[0xef, 0xbb, 0xbf])
            .unwrap_or(&raw)
            .to_vec();
        // Reject entire files with any error: never benchmark silent partial parsing.
        if pokerforge::process(&bytes).errors != 0 {
            *skipped += 1;
            return Ok(());
        }
        let mut starts = Vec::new();
        pokerforge::scan(&bytes, |offset, _| starts.push(offset));
        let ranges = starts
            .iter()
            .enumerate()
            .map(|(i, &s)| s..starts.get(i + 1).copied().unwrap_or(bytes.len()))
            .collect();
        out.push(Input { bytes, ranges });
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).unwrap_or_else(|| "data".into());
    let mut inputs = Vec::new();
    let mut skipped = 0;
    load(Path::new(&path), &mut inputs, &mut skipped)?;
    if inputs.is_empty() {
        return Err("no valid files".into());
    }
    let bytes: usize = inputs.iter().map(|i| i.bytes.len()).sum();
    let hands: usize = inputs.iter().map(|i| i.ranges.len()).sum();
    println!(
        "Files: {}; excluded invalid/unsupported files: {skipped}; Bytes: {bytes}; Hands: {hands}",
        inputs.len()
    );
    println!("phase,run,seconds,MBps");
    // Rotate order to reduce systematic warm-up/thermal bias.
    for run in 0..5 {
        for turn in 0..3 {
            let phase = (run + turn) % 3;
            let start = Instant::now();
            for _ in 0..20 {
                for input in &inputs {
                    match phase {
                        0 => {
                            let mut n = 0u64;
                            for line in black_box(&input.bytes).split_inclusive(|&b| b == b'\n') {
                                if line.starts_with(b"Winamax Poker -") {
                                    n += 1;
                                }
                            }
                            black_box(n);
                        }
                        1 => {
                            for range in &input.ranges {
                                black_box(
                                    pokerforge::parse_hand(black_box(&input.bytes[range.clone()]))
                                        .unwrap(),
                                );
                            }
                        }
                        _ => {
                            black_box(pokerforge::process(black_box(&input.bytes)));
                        }
                    }
                }
            }
            let seconds = start.elapsed().as_secs_f64();
            println!(
                "{},{},{seconds:.6},{:.2}",
                [
                    "scalar_detection_reference",
                    "preindexed_parse_no_checksum",
                    "full_pipeline"
                ][phase],
                run + 1,
                bytes as f64 * 20.0 / 1e6 / seconds
            );
        }
    }
    Ok(())
}
