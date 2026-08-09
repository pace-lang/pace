use std::collections::HashMap;
use crate::function::Function;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDef {
    pub name: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Program {
    pub functions: HashMap<String, Function>,
    pub classes: HashMap<String, ClassDef>,
}

impl Program {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            classes: HashMap::new(),
        }
    }
}
