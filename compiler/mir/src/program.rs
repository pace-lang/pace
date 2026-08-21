use crate::function::Function;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDef {
    pub name: String,
    pub is_struct: bool,
    pub is_actor: bool,
    pub fields: Vec<String>,
    pub static_fields: Vec<String>,
    pub weak_fields: std::collections::HashSet<String>,
    pub reference_fields: std::collections::HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForeignAbiType {
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Pointer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignFunction {
    pub name: String,
    pub symbol: String,
    pub param_types: Vec<ForeignAbiType>,
    pub return_type: Option<ForeignAbiType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariantDef {
    pub name: String,
    pub reference_payloads: std::collections::HashSet<usize>,
    pub struct_payloads: std::collections::HashMap<usize, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<EnumVariantDef>,
}

#[derive(Debug, Clone, Default)]
pub struct Program {
    pub functions: HashMap<String, Function>,
    pub classes: HashMap<String, ClassDef>,
    pub enums: HashMap<String, EnumDef>,
    pub foreign_functions: HashMap<String, ForeignFunction>,
}

impl Program {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            classes: HashMap::new(),
            enums: HashMap::new(),
            foreign_functions: HashMap::new(),
        }
    }
}
