use session::Symbol;
use std::collections::HashMap;

#[derive(Debug)]
pub struct ScopeStack {
    /// Tracks how many times a symbol is currently active across all accessible scopes
    active_counts: HashMap<Symbol, usize>,
    /// Tracks which symbols were declared in which scope (by depth)
    scope_decls: Vec<Vec<Symbol>>,
}

impl Default for ScopeStack {
    fn default() -> Self {
        Self::new()
    }
}

impl ScopeStack {
    pub fn new() -> Self {
        Self {
            active_counts: HashMap::new(),
            scope_decls: vec![Vec::new()], // Start with a global scope
        }
    }

    pub fn push_scope(&mut self) {
        self.scope_decls.push(Vec::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scope_decls.len() > 1 {
            let decls = self.scope_decls.pop().unwrap();
            for sym in decls {
                if let Some(count) = self.active_counts.get_mut(&sym) {
                    *count -= 1;
                    if *count == 0 {
                        self.active_counts.remove(&sym);
                    }
                }
            }
        } else {
            panic!("Cannot pop the global scope.");
        }
    }

    /// Attempts to declare a name in the current innermost scope.
    /// Returns true if successful, false if it was already declared in the *same* scope (re-declaration error).
    pub fn declare(&mut self, name: Symbol) -> bool {
        let current_decls = self.scope_decls.last_mut().unwrap();
        if current_decls.contains(&name) {
            return false;
        }
        current_decls.push(name);
        *self.active_counts.entry(name).or_insert(0) += 1;
        true
    }

    /// Checks if a name resolves to a valid declaration in any accessible scope.
    pub fn resolve(&self, name: Symbol) -> bool {
        self.active_counts.contains_key(&name)
    }
}
