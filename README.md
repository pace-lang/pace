<div align="center">
  <img src="banner.png" alt="Pace Language Banner" width="100%" />

  # Pace Language
  *A fast, memory-safe, statically typed programming language.*
</div>

---

## ⚡ Overview
Pace is a compiled, statically typed language designed for the modern cloud and systems programming era. It aims for Dart/Swift-level readability without sacrificing Rust-level safety and performance.

### Core Philosophy
- **Fewer Concepts, Explicit Semantics**: No magic behavior.
- **Data vs Identity**: A strict distinction between Value Semantics (`struct` - Data) and Reference Semantics (`class` - Identity).
- **No Implicit Nulls**: The concept of "null" only exists within algebraic `Option<T>` types.
- **Expected Failures over Exceptions**: Traditional `try/catch` is banned in favor of `Result<T, E>`.
- **Compiler-Driven Memory**: Features an advanced ARC memory model with Copy-on-Write (COW) collections, escape analysis, and no manual memory management.

## 💻 Syntax Example

```pace
import http.Client as HttpClient

trait Serializable {
    fn toJson() -> string
}

class User with Serializable {
    name: string
    
    init(name: string) {
        self.name = name
    }
    
    override fn toJson() -> string {
        return "{ \"name\": \"{self.name}\" }"
    }
}

// Expected errors are explicitly typed
fn fetchUser(id: int) -> Result<User, string> {
    if id < 0 {
        return error("Invalid ID")
    }
    return ok(User("Alice"))
}

fn process() -> Result<string, string> {
    // The `?` operator propagates errors upward seamlessly
    let user = fetchUser(1)?
    
    print(user.toJson())
    
    return ok("Success")
}
```

## 🚀 Key Capabilities
- **Collections**: `List<T>`, `Set<T>`, and `Map<K, V>` are standard library types utilizing efficient COW (Copy-On-Write) memory semantics.
- **Algebraic Enums**: Variants can hold associated data, safely unpacked via `match`.
- **Concurrency**: First-class support for `async`, `await`, and isolated `actor` boundaries to eliminate arbitrary shared mutable state.
- **FFI**: Seamless interaction with C-ABI natively through `extern` and `unsafe` blocks.

## 🛠️ The Ecosystem
- `pace build`: Compiles projects using the native `pace.toml` package manager.
- `pace run`: Compiles and executes code in one step.
- `pace test`: Built-in standard testing framework.
