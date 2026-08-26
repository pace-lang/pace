use cranelift::prelude::*;
use cranelift_module::{Module, FuncId, DataDescription, Linkage};
use pace_ast::{Expr, Stmt, BinaryOp};
use std::collections::HashMap;
use crate::compiler::ClassLayout;
use crate::compiler::CodegenError;

#[derive(Clone, Debug, PartialEq)]
pub enum VarType {
    Int,
    Float,
    String,
    Bool,
    Object(String),
    Struct(String),
    Enum(String),
    Nullable(Box<VarType>),
    Promise(Box<VarType>),
    Unknown,
}

impl VarType {
    pub fn to_cranelift_type(&self) -> cranelift::prelude::Type {
        match self {
            VarType::Float => cranelift::prelude::types::F64,
            _ => cranelift::prelude::types::I64, // Pointers and integers are I64
        }
    }
}

pub fn parse_vartype(s: &str, current_class: Option<&str>) -> VarType {
    let is_nullable = s.ends_with('?');
    let base_name = if is_nullable {
        &s[..s.len() - 1]
    } else {
        s
    };
    
    let base_ty = match base_name {
        "Int" => VarType::Int,
        "Float" => VarType::Float,
        "String" => VarType::String,
        "Bool" => VarType::Bool,
        "Self" => VarType::Object(current_class.unwrap_or("Self").to_string()),
        other => {
            if other.starts_with("Result_") || other.starts_with("Option_") {
                VarType::Enum(other.to_string())
            } else {
                VarType::Object(other.to_string())
            }
        }
    };
    
    if is_nullable {
        VarType::Nullable(Box::new(base_ty))
    } else {
        base_ty
    }
}

pub struct Translator;

impl Translator {
    pub fn translate_stmt(
        module: &mut impl Module,
        funcs: &HashMap<String, FuncId>,
        class_layouts: &HashMap<String, ClassLayout>,
        struct_layouts: &HashMap<String, crate::compiler::StructLayout>,
        enum_layouts: &HashMap<String, crate::compiler::EnumLayout>,
        builder: &mut FunctionBuilder,
        stmt: &Stmt,
        variables: &mut HashMap<String, (Variable, VarType)>,
        var_index: &mut usize,
        func_returns: &HashMap<String, VarType>,
        string_cache: &mut HashMap<String, String>,
        string_id: &mut usize,
    ) -> Result<(Value, bool), CodegenError> {
        match stmt {
            Stmt::VarDecl { name, initializer, .. } => {
                let mut var_ty = VarType::Unknown;
                let val = if let Some(expr) = initializer {
                    var_ty = Self::get_expr_type(expr, variables, func_returns, struct_layouts, class_layouts);
                    let mut val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, expr, variables, var_index, func_returns, string_cache, string_id)?;
                    
                    if let VarType::Struct(name) = &var_ty {
                        val = Self::copy_struct(module, struct_layouts, builder, name, val);
                    }
                    val
                } else {
                    builder.ins().iconst(types::I64, 0)
                };
                let val_ty = builder.func.dfg.value_type(val);
                let var = builder.declare_var(val_ty);
                builder.def_var(var, val);
                variables.insert(name.clone(), (var, var_ty));
                *var_index += 1;
                Ok((val, false))
            }
            Stmt::Expr(expr) => {
                let ty = Self::get_expr_type(expr, variables, func_returns, struct_layouts, class_layouts);
                let val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, expr, variables, var_index, func_returns, string_cache, string_id)?;
                if matches!(ty, VarType::Object(_)) {
                    let release_id = *funcs.get("release").unwrap();
                    let local_release = module.declare_func_in_func(release_id, builder.func);
                    builder.ins().call(local_release, &[val]);
                }
                Ok((val, false))
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let cond_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, condition, variables, var_index, func_returns, string_cache, string_id)?;
                
                let then_block = builder.create_block();
                let else_block = builder.create_block();
                let merge_block = builder.create_block();
                
                builder.ins().brif(cond_val, then_block, &[], else_block, &[]);
                
                // Then
                builder.switch_to_block(then_block);
                builder.seal_block(then_block);
                let (_then_res, then_term) = Self::translate_stmt(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, then_branch, variables, var_index, func_returns, string_cache, string_id)?;
                if !then_term {
                    builder.ins().jump(merge_block, &[]);
                }
                
                // Else
                builder.switch_to_block(else_block);
                builder.seal_block(else_block);
                let (_else_res, else_term) = if let Some(elb) = else_branch {
                    Self::translate_stmt(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, elb, variables, var_index, func_returns, string_cache, string_id)?
                } else {
                    (builder.ins().iconst(types::I64, 0), false)
                };
                if !else_term {
                    builder.ins().jump(merge_block, &[]);
                }
                
                let is_terminated = then_term && else_term;
                
                // Merge
                builder.switch_to_block(merge_block);
                builder.seal_block(merge_block);
                
                let res = builder.ins().iconst(types::I64, 0);
                
                if is_terminated {
                    builder.ins().return_(&[res]);
                }
                
