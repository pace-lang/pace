use std::collections::HashMap;

/// A lightweight, interned symbol representing a string.
/// Using a `u32` ensures it's small, copyable, and cheap to hash/compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol(pub u32);

pub struct Interner {
    map: HashMap<String, Symbol>,
    vec: Vec<String>,
}

impl Interner {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            vec: Vec::new(),
        }
    }

    /// Interns a string and returns its unique Symbol.
    pub fn intern(&mut self, string: &str) -> Symbol {
        if let Some(&symbol) = self.map.get(string) {
            return symbol;
        }
        let symbol = Symbol(self.vec.len() as u32);
        self.vec.push(string.to_string());
        self.map.insert(string.to_string(), symbol);
        symbol
    }

    /// Retrieves the string associated with a Symbol.
    pub fn lookup(&self, symbol: Symbol) -> &str {
        &self.vec[symbol.0 as usize]
    }
}

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}
