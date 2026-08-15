use ast::Stmt;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct GenericDefinitionRegistry<'a> {
    classes: HashMap<session::Symbol, Stmt<'a>>,
    functions: HashMap<session::Symbol, Stmt<'a>>,
}

impl<'a> GenericDefinitionRegistry<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_class(&mut self, name: session::Symbol, stmt: Stmt<'a>) {
        self.classes.insert(name, stmt);
    }

    pub fn register_function(&mut self, name: session::Symbol, stmt: Stmt<'a>) {
        self.functions.insert(name, stmt);
    }

    pub fn get_class(&self, name: session::Symbol) -> Option<&Stmt<'a>> {
        self.classes.get(&name)
    }

    pub fn get_function(&self, name: session::Symbol) -> Option<&Stmt<'a>> {
        self.functions.get(&name)
    }
}
