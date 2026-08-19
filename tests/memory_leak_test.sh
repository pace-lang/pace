#!/bin/bash

# Exit on error
set -e

echo "Building Pace Compiler..."
cargo build --release

PACE_BIN="./target/release/pace"

# Test cases
TESTS=(
    "tests/ui/15_performance/map_insert_heavy.pace"
    "tests/ui/15_performance/struct_copy_heavy.pace"
    "tests/ui/13_stdlib/map_advanced.pace"
)

echo "------------------------------------------------"
echo "Running Memory Leak Tests using Valgrind"
echo "------------------------------------------------"

for test_file in "${TESTS[@]}"; do
    echo "Building $test_file..."
    
    # pace build outputs to target/debug or target/release depending on --release
    cargo run -- build $test_file
    
    # Pace outputs the binary to <dir>/target/debug/<basename_of_dir>
    DIR=$(dirname "$test_file")
    DIR_NAME=$(basename "$DIR")
    OUT_BIN="$DIR/target/debug/$DIR_NAME"
    
    if [ ! -f "$OUT_BIN" ]; then
        echo "Error: Could not find output binary $OUT_BIN"
        exit 1
    fi
    
    echo "Running Valgrind on $OUT_BIN..."
    valgrind --leak-check=full --error-exitcode=1 "$OUT_BIN"
    
    echo "$test_file passed memory leak check!"
    echo "------------------------------------------------"
done

echo "All tests passed successfully with no memory leaks!"