                Ok((res, is_terminated))
            }
            Stmt::While { condition, body } => {
                let cond_block = builder.create_block();
                let body_block = builder.create_block();
                let exit_block = builder.create_block();
                
                builder.ins().jump(cond_block, &[]);
                builder.switch_to_block(cond_block);
                
                let cond_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, condition, variables, var_index, func_returns, string_cache, string_id)?;
                builder.ins().brif(cond_val, body_block, &[], exit_block, &[]);
                
                builder.seal_block(body_block);
                builder.switch_to_block(body_block);
                
                let (_, body_term) = Self::translate_stmt(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, body, variables, var_index, func_returns, string_cache, string_id)?;
                if !body_term {
                    builder.ins().jump(cond_block, &[]);
                }
                
                builder.seal_block(cond_block);
                
                builder.switch_to_block(exit_block);
                builder.seal_block(exit_block);
                
                Ok((builder.ins().iconst(types::I64, 0), false))
            }
            Stmt::Match { expr, arms } => {
                let expr_ty = Self::get_expr_type(expr, variables, func_returns, struct_layouts, class_layouts);
                let enum_name = if let VarType::Object(name) = expr_ty {
                    name
                } else if let VarType::Enum(name) = expr_ty {
                    name
                } else {
                    return Err(CodegenError { message: "Match expression must be an Enum type".to_string() });
                };
                let enum_layout = enum_layouts.get(&enum_name).unwrap().clone();
                
                let obj_ptr = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, expr, variables, var_index, func_returns, string_cache, string_id)?;
                
                let tag_val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), obj_ptr, 8);
                
                let mut blocks = Vec::new();
                for _ in 0..arms.len() {
                    blocks.push(builder.create_block());
                }
                let end_block = builder.create_block();
                
                for (i, (pattern, _)) in arms.iter().enumerate() {
                    if let pace_ast::Pattern::Variant { variant_name, .. } = pattern {
                        let (tag_id, _) = enum_layout.variants.get(variant_name).unwrap();
                        let next_check = builder.create_block();
                        let expected_tag = builder.ins().iconst(types::I64, *tag_id as i64);
                        let is_match = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::Equal, tag_val, expected_tag);
                        builder.ins().brif(is_match, blocks[i], &[], next_check, &[]);
                        
                        builder.seal_block(next_check);
                        builder.switch_to_block(next_check);
                    } else if matches!(pattern, pace_ast::Pattern::Wildcard) {
                        builder.ins().jump(blocks[i], &[]);
                        break; // No more checks needed
                    } else {
                        return Err(CodegenError { message: "Only Variant and Wildcard patterns are supported in Enums right now".to_string() });
                    }
                }
                builder.ins().jump(end_block, &[]); // Fallback if no match (shouldn't happen if exhaustive)
                
                for (i, (pattern, body)) in arms.iter().enumerate() {
                    let block = blocks[i];
                    builder.seal_block(block);
                    builder.switch_to_block(block);
                    
                    if let pace_ast::Pattern::Variant { variant_name, fields: pat_fields, .. } = pattern {
                        let (_, variant_types) = enum_layout.variants.get(variant_name).unwrap();
                        
                        if let Some(pat_fields) = pat_fields {
                            let mut offset = 16;
                            for (j, pat) in pat_fields.iter().enumerate() {
                                if let pace_ast::Pattern::Variable(var_name, _span) = pat {
                                    let field_val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), obj_ptr, offset);
                                    let var = builder.declare_var(types::I64);
                                    builder.def_var(var, field_val);
                                    variables.insert(var_name.clone(), (var, variant_types[j].clone()));
                                    *var_index += 1;
                                }
                                offset += 8;
                            }
                        }
                    }
                    
                    let (_, term) = Self::translate_stmt(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, body, variables, var_index, func_returns, string_cache, string_id)?;
                    if !term {
                        builder.ins().jump(end_block, &[]);
                    }
                }
                
                builder.seal_block(end_block);
                builder.switch_to_block(end_block);
                Ok((builder.ins().iconst(types::I64, 0), false))
            }
            Stmt::Loop { body } => {
                let body_block = builder.create_block();
                
                builder.ins().jump(body_block, &[]);
                
                builder.switch_to_block(body_block);
                
                let (_, body_term) = Self::translate_stmt(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, body, variables, var_index, func_returns, string_cache, string_id)?;
                if !body_term {
                    builder.ins().jump(body_block, &[]);
                }
                
                builder.seal_block(body_block);
                Ok((builder.ins().iconst(types::I64, 0), true))
            }
            Stmt::ForIn { item, iterable, body, .. } => {
                let iter_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, iterable, variables, var_index, func_returns, string_cache, string_id)?;
                let iter_ty = Self::get_expr_type(iterable, variables, func_returns, struct_layouts, class_layouts);
                
                let (length_offset, get_offset) = if let VarType::Object(type_name) = &iter_ty {
                    let layout = class_layouts.get(type_name).unwrap();
                    let l = layout.methods.get("length").unwrap().clone();
                    let g = layout.methods.get("get").unwrap().clone();
                    (l, g)
                } else {
                    return Err(CodegenError { message: "ForIn iterable must be an object implementing length() and get()".to_string() });
                };
                
                let ptr_ty = module.target_config().pointer_type();
                
                let idx_var = builder.declare_var(types::I64);
                let init_val = builder.ins().iconst(types::I64, 0);
                builder.def_var(idx_var, init_val);
                
                let cond_block = builder.create_block();
                let body_block = builder.create_block();
                let exit_block = builder.create_block();
                
                builder.ins().jump(cond_block, &[]);
                builder.switch_to_block(cond_block);
                
                // Get length
                let vtable_ptr_len = builder.ins().load(ptr_ty, cranelift::prelude::MemFlagsData::new(), iter_val, 8);
                let method_ptr_len = builder.ins().load(ptr_ty, cranelift::prelude::MemFlagsData::new(), vtable_ptr_len, length_offset as i32);
                let mut sig_len = module.make_signature();
                sig_len.params.push(cranelift::prelude::AbiParam::new(ptr_ty));
                sig_len.returns.push(cranelift::prelude::AbiParam::new(types::I64));
                let sig_len_ref = builder.import_signature(sig_len);
                let callee_len = builder.ins().call_indirect(sig_len_ref, method_ptr_len, &[iter_val]);
                let len_val = builder.inst_results(callee_len)[0];
                
                let curr_idx = builder.use_var(idx_var);
                let is_less = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::SignedLessThan, curr_idx, len_val);
                builder.ins().brif(is_less, body_block, &[], exit_block, &[]);
                
                builder.seal_block(body_block);
                builder.switch_to_block(body_block);
                
                // Get item
                let vtable_ptr_get = builder.ins().load(ptr_ty, cranelift::prelude::MemFlagsData::new(), iter_val, 8);
                let method_ptr_get = builder.ins().load(ptr_ty, cranelift::prelude::MemFlagsData::new(), vtable_ptr_get, get_offset as i32);
                let mut sig_get = module.make_signature();
                sig_get.params.push(cranelift::prelude::AbiParam::new(ptr_ty));
                sig_get.params.push(cranelift::prelude::AbiParam::new(types::I64));
                sig_get.returns.push(cranelift::prelude::AbiParam::new(types::I64));
                let sig_get_ref = builder.import_signature(sig_get);
                let callee_get = builder.ins().call_indirect(sig_get_ref, method_ptr_get, &[iter_val, curr_idx]);
                let item_val = builder.inst_results(callee_get)[0];
                
                let item_var = builder.declare_var(types::I64);
                builder.def_var(item_var, item_val);
                variables.insert(item.clone(), (item_var, VarType::Unknown));
                *var_index += 1;
                
                let (_, body_term) = Self::translate_stmt(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, body, variables, var_index, func_returns, string_cache, string_id)?;
                if !body_term {
                    let one = builder.ins().iconst(types::I64, 1);
                    let next_idx = builder.ins().iadd(curr_idx, one);
                    builder.def_var(idx_var, next_idx);
                    builder.ins().jump(cond_block, &[]);
                }
                
                builder.seal_block(cond_block);
                builder.switch_to_block(exit_block);
                builder.seal_block(exit_block);
                
                Ok((builder.ins().iconst(types::I64, 0), false))
            }
            Stmt::Module { body, .. } | Stmt::Block(body) => {
                let initial_vars: Vec<String> = variables.keys().cloned().collect();
                let mut last_val = builder.ins().iconst(types::I64, 0);
                let mut terminated = false;
                for s in body {
                    if !terminated {
                        let (val, term) = Self::translate_stmt(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, s, variables, var_index, func_returns, string_cache, string_id)?;
                        last_val = val;
                        terminated = term;
                    }
                }
                
                // Release local object variables
                if !terminated {
                    let current_vars: Vec<String> = variables.keys().cloned().collect();
                    for var_name in current_vars {
                        if !initial_vars.contains(&var_name) {
                            let (var, ty) = variables.get(&var_name).unwrap().clone();
                            if matches!(ty, VarType::Object(_)) {
                                let obj_val = builder.use_var(var);
                                let release_id = *funcs.get("release").unwrap_or_else(|| panic!("release not found"));
                                let local_release = module.declare_func_in_func(release_id, builder.func);
                                builder.ins().call(local_release, &[obj_val]);
                            }
                            variables.remove(&var_name);
                        }
                    }
                } else {
                    // Just remove from scope without emitting instructions, since block is filled.
                    // This causes a memory leak on early return, but prevents Cranelift panic.
                    // Proper fix requires a unified return block.
                    let current_vars: Vec<String> = variables.keys().cloned().collect();
                    for var_name in current_vars {
                        if !initial_vars.contains(&var_name) {
                            variables.remove(&var_name);
                        }
                    }
                }
                
                Ok((last_val, terminated))
            }
            Stmt::Return(expr_opt) => {
                let ret_val = if let Some(expr) = expr_opt {
                    let mut val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, expr, variables, var_index, func_returns, string_cache, string_id)?;
                    let val_ty = Self::get_expr_type(expr, variables, func_returns, struct_layouts, class_layouts);
                    if let VarType::Struct(name) = &val_ty {
                        val = Self::copy_struct(module, struct_layouts, builder, name, val);
                    }
                    val
                } else {
                    builder.ins().iconst(types::I64, 0)
                };
                
                // Release all active local object variables
                for (_var_name, (var, ty)) in variables.iter() {
                    if matches!(ty, VarType::Object(_)) {
                        let obj_val = builder.use_var(*var);
                        let release_id = *funcs.get("release").unwrap_or_else(|| panic!("release not found"));
                        let local_release = module.declare_func_in_func(release_id, builder.func);
                        builder.ins().call(local_release, &[obj_val]);
                    }
                }
                
                builder.ins().return_(&[ret_val]);
                Ok((ret_val, true))
            }
            _ => Ok((builder.ins().iconst(types::I64, 0), false))
        }
    }
    
    pub fn get_expr_type(expr: &Expr, variables: &HashMap<String, (Variable, VarType)>, func_returns: &HashMap<String, VarType>, struct_layouts: &HashMap<String, crate::compiler::StructLayout>, class_layouts: &HashMap<String, crate::compiler::ClassLayout>) -> VarType {
        match expr {
            Expr::IntLiteral(_) => VarType::Int,
            Expr::FloatLiteral(_) => VarType::Float,
            Expr::StringLiteral(_) => VarType::String,
            Expr::InterpolatedString(_) => VarType::String,
            Expr::BoolLiteral(_) => VarType::Bool,
            Expr::Identifier(name) => {
                if let Some((_, ty)) = variables.get(name) {
                    ty.clone()
                } else {
                    VarType::Unknown
                }
            }
            Expr::Binary { left, .. } => Self::get_expr_type(left, variables, func_returns, struct_layouts, class_layouts), // simplified
            Expr::MemberAccess { object, property, .. } => {
                if let Expr::Identifier(obj_name) = &**object {
                    if let Some(layout) = class_layouts.get(obj_name) {
                        if let Some((_, f_ty)) = layout.static_fields.get(property) {
                            return f_ty.clone();
                        }
                    } else if let Some(layout) = struct_layouts.get(obj_name) {
                        if let Some((_, f_ty)) = layout.static_fields.get(property) {
                            return f_ty.clone();
                        }
                    }
                }
                
                let obj_ty = Self::get_expr_type(object, variables, func_returns, struct_layouts, class_layouts);
                if let VarType::Object(obj_name) = obj_ty {
                    if let Some(layout) = class_layouts.get(&obj_name)
                        && let Some(f_ty) = layout.fields.get(property) {
                            return f_ty.1.clone();
                        }
                } else if let VarType::Struct(obj_name) = obj_ty
                    && let Some(layout) = struct_layouts.get(&obj_name)
                        && let Some(f_ty) = layout.fields.get(property) {
                            return f_ty.1.clone();
                        }
                VarType::Unknown
            }
            Expr::OptionalMemberAccess { object, property } => {
                let obj_ty = Self::get_expr_type(object, variables, func_returns, struct_layouts, class_layouts);
                if let VarType::Object(obj_name) = obj_ty {
                    if let Some(layout) = class_layouts.get(&obj_name)
                        && let Some(f_ty) = layout.fields.get(property) {
                            return f_ty.1.clone();
                        }
                } else if let VarType::Struct(obj_name) = obj_ty
                    && let Some(layout) = struct_layouts.get(&obj_name)
                        && let Some(f_ty) = layout.fields.get(property) {
                            return f_ty.1.clone();
                        }
                VarType::Unknown
            }
            Expr::Unwrap(inner) => {
                Self::get_expr_type(inner, variables, func_returns, struct_layouts, class_layouts)
            }
            Expr::NullCoalesce { left, right } => {
                let left_ty = Self::get_expr_type(left, variables, func_returns, struct_layouts, class_layouts);
                if matches!(left_ty, VarType::Unknown) {
                    Self::get_expr_type(right, variables, func_returns, struct_layouts, class_layouts)
                } else {
                    left_ty
                }
            }
            Expr::Null => VarType::Unknown,
            Expr::Call { callee, .. } => {
                if let Expr::Identifier(name) = &**callee {
                    if let Some(ty) = func_returns.get(name) {
                        return ty.clone();
                    } else if struct_layouts.contains_key(name) {
                        return VarType::Struct(name.clone());
                    } else if class_layouts.contains_key(name) {
                        return VarType::Object(name.clone());
                    } else if name.starts_with("Result_") || name.starts_with("Option_") || name.contains("__Result_") || name.contains("__Option_") {
                        return VarType::Enum(name.clone());
                    } else if let Some(pos) = name.rfind("__") {
                        let base_name = &name[pos + 2..];
                        if base_name.chars().next().is_some_and(|c| c.is_uppercase()) {
                            return VarType::Object(name.clone());
                        }
                    }
                } else if let Expr::MemberAccess { object, property, .. } = &**callee {
                    if let Expr::Identifier(obj_name) = &**object
                        && (obj_name.starts_with("Result_") || obj_name.starts_with("Option_")) {
                            return VarType::Enum(obj_name.clone());
                        }
                    let obj_ty = Self::get_expr_type(object, variables, func_returns, struct_layouts, class_layouts);
                    if let VarType::Object(obj_name) = obj_ty {
                        let full_name = format!("{}_{}", obj_name, property);
                        if let Some(ty) = func_returns.get(&full_name) {
                            return ty.clone();
                        }
                        return VarType::Unknown;
                    } else if let VarType::Struct(obj_name) = obj_ty {
                        let full_name = format!("{}_{}", obj_name, property);
                        if let Some(ty) = func_returns.get(&full_name) {
                            return ty.clone();
                        }
                    }
                }
                VarType::Unknown
            }
            Expr::Try(inner) => {
                let inner_ty = Self::get_expr_type(inner, variables, func_returns, struct_layouts, class_layouts);
                if let VarType::Enum(name) = inner_ty {
                    if name.starts_with("Result_") {
                        let parts: Vec<&str> = name.split('_').collect();
                        if parts.len() >= 3 {
                            return parse_vartype(parts[1], None);
                        }
                    } else if name.starts_with("Option_") {
                        let parts: Vec<&str> = name.split('_').collect();
                        if parts.len() >= 2 {
                            return parse_vartype(parts[1], None);
                        }
                    }
                }
                VarType::Unknown
            }
            Expr::Await(inner) => {
                let inner_ty = Self::get_expr_type(inner, variables, func_returns, struct_layouts, class_layouts);
                if let VarType::Promise(t) = inner_ty {
                    *t
                } else {
                    VarType::Unknown
                }
            }
            _ => VarType::Unknown,
        }
    }

    fn copy_struct(
        module: &mut impl Module,
        struct_layouts: &HashMap<String, crate::compiler::StructLayout>,
        builder: &mut FunctionBuilder,
        name: &str,
        src_ptr: Value,
    ) -> Value {
        let layout = struct_layouts.get(name).unwrap();
        let ptr_ty = module.target_config().pointer_type();
        
        let size = layout.size as u32;
        let slot_data = cranelift::prelude::StackSlotData::new(cranelift::prelude::StackSlotKind::ExplicitSlot, size, 0);
        let slot = builder.create_sized_stack_slot(slot_data);
        let dst_ptr = builder.ins().stack_addr(ptr_ty, slot, 0);
        
        // Copy each field
        for &(offset, _) in layout.fields.values() {
            let field_val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), src_ptr, offset as i32);
            builder.ins().store(cranelift::prelude::MemFlagsData::new(), field_val, dst_ptr, offset as i32);
        }
        
        dst_ptr
    }

    pub fn translate_expr(
        module: &mut impl Module,
        funcs: &HashMap<String, FuncId>,
        class_layouts: &HashMap<String, crate::compiler::ClassLayout>,
        struct_layouts: &HashMap<String, crate::compiler::StructLayout>,
        enum_layouts: &HashMap<String, crate::compiler::EnumLayout>,
        builder: &mut FunctionBuilder,
        expr: &Expr,
        variables: &mut HashMap<String, (Variable, VarType)>,
        var_index: &mut usize,
        func_returns: &HashMap<String, VarType>,
        string_cache: &mut HashMap<String, String>,
        string_id: &mut usize,
    ) -> Result<Value, CodegenError> {
        match expr {
            Expr::IntLiteral(i) => Ok(builder.ins().iconst(types::I64, *i)),
            Expr::FloatLiteral(f) => Ok(builder.ins().f64const(*f)),
            Expr::StringLiteral(s) => {
                
                
                let string_name = if let Some(name) = string_cache.get(s) {
                    name.clone()
                } else {
                    let id = *string_id; *string_id += 1;
                    let name = format!("__str_const_{}", id);
                    
                    let mut data_ctx = DataDescription::new();
                    let mut bytes = s.clone().into_bytes();
                    bytes.push(0); // Null terminator
                    data_ctx.define(bytes.into_boxed_slice());
                    
                    let data_id = module.declare_data(&name, Linkage::Local, false, false).unwrap();
                    module.define_data(data_id, &data_ctx).unwrap();
                    
                    string_cache.insert(s.clone(), name.clone());
                    name
                };
                
                let data_id = module.declare_data(&string_name, Linkage::Local, false, false).unwrap();
                let local_id = module.declare_data_in_func(data_id, builder.func);
                let ptr_ty = module.target_config().pointer_type();
                Ok(builder.ins().symbol_value(ptr_ty, local_id))
            }
            Expr::InterpolatedString(parts) => {
                if parts.is_empty() {
                    let mut data_ctx = DataDescription::new();
                    data_ctx.define(vec![0].into_boxed_slice());
                    let id = *string_id; *string_id += 1;
                    let string_name = format!("__empty_str_{}", id);
                    let data_id = module.declare_data(&string_name, Linkage::Local, false, false).unwrap();
                    module.define_data(data_id, &data_ctx).unwrap();
                    let local_id = module.declare_data_in_func(data_id, builder.func);
                    let ptr_ty = module.target_config().pointer_type();
                    return Ok(builder.ins().symbol_value(ptr_ty, local_id));
                }
                
                let mut current_val = None;
                for part in parts {
                    let part_ty = Self::get_expr_type(part, variables, func_returns, struct_layouts, class_layouts);
                    let val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, part, variables, var_index, func_returns, string_cache, string_id)?;
                    
                    let str_val = if part_ty == VarType::String {
                        val
                    } else if part_ty == VarType::Float {
                        let mut float_val = val;
                        if builder.func.dfg.value_type(val) == types::I64 {
                            float_val = builder.ins().bitcast(types::F64, cranelift::prelude::MemFlagsData::new(), val);
                        }
                        let to_str = module.declare_func_in_func(*funcs.get("float_to_string").unwrap(), builder.func);
                        let call = builder.ins().call(to_str, &[float_val]);
                        builder.inst_results(call)[0]
                    } else if part_ty == VarType::Bool {
                        let to_str = module.declare_func_in_func(*funcs.get("bool_to_string").unwrap(), builder.func);
                        let call = builder.ins().call(to_str, &[val]);
                        builder.inst_results(call)[0]
                    } else { // Assume Int
                        let to_str = module.declare_func_in_func(*funcs.get("int_to_string").unwrap(), builder.func);
                        let call = builder.ins().call(to_str, &[val]);
                        builder.inst_results(call)[0]
                    };
                    
                    if let Some(prev) = current_val {
                        let concat = module.declare_func_in_func(*funcs.get("concat_strings").unwrap(), builder.func);
                        let call = builder.ins().call(concat, &[prev, str_val]);
                        current_val = Some(builder.inst_results(call)[0]);
                    } else {
                        current_val = Some(str_val);
                    }
                }
                
                Ok(current_val.unwrap())
            }
            Expr::BoolLiteral(b) => {
                let val = if *b { 1 } else { 0 };
                Ok(builder.ins().iconst(types::I64, val))
            }
            Expr::Identifier(name) => {
                if let Some((var, ty)) = variables.get(name) {
                    let val = builder.use_var(*var);
                    if matches!(ty, VarType::Object(_)) {
                        let retain_id = *funcs.get("retain").unwrap();
                        let local_retain = module.declare_func_in_func(retain_id, builder.func);
                        builder.ins().call(local_retain, &[val]);
                    }
                    Ok(val)
                } else {
                    Err(CodegenError { message: format!("Undefined variable: {} (enum_layouts: {:?})", name, enum_layouts.keys().collect::<Vec<_>>()) })
                }
            }
            Expr::Binary { left, op, right } => {
                let lhs = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, left, variables, var_index, func_returns, string_cache, string_id)?;
                let rhs = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, right, variables, var_index, func_returns, string_cache, string_id)?;
                
                let ty = builder.func.dfg.value_type(lhs);
                let is_float = ty == types::F64;
                
                match op {
                    BinaryOp::Add => {
                        if is_float { Ok(builder.ins().fadd(lhs, rhs)) } else { Ok(builder.ins().iadd(lhs, rhs)) }
                    }
                    BinaryOp::Sub => {
                        if is_float { Ok(builder.ins().fsub(lhs, rhs)) } else { Ok(builder.ins().isub(lhs, rhs)) }
                    }
                    BinaryOp::Mul => {
                        if is_float { Ok(builder.ins().fmul(lhs, rhs)) } else { Ok(builder.ins().imul(lhs, rhs)) }
                    }
                    BinaryOp::Div => {
                        if is_float { Ok(builder.ins().fdiv(lhs, rhs)) } else { Ok(builder.ins().sdiv(lhs, rhs)) }
                    }
                    BinaryOp::Mod => {
                        if is_float {
                            // Cranelift doesn't have a native frem instruction, so we'll throw an error or trap.
                            // But for now, we just trap or unimplemented, or for integer just do srem.
                            // Actually since float mod isn't widely used in benchmarks, we'll just panic for float mod.
                            panic!("Float modulo not supported yet");
                        } else {
                            Ok(builder.ins().srem(lhs, rhs))
                        }
                    }
                    BinaryOp::Eq => {
                        if is_float {
                            let c = builder.ins().fcmp(FloatCC::Equal, lhs, rhs);
                            Ok(builder.ins().uextend(types::I64, c))
                        } else {
                            let c = builder.ins().icmp(IntCC::Equal, lhs, rhs);
                            Ok(builder.ins().uextend(types::I64, c))
                        }
                    }
                    BinaryOp::NotEq => {
                        if is_float {
                            let c = builder.ins().fcmp(FloatCC::NotEqual, lhs, rhs);
                            Ok(builder.ins().uextend(types::I64, c))
                        } else {
                            let c = builder.ins().icmp(IntCC::NotEqual, lhs, rhs);
                            Ok(builder.ins().uextend(types::I64, c))
                        }
                    }
                    BinaryOp::Less => {
                        if is_float {
                            let c = builder.ins().fcmp(FloatCC::LessThan, lhs, rhs);
                            Ok(builder.ins().uextend(types::I64, c))
                        } else {
                            let c = builder.ins().icmp(IntCC::SignedLessThan, lhs, rhs);
                            Ok(builder.ins().uextend(types::I64, c))
                        }
                    }
                    BinaryOp::LessEq => {
                        if is_float {
                            let c = builder.ins().fcmp(FloatCC::LessThanOrEqual, lhs, rhs);
                            Ok(builder.ins().uextend(types::I64, c))
                        } else {
                            let c = builder.ins().icmp(IntCC::SignedLessThanOrEqual, lhs, rhs);
                            Ok(builder.ins().uextend(types::I64, c))
                        }
                    }
                    BinaryOp::Greater => {
                        if is_float {
                            let c = builder.ins().fcmp(FloatCC::GreaterThan, lhs, rhs);
                            Ok(builder.ins().uextend(types::I64, c))
                        } else {
                            let c = builder.ins().icmp(IntCC::SignedGreaterThan, lhs, rhs);
                            Ok(builder.ins().uextend(types::I64, c))
                        }
                    }
                    BinaryOp::GreaterEq => {
                        if is_float {
                            let c = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, lhs, rhs);
                            Ok(builder.ins().uextend(types::I64, c))
                        } else {
                            let c = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, lhs, rhs);
                            Ok(builder.ins().uextend(types::I64, c))
                        }
                    }
                    BinaryOp::And => {
                        Ok(builder.ins().band(lhs, rhs)) // bitwise AND works for booleans represented as 0/1 integers
                    }
                    BinaryOp::Or => {
                        Ok(builder.ins().bor(lhs, rhs)) // bitwise OR works for booleans represented as 0/1 integers
                    }
                }
            }
            Expr::Await(inner) => {
                let promise_ptr = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, inner, variables, var_index, func_returns, string_cache, string_id)?;
                let await_id = *funcs.get("__pace_promise_await").unwrap();
                let local_await = module.declare_func_in_func(await_id, builder.func);
                let call = builder.ins().call(local_await, &[promise_ptr]);
                Ok(builder.inst_results(call)[0])
            }
            Expr::Try(inner) => {
                let inner_ptr = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, inner, variables, var_index, func_returns, string_cache, string_id)?;
                
                // Read the tag at offset 8 (0 = Ok/Some, 1 = Err/None usually based on how variants are sorted)
                // Wait, we need to know exactly which tag is Ok/Err. 
                // We'll dynamically look up the tag ID of "Ok" or "Some".
                let inner_ty = Self::get_expr_type(inner, variables, func_returns, struct_layouts, class_layouts);
                let enum_name = if let VarType::Enum(name) = inner_ty { name } else { return Err(CodegenError { message: "? operator used on non-enum".to_string() }); };
                let enum_layout = enum_layouts.get(&enum_name).unwrap();
                
                // Determine which tags represent the success and failure
                let is_result = enum_name.starts_with("Result_");
                let (success_tag, _) = if is_result {
                    enum_layout.variants.get("Ok").unwrap()
                } else {
                    enum_layout.variants.get("Some").unwrap()
                };
                
                let tag_val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), inner_ptr, 8);
                let expected_tag = builder.ins().iconst(types::I64, *success_tag as i64);
                let is_success = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::Equal, tag_val, expected_tag);
                
                let continue_block = builder.create_block();
                let err_block = builder.create_block();
                
                builder.ins().brif(is_success, continue_block, &[], err_block, &[]);
                
                // Error Block: Return the whole enum from the function
                builder.seal_block(err_block);
                builder.switch_to_block(err_block);
                builder.ins().return_(&[inner_ptr]);
                
                // Continue Block: Extract the first field of the Ok/Some variant
                builder.seal_block(continue_block);
                builder.switch_to_block(continue_block);
                
                // The value of Ok/Some is at offset 16 (since tag is 8, ARC is 0)
                // Note: Only works if Ok/Some has a 64-bit primitive or pointer (which is true for our types right now)
                let val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), inner_ptr, 16);
                Ok(val)
            }
            Expr::Assign { target, value } => {
                let mut val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, value, variables, var_index, func_returns, string_cache, string_id)?;
                let val_ty = Self::get_expr_type(value, variables, func_returns, struct_layouts, class_layouts);
                if let VarType::Struct(name) = &val_ty {
                    val = Self::copy_struct(module, struct_layouts, builder, name, val);
                }
                if let Expr::Identifier(name) = &**target {
                    if let Some((var, ty)) = variables.get(name) {
                        if matches!(ty, VarType::Object(_)) {
                            // Release old value
                            let old_val = builder.use_var(*var);
                            let release_id = *funcs.get("release").unwrap();
                            let local_release = module.declare_func_in_func(release_id, builder.func);
                            builder.ins().call(local_release, &[old_val]);
                            
                            // Retain new value for the variable (caller gets the original +1)
                            let retain_id = *funcs.get("retain").unwrap();
                            let local_retain = module.declare_func_in_func(retain_id, builder.func);
                            builder.ins().call(local_retain, &[val]);
                        }
                        builder.def_var(*var, val);
                        Ok(val)
                    } else {
                        Err(CodegenError { message: format!("Variable '{}' not found in JIT environment", name) })
                    }
                } else if let Expr::MemberAccess { object, property, .. } = &**target {
                    if let Expr::Identifier(obj_name) = &**object {
                        let maybe_static_field = if let Some(layout) = class_layouts.get(obj_name) {
                            layout.static_fields.get(property)
                        } else if let Some(layout) = struct_layouts.get(obj_name) {
                            layout.static_fields.get(property)
                        } else {
                            None
                        };
                        
                        if let Some(&(data_id, ref f_ty)) = maybe_static_field {
                            let ptr_ty = module.target_config().pointer_type();
                            let data_ref = module.declare_data_in_func(data_id, builder.func);
                            let addr = builder.ins().symbol_value(ptr_ty, data_ref);
                            
                            if matches!(f_ty, VarType::Object(_)) {
                                let old_val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), addr, 0);
                                let release_id = *funcs.get("release").unwrap();
                                let local_release = module.declare_func_in_func(release_id, builder.func);
                                builder.ins().call(local_release, &[old_val]);
                                
                                let retain_id = *funcs.get("retain").unwrap();
                                let local_retain = module.declare_func_in_func(retain_id, builder.func);
                                builder.ins().call(local_retain, &[val]);
                            }
                            
                            builder.ins().store(cranelift::prelude::MemFlagsData::new(), val, addr, 0);
                            return Ok(val);
                        }
                    }

                    let obj_ptr = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, object, variables, var_index, func_returns, string_cache, string_id)?;
                    
                    let obj_type = Self::get_expr_type(object, variables, func_returns, struct_layouts, class_layouts);
                    let (f_offset, f_ty) = match obj_type {
                        VarType::Object(name) => {
                            let layout = class_layouts.get(&name).unwrap();
                            layout.fields.get(property).unwrap().clone()
                        }
                        VarType::Struct(name) => {
                            let layout = struct_layouts.get(&name).unwrap();
                            layout.fields.get(property).unwrap().clone()
                        }
                        _ => panic!("MemberAccess assign on non-object type: {:?}", obj_type),
                    };
                    
                    if matches!(f_ty, VarType::Object(_)) {
                        let old_val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), obj_ptr, f_offset as i32);
                        let release_id = *funcs.get("release").unwrap();
                        let local_release = module.declare_func_in_func(release_id, builder.func);
                        builder.ins().call(local_release, &[old_val]);
                        
                        let retain_id = *funcs.get("retain").unwrap();
                        let local_retain = module.declare_func_in_func(retain_id, builder.func);
                        builder.ins().call(local_retain, &[val]);
                    }
                    
                    builder.ins().store(cranelift::prelude::MemFlagsData::new(), val, obj_ptr, f_offset as i32);
                    Ok(val)
                } else {
                    Err(CodegenError { message: "Invalid assignment target".to_string() })
                }
            }
            Expr::Call { callee, args } => {
                if let Expr::Identifier(func_name) = &**callee {
                    if func_name == "print" {
                        let arg_expr = &args[0];
                        let arg_ty = Self::get_expr_type(arg_expr, variables, func_returns, struct_layouts, class_layouts);
                        
                        let arg_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, arg_expr, variables, var_index, func_returns, string_cache, string_id)?;
                        let ty = builder.func.dfg.value_type(arg_val);
                        
                        let target_name = if ty == types::F64 {
                            "print_float"
                        } else if arg_ty == VarType::String {
                            "print_string"
                        } else {
                            "print_int" // Fallback to int
                        };
                        
                        let func_id = *funcs.get(target_name).unwrap();
                        let local_func = module.declare_func_in_func(func_id, builder.func);
                        let call = builder.ins().call(local_func, &[arg_val]);
                        
                        let results = builder.inst_results(call);
                        if results.is_empty() {
                            return Ok(builder.ins().iconst(types::I64, 0));
                        } else {
                            return Ok(results[0]);
                        }
                    }
                    else if let Some(&func_id) = funcs.get(func_name) {
                        let local_func = module.declare_func_in_func(func_id, builder.func);
                        let mut arg_vals = Vec::new();
                        for arg in args {
                            let mut arg_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, arg, variables, var_index, func_returns, string_cache, string_id)?;
                            let arg_ty = Self::get_expr_type(arg, variables, func_returns, struct_layouts, class_layouts);
                            if let VarType::Struct(name) = &arg_ty {
                                arg_val = Self::copy_struct(module, struct_layouts, builder, name, arg_val);
                            }
                            arg_vals.push(arg_val);
                        }
                        let call = builder.ins().call(local_func, &arg_vals);
                        
                        let results = builder.inst_results(call);
                        if results.is_empty() {
                            return Ok(builder.ins().iconst(types::I64, 0));
                        } else {
                            return Ok(results[0]);
                        }
                    } else if let Some(layout) = class_layouts.get(func_name) {
                        let ptr_ty = module.target_config().pointer_type();
                        
                        let malloc_id = *funcs.get("malloc").unwrap();
                        let local_malloc = module.declare_func_in_func(malloc_id, builder.func);
                        
                        let size = 16 + layout.fields.len() * 8;
                        let size_val = builder.ins().iconst(types::I64, size as i64);
                        
                        let call = builder.ins().call(local_malloc, &[size_val]);
                        let obj_ptr = builder.inst_results(call)[0];
                        
                        // Set ARC count to 1
                        let one = builder.ins().iconst(types::I64, 1);
                        builder.ins().store(cranelift::prelude::MemFlagsData::new(), one, obj_ptr, 0);
                        
                        // Set VTable
                        let vtable_gv = module.declare_data_in_func(layout.vtable_id, builder.func);
                        let vtable_addr = builder.ins().symbol_value(ptr_ty, vtable_gv);
                        builder.ins().store(cranelift::prelude::MemFlagsData::new(), vtable_addr, obj_ptr, 8);
                        
                        let zero = builder.ins().iconst(types::I64, 0);
                        for (field_name, &(offset, _)) in &layout.fields {
                            if field_name == "__mailbox" {
                                let mb_create_id = *funcs.get("__pace_mailbox_create").unwrap();
                                let local_mb_create = module.declare_func_in_func(mb_create_id, builder.func);
                                let mb_call = builder.ins().call(local_mb_create, &[]);
                                let mb_ptr = builder.inst_results(mb_call)[0];
                                builder.ins().store(cranelift::prelude::MemFlagsData::new(), mb_ptr, obj_ptr, offset as i32);
                            } else {
                                builder.ins().store(cranelift::prelude::MemFlagsData::new(), zero, obj_ptr, offset as i32);
                            }
                        }
                        
                        // Call init if it exists
                        let init_name = format!("{}_init", func_name);
                        if let Some(&init_id) = funcs.get(&init_name) {
                            let local_init = module.declare_func_in_func(init_id, builder.func);
                            let mut arg_vals = vec![obj_ptr];
                            for arg in args {
                                arg_vals.push(Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, arg, variables, var_index, func_returns, string_cache, string_id)?);
                            }
                            builder.ins().call(local_init, &arg_vals);
                        }
                        
                        return Ok(obj_ptr);
                    } else if let Some(layout) = struct_layouts.get(func_name) {
                        let ptr_ty = module.target_config().pointer_type();
                        let size = layout.size as u32;
                        let slot_data = cranelift::prelude::StackSlotData::new(cranelift::prelude::StackSlotKind::ExplicitSlot, size, 0);
                        let slot = builder.create_sized_stack_slot(slot_data);
                        let obj_ptr = builder.ins().stack_addr(ptr_ty, slot, 0);
                        
                        // Create a sorted list of fields by offset to map args correctly
                        let mut sorted_fields: Vec<_> = layout.fields.iter().collect();
                        sorted_fields.sort_by_key(|&(_, &(offset, _))| offset);
                        
                        for (i, arg) in args.iter().enumerate() {
                            if let Some((_, (offset, _))) = sorted_fields.get(i) {
                                let mut arg_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, arg, variables, var_index, func_returns, string_cache, string_id)?;
                                let arg_ty = Self::get_expr_type(arg, variables, func_returns, struct_layouts, class_layouts);
                                if let VarType::Struct(name) = &arg_ty {
                                    arg_val = Self::copy_struct(module, struct_layouts, builder, name, arg_val);
                                }
                                builder.ins().store(cranelift::prelude::MemFlagsData::new(), arg_val, obj_ptr, *offset as i32);
                            }
                        }
                        
                        return Ok(obj_ptr);
                    }
                } else if let Expr::MemberAccess { object, property, .. } = &**callee {
                    if let Expr::Identifier(obj_name) = &**object {
                        if enum_layouts.contains_key(obj_name) {
                            let constructor_name = format!("{}_{}", obj_name, property);
                            let func_id = funcs.get(&constructor_name)
                                .unwrap_or_else(|| panic!("Enum constructor {} not found", constructor_name));
                            let local_callee = module.declare_func_in_func(*func_id, builder.func);
                            
                            let mut arg_vals = Vec::new();
                            for arg in args {
                                arg_vals.push(Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, arg, variables, var_index, func_returns, string_cache, string_id)?);
                            }
                            
                            let call = builder.ins().call(local_callee, &arg_vals);
                            return Ok(builder.inst_results(call)[0]);
                        } else if class_layouts.contains_key(obj_name) || struct_layouts.contains_key(obj_name) {
                            // STATIC METHOD CALL!
                            let static_method_name = format!("{}_{}", obj_name, property);
                            let func_id = funcs.get(&static_method_name)
                                .unwrap_or_else(|| panic!("Static method {} not found", static_method_name));
                            let local_callee = module.declare_func_in_func(*func_id, builder.func);
                            
                            let mut arg_vals = Vec::new();
                            for arg in args {
                                let mut arg_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, arg, variables, var_index, func_returns, string_cache, string_id)?;
                                let arg_ty = Self::get_expr_type(arg, variables, func_returns, struct_layouts, class_layouts);
                                if let VarType::Struct(name) = &arg_ty {
                                    arg_val = Self::copy_struct(module, struct_layouts, builder, name, arg_val);
                                }
                                arg_vals.push(arg_val);
                            }
                            
                            let call = builder.ins().call(local_callee, &arg_vals);
                            let results = builder.inst_results(call);
                            if results.is_empty() {
                                return Ok(builder.ins().iconst(types::I64, 0));
                            } else {
                                return Ok(results[0]);
                            }
                        }
                    }

                    let obj_ptr = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, object, variables, var_index, func_returns, string_cache, string_id)?;
                    let ptr_ty = module.target_config().pointer_type();
                    
                    let obj_type = Self::get_expr_type(object, variables, func_returns, struct_layouts, class_layouts);
                    let (m_offset, is_actor) = if let VarType::Object(type_name) = &obj_type {
                        let layout = class_layouts.get(type_name)
                            .unwrap_or_else(|| panic!("Class or interface {} not found in layouts", type_name));
                        (*layout.methods.get(property).unwrap_or_else(|| panic!("Method {} not found in {}", property, type_name)), layout.fields.contains_key("__mailbox"))
                    } else {
                        let layout = class_layouts.values().find(|l| l.methods.contains_key(property))
                            .unwrap_or_else(|| panic!("Method {} not found in any class layout", property));
                        (*layout.methods.get(property).unwrap(), false)
                    };
                    
                    let vtable_ptr = builder.ins().load(ptr_ty, cranelift::prelude::MemFlagsData::new(), obj_ptr, 8);
                    let method_ptr = builder.ins().load(ptr_ty, cranelift::prelude::MemFlagsData::new(), vtable_ptr, m_offset as i32);
                    
                    let mut arg_vals = vec![obj_ptr];
                    for arg in args {
                        let mut arg_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, arg, variables, var_index, func_returns, string_cache, string_id)?;
                        let arg_ty = Self::get_expr_type(arg, variables, func_returns, struct_layouts, class_layouts);
                        if let VarType::Struct(name) = &arg_ty {
                            arg_val = Self::copy_struct(module, struct_layouts, builder, name, arg_val);
                        }
                        arg_vals.push(arg_val);
                    }

                    if is_actor {
                        let promise_create_id = *funcs.get("__pace_promise_create").unwrap();
                        let local_promise_create = module.declare_func_in_func(promise_create_id, builder.func);
                        let promise_call = builder.ins().call(local_promise_create, &[]);
                        let promise_ptr = builder.inst_results(promise_call)[0];
                        
                        let layout = class_layouts.get(if let VarType::Object(name) = &obj_type { name } else { unreachable!() }).unwrap();
                        let mb_offset = layout.fields.get("__mailbox").unwrap().0;
                        let mailbox_ptr = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), obj_ptr, mb_offset as i32);
                        
                        let malloc_id = *funcs.get("malloc").unwrap();
                        let local_malloc = module.declare_func_in_func(malloc_id, builder.func);
                        let tuple_size = arg_vals.len() * 8;
                        let size_val = builder.ins().iconst(types::I64, tuple_size as i64);
                        let malloc_call = builder.ins().call(local_malloc, &[size_val]);
                        let tuple_ptr = builder.inst_results(malloc_call)[0];
                        
                        for (i, val) in arg_vals.iter().enumerate() {
                            builder.ins().store(cranelift::prelude::MemFlagsData::new(), *val, tuple_ptr, (i * 8) as i32);
                        }
                        
                        let mb_send_id = *funcs.get("__pace_mailbox_send").unwrap();
                        let local_mb_send = module.declare_func_in_func(mb_send_id, builder.func);
                        builder.ins().call(local_mb_send, &[mailbox_ptr, method_ptr, tuple_ptr, promise_ptr]);
                        
                        return Ok(promise_ptr);
                    } else {
                        let mut sig = module.make_signature();
                        sig.params.push(AbiParam::new(ptr_ty)); // self
                        for _ in args {
                            sig.params.push(AbiParam::new(types::I64));
                        }
                        sig.returns.push(AbiParam::new(types::I64));
                        
                        let sig_ref = builder.import_signature(sig);
                        let call = builder.ins().call_indirect(sig_ref, method_ptr, &arg_vals);
                        
                        let results = builder.inst_results(call);
                        if results.is_empty() {
                            return Ok(builder.ins().iconst(types::I64, 0));
                        } else {
                            return Ok(results[0]);
                        }
                    }
                }
                Err(CodegenError { message: format!("Cannot resolve function call: {:?}", callee) })
            }
            Expr::MemberAccess { object, property, .. } => {
                if let Expr::Identifier(obj_name) = &**object {
                    let maybe_static_field = if let Some(layout) = class_layouts.get(obj_name) {
                        layout.static_fields.get(property)
                    } else if let Some(layout) = struct_layouts.get(obj_name) {
                        layout.static_fields.get(property)
                    } else {
                        None
                    };
                    
                    if let Some(&(data_id, ref f_ty)) = maybe_static_field {
                        let ptr_ty = module.target_config().pointer_type();
                        let data_ref = module.declare_data_in_func(data_id, builder.func);
                        let addr = builder.ins().symbol_value(ptr_ty, data_ref);
                        let val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), addr, 0);
                        
                        if matches!(f_ty, VarType::Object(_)) {
                            let retain_id = *funcs.get("retain").unwrap();
                            let local_retain = module.declare_func_in_func(retain_id, builder.func);
                            builder.ins().call(local_retain, &[val]);
                        }
                        
                        return Ok(val);
                    }
                }
                
                let obj_ptr = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, object, variables, var_index, func_returns, string_cache, string_id)?;
                
                let obj_type = Self::get_expr_type(object, variables, func_returns, struct_layouts, class_layouts);
                let (f_offset, f_ty) = match obj_type {
                    VarType::Object(name) => {
                        let layout = class_layouts.get(&name).unwrap();
                        layout.fields.get(property).unwrap().clone()
                    }
                    VarType::Struct(name) => {
                        let layout = struct_layouts.get(&name).unwrap();
                        layout.fields.get(property).unwrap().clone()
                    }
                    _ => panic!("MemberAccess on non-object type: {:?}", obj_type),
                };
                
                let val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), obj_ptr, f_offset as i32);
                
                if matches!(f_ty, VarType::Object(_)) {
                    let retain_id = *funcs.get("retain").unwrap();
                    let local_retain = module.declare_func_in_func(retain_id, builder.func);
                    builder.ins().call(local_retain, &[val]);
                }
                
                
                Ok(val)
            }
            Expr::Null => Ok(builder.ins().iconst(types::I64, 0)),
            Expr::Unwrap(inner) => {
                let inner_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, inner, variables, var_index, func_returns, string_cache, string_id)?;
                
                let is_null = builder.ins().icmp_imm_u(cranelift::prelude::IntCC::Equal, inner_val, 0);
                
                // Trap if null
                let trap_block = builder.create_block();
                let cont_block = builder.create_block();
                
                builder.ins().brif(is_null, trap_block, &[], cont_block, &[]);
                
                builder.switch_to_block(trap_block);
                builder.seal_block(trap_block);
                builder.ins().trap(cranelift::prelude::TrapCode::user(1).unwrap()); // Null pointer dereference
                
                builder.switch_to_block(cont_block);
                builder.seal_block(cont_block);
                
                Ok(inner_val)
            }
            Expr::NullCoalesce { left, right } => {
                let left_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, left, variables, var_index, func_returns, string_cache, string_id)?;
                
                let is_null = builder.ins().icmp_imm_u(cranelift::prelude::IntCC::Equal, left_val, 0);
                
                let right_block = builder.create_block();
                let merge_block = builder.create_block();
                builder.append_block_param(merge_block, types::I64);
                
                builder.ins().brif(is_null, right_block, &[], merge_block, &[cranelift::codegen::ir::BlockArg::Value(left_val)]);
                
                builder.switch_to_block(right_block);
                builder.seal_block(right_block);
                let right_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, right, variables, var_index, func_returns, string_cache, string_id)?;
                builder.ins().jump(merge_block, &[cranelift::codegen::ir::BlockArg::Value(right_val)]);
                
                builder.switch_to_block(merge_block);
                builder.seal_block(merge_block);
                
                let result = builder.block_params(merge_block)[0];
                Ok(result)
            }
            Expr::OptionalMemberAccess { object, property } => {
                let obj_ptr = Self::translate_expr(module, funcs, class_layouts, struct_layouts, enum_layouts, builder, object, variables, var_index, func_returns, string_cache, string_id)?;
                
                let is_null = builder.ins().icmp_imm_u(cranelift::prelude::IntCC::Equal, obj_ptr, 0);
                
                let access_block = builder.create_block();
                let merge_block = builder.create_block();
                builder.append_block_param(merge_block, types::I64);
                
                let zero_val = builder.ins().iconst(types::I64, 0);
                builder.ins().brif(is_null, merge_block, &[cranelift::codegen::ir::BlockArg::Value(zero_val)], access_block, &[]);
                
                builder.switch_to_block(access_block);
                builder.seal_block(access_block);
                
                let obj_type = Self::get_expr_type(object, variables, func_returns, struct_layouts, class_layouts);
                let (f_offset, f_ty) = match obj_type {
                    VarType::Object(name) => {
                        let layout = class_layouts.get(&name).unwrap();
                        layout.fields.get(property).unwrap().clone()
                    }
                    VarType::Struct(name) => {
                        let layout = struct_layouts.get(&name).unwrap();
                        layout.fields.get(property).unwrap().clone()
                    }
                    _ => panic!("OptionalMemberAccess on non-object type: {:?}", obj_type),
                };
                
                let val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), obj_ptr, f_offset as i32);
                
                if matches!(f_ty, VarType::Object(_)) {
                    let retain_id = *funcs.get("retain").unwrap();
                    let local_retain = module.declare_func_in_func(retain_id, builder.func);
                    builder.ins().call(local_retain, &[val]);
                }
                
                builder.ins().jump(merge_block, &[cranelift::codegen::ir::BlockArg::Value(val)]);
                
                builder.switch_to_block(merge_block);
                builder.seal_block(merge_block);
                
                let result = builder.block_params(merge_block)[0];
                Ok(result)
            }
            _ => Err(CodegenError { message: format!("Cannot translate expression: {:?}", expr) })
        }
    }
}
