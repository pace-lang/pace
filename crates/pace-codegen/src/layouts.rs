use crate::translator::VarType;
use cranelift_module::{DataId, FuncId};
use miette::Diagnostic;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct StructLayout {
    pub name: ustr::Ustr,
    pub fields: HashMap<ustr::Ustr, (usize, VarType)>,
    pub static_fields: HashMap<ustr::Ustr, (DataId, VarType)>,
    pub size: usize,
}

#[derive(Debug, Clone)]
pub struct ClassLayout {
    pub name: ustr::Ustr,
    pub fields: HashMap<ustr::Ustr, (usize, VarType)>,
    pub methods: HashMap<ustr::Ustr, usize>,
    pub static_fields: HashMap<ustr::Ustr, (DataId, VarType)>,
    pub vtable_id: DataId,
}

#[derive(Debug, Clone)]
pub struct InterfaceLayout {
    pub name: ustr::Ustr,
    pub methods: HashMap<ustr::Ustr, usize>,
}

#[derive(Error, Diagnostic, Debug)]
#[error("Codegen error: {message}")]
#[diagnostic(code(pace::codegen_error))]
pub struct CodegenError {
    pub message: String,
}

#[derive(Clone)]
pub struct EnumLayout {
    pub name: ustr::Ustr,
    pub max_size: u64,
    pub variants: HashMap<ustr::Ustr, (u64, Vec<VarType>)>,
    pub drop_func_id: FuncId,
}
