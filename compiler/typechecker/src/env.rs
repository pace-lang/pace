use std::collections::HashMap;
use ast::types::Type;

#[derive(Debug)]
pub struct Scope {
    types: HashMap<String, Type>,
    mutables: HashMap<String, bool>,
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

impl Scope {
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
            mutables: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: String, ty: Type) {
        self.types.insert(name.clone(), ty);
        self.mutables.insert(name, false);
    }
    
    pub fn insert_var(&mut self, name: String, ty: Type, is_mutable: bool) {
        self.types.insert(name.clone(), ty);
        self.mutables.insert(name, is_mutable);
    }

    pub fn get(&self, name: &str) -> Option<&Type> {
        self.types.get(name)
    }

    pub fn is_mutable(&self, name: &str) -> Option<bool> {
        self.mutables.get(name).copied()
    }
}

#[derive(Debug)]
pub struct TypeEnvironment {
    scopes: Vec<Scope>,
}

impl Default for TypeEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeEnvironment {
    pub fn new() -> Self {
        let mut global_scope = Scope::new();
        global_scope.insert("print".into(), Type::BuiltinFunc);
        Self {
            scopes: vec![global_scope],
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

    pub fn declare(&mut self, name: String, ty: Type) {
        let current_scope = self.scopes.last_mut().unwrap();
        current_scope.insert(name, ty);
    }

    pub fn declare_var(&mut self, name: String, ty: Type, is_mutable: bool) {
        let current_scope = self.scopes.last_mut().unwrap();
        current_scope.insert_var(name, ty, is_mutable);
    }

    pub fn resolve(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        None
    }

    pub fn is_mutable(&self, name: &str) -> bool {
        for scope in self.scopes.iter().rev() {
            if let Some(mutability) = scope.is_mutable(name) {
                return mutability;
            }
        }
        false
    }
}
