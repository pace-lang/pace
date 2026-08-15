use std::collections::HashSet;
use session::Symbol;

#[derive(Debug, Default)]
pub struct Scope {
    declared_names: HashSet<Symbol>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            declared_names: HashSet::new(),
        }
    }

    pub fn declare(&mut self, name: Symbol) -> bool {
        self.declared_names.insert(name)
    }

    pub fn is_declared_locally(&self, name: Symbol) -> bool {
        self.declared_names.contains(&name)
    }
}

#[derive(Debug)]
pub struct ScopeStack {
    scopes: Vec<Scope>,
}

impl Default for ScopeStack {
    fn default() -> Self {
        Self::new()
    }
}

impl ScopeStack {
    pub fn new() -> Self {
        // Start with a global scope
        Self {
            scopes: vec![Scope::new()],
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        } else {
            panic!("Cannot pop the global scope.");
        }
    }

    /// Attempts to declare a name in the current innermost scope.
    /// Returns true if successful, false if it was already declared in the *same* scope (re-declaration error).
    pub fn declare(&mut self, name: Symbol) -> bool {
        let current_scope = self.scopes.last_mut().unwrap();
        current_scope.declare(name)
    }

    /// Checks if a name resolves to a valid declaration in any accessible scope (innermost to outermost).
    pub fn resolve(&self, name: Symbol) -> bool {
        for scope in self.scopes.iter().rev() {
            if scope.is_declared_locally(name) {
                return true;
            }
        }
        false
    }
}
