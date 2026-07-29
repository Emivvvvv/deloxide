#!/usr/bin/env bash
set -euo pipefail

cargo doc --no-deps --all-features
mdbook build docs
