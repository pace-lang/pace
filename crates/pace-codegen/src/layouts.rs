use crate::translator::VarType;
use cranelift_module::{DataId, FuncId};
use miette::Diagnostic;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct StructLayout {
    pub name: String,
    pub fields: HashMap<String, (usize, VarType)>,
    pub static_fields: HashMap<String, (DataId, VarType)>,
    pub size: usize,
}

#[derive(Debug, Clone)]
pub struct ClassLayout {
    pub name: String,
    pub fields: HashMap<String, (usize, VarType)>,
    pub methods: HashMap<String, usize>,
    pub static_fields: HashMap<String, (DataId, VarType)>,
    pub vtable_id: DataId,
}

#[derive(Debug, Clone)]
pub struct InterfaceLayout {
    pub name: String,
    pub methods: HashMap<String, usize>,
}

#[derive(Error, Diagnostic, Debug)]
#[error("Codegen error: {message}")]
#[diagnostic(code(pace::codegen_error))]
pub struct CodegenError {
    pub message: String,
}

#[derive(Clone)]
pub struct EnumLayout {
    pub name: String,
    pub max_size: u64,
    pub variants: HashMap<String, (u64, Vec<VarType>)>,
    pub drop_func_id: FuncId,
}
