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
| Rust | $\color{#16a34a}{\text{21.289 ms}}$ | 14.11 MB | 99% | 70.155 ms |
| Zig | $\color{#16a34a}{\text{23.200 ms}}$ | 14.11 MB | 99% | 12389.774 ms |
| Go | $\color{#ca8a04}{\text{44.919 ms}}$ | 14.11 MB | 101% | 214.776 ms |
| Java | $\color{#ca8a04}{\text{57.605 ms}}$ | 39.74 MB | 108% | 427.142 ms |
| Dart | $\color{#ca8a04}{\text{60.905 ms}}$ | 14.11 MB | 100% | 1499.485 ms |
| **Pace** | $\color{#dc2626}{\mathbf{61.751 ms}}$ | 14.11 MB | 100% | 461.938 ms |
| Python | $\color{#dc2626}{\text{884.569 ms}}$ | 14.11 MB | 100% | N/A |


### LOOPS (N=10M)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Zig | $\color{#16a34a}{\text{1.062 ms}}$ | 14.11 MB | 94% | 11955.699 ms |
| Rust | $\color{#16a34a}{\text{1.384 ms}}$ | 14.11 MB | 95% | 65.204 ms |
| Go | $\color{#ca8a04}{\text{4.385 ms}}$ | 14.11 MB | 105% | 42.581 ms |
| Dart | $\color{#ca8a04}{\text{8.415 ms}}$ | 14.11 MB | 100% | 1369.021 ms |
| **Pace** | $\color{#ca8a04}{\mathbf{8.535 ms}}$ | 14.11 MB | 99% | 346.159 ms |
| Java | $\color{#dc2626}{\text{29.717 ms}}$ | 39.98 MB | 116% | 412.139 ms |
| Python | $\color{#dc2626}{\text{358.882 ms}}$ | 14.11 MB | 100% | N/A |


### MAPS (N=10K)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Zig | $\color{#16a34a}{\text{1.724 ms}}$ | 14.11 MB | 96% | 12144.088 ms |
| Rust | $\color{#16a34a}{\text{2.047 ms}}$ | 14.11 MB | 96% | 182.485 ms |
| Go | $\color{#ca8a04}{\text{2.902 ms}}$ | 14.11 MB | 106% | 39.513 ms |
| **Pace** | $\color{#ca8a04}{\mathbf{3.526 ms}}$ | 14.11 MB | 97% | 358.558 ms |
| Dart | $\color{#ca8a04}{\text{4.155 ms}}$ | 14.11 MB | 99% | 1363.333 ms |
| Python | $\color{#dc2626}{\text{12.075 ms}}$ | 14.11 MB | 99% | N/A |
| Java | $\color{#dc2626}{\text{39.477 ms}}$ | 42.28 MB | 105% | 415.082 ms |


### STARTUP TIME

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Zig | $\color{#16a34a}{\text{0.997 ms}}$ | 14.11 MB | 94% | 140.749 ms |
| **Pace** | $\color{#16a34a}{\mathbf{1.371 ms}}$ | 14.11 MB | 94% | 355.450 ms |
| Rust | $\color{#ca8a04}{\text{1.379 ms}}$ | 14.11 MB | 96% | 62.759 ms |
| Go | $\color{#ca8a04}{\text{1.827 ms}}$ | 14.11 MB | 104% | 33.314 ms |
| Dart | $\color{#ca8a04}{\text{2.983 ms}}$ | 14.11 MB | 100% | 1364.982 ms |
| Python | $\color{#dc2626}{\text{10.960 ms}}$ | 14.11 MB | 99% | N/A |
| Java | $\color{#dc2626}{\text{25.004 ms}}$ | 39.36 MB | 117% | 377.268 ms |


### STRING CONCAT (N=10K)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Rust | $\color{#16a34a}{\text{1.380 ms}}$ | 14.11 MB | 95% | 81.607 ms |
| **Pace** | $\color{#16a34a}{\mathbf{1.569 ms}}$ | 14.11 MB | 96% | 354.351 ms |
| Go | $\color{#ca8a04}{\text{2.388 ms}}$ | 14.11 MB | 104% | 35.765 ms |
| Dart | $\color{#ca8a04}{\text{3.337 ms}}$ | 14.11 MB | 100% | 1354.485 ms |
| Python | $\color{#dc2626}{\text{11.200 ms}}$ | 14.11 MB | 99% | N/A |
| Java | $\color{#dc2626}{\text{27.387 ms}}$ | 40.08 MB | 121% | 426.635 ms |


*Legend: $\color{#16a34a}{\text{Top Tier}}$ | $\color{#ca8a04}{\text{Average}}$ | $\color{#dc2626}{\text{Slowest}}$*
