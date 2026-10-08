use std::{collections::BTreeMap, fs, path::Path};
fn audit(path: &Path, counts: &mut BTreeMap<&'static str, u64>) -> std::io::Result<()> {
    if path.is_dir() {
        for e in fs::read_dir(path)? {
            let e = e?;
            if !e.file_type()?.is_symlink() {
                audit(&e.path(), counts)?;
            }
        }
    } else if path
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("txt"))
    {
        let input = fs::read(path)?;
        let mut local = BTreeMap::new();
        pokerforge::scan(&input, |_, result| {
            if let Err(e) = result {
                *local.entry(e).or_insert(0u64) += 1;
                *counts.entry(e).or_insert(0) += 1;
            }
        });
        for (reason, n) in local {
            println!("{}: {n} — {reason}", path.display());
        }
    }
    Ok(())
}
fn main() -> std::io::Result<()> {
    let path = std::env::args_os().nth(1).unwrap_or_else(|| "data".into());
    let mut counts = BTreeMap::new();
    audit(Path::new(&path), &mut counts)?;
    println!("Errors by reason: {counts:?}");
    Ok(())
}
