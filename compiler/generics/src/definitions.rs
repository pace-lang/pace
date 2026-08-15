use ast::Stmt;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct GenericDefinitionRegistry {
    classes: HashMap<session::Symbol, Stmt>,
    functions: HashMap<session::Symbol, Stmt>,
}

impl GenericDefinitionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_class(&mut self, name: session::Symbol, stmt: Stmt) {
        self.classes.insert(name, stmt);
    }

    pub fn register_function(&mut self, name: session::Symbol, stmt: Stmt) {
        self.functions.insert(name, stmt);
    }

    pub fn get_class(&self, name: session::Symbol) -> Option<&Stmt> {
        self.classes.get(&name)
    }

    pub fn get_function(&self, name: session::Symbol) -> Option<&Stmt> {
        self.functions.get(&name)
    }
}
