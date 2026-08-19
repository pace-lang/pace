# Changelog

All notable changes to the Pace language will be documented in this file.

## [Unreleased]

### Added
- **Struct Value Semantics**: Structs are now treated strictly as deep-copied value types. Updating properties of a copied struct variable no longer incorrectly shares memory with the original instance.
- **Micro-benchmarks**: Added native performance benchmarks for `Map` insertions, recursive functions (Fibonacci), `Struct` deep copies, and Sieve of Eratosthenes. 
- **Memory Leak Test Suite**: Created a Valgrind test wrapper (`tests/memory_leak_test.sh`) to guarantee zero memory leaks across new native collections.

### Fixed
- Fixed `[P3001]` compiler bug preventing nested structs from typechecking (e.g., `Rect` containing `Point`).
- Fixed invalid ARC `MemCopy` behaviors causing segfaults on uninitialized struct stack slots.
- Fixed `List` and `Map` native methods (`map.length`, `map.set`) incorrectly being mapped to Rust APIs, replacing them with proper Pace APIs.
- Added `hash` and `equals` Compiler Intrinsics to support hashing and equality comparisons for generic types.
- Added Compiler Intrinsics in Cranelift backend to directly lower `ptrRead` and `ptrWrite` foreign functions into zero-overhead load/store instructions.
- Added support for Extension Methods (`extend ClassName { ... }` and `extend<T> GenericClass<T> { ... }`), enabling adding new methods to existing classes, structs, and primitive types from outside their original declarations.
- Added support for the Ternary Operator (`condition ? true_expr : false_expr`).
- Added `pace clean` command to remove the target directory.

### Changed
- Refactored `Map<K, V>` and `Set<T>` to be completely natively implemented in Pace using the new memory primitives instead of relying on the C runtime.
- Refactored `List<T>` to be completely natively implemented in Pace using the new memory primitives instead of relying on the C runtime.
- Refactored the Pace Standard Library (Stdlib) to use Extension Methods instead of standalone generic functions. Methods for Strings (`s.len()`), Arrays (`arr.push()`), Options, and Maps are now natively accessed via dot-syntax.
- Removed outdated `list*` FFI functions from the C runtime.
- Standardized "Not implemented yet" warning messages in the CLI.
- Updated `pace init` error messages to use standard diagnostic formatting.

### Fixed
- Fixed critical memory corruption bug where `Struct` property access (`GetProperty`/`SetProperty`) was reading/writing out-of-bounds memory by incorrectly applying a 24-byte Class object header offset.
- Fixed a bug where `Struct` assignments (e.g. `var p2 = p1`) were incorrectly sharing memory pointers (reference semantics) instead of deep-copying memory (value semantics). Struct variables now properly copy their bitwise values into independent stack slots.
- Upgraded the compiler Typechecker to support dynamic generic monomorphization and extension resolution for built-in primitive types (like Arrays `[T]`).
- Fixed color output variables in `installer/install.sh` by using `printf`.
