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
