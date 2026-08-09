# Pace Programming Language

Pace is a modern, statically typed, and natively compiled programming language designed to prioritize clarity, predictability, and excellent developer experience. 

## Project Structure
This repository contains the Pace compiler, standard library, and associated tooling, written in Rust.

The project is structured as a Cargo Workspace:
- `compiler/`: Contains the modular compiler pipeline (lexer, parser, ast, typechecker, etc.)
- `runtime/`: (Planned) Runtime utilities for Pace binaries.
- `std/`: (Planned) The Pace standard library.

## Getting Started

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) `1.97.1` or later.

### Building from Source
To compile the entire workspace:
```bash
cargo build
```

To run the unit tests across all sub-crates:
```bash
cargo test
```

## Contributing
Please refer to the `Pace_Compiler_Development_Guide.md` (in the design docs) for architectural guidelines. Pace is developed strictly from the language-semantics outward.
