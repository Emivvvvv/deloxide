#!/usr/bin/env bash
set -euo pipefail

release_tag="${1:-}"
package_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)"
expected_tag="v${package_version}"

if [[ "$release_tag" != "$expected_tag" ]]; then
  echo "tag ${release_tag:-<missing>} must equal ${expected_tag}" >&2
  exit 1
fi

grep -Fq "## [${package_version}]" CHANGELOG.md
cargo fmt --all -- --check
cargo clippy --lib --bins --examples --all-features -- -D warnings
cargo test --lib
scripts/check_docs.sh
package_args=()
if [[ "${DELOXIDE_ALLOW_DIRTY:-0}" == "1" ]]; then
  package_args+=(--allow-dirty)
fi
cargo package "${package_args[@]}"
