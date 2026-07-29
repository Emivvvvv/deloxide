#!/usr/bin/env bash
set -euo pipefail

cargo check --examples --all-features
cargo test --doc --all-features
cargo doc --no-deps --all-features

mdbook_deps=$(mktemp -d)
cleanup_mdbook_deps() {
  find "$mdbook_deps" -type l -delete
  rmdir "$mdbook_deps"
}
trap cleanup_mdbook_deps EXIT

while IFS= read -r -d '' library; do
  case "$(basename "$library")" in
    libdeloxide.dylib|libdeloxide.so|deloxide.dll) continue ;;
  esac
  ln -s "$PWD/$library" "$mdbook_deps/$(basename "$library")"
done < <(
  find target/debug/deps -maxdepth 1 -type f \
    \( -name '*.rlib' -o -name '*.dylib' -o -name '*.so' -o -name '*.dll' \) \
    -print0
)

rustdoc_dir=$(dirname "$(rustup which rustdoc)")
PATH="$rustdoc_dir:$PATH" mdbook test docs -L "$mdbook_deps"
mdbook build docs
python3 scripts/check_doc_links.py README.md docs/src

if rg -ni 'zero false positives|zero overhead|detects every deadlock|100% accurate|use Deloxide for everything else' README.md docs/src; then
  echo "Unsupported absolute claim found" >&2
  exit 1
fi
