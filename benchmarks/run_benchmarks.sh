#!/bin/bash

# Ensure Pace is built
cargo build --release -p pace-cli

echo "--- FIBONACCI BENCHMARK (N=35) ---"
cd benchmarks/fibonacci

echo "> Running Pace..."
../../target/release/pace build bench.pace > /dev/null 2>&1
python3 ../../time_it.py ./build/bench

echo "> Running Rust..."
rustc -O bench.rs
python3 ../../time_it.py ./bench

echo "> Running Zig..."
zig build-exe -O ReleaseFast bench.zig
python3 ../../time_it.py ./bench

echo "> Running Go..."
go build bench.go
python3 ../../time_it.py ./bench

echo "> Running Java..."
javac --release 17 Bench.java
python3 ../../time_it.py java Bench

echo "> Running Dart..."
dart compile exe bench.dart -o bench_dart > /dev/null 2>&1
python3 ../../time_it.py ./bench_dart

echo "> Running Python..."
python3 ../../time_it.py python3 bench.py

cd ..

echo "--- LOOPS BENCHMARK (N=10M) ---"
cd loops

echo "> Running Pace..."
../../target/release/pace build bench.pace > /dev/null 2>&1
python3 ../../time_it.py ./build/bench

echo "> Running Rust..."
rustc -O bench.rs
python3 ../../time_it.py ./bench

echo "> Running Zig..."
zig build-exe -O ReleaseFast bench.zig
python3 ../../time_it.py ./bench

echo "> Running Go..."
go build bench.go
python3 ../../time_it.py ./bench

echo "> Running Java..."
javac --release 17 Bench.java
python3 ../../time_it.py java Bench

echo "> Running Dart..."
dart compile exe bench.dart -o bench_dart > /dev/null 2>&1
python3 ../../time_it.py ./bench_dart

echo "> Running Python..."
python3 ../../time_it.py python3 bench.py

cd ..


echo "--- STRING CONCAT BENCHMARK (N=10K) ---"
cd string_concat
echo "> Running Pace..."
../../target/release/pace build bench.pace > /dev/null 2>&1
python3 ../../time_it.py ./build/bench

echo "> Running Rust..."
rustc -O bench.rs
python3 ../../time_it.py ./bench

# echo "> Running Zig..."
# zig build-exe -O ReleaseFast bench.zig
# python3 ../../time_it.py ./bench

echo "> Running Go..."
go build bench.go
python3 ../../time_it.py ./bench

echo "> Running Java..."
javac --release 17 Bench.java
python3 ../../time_it.py java Bench

echo "> Running Dart..."
dart compile exe bench.dart -o bench_dart > /dev/null 2>&1
python3 ../../time_it.py ./bench_dart

echo "> Running Python..."
python3 ../../time_it.py python3 bench.py

cd ..


echo "--- MAPS BENCHMARK (N=10K) ---"
cd maps
echo "> Running Pace..."
../../target/release/pace build bench.pace > /dev/null 2>&1
python3 ../../time_it.py ./build/bench

echo "> Running Rust..."
rustc -O bench.rs
python3 ../../time_it.py ./bench

echo "> Running Zig..."
zig build-exe -O ReleaseFast bench.zig
python3 ../../time_it.py ./bench

echo "> Running Go..."
go build bench.go
python3 ../../time_it.py ./bench

echo "> Running Java..."
javac --release 17 Bench.java
python3 ../../time_it.py java Bench

echo "> Running Dart..."
dart compile exe bench.dart -o bench_dart > /dev/null 2>&1
python3 ../../time_it.py ./bench_dart

echo "> Running Python..."
python3 ../../time_it.py python3 bench.py

cd ../..
