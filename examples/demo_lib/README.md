# demo_lib

A sample Pace library package demonstrating modular code organization, generic classes, and native testing.

## Features

- **Math Utilities**: Pure functions (`add`, `multiply`) with type-safe parameters.
- **Generic Box Container**: `Box<T>` class with generic type parameters (`T`), encapsulation, and `get()`/`set()` methods.
- **Native Testing**: Comprehensive unit tests in `tests/` using Pace's built-in `assert()` and the `pace test` runner.

## Directory Structure

```text
demo_lib/
├── pace.toml          # Package manifest
├── src/
│   └── lib.pace       # Library exports (functions, Box<T>)
└── tests/
    └── lib.test.pace  # Unit tests using assert(...)
```

## Running Tests

Run the test suite from the package directory:

```bash
pace test
```

Or target specific tests using filters:

```bash
pace test lib
```

## Example Usage

```pace
import demo_lib

fn main() {
    hello()

    let sum = add(10, 20)
    assert(sum == 30, "sum should equal 30")

    let box = Box<int>(value: 42)
    assert(box.get() == 42, "box value should be 42")
    box.set(100)
    assert(box.get() == 100, "box value after set should be 100")
}
```
