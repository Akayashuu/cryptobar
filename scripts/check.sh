#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

run() {
    if [ -n "${CI:-}" ]; then
        "$@"
    else
        systemd-run --user --scope -p MemoryMax=6G -q "$@"
    fi
}

run cargo fmt --all --check
./scripts/check-file-size.sh
python3 -I scripts/check-no-comments.py --self-test
./scripts/check-no-comments.sh
./scripts/check-no-dashes.sh
run cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
run cargo clippy --locked --workspace --all-targets --no-default-features -- -D warnings
run cargo test --locked --workspace --all-features
run cargo test --locked --workspace --no-default-features
RUSTDOCFLAGS="-D warnings" run cargo doc --locked --workspace --no-deps
run cargo deny check
run cargo machete
