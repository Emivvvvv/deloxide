#!/usr/bin/env bash
set -euo pipefail

cargo check --examples
cargo test --doc
cargo doc --no-deps --all-features
mdbook build docs

if rg -ni 'zero false positives|zero overhead|detects every deadlock|100% accurate' README.md docs/src; then
  echo "Unsupported absolute claim found" >&2
  exit 1
fi
