# Pace Language

Pace is a fast, memory-safe, statically typed programming language.

## Overview
This repository contains the completely Pace compiler and standard library.

## Syntax Example

```pace
import http

interface Repository {
    async func getUser(id: Int) -> User?
}

class UserService implement Repository {
    private let client: HttpClient

    init(client: HttpClient) {
        self.client = client
    }

    async func getUser(id: Int) -> User? {
        let response = await client.get("/users/{id}")

        if response.status != 200 {
            return null
        }

        return response.json()
    }
}

actor UserCache {
    private var users: Map<Int, User> = {}

    func get(id: Int) -> User? {
        return users[id]
    }

    func set(user: User) {
        users[user.id] = user
    }
}

async func main() {
    let service = UserService(client: HttpClient())

    let user = await service.getUser(id: 42)

    if user != null {
        print("Hello {user.name}")
    } else {
        print("User not found")
    }
}
```

## Core Features
- **Variables**: `let` (immutable), `var` (mutable), `const` (compile-time constant).
- **Concurrency**: First-class support for `async`, `await`, `actor`, and `spawn`.
- **Null Safety**: Optional types `T?` with explicit `null` checking.
- **Classes & Interfaces**: Full object-oriented features with `class`, `struct`, and `interface`.

## Benchmarks

Pace is built for speed, generating highly optimized native code via Cranelift. Below are the benchmark results comparing Pace to other popular languages. 

*Tested on **AMD Ryzen 7 7730U**, **14GiB RAM**, **Ubuntu Linux x86_64**.*

| Benchmark | Pace | Rust | Zig | Go | Java | Dart | Python |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Fibonacci** (N=35) | **0.11s / 2.2MB / 98%** | 0.03s / 1.9MB / 100% | 0.02s / 0.6MB / 91% | 0.04s / 2.1MB / 100% | 0.05s / 38MB / 105% | 0.06s / 6.5MB / 98% | 0.90s / 9.7MB / 99% |
| **Loops** (N=10M) | **0.00s / 2.2MB / 100%** | 0.00s / 2.2MB / 100% | 0.00s / 0.5MB / 87% | 0.00s / 2.1MB / 100% | 0.03s / 37MB / 110% | 0.00s / 6.8MB / 100% | 0.36s / 9.6MB / 99% |
| **String Concat** (N=10K) | **0.00s / 2.5MB / 100%** | 0.00s / 2.2MB / 100% | *(Skipped)* | 0.00s / 2.1MB / 83% | 0.04s / 38MB / 85% | 0.00s / 6.7MB / 66% | 0.01s / 9.7MB / 85% |
| **Maps** (N=10K) | **0.00s / 7.6MB / 87%** | 0.00s / 2.8MB / 0% | 0.00s / 1.1MB / 100% | 0.00s / 2.6MB / 100% | 0.05s / 40MB / 130% | 0.00s / 7.4MB / 100% | 0.01s / 10MB / 93% |

*(Format: Execution Time / Peak RAM / CPU Usage)*
