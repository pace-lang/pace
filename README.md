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

*Tested on **AMD Ryzen 7 7730U**, **14GiB RAM**, **Ubuntu Linux x86_64**.*

### FIBONACCI (N=35)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Rust | $\color{#16a34a}{\text{22.205 ms}}$ | 14.21 MB | 99% | 67.433 ms |
| Zig | $\color{#16a34a}{\text{23.977 ms}}$ | 14.21 MB | 99% | 14037.602 ms |
| Go | $\color{#ca8a04}{\text{46.374 ms}}$ | 14.21 MB | 101% | 35.418 ms |
| Java | $\color{#ca8a04}{\text{64.469 ms}}$ | 39.84 MB | 109% | 487.703 ms |
| **Pace** | $\color{#ca8a04}{\mathbf{72.503 ms}}$ | 13.99 MB | 100% | 542.860 ms |
| Dart | $\color{#dc2626}{\text{66.176 ms}}$ | 14.21 MB | 100% | 1504.832 ms |
| Python | $\color{#dc2626}{\text{998.840 ms}}$ | 14.21 MB | 100% | N/A |


### LOOPS (N=10M)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Rust | $\color{#16a34a}{\text{1.316 ms}}$ | 14.21 MB | 95% | 68.887 ms |
| Zig | $\color{#16a34a}{\text{1.414 ms}}$ | 14.21 MB | 90% | 14004.470 ms |
| Go | $\color{#ca8a04}{\text{5.931 ms}}$ | 14.21 MB | 105% | 42.433 ms |
| Dart | $\color{#ca8a04}{\text{9.345 ms}}$ | 14.21 MB | 99% | 1500.934 ms |
| **Pace** | $\color{#ca8a04}{\mathbf{10.480 ms}}$ | 14.17 MB | 98% | 466.718 ms |
| Java | $\color{#dc2626}{\text{32.420 ms}}$ | 40.03 MB | 116% | 485.126 ms |
| Python | $\color{#dc2626}{\text{409.751 ms}}$ | 14.21 MB | 100% | N/A |


### MAPS (N=10K)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Zig | $\color{#16a34a}{\text{1.807 ms}}$ | 14.21 MB | 95% | 14397.531 ms |
| Rust | $\color{#16a34a}{\text{2.179 ms}}$ | 14.21 MB | 95% | 204.881 ms |
| Go | $\color{#ca8a04}{\text{3.166 ms}}$ | 14.21 MB | 104% | 37.052 ms |
| Dart | $\color{#ca8a04}{\text{4.683 ms}}$ | 14.33 MB | 99% | 1583.090 ms |
| **Pace** | $\color{#16a34a}{\mathbf{3.765 ms}}$ | 14.17 MB | 97% | 458.117 ms |
| Python | $\color{#dc2626}{\text{14.433 ms}}$ | 14.33 MB | 99% | N/A |
| Java | $\color{#dc2626}{\text{44.839 ms}}$ | 42.24 MB | 109% | 486.018 ms |


### STARTUP TIME

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Zig | $\color{#16a34a}{\text{1.061 ms}}$ | 14.33 MB | 93% | 158.204 ms |
| Rust | $\color{#16a34a}{\text{1.427 ms}}$ | 14.33 MB | 95% | 60.750 ms |
| Go | $\color{#ca8a04}{\text{1.805 ms}}$ | 14.33 MB | 105% | 97.170 ms |
| **Pace** | $\color{#ca8a04}{\mathbf{3.258 ms}}$ | 14.17 MB | 107% | 458.844 ms |
| Dart | $\color{#ca8a04}{\text{3.411 ms}}$ | 14.33 MB | 99% | 1544.441 ms |
| Python | $\color{#dc2626}{\text{13.372 ms}}$ | 14.33 MB | 99% | N/A |
| Java | $\color{#dc2626}{\text{27.923 ms}}$ | 39.28 MB | 116% | 437.988 ms |


### STRING CONCAT (N=10K)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Rust | $\color{#16a34a}{\text{1.455 ms}}$ | 14.33 MB | 95% | 84.745 ms |
| **Pace** | $\color{#16a34a}{\mathbf{3.452 ms}}$ | 14.17 MB | 95% | 662.423 ms |
| Go | $\color{#ca8a04}{\text{2.508 ms}}$ | 14.33 MB | 106% | 41.389 ms |
| Dart | $\color{#ca8a04}{\text{3.736 ms}}$ | 14.33 MB | 99% | 1517.867 ms |
| Python | $\color{#dc2626}{\text{12.957 ms}}$ | 14.33 MB | 99% | N/A |
| Java | $\color{#dc2626}{\text{30.296 ms}}$ | 39.91 MB | 120% | 519.915 ms |


*Legend: $\color{#16a34a}{\text{Top Tier}}$ | $\color{#ca8a04}{\text{Average}}$ | $\color{#dc2626}{\text{Slowest}}$*
