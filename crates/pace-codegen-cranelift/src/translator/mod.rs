use crate::context::CodegenContext;
use crate::layouts::{EnumLayout, StructLayout};
use cranelift::prelude::*;
use cranelift_module::Module;
use std::collections::HashMap;

pub mod expr;
pub mod stmt;
pub mod mir;
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

pub fn parse_type_annotation(
    ann: &pace_ast::TypeAnnotation,
    current_class: Option<&str>,
    struct_layouts: Option<&HashMap<ustr::Ustr, StructLayout>>,
    enum_layouts: Option<&HashMap<ustr::Ustr, EnumLayout>>,
) -> VarType {
    if ann.is_function {
        let mut params = Vec::new();
        if let Some(fn_params) = &ann.function_params {
            for p in fn_params {
                params.push(parse_type_annotation(
                    p,
                    current_class,
                    struct_layouts,
                    enum_layouts,
                ));
            }
        }
        let ret = if let Some(r) = &ann.function_return {
            Box::new(parse_type_annotation(
                r,
                current_class,
                struct_layouts,
                enum_layouts,
            ))
        } else {
            Box::new(VarType::Unknown)
        };
        let base = VarType::Function(params, ret);
        if ann.is_nullable {
            return VarType::Nullable(Box::new(base));
        }
        return base;
    }

    parse_vartype(&ann.name, current_class, struct_layouts, enum_layouts)
}

pub fn parse_vartype(
    s: &str,
    current_class: Option<&str>,
    struct_layouts: Option<&HashMap<ustr::Ustr, StructLayout>>,
    enum_layouts: Option<&HashMap<ustr::Ustr, EnumLayout>>,
) -> VarType {
    let is_nullable = s.ends_with('?');
    let base_name = if is_nullable { &s[..s.len() - 1] } else { s };

    let base_ty = match base_name {
        "Int" => VarType::Int,
        "Float" => VarType::Float,
        "Byte" => VarType::Byte,
        "String" => VarType::String,
        "Bool" => VarType::Bool,
        "Self" => VarType::Object(current_class.unwrap_or(&ustr::Ustr::from("Self")).into()),
        other => {
            if let Some(enums) = enum_layouts {
                if enums.contains_key(&ustr::Ustr::from(other)) {
                    return VarType::Enum(ustr::Ustr::from(other));
                }
            } else if other.starts_with("Result_") || other.starts_with("Option_") {
                return VarType::Enum(ustr::Ustr::from(other));
            }

            if let Some(structs) = struct_layouts
                && structs.contains_key(&ustr::Ustr::from(other))
            {
                return VarType::Struct(other.to_string());
            }

            VarType::Object(ustr::Ustr::from(other))
        }
    };

    if is_nullable {
        VarType::Nullable(Box::new(base_ty))
    } else {
        base_ty
    }
}

pub struct Translator<'a, 'b, M: Module> {
    pub arena: &'a pace_ast::arena::AstArena,
    pub context: &'a mut CodegenContext<M>,
    pub builder: &'a mut FunctionBuilder<'b>,
    pub variables: &'a mut HashMap<ustr::Ustr, (Variable, VarType)>,
    pub var_index: &'a mut usize,
    pub func_returns: &'a HashMap<ustr::Ustr, VarType>,
    pub pending_closures: &'a mut Vec<(ustr::Ustr, pace_ast::Expr, Vec<(ustr::Ustr, VarType)>)>,
    pub is_global_context: bool,
}

impl<'a, 'b, M: Module> Translator<'a, 'b, M> {}
