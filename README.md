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
| Zig | $\color{#16a34a}{\text{26.464 ms}}$ | 14.16 MB | 99% | 16628.631 ms |
| Rust | $\color{#16a34a}{\text{41.322 ms}}$ | 14.16 MB | 99% | 145.927 ms |
| Go | $\color{#ca8a04}{\text{50.530 ms}}$ | 14.16 MB | 101% | 47.436 ms |
| Dart | $\color{#ca8a04}{\text{65.865 ms}}$ | 14.16 MB | 100% | 1731.250 ms |
| Java | $\color{#ca8a04}{\text{66.307 ms}}$ | 39.79 MB | 109% | 698.408 ms |
| **Pace** | $\color{#dc2626}{\mathbf{118.264 ms}}$ | 14.16 MB | 100% | 766.492 ms |
| Python | $\color{#dc2626}{\text{932.859 ms}}$ | 14.16 MB | 100% | N/A |


### LOOPS (N=10M)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Zig | $\color{#16a34a}{\text{1.294 ms}}$ | 14.16 MB | 93% | 14913.735 ms |
| Rust | $\color{#16a34a}{\text{1.797 ms}}$ | 14.16 MB | 94% | 127.364 ms |
| Go | $\color{#ca8a04}{\text{5.916 ms}}$ | 14.16 MB | 107% | 47.799 ms |
| Dart | $\color{#ca8a04}{\text{9.753 ms}}$ | 14.16 MB | 99% | 1598.140 ms |
| **Pace** | $\color{#ca8a04}{\mathbf{9.877 ms}}$ | 14.16 MB | 98% | 443.797 ms |
| Java | $\color{#dc2626}{\text{38.434 ms}}$ | 39.97 MB | 114% | 534.776 ms |
| Python | $\color{#dc2626}{\text{371.271 ms}}$ | 14.16 MB | 100% | N/A |


### MAPS (N=10K)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Rust | $\color{#16a34a}{\text{2.390 ms}}$ | 14.16 MB | 95% | 248.423 ms |
| Zig | $\color{#16a34a}{\text{2.765 ms}}$ | 14.16 MB | 94% | 16200.452 ms |
| Go | $\color{#ca8a04}{\text{4.481 ms}}$ | 14.16 MB | 105% | 52.622 ms |
| **Pace** | $\color{#ca8a04}{\mathbf{5.501 ms}}$ | 14.16 MB | 97% | 479.896 ms |
| Dart | $\color{#ca8a04}{\text{7.314 ms}}$ | 14.16 MB | 99% | 1686.463 ms |
| Python | $\color{#dc2626}{\text{15.219 ms}}$ | 14.16 MB | 99% | N/A |
| Java | $\color{#dc2626}{\text{55.289 ms}}$ | 42.13 MB | 114% | 593.210 ms |


### STARTUP TIME

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Zig | $\color{#16a34a}{\text{1.305 ms}}$ | 14.16 MB | 92% | 170.773 ms |
| **Pace** | $\color{#16a34a}{\mathbf{1.688 ms}}$ | 14.16 MB | 94% | 441.013 ms |
| Rust | $\color{#ca8a04}{\text{1.775 ms}}$ | 14.16 MB | 94% | 68.897 ms |
| Go | $\color{#ca8a04}{\text{2.451 ms}}$ | 14.16 MB | 105% | 34.609 ms |
| Dart | $\color{#ca8a04}{\text{4.456 ms}}$ | 14.16 MB | 98% | 1751.042 ms |
| Python | $\color{#dc2626}{\text{15.432 ms}}$ | 14.16 MB | 99% | N/A |
| Java | $\color{#dc2626}{\text{39.141 ms}}$ | 39.19 MB | 115% | 519.197 ms |


### STRING CONCAT (N=10K)

| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |
| :--- | :--- | :--- | :--- | :--- |
| Rust | $\color{#16a34a}{\text{2.028 ms}}$ | 14.16 MB | 93% | 103.827 ms |
| **Pace** | $\color{#16a34a}{\mathbf{2.161 ms}}$ | 14.16 MB | 95% | 441.968 ms |
| Go | $\color{#ca8a04}{\text{3.168 ms}}$ | 14.16 MB | 107% | 50.059 ms |
| Dart | $\color{#ca8a04}{\text{7.007 ms}}$ | 14.16 MB | 97% | 1847.660 ms |
| Python | $\color{#dc2626}{\text{17.056 ms}}$ | 14.16 MB | 99% | N/A |
| Java | $\color{#dc2626}{\text{42.912 ms}}$ | 39.89 MB | 119% | 586.585 ms |


*Legend: $\color{#16a34a}{\text{Top Tier}}$ | $\color{#ca8a04}{\text{Average}}$ | $\color{#dc2626}{\text{Slowest}}$*
