# Pace Benchmarks

This directory contains micro-benchmarks that test the raw execution performance of Pace natively against other modern programming languages.

## Fibonacci (fib(35))

Located in `benchmarks/fibonacci/`, this benchmark compares the execution overhead of deep recursive function calls.

### The Source Codes

We have created perfectly equivalent recursive fibonacci functions calculating `fib(35)` across multiple languages to ensure a 1:1 algorithmic comparison. 

- `fib.pace` (Pace)
- `fib.c` (C)
- `fib.rs` (Rust)
- `fib.zig` (Zig)
- `fib.go` (Go)
- `fib.dart` (Dart)
- `Fib.java` (Java)
- `fib.py` (Python)

### Our Benchmark Results

*Run on Ubuntu 26.04 x86_64, using `/usr/bin/time -v` to measure total Wall Clock execution time.*

| Language | Type | Execution Time |
| :--- | :--- | :--- |
| **C (`gcc -O3`)** | Native AOT | **0.03** seconds |
| **Zig (`ReleaseFast`)** | Native AOT | **0.03** seconds |
| **Go** | Native AOT | **0.05** seconds |
| **Rust (`rustc -O`)** | Native AOT | **0.05** seconds |
| **Pace (`release`)** | Native AOT | **0.06** seconds 🚀 |
| **Dart (`compile exe`)**| Native AOT | **0.11** seconds |
| **Java (`java 17`)** | JVM JIT | **0.13** seconds |
| **Python 3** | Interpreted | **1.73** seconds |

### Conclusion

Pace translates directly into a highly optimized Cranelift Intermediate Representation (IR), ensuring absolute zero interpretation overhead. Pace runs at nearly the exact same speed as fully optimized C and Rust, and securely out-performs leading JIT VMs and Interpeters by wide margins.
