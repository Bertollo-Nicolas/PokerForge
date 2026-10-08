use pokerforge::{
    DEFAULT_MAX_HAND_BYTES, Stats, process, process_blocks, process_reader_limited, scan,
    scan_reader_limited,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
fn files(p: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if p.is_dir() {
        for e in fs::read_dir(p)? {
            let e = e?;
            if !e.file_type()?.is_symlink() {
                files(&e.path(), out)?;
            }
        }
    } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("txt")) {
        out.push(p.to_owned());
    }
    Ok(())
}
fn worker(
    paths: &[PathBuf],
    buffers: &[Vec<u8>],
    memory: bool,
    repeat: usize,
    next: &AtomicUsize,
    stream_bytes: Option<usize>,
    max_hand_bytes: usize,
) -> std::io::Result<(Stats, u64)> {
    let mut stats = Stats::default();
    let mut bytes = 0;
    loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= paths.len() {
            break;
        }
        for _ in 0..repeat {
            if let Some(size) = stream_bytes {
                let (s, n) =
                    process_reader_limited(fs::File::open(&paths[i])?, size, max_hand_bytes)
                        .map_err(|e| {
                            std::io::Error::new(e.kind(), format!("{}: {e}", paths[i].display()))
                        })?;
                stats.merge(s);
                bytes += n;
                continue;
            }
            let owned;
            let input = if memory {
                &buffers[i]
            } else {
                owned = fs::read(&paths[i])?;
                &owned
            };
            bytes += input.len() as u64;
            stats.merge(process(input));
        }
    }
    Ok((stats, bytes))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 2 || args[1] == "--help" {
        println!(
            "Usage: pokerforge PATH [--memory] [--repeat N] [--threads N] [--block-bytes N] [--stream-bytes N] [--max-hand-bytes N]\nDefault: single-thread, read + parse. PATH is a TXT file or recursive directory.\n--block-bytes opts into parallel blocks within each file (files processed sequentially).\n--stream-bytes reads reusable buffers; incompatible with --memory/--block-bytes. Threads distribute files. Default max hand/preamble: 8 MiB; exceeded limit is fatal.\nCounters and checksum include repetitions; Files counts unique paths. MB = 1,000,000 bytes."
        );
        return Ok(());
    }
    let mut memory = false;
    let mut repeat = 1usize;
    let mut threads = 1usize;
    let mut block_bytes = None;
    let mut stream_bytes = None;
    let mut hand_limit = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--memory" => memory = true,
            "--repeat" => {
                i += 1;
                repeat = args.get(i).ok_or("missing repeat")?.parse()?;
            }
            "--threads" => {
                i += 1;
                threads = args.get(i).ok_or("missing threads")?.parse()?;
            }
            "--block-bytes" => {
                i += 1;
                block_bytes = Some(args.get(i).ok_or("missing block size")?.parse::<usize>()?);
            }
            "--stream-bytes" => {
                i += 1;
                stream_bytes = Some(args.get(i).ok_or("missing stream size")?.parse::<usize>()?);
            }
            "--max-hand-bytes" => {
                i += 1;
                hand_limit = Some(args.get(i).ok_or("missing hand limit")?.parse::<usize>()?);
            }
            _ => return Err("unknown argument".into()),
        }
        i += 1;
    }
    if repeat == 0 || threads == 0 || block_bytes == Some(0) || stream_bytes == Some(0) {
        return Err("repeat and threads must be positive".into());
    }
    if stream_bytes.is_some() && (memory || block_bytes.is_some()) {
        return Err("--stream-bytes is incompatible with --memory and --block-bytes".into());
    }
    if hand_limit == Some(0) || (hand_limit.is_some() && stream_bytes.is_none()) {
        return Err("--max-hand-bytes requires streaming and a positive limit".into());
    }
    let max_hand_bytes = hand_limit.unwrap_or(DEFAULT_MAX_HAND_BYTES);
    let mut paths = vec![];
    files(Path::new(&args[1]), &mut paths)?;
    paths.sort();
    if paths.is_empty() {
        return Err("no TXT files".into());
    }
    let buffers = if memory {
        paths.iter().map(fs::read).collect::<Result<Vec<_>, _>>()?
    } else {
        vec![]
    };
    let mut active = threads.min(paths.len());
    let next = AtomicUsize::new(0);
    let start = Instant::now();
    let (stats, bytes) = if let Some(target) = block_bytes {
        let mut stats = Stats::default();
        let mut bytes = 0;
        active = 1;
        for (i, path) in paths.iter().enumerate() {
            if memory {
                let (s, workers) = process_blocks(&buffers[i], target, threads, repeat);
                stats.merge(s);
                bytes += buffers[i].len() as u64 * repeat as u64;
                active = active.max(workers);
            } else {
                for _ in 0..repeat {
                    let input = fs::read(path)?;
                    let (s, workers) = process_blocks(&input, target, threads, 1);
                    stats.merge(s);
                    bytes += input.len() as u64;
                    active = active.max(workers);
                }
            }
        }
        (stats, bytes)
    } else if active == 1 {
        worker(
            &paths,
            &buffers,
            memory,
            repeat,
            &next,
            stream_bytes,
            max_hand_bytes,
        )?
    } else {
        std::thread::scope(|scope| -> std::io::Result<(Stats, u64)> {
            let handles: Vec<_> = (0..active)
                .map(|_| {
                    scope.spawn(|| {
                        worker(
                            &paths,
                            &buffers,
                            memory,
                            repeat,
                            &next,
                            stream_bytes,
                            max_hand_bytes,
                        )
                    })
                })
                .collect();
            let mut stats = Stats::default();
            let mut bytes = 0;
            for handle in handles {
                let (s, b) = handle
                    .join()
                    .map_err(|_| std::io::Error::other("worker panicked"))??;
                stats.merge(s);
                bytes += b;
            }
            Ok((stats, bytes))
        })?
    };
    let elapsed = start.elapsed().as_secs_f64();
    if let Some(size) = stream_bytes {
        println!("Stream buffer bytes: {size}");
    }
    println!(
        "Scheduling: {}",
        if block_bytes.is_some() {
            "blocks within each file"
        } else {
            "files"
        }
    );
    println!(
        "Mode: {}\nWorkers: {active}\nInput: {}\nRepeat: {repeat}\nFiles: {}\nBytes: {bytes}\nHands: {}\nPlayers: {}\nActions: {}\nParse errors: {}\nElapsed seconds: {elapsed:.6}\nMB/s: {:.2}\nHands/s: {:.0}\nActions/s: {:.0}\nChecksum: {:016x}",
        if active == 1 {
            "single-thread"
        } else {
            "multi-thread"
        },
        if memory { "memory" } else { "read + parse" },
        paths.len(),
        stats.hands,
        stats.players,
        stats.actions,
        stats.errors,
        bytes as f64 / 1e6 / elapsed,
        stats.hands as f64 / elapsed,
        stats.actions as f64 / elapsed,
        stats.checksum
    );
    if stats.errors > 0 {
        let mut shown = 0;
        for (i, path) in paths.iter().enumerate() {
            if let Some(size) = stream_bytes {
                scan_reader_limited(
                    fs::File::open(path)?,
                    size,
                    max_hand_bytes,
                    |offset, result| {
                        if let Err(e) = result
                            && shown < 10
                        {
                            eprintln!("{} byte {}: {}", path.display(), offset, e);
                            shown += 1;
                        }
                    },
                )?;
                if shown >= 10 {
                    break;
                }
                continue;
            }
            let owned;
            let input = if memory {
                &buffers[i]
            } else {
                owned = fs::read(path)?;
                &owned
            };
            scan(input, |offset, result| {
                if let Err(e) = result
                    && shown < 10
                {
                    eprintln!("{} byte {}: {}", path.display(), offset, e);
                    shown += 1;
                }
            });
            if shown >= 10 {
                break;
            }
        }
        std::process::exit(1);
    }
    Ok(())
}
