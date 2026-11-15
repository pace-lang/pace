# demo_app

An end-to-end showcase application demonstrating core Pace language capabilities, package dependencies, and the native testing framework.

## Features Showcase

- **Language Primitives**: Operators, loops, conditionals, pattern matching, enums, null-safety, and closures.
- **Collections**: Native dynamic `List<T>` and hash-map `Map<K, V>` collections.
- **Object-Oriented Programming**: Struct value semantics, class reference semantics, and single inheritance.
- **Generics**: Generic functions, classes, and monomorphization.
- **Concurrency**: `async`/`await` fibers and multi-threaded actors.
- **Native Testing**: Test suites in `tests/*.test.pace` validating collections and types with `assert()`.

## Directory Structure

```text
demo_app/
├── pace.toml          # Package manifest (depends on demo_lib)
├── src/
│   ├── main.pace      # Application entry point
│   ├── collections.pace
│   ├── structs_classes.pace
│   └── ...
└── tests/
    ├── collections.test.pace  # Tests for List and Map collections
    └── types.test.pace        # Tests for Structs, Classes, and Inheritance
```

## Running the Application

To build and execute the application:

```bash
pace run
```

## Running Tests

To run the automated test suite using the Bun-grade `pace test` runner:

```bash
pace test
```

To filter tests by name:

```bash
pace test collections
```
