# Changelog

All notable changes to the Pace language will be documented in this file.

## [Unreleased]

### Added
- **Async & Actors Core**: Implemented compiler frontend and state machine (MIR) lowering for `async/await` and Actors. The compiler now correctly typechecks async contexts, hoists `async` functions to return `Task<T>`, tracks local variable scopes across await boundaries via state structs, and dispatches actor method calls.
- **Zero-Cost Interfaces**: Implemented implicit monomorphization for interface-based function parameters, completely eliminating virtual tables and dynamic dispatch overhead. The compiler now automatically synthesizes generic parameters for interfaces to dispatch them statically!
- **Partial Generic Arguments**: Added support for partial specification of generic type parameters. Developers can now explicitly provide some generics while letting the compiler seamlessly infer the rest.
- **HTTP Client**: Added `HttpClientOptions` to support configuring connection timeouts and custom User-Agent strings dynamically.
- **HTTP Server**: Added `HttpServerRequest.getBody()` to allow reading POST/PUT request bodies, and `HttpServerRequest.getQuery()` to natively extract URL query parameters. Added `HttpServerResponse` class to allow customizing HTTP response headers and status codes dynamically. Added support for extracting request headers.
- **HTTP Benchmarks**: Added `benchmarks/http_req` benchmarking suite for network tests, alongside the existing `http_server` benchmark. Pace HTTP Server now comfortably achieves ~4,800 RPS, beating Python and Dart.
- **JSON Standard Library**: Implemented a pure recursive descent JSON parser (`parseJson`) inside Pace.
- **JSON Micro-benchmarks**: Added comprehensive JSON parsing benchmarks in `benchmarks/json_parse/` to track performance against Python and Dart.
- **Native String Scanning Helpers**: Added `stringSkipWhitespace` and `stringFindStringEnd` to the native runtime to provide O(1) loop speedups for string operations, greatly optimizing JSON parsing performance.
- **Struct Value Semantics**: Structs are now treated strictly as deep-copied value types. Updating properties of a copied struct variable no longer incorrectly shares memory with the original instance.
- **Micro-benchmarks**: Added native performance benchmarks for `Map` insertions, recursive functions (Fibonacci), `Struct` deep copies, and Sieve of Eratosthenes. 
- **Memory Leak Test Suite**: Created a Valgrind test wrapper (`tests/memory_leak_test.sh`) to guarantee zero memory leaks across new native collections.

### Fixed
- Fixed cascading `Unknown type 'Any'` errors when parsing invalid or unknown generic types, drastically improving compiler error quality and readability.
- Resolved `unused_mut`, `unused_variables`, and `dead_code` warnings across the compiler architecture.
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
- Fixed memory corruption bug where extracting a `Struct` payload from a generic `Enum` (e.g. `Option<Point>`) mistakenly incremented the struct's padding by incorrectly applying an ARC `Retain` instruction.
- Fixed a bug where `Result` enums with primitive success/error types inside `try_expr` (the `?` operator) were failing to generate the correct monomorphized Enum names.
- Fixed critical memory corruption bug where `Struct` property access (`GetProperty`/`SetProperty`) was reading/writing out-of-bounds memory by incorrectly applying a 24-byte Class object header offset.
- Fixed a bug where `Struct` assignments (e.g. `var p2 = p1`) were incorrectly sharing memory pointers (reference semantics) instead of deep-copying memory (value semantics). Struct variables now properly copy their bitwise values into independent stack slots.
- Upgraded the compiler Typechecker to support dynamic generic monomorphization and extension resolution for built-in primitive types (like Arrays `[T]`).
- Fixed color output variables in `installer/install.sh` by using `printf`.
