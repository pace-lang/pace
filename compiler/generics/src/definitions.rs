use ast::Stmt;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct GenericDefinitionRegistry<'a> {
    classes: HashMap<session::Symbol, Stmt<'a>>,
    functions: HashMap<session::Symbol, Stmt<'a>>,
    interfaces: HashMap<session::Symbol, Stmt<'a>>,
    extensions: HashMap<session::Symbol, Vec<Stmt<'a>>>,
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

    pub fn register_interface(&mut self, name: session::Symbol, stmt: Stmt<'a>) {
        self.interfaces.insert(name, stmt);
    }

    pub fn register_extension(&mut self, target_name: session::Symbol, stmt: Stmt<'a>) {
        self.extensions.entry(target_name).or_default().push(stmt);
    }

    pub fn get_class(&self, name: session::Symbol) -> Option<&Stmt<'a>> {
        self.classes.get(&name)
    }

    pub fn get_function(&self, name: session::Symbol) -> Option<&Stmt<'a>> {
        self.functions.get(&name)
    }

    pub fn get_interface(&self, name: session::Symbol) -> Option<&Stmt<'a>> {
        self.interfaces.get(&name)
    }

    pub fn get_extensions(&self, target_name: session::Symbol) -> Option<&Vec<Stmt<'a>>> {
        self.extensions.get(&target_name)
    }
}
