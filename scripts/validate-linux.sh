#!/usr/bin/env bash
# Run from any directory; requires Rust stable/nightly and cargo-fuzz.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
seconds="${1:-300}"
if [[ ! "$seconds" =~ ^[1-9][0-9]*$ ]]; then
    echo "Usage: bash scripts/validate-linux.sh [positive fuzz duration in seconds]" >&2
    exit 2
fi
cargo +stable fmt --check
cargo +stable fmt --manifest-path fuzz/Cargo.toml --check
cargo +stable clippy --locked --all-targets -- -D warnings
cargo +stable test --locked --release
cargo +stable run --locked --release --example fuzz_smoke -- 100000
cargo +stable run --locked --release -- tests/fixtures --stream-bytes 17
mkdir -p fuzz/corpus/parser
cp tests/fixtures/*.txt fuzz/corpus/parser/
cargo +nightly fuzz run parser -- -max_total_time="$seconds" -max_len=65536
