#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Building pace-cli (release)..."
cargo build --manifest-path "$REPO_ROOT/compiler/Cargo.toml" --release -p pace-cli

cd "$SCRIPT_DIR"
python3 pace_bench.py
