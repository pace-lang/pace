# Pace Programming Language

> [!WARNING]
> Pace is an experimental hobby project in early active development. The compiler, language syntax, and standard library are subject to frequent breaking changes. It is not currently intended for production use.

Pace is a modern, statically typed, and natively compiled programming language designed to prioritize safety, predictable performance, and an excellent developer experience. 

It aims to bridge the gap between low-level systems programming and high-level application development by combining **C-like speed** with **Swift-like ergonomics**.

## What is Pace?

At its core, Pace is built on three pillars:
1. **Safety**: Strict, compiler-enforced null safety and strong typing prevent entire classes of runtime errors.
2. **Predictable Performance**: Pace uses deterministic Automatic Reference Counting (ARC) rather than a tracing garbage collector. This guarantees memory safety without unpredictable pause times, making it ideal for latency-sensitive applications.
3. **Expressiveness**: First-class support for both Object-Oriented (Classes, Interfaces) and Functional (Algebraic Data Types, Pattern Matching) paradigms.

## Core Language Features

- **Classes & Interfaces**: Full support for `class` definitions with stateful properties, constructors (`init`), and methods. Classes can define robust contracts using the `interface` and `implements` keywords.
- **Algebraic Data Types (ADTs)**: Express complex domain logic using `enum` variants with payload support, combined with powerful and exhaustive `match` pattern-matching expressions.
- **Null Safety & Optionals**: Goodbye `NullPointerException`! Pace features strict compiler-enforced null safety. The `T?` syntax represents a nullable reference type, forcing developers to handle absence explicitly at compile time.
- **Native Arrays**: Support for dynamically sized arrays with strict bounds checking and native, contiguous memory layouts.
- **Module & Package System**: Cleanly organize code with the `package` system, allowing seamless dependency resolution across the workspace.

## Memory Management

Pace uses deterministic Automatic Reference Counting (ARC) instead of a tracing garbage collector, ensuring predictable performance without pause times.

- **ARC Engine**: Objects are automatically retained and released by the compiler's MIR lowering phase, completely abstracting memory management away from the developer without sacrificing performance.
- **Weak References**: Built-in support for `weak` variables to safely break reference cycles in complex data structures. 
- **Thread Safety**: ARC operations are designed to be thread-safe from day one, laying the structural groundwork for future concurrency features.

## Benchmarks

Pace includes several benchmarks to compare performance against other languages:

- **Fibonacci**: Measures recursive function call overhead.
- **JSON Parsing**: Measures string manipulation, heap allocation, and ARC performance.

To run the Fibonacci benchmark:

```bash
# Requires Python, Node, Dart, Go, Zig, Rust, Java, C
cd benchmarks/fibonacci
./run_benchmarks.sh
```

To run the JSON parse benchmark, you can use the scripts provided in the `benchmarks/json_parse` directory for various languages (Python, Dart, Java, etc.).

```bash
# Build and run Pace JSON benchmark (Compiles to native machine code)
cli build benchmarks/json_parse/json.pace
time benchmarks/json_parse/target/debug/json_parse

# Run Python JSON benchmark (Uses native C extension)
python3 benchmarks/json_parse/bench.py

# Run Dart JSON benchmark (JIT compiled)
dart run benchmarks/json_parse/json.dart
```

These files are provided directly in the repository so you can independently verify our performance claims.

## Raw Performance

Because Pace compiles directly into native machine code (using Cranelift) rather than running in an interpreter or JIT VM, it boasts extreme execution speeds on par with C and Rust. 

For full details and scripts on our recursive `fib(35)` and prime sieving benchmarks comparing Pace to Rust, C, Dart, Java, and Python, see the [Benchmarks Suite](./benchmarks/README.md).

## Compiler Architecture

The Pace compiler is a multi-pass, modern architecture built in Rust, heavily inspired by industry-leading compiler designs:

1. **Frontend**: A fast, recursive descent parser building a strictly typed Abstract Syntax Tree (AST).
2. **Semantic Analysis**: Advanced name resolution, scope tracking, and a robust Typechecker.
3. **MIR Lowering**: Lowers the AST into a Mid-level Intermediate Representation (MIR) that explicitly models control flow and memory (ARC) operations.
4. **Backend (Codegen)**: Translates MIR directly into highly optimized native machine code using the **Cranelift** backend.
5. **Development VM**: Includes a built-in bytecode VM for rapid testing, debugging, and development.
6. **Diagnostics Engine**: Beautiful, structured error reporting with precise source spans (inspired by `miette`).

## Install Pace Toolchain

Pace is distributed as a prebuilt native toolchain. **You do not need Rust or Cargo to use Pace.**

### Linux / macOS

```bash title="Terminal"
curl -fsSL https://raw.githubusercontent.com/pace-lang/pace/main/installer/install.sh | bash
```

### Windows (PowerShell)

```powershell title="PowerShell"
irm https://raw.githubusercontent.com/pace-lang/pace/main/installer/install.ps1 | iex
```

### Verify Installation

Open a new terminal and run:

```bash title="Terminal"
pace --version
```

### Create Your First Project

```bash title="Terminal"
pace new hello
cd hello
pace run
```

## Building from source (Contributors)

This section is only for Pace compiler contributors. If you just want to use Pace, follow the installation steps above.

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) `1.97.1` or later.

### Building the Workspace
To compile the entire workspace:
```bash
cargo build --release
```

To run the unit tests and golden tests across all sub-crates:
```bash
cargo test
```

### Running Pace from Source
Pace includes a CLI for executing programs during development:
```bash
cargo run --bin pace -- run path/to/file.pace
```
