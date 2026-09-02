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

### ACTORS (N=100K MSGS)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| **Pace** | $\color{#ca8a04}{\mathbf{110.316 ms}}$ | 14.18 MB | 115% | 258.635 ms |


### CLASSES (N=1M)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Rust | $\color{#16a34a}{\text{1.864 ms}}$ | 14.30 MB | 87% | 135.505 ms |
| Go | $\color{#16a34a}{\text{4.256 ms}}$ | 14.30 MB | 110% | 72.871 ms |
| Dart | $\color{#ca8a04}{\text{7.375 ms}}$ | 14.43 MB | 98% | 2520.503 ms |
| Java | $\color{#ca8a04}{\text{71.513 ms}}$ | 46.16 MB | 121% | 852.815 ms |
| **Pace** | $\color{#dc2626}{\mathbf{92.623 ms}}$ | 47.98 MB | 100% | 263.074 m ms |
| Python | $\color{#dc2626}{\text{146.560 ms}}$ | 14.43 MB | 100% | N/A |


### FIBONACCI (N=35)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Rust | $\color{#16a34a}{\text{19.804 ms}}$ | 14.43 MB | 99% | 74.948 ms |
| Zig | $\color{#16a34a}{\text{21.732 ms}}$ | 14.43 MB | 99% | 12959.646 ms |
| Go | $\color{#ca8a04}{\text{42.941 ms}}$ | 14.43 MB | 102% | 38.960 ms |
| Java | $\color{#ca8a04}{\text{58.721 ms}}$ | 39.85 MB | 109% | 420.764 ms |
| **Pace** | $\color{#ca8a04}{\mathbf{59.842 ms}}$ | 14.43 MB | 100% | 139.987 m ms |
| Dart | $\color{#dc2626}{\text{63.788 ms}}$ | 14.43 MB | 100% | 1438.792 ms |
| Python | $\color{#dc2626}{\text{909.680 ms}}$ | 14.43 MB | 100% | N/A |


### LOOPS (N=10M)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Zig | $\color{#16a34a}{\text{0.451 ms}}$ | 14.43 MB | 87% | 12081.044 ms |
| Rust | $\color{#16a34a}{\text{1.013 ms}}$ | 14.43 MB | 88% | 102.638 ms |
| Go | $\color{#ca8a04}{\text{3.839 ms}}$ | 14.43 MB | 106% | 31.253 ms |
| **Pace** | $\color{#ca8a04}{\mathbf{6.640 ms}}$ | 14.43 MB | 97% | 135.271 ms
|
| Dart | $\color{#ca8a04}{\text{7.519 ms}}$ | 14.43 MB | 100% | 1334.411 ms |
| Java | $\color{#dc2626}{\text{28.637 ms}}$ | 40.04 MB | 116% | 387.984 ms |
| Python | $\color{#dc2626}{\text{350.679 ms}}$ | 14.43 MB | 100% | N/A |


### MAPS (N=10K)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Zig | $\color{#16a34a}{\text{1.184 ms}}$ | 14.43 MB | 93% | 12075.971 ms |
| Rust | $\color{#16a34a}{\text{1.410 ms}}$ | 14.43 MB | 94% | 174.258 ms |
| Go | $\color{#ca8a04}{\text{2.436 ms}}$ | 14.43 MB | 107% | 33.110 ms |
| **Pace** | $\color{#ca8a04}{\mathbf{2.831 ms}}$ | 14.43 MB | 97% | 137.948 ms
|
| Dart | $\color{#ca8a04}{\text{3.463 ms}}$ | 14.43 MB | 99% | 1362.594 ms |
| Python | $\color{#dc2626}{\text{11.748 ms}}$ | 14.43 MB | 99% | N/A |
| Java | $\color{#dc2626}{\text{38.981 ms}}$ | 41.93 MB | 105% | 412.697 ms |


### STARTUP TIME

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Zig | $\color{#16a34a}{\text{0.422 ms}}$ | 14.43 MB | 86% | 143.518 ms |
| **Pace** | $\color{#16a34a}{\mathbf{0.665 ms}}$ | 14.43 MB | 90% | 125.378 ms
|
| Rust | $\color{#ca8a04}{\text{0.800 ms}}$ | 14.43 MB | 91% | 54.686 ms |
| Go | $\color{#ca8a04}{\text{1.307 ms}}$ | 14.43 MB | 106% | 27.591 ms |
| Dart | $\color{#ca8a04}{\text{2.441 ms}}$ | 14.43 MB | 99% | 1324.289 ms |
| Python | $\color{#dc2626}{\text{10.129 ms}}$ | 14.43 MB | 98% | N/A |
| Java | $\color{#dc2626}{\text{24.221 ms}}$ | 39.37 MB | 117% | 369.354 ms |


### STRING CONCAT (N=10K)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Rust | $\color{#16a34a}{\text{0.830 ms}}$ | 14.43 MB | 92% | 77.306 ms |
| **Pace** | $\color{#16a34a}{\mathbf{1.158 ms}}$ | 14.43 MB | 93% | 129.873 ms
|
| Go | $\color{#ca8a04}{\text{1.461 ms}}$ | 14.43 MB | 107% | 32.928 ms |
| Dart | $\color{#ca8a04}{\text{2.731 ms}}$ | 14.43 MB | 99% | 1348.961 ms |
| Python | $\color{#dc2626}{\text{10.542 ms}}$ | 14.43 MB | 99% | N/A |
| Java | $\color{#dc2626}{\text{25.963 ms}}$ | 40.09 MB | 122% | 404.651 ms |

*Legend: $\color{#16a34a}{\text{Top Tier}}$ | $\color{#ca8a04}{\text{Average}}$ | $\color{#dc2626}{\text{Slowest}}$*
