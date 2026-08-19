#!/bin/bash
set -e

echo "=== Benchmarking Python Server ==="
python3 benchmarks/http_server/server.py &
PY_PID=$!
sleep 1
python3 benchmarks/http_server/load_test.py 3001
kill $PY_PID
sleep 1

echo ""
echo "=== Benchmarking Dart Server ==="
dart run benchmarks/http_server/server.dart &
DART_PID=$!
sleep 3 # Dart takes a bit longer to compile and start
python3 benchmarks/http_server/load_test.py 3002
kill $DART_PID
sleep 1

echo ""
echo "=== Benchmarking Pace Server ==="
./target/release/cli run benchmarks/http_server/server.pace &
PACE_PID=$!
sleep 2
python3 benchmarks/http_server/load_test.py 3000
kill $PACE_PID
