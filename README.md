# Pace Language

Pace is a fast, memory-safe, statically typed programming language.

## Overview
This repository contains the completely Pace compiler and standard library.

## Syntax Example

```pace
interface Counter {
    func increment();
    func getValue() -> Int;
}

class SimpleCounter implement Counter {
    private var count: Int = 0;

    func init() {}

    func increment() {
        self.count = self.count + 1;
    }

    func getValue() -> Int {
        return self.count;
    }
}

async func main() {
    let counter = SimpleCounter();
    counter.increment();
    counter.increment();
    
    let value = counter.getValue();
    print("Counter value: ${value}");
}
```

## Core Features
- **Variables**: `let` (immutable), `var` (mutable), `const` (compile-time constant).
- **Concurrency**: First-class support for `async`, `await`, `actor`, and `spawn`.
- **Null Safety**: Optional types `T?` with explicit `null` checking.
- **Classes & Interfaces**: Full object-oriented features with `class`, `struct`, and `interface`.

## Benchmarks

Pace is built for speed, generating highly optimized native code via Cranelift. Below are the benchmark results comparing Pace to other popular languages.

*Tested on **AMD Ryzen 7 7730U with Radeon Graphics**, **15GiB RAM**, **Linux x86_64**.*

**Tool Versions:**
- **Rust**: rustc 1.98.0
- **Zig**: 0.16.0
- **Go**: go1.26.6
- **Java**: javac 21.0.12
- **Dart**: 3.13.1
- **Python**: 3.14.4
- **Pace**: 0.1.0

### FIBONACCI (N=35)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Rust | $\color{#16a34a}{\text{21.191 ms}}$ | 14.15 MB | 99% | 69.463 ms |
| Zig | $\color{#16a34a}{\text{22.994 ms}}$ | 14.15 MB | 100% | 13013.531 ms |
| Go | $\color{#ca8a04}{\text{45.180 ms}}$ | 14.28 MB | 101% | 34.039 ms |
| Java | $\color{#ca8a04}{\text{61.165 ms}}$ | 39.77 MB | 108% | 417.007 ms |
| Dart | $\color{#ca8a04}{\text{61.230 ms}}$ | 14.28 MB | 100% | 6218.144 ms |
| **Pace** | $\color{#dc2626}{\mathbf{63.292 ms}}$ | 14.15 MB | 100% | 394.202 ms |
| Python | $\color{#dc2626}{\text{934.565 ms}}$ | 14.28 MB | 100% | N/A |

### LOOPS (N=10M)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Zig | $\color{#16a34a}{\text{0.993 ms}}$ | 14.28 MB | 94% | 12647.437 ms |
| Rust | $\color{#16a34a}{\text{1.456 ms}}$ | 14.28 MB | 96% | 61.000 ms |
| Go | $\color{#ca8a04}{\text{4.563 ms}}$ | 14.28 MB | 106% | 36.212 ms |
| Dart | $\color{#ca8a04}{\text{8.613 ms}}$ | 14.28 MB | 100% | 1438.086 ms |
| **Pace** | $\color{#ca8a04}{\mathbf{8.989 ms}}$ | 14.28 MB | 99% | 369.810 ms |
| Java | $\color{#dc2626}{\text{31.044 ms}}$ | 40.07 MB | 116% | 420.229 ms |
| Python | $\color{#dc2626}{\text{375.042 ms}}$ | 14.28 MB | 100% | N/A |

### MAPS (N=10K)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Zig | $\color{#16a34a}{\text{1.844 ms}}$ | 14.28 MB | 96% | 13072.036 ms |
| Rust | $\color{#16a34a}{\text{2.046 ms}}$ | 14.28 MB | 96% | 183.582 ms |
| **Pace** | $\color{#ca8a04}{\mathbf{3.370 ms}}$ | 14.28 MB | 98% | 439.561 ms |
| Go | $\color{#ca8a04}{\text{3.382 ms}}$ | 14.28 MB | 107% | 46.336 ms |
| Dart | $\color{#ca8a04}{\text{4.286 ms}}$ | 14.28 MB | 99% | 1448.079 ms |
| Python | $\color{#dc2626}{\text{13.208 ms}}$ | 14.28 MB | 99% | N/A |
| Java | $\color{#dc2626}{\text{41.746 ms}}$ | 42.11 MB | 108% | 431.997 ms |

### STARTUP TIME

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Zig | $\color{#16a34a}{\text{0.987 ms}}$ | 14.28 MB | 94% | 150.302 ms |
| Rust | $\color{#16a34a}{\text{1.387 ms}}$ | 14.28 MB | 96% | 59.745 ms |
| **Pace** | $\color{#ca8a04}{\mathbf{1.420 ms}}$ | 14.28 MB | 96% | 379.617 ms |
| Go | $\color{#ca8a04}{\text{1.742 ms}}$ | 14.28 MB | 105% | 35.091 ms |
| Dart | $\color{#ca8a04}{\text{2.933 ms}}$ | 14.28 MB | 100% | 1408.807 ms |
| Python | $\color{#dc2626}{\text{11.267 ms}}$ | 14.28 MB | 99% | N/A |
| Java | $\color{#dc2626}{\text{28.604 ms}}$ | 39.34 MB | 116% | 404.226 ms |

### STRING CONCAT (N=10K)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Rust | $\color{#16a34a}{\text{1.410 ms}}$ | 14.28 MB | 95% | 77.426 ms |
| **Pace** | $\color{#16a34a}{\mathbf{1.538 ms}}$ | 14.28 MB | 96% | 383.820 ms |
| Go | $\color{#ca8a04}{\text{1.926 ms}}$ | 14.28 MB | 106% | 41.563 ms |
| Dart | $\color{#ca8a04}{\text{3.448 ms}}$ | 14.28 MB | 100% | 1426.701 ms |
| Python | $\color{#dc2626}{\text{11.405 ms}}$ | 14.28 MB | 99% | N/A |
| Java | $\color{#dc2626}{\text{29.142 ms}}$ | 39.92 MB | 121% | 447.297 ms |


*Legend: $\color{#16a34a}{\text{Top Tier}}$ | $\color{#ca8a04}{\text{Average}}$ | $\color{#dc2626}{\text{Slowest}}$*
