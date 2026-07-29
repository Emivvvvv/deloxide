#!/usr/bin/env bash
set -euo pipefail

cargo check --examples --all-features
cargo test --doc --all-features
cargo doc --no-deps --all-features
mdbook test docs -L target/debug/deps
mdbook build docs
python3 scripts/check_doc_links.py README.md docs/src

if rg -ni 'zero false positives|zero overhead|detects every deadlock|100% accurate|use Deloxide for everything else' README.md docs/src; then
  echo "Unsupported absolute claim found" >&2
  exit 1
fi
