pub mod mir;
use cranelift_module::Module;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum VarType {
    Int,
    Float,
    Byte,
    String,
    Bool,
    Object(ustr::Ustr),
    Struct(String),
    Enum(ustr::Ustr),
    Nullable(Box<VarType>),
    Promise(Box<VarType>),
    Function(Vec<VarType>, Box<VarType>),
    Unknown,
}

impl VarType {
    pub fn to_cranelift_type(&self) -> cranelift::prelude::Type {
        match self {
            VarType::Float => cranelift::prelude::types::F64,
            VarType::Byte => cranelift::prelude::types::I8,
            _ => cranelift::prelude::types::I64, // Pointers and integers are I64
        }
    }
}
