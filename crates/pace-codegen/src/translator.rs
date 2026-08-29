use crate::context::CodegenContext;
use crate::layouts::{CodegenError, EnumLayout, StructLayout};
use cranelift::prelude::*;
use cranelift_module::{DataDescription, Linkage, Module};
use pace_ast::{BinaryOp, Expr, Stmt};
use std::collections::HashMap;

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
    Function(Vec<VarType>, Box<VarType>),
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

pub fn parse_type_annotation(
    ann: &pace_ast::TypeAnnotation,
    current_class: Option<&str>,
    struct_layouts: Option<&HashMap<String, StructLayout>>,
    enum_layouts: Option<&HashMap<String, EnumLayout>>,
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
    struct_layouts: Option<&HashMap<String, StructLayout>>,
    enum_layouts: Option<&HashMap<String, EnumLayout>>,
) -> VarType {
    let is_nullable = s.ends_with('?');
    let base_name = if is_nullable { &s[..s.len() - 1] } else { s };

    let base_ty = match base_name {
        "Int" => VarType::Int,
        "Float" => VarType::Float,
        "String" => VarType::String,
        "Bool" => VarType::Bool,
        "Self" => VarType::Object(current_class.unwrap_or("Self").to_string()),
        other => {
            if let Some(enums) = enum_layouts {
                if enums.contains_key(other) {
                    return VarType::Enum(other.to_string());
                }
            } else if other.starts_with("Result_") || other.starts_with("Option_") {
                return VarType::Enum(other.to_string());
            }

            if let Some(structs) = struct_layouts {
                if structs.contains_key(other) {
                    return VarType::Struct(other.to_string());
                }
            }

            VarType::Object(other.to_string())
        }
    };

    if is_nullable {
        VarType::Nullable(Box::new(base_ty))
    } else {
        base_ty
    }
}

pub struct Translator<'a, 'b, M: Module> {
    pub context: &'a mut CodegenContext<M>,
    pub builder: &'a mut FunctionBuilder<'b>,
    pub variables: &'a mut HashMap<String, (Variable, VarType)>,
    pub var_index: &'a mut usize,
    pub func_returns: &'a HashMap<String, VarType>,
    pub pending_closures: &'a mut Vec<(String, pace_ast::Expr, Vec<(String, VarType)>)>,
    pub is_global_context: bool,
}

impl<'a, 'b, M: Module> Translator<'a, 'b, M> {
    pub fn translate_stmt(&mut self, stmt: &Stmt) -> Result<(Value, bool), CodegenError> {
        match stmt {
            Stmt::VarDecl {
                name, initializer, ..
            } => {
                let mut var_ty = VarType::Unknown;
                let val = if let Some(expr) = initializer {
                    var_ty = self.get_expr_type(expr);
                    let mut val = self.translate_expr(expr)?;

                    if let VarType::Struct(name) = &var_ty {
                        val = self.copy_struct(name, val);
                    }
                    val
                } else {
                    self.builder.ins().iconst(types::I64, 0)
                };

                if self.is_global_context {
                    if let Some(&data_id) = self.context.global_vars.get(name) {
                        let local_data = self
                            .context
                            .module
                            .declare_data_in_func(data_id, self.builder.func);
                        let ptr = self.builder.ins().symbol_value(
                            self.context.module.target_config().pointer_type(),
                            local_data,
                        );
                        self.builder.ins().store(
                            cranelift::prelude::MemFlagsData::new(),
                            val,
                            ptr,
                            0,
                        );
                        return Ok((val, false));
                    }
                }

                let val_ty = self.builder.func.dfg.value_type(val);
                let var = self.builder.declare_var(val_ty);
                self.builder.def_var(var, val);
                self.variables.insert(name.clone(), (var, var_ty));
                *self.var_index += 1;
                Ok((val, false))
            }
            Stmt::Expr(expr) => {
                let ty = self.get_expr_type(expr);
                let val = self.translate_expr(expr)?;
                if matches!(ty, VarType::Object(_)) {
                    let release_id = *self.context.funcs.get("release").unwrap();
                    let local_release = self
                        .context
                        .module
                        .declare_func_in_func(release_id, self.builder.func);
                    self.builder.ins().call(local_release, &[val]);
                }
                Ok((val, false))
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_val = self.translate_expr(condition)?;

                let then_block = self.builder.create_block();
                let else_block = self.builder.create_block();
                let merge_block = self.builder.create_block();

                self.builder
                    .ins()
                    .brif(cond_val, then_block, &[], else_block, &[]);

                // Then
                self.builder.switch_to_block(then_block);
                self.builder.seal_block(then_block);
                let (_then_res, then_term) = self.translate_stmt(then_branch)?;
                if !then_term {
                    self.builder.ins().jump(merge_block, &[]);
                }

                // Else
                self.builder.switch_to_block(else_block);
                self.builder.seal_block(else_block);
                let (_else_res, else_term) = if let Some(elb) = else_branch {
                    self.translate_stmt(elb)?
                } else {
                    (self.builder.ins().iconst(types::I64, 0), false)
                };
                if !else_term {
                    self.builder.ins().jump(merge_block, &[]);
                }

                let is_terminated = then_term && else_term;

                // Merge
                self.builder.switch_to_block(merge_block);
                self.builder.seal_block(merge_block);

                let res = self.builder.ins().iconst(types::I64, 0);

                if is_terminated {
                    self.builder.ins().return_(&[res]);
                }

                Ok((res, is_terminated))
            }
            Stmt::While { condition, body } => {
                let cond_block = self.builder.create_block();
                let body_block = self.builder.create_block();
                let exit_block = self.builder.create_block();

                self.builder.ins().jump(cond_block, &[]);
                self.builder.switch_to_block(cond_block);

                let cond_val = self.translate_expr(condition)?;
                self.builder
                    .ins()
                    .brif(cond_val, body_block, &[], exit_block, &[]);

                self.builder.seal_block(body_block);
                self.builder.switch_to_block(body_block);

                let (_, body_term) = self.translate_stmt(body)?;
                if !body_term {
                    self.builder.ins().jump(cond_block, &[]);
                }

                self.builder.seal_block(cond_block);

                self.builder.switch_to_block(exit_block);
                self.builder.seal_block(exit_block);

                Ok((self.builder.ins().iconst(types::I64, 0), false))
            }
            Stmt::Match { expr, arms } => {
                let expr_ty = self.get_expr_type(expr);
                let enum_name = if let VarType::Object(name) = expr_ty {
                    name
                } else if let VarType::Enum(name) = expr_ty {
                    name
                } else {
                    return Err(CodegenError {
                        message: "Match expression must be an Enum type".to_string(),
                    });
                };
                let enum_layout = self.context.enum_layouts.get(&enum_name).unwrap().clone();

                let obj_ptr = self.translate_expr(expr)?;

                let tag_val = self.builder.ins().load(
                    types::I64,
                    cranelift::prelude::MemFlagsData::new(),
                    obj_ptr,
                    8,
                );

                let mut blocks = Vec::new();
                for _ in 0..arms.len() {
                    blocks.push(self.builder.create_block());
                }
                let end_block = self.builder.create_block();

                for (i, (pattern, _)) in arms.iter().enumerate() {
                    if let pace_ast::Pattern::Variant { variant_name, .. } = pattern {
                        let (tag_id, _) = enum_layout.variants.get(variant_name).unwrap();
                        let next_check = self.builder.create_block();
                        let expected_tag = self.builder.ins().iconst(types::I64, *tag_id as i64);
                        let is_match = self.builder.ins().icmp(
                            cranelift::codegen::ir::condcodes::IntCC::Equal,
                            tag_val,
                            expected_tag,
                        );
                        self.builder
                            .ins()
                            .brif(is_match, blocks[i], &[], next_check, &[]);

                        self.builder.seal_block(next_check);
                        self.builder.switch_to_block(next_check);
                    } else if matches!(pattern, pace_ast::Pattern::Wildcard) {
                        self.builder.ins().jump(blocks[i], &[]);
                        break; // No more checks needed
                    } else {
                        return Err(CodegenError { message: "Only Variant and Wildcard patterns are supported in Enums right now".to_string() });
                    }
                }
                self.builder.ins().jump(end_block, &[]); // Fallback if no match (shouldn't happen if exhaustive)

                for (i, (pattern, body)) in arms.iter().enumerate() {
                    let block = blocks[i];
                    self.builder.seal_block(block);
                    self.builder.switch_to_block(block);

                    if let pace_ast::Pattern::Variant {
                        variant_name,
                        fields: pat_fields,
                        ..
                    } = pattern
                    {
                        let (_, variant_types) = enum_layout.variants.get(variant_name).unwrap();

                        if let Some(pat_fields) = pat_fields {
                            let mut offset = 16;
                            for (j, pat) in pat_fields.iter().enumerate() {
                                if let pace_ast::Pattern::Variable(var_name, _span) = pat {
                                    let field_val = self.builder.ins().load(
                                        types::I64,
                                        cranelift::prelude::MemFlagsData::new(),
                                        obj_ptr,
                                        offset,
                                    );
                                    let var = self.builder.declare_var(types::I64);
                                    self.builder.def_var(var, field_val);
                                    self.variables
                                        .insert(var_name.clone(), (var, variant_types[j].clone()));
                                    *self.var_index += 1;
                                }
                                offset += 8;
                            }
                        }
                    }

                    let (_, term) = self.translate_stmt(body)?;
                    if !term {
                        self.builder.ins().jump(end_block, &[]);
                    }
                }

                self.builder.seal_block(end_block);
                self.builder.switch_to_block(end_block);
                Ok((self.builder.ins().iconst(types::I64, 0), false))
            }
            Stmt::Loop { body } => {
                let body_block = self.builder.create_block();

                self.builder.ins().jump(body_block, &[]);

                self.builder.switch_to_block(body_block);

                let (_, body_term) = self.translate_stmt(body)?;
                if !body_term {
                    self.builder.ins().jump(body_block, &[]);
                }

                self.builder.seal_block(body_block);
                Ok((self.builder.ins().iconst(types::I64, 0), true))
            }
            Stmt::ForIn {
                item,
                iterable,
                body,
                ..
            } => {
                let iter_val = self.translate_expr(iterable)?;
                let iter_ty = self.get_expr_type(iterable);

                let (length_offset, get_offset) = if let VarType::Object(type_name) = &iter_ty {
                    let layout = self.context.class_layouts.get(type_name).unwrap();
                    let l = layout.methods.get("length").unwrap().clone();
                    let g = layout.methods.get("get").unwrap().clone();
                    (l, g)
                } else {
                    return Err(CodegenError {
                        message: "ForIn iterable must be an object implementing length() and get()"
                            .to_string(),
                    });
                };

                let ptr_ty = self.context.module.target_config().pointer_type();

                let idx_var = self.builder.declare_var(types::I64);
                let init_val = self.builder.ins().iconst(types::I64, 0);
                self.builder.def_var(idx_var, init_val);

                let cond_block = self.builder.create_block();
                let body_block = self.builder.create_block();
                let exit_block = self.builder.create_block();

                self.builder.ins().jump(cond_block, &[]);
                self.builder.switch_to_block(cond_block);

                // Get length
                let vtable_ptr_len = self.builder.ins().load(
                    ptr_ty,
                    cranelift::prelude::MemFlagsData::new(),
                    iter_val,
                    8,
                );
                let method_ptr_len = self.builder.ins().load(
                    ptr_ty,
                    cranelift::prelude::MemFlagsData::new(),
                    vtable_ptr_len,
                    length_offset as i32,
                );
                let mut sig_len = self.context.module.make_signature();
                sig_len
                    .params
                    .push(cranelift::prelude::AbiParam::new(ptr_ty));
                sig_len
                    .returns
                    .push(cranelift::prelude::AbiParam::new(types::I64));
                let sig_len_ref = self.builder.import_signature(sig_len);
                let callee_len =
                    self.builder
                        .ins()
                        .call_indirect(sig_len_ref, method_ptr_len, &[iter_val]);
                let len_val = self.builder.inst_results(callee_len)[0];

                let curr_idx = self.builder.use_var(idx_var);
                let is_less = self.builder.ins().icmp(
                    cranelift::codegen::ir::condcodes::IntCC::SignedLessThan,
                    curr_idx,
                    len_val,
                );
                self.builder
                    .ins()
                    .brif(is_less, body_block, &[], exit_block, &[]);

                self.builder.seal_block(body_block);
                self.builder.switch_to_block(body_block);

                // Get item
                let vtable_ptr_get = self.builder.ins().load(
                    ptr_ty,
                    cranelift::prelude::MemFlagsData::new(),
                    iter_val,
                    8,
                );
                let method_ptr_get = self.builder.ins().load(
                    ptr_ty,
                    cranelift::prelude::MemFlagsData::new(),
                    vtable_ptr_get,
                    get_offset as i32,
                );
                let mut sig_get = self.context.module.make_signature();
                sig_get
                    .params
                    .push(cranelift::prelude::AbiParam::new(ptr_ty));
                sig_get
                    .params
                    .push(cranelift::prelude::AbiParam::new(types::I64));
                sig_get
                    .returns
                    .push(cranelift::prelude::AbiParam::new(types::I64));
                let sig_get_ref = self.builder.import_signature(sig_get);
                let callee_get = self.builder.ins().call_indirect(
                    sig_get_ref,
                    method_ptr_get,
                    &[iter_val, curr_idx],
                );
                let item_val = self.builder.inst_results(callee_get)[0];

                let item_var = self.builder.declare_var(types::I64);
                self.builder.def_var(item_var, item_val);
                self.variables
                    .insert(item.clone(), (item_var, VarType::Unknown));
                *self.var_index += 1;

                let (_, body_term) = self.translate_stmt(body)?;
                if !body_term {
                    let one = self.builder.ins().iconst(types::I64, 1);
                    let next_idx = self.builder.ins().iadd(curr_idx, one);
                    self.builder.def_var(idx_var, next_idx);
                    self.builder.ins().jump(cond_block, &[]);
                }

                self.builder.seal_block(cond_block);
                self.builder.switch_to_block(exit_block);
                self.builder.seal_block(exit_block);

                Ok((self.builder.ins().iconst(types::I64, 0), false))
            }
            Stmt::Module { body, .. } | Stmt::Block(body) => {
                let initial_vars: Vec<String> = self.variables.keys().cloned().collect();
                let mut last_val = self.builder.ins().iconst(types::I64, 0);
                let mut terminated = false;
                for s in body {
                    if !terminated {
                        let (val, term) = self.translate_stmt(s)?;
                        last_val = val;
                        terminated = term;
                    }
                }

                // Release local object variables
                if !terminated {
                    let current_vars: Vec<String> = self.variables.keys().cloned().collect();
                    for var_name in current_vars {
                        if !initial_vars.contains(&var_name) {
                            let (var, ty) = self.variables.get(&var_name).unwrap().clone();
                            if matches!(ty, VarType::Object(_)) {
                                let obj_val = self.builder.use_var(var);
                                let release_id = *self
                                    .context
                                    .funcs
                                    .get("release")
                                    .unwrap_or_else(|| panic!("release not found"));
                                let local_release = self
                                    .context
                                    .module
                                    .declare_func_in_func(release_id, self.builder.func);
                                self.builder.ins().call(local_release, &[obj_val]);
                            }
                            self.variables.remove(&var_name);
                        }
                    }
                } else {
                    // Just remove from scope without emitting instructions, since block is filled.
                    // This causes a memory leak on early return, but prevents Cranelift panic.
                    // Proper fix requires a unified return block.
                    let current_vars: Vec<String> = self.variables.keys().cloned().collect();
                    for var_name in current_vars {
                        if !initial_vars.contains(&var_name) {
                            self.variables.remove(&var_name);
                        }
                    }
                }

                Ok((last_val, terminated))
            }
            Stmt::Return(expr_opt) => {
                let ret_val = if let Some(expr) = expr_opt {
                    let mut val = self.translate_expr(expr)?;
                    let val_ty = self.get_expr_type(expr);
                    if let VarType::Struct(name) = &val_ty {
                        val = self.copy_struct(name, val);
                    }
                    val
                } else {
                    self.builder.ins().iconst(types::I64, 0)
                };

                // Release all active local object variables
                for (_var_name, (var, ty)) in self.variables.iter() {
                    if matches!(ty, VarType::Object(_)) {
                        let obj_val = self.builder.use_var(*var);
                        let release_id = *self
                            .context
                            .funcs
                            .get("release")
                            .unwrap_or_else(|| panic!("release not found"));
                        let local_release = self
                            .context
                            .module
                            .declare_func_in_func(release_id, self.builder.func);
                        self.builder.ins().call(local_release, &[obj_val]);
                    }
                }

                self.builder.ins().return_(&[ret_val]);
                Ok((ret_val, true))
            }
            _ => Ok((self.builder.ins().iconst(types::I64, 0), false)),
        }
    }

    pub fn get_expr_type(&self, expr: &Expr) -> VarType {
        match expr {
            Expr::IntLiteral(_) => VarType::Int,
            Expr::FloatLiteral(_) => VarType::Float,
            Expr::StringLiteral(_) => VarType::String,
            Expr::InterpolatedString(_) => VarType::String,
            Expr::BoolLiteral(_) => VarType::Bool,
            Expr::Identifier(name, _) => {
                if let Some((_, ty)) = self.variables.get(name) {
                    ty.clone()
                } else {
                    VarType::Unknown
                }
            }
            Expr::Binary { left, .. } => self.get_expr_type(left), // simplified
            Expr::MemberAccess {
                object,
                property,
                computed_class: _,
                is_static_operator: _,
            } => {
                if let Expr::Identifier(obj_name, _) = &**object {
                    if let Some(layout) = self.context.class_layouts.get(obj_name) {
                        if let Some((_, f_ty)) = layout.static_fields.get(property) {
                            return f_ty.clone();
                        }
                    } else if let Some(layout) = self.context.struct_layouts.get(obj_name) {
                        if let Some((_, f_ty)) = layout.static_fields.get(property) {
                            return f_ty.clone();
                        }
                    }
                }

                let obj_ty = self.get_expr_type(object);
                if let VarType::Object(obj_name) = obj_ty {
                    if let Some(layout) = self.context.class_layouts.get(&obj_name)
                        && let Some(f_ty) = layout.fields.get(property)
                    {
                        return f_ty.1.clone();
                    }
                } else if let VarType::Struct(obj_name) = obj_ty
                    && let Some(layout) = self.context.struct_layouts.get(&obj_name)
                    && let Some(f_ty) = layout.fields.get(property)
                {
                    return f_ty.1.clone();
                }
                VarType::Unknown
            }
            Expr::OptionalMemberAccess { object, property } => {
                let obj_ty = self.get_expr_type(object);
                if let VarType::Object(obj_name) = obj_ty {
                    if let Some(layout) = self.context.class_layouts.get(&obj_name)
                        && let Some(f_ty) = layout.fields.get(property)
                    {
                        return f_ty.1.clone();
                    }
                } else if let VarType::Struct(obj_name) = obj_ty
                    && let Some(layout) = self.context.struct_layouts.get(&obj_name)
                    && let Some(f_ty) = layout.fields.get(property)
                {
                    return f_ty.1.clone();
                }
                VarType::Unknown
            }
            Expr::Unwrap(inner) => {
                let inner_ty = self.get_expr_type(inner);
                if let VarType::Nullable(nested) = inner_ty {
                    *nested
                } else {
                    inner_ty
                }
            }
            Expr::NullCoalesce { left, right } => {
                let left_ty = self.get_expr_type(left);
                if matches!(left_ty, VarType::Unknown) {
                    self.get_expr_type(right)
                } else {
                    left_ty
                }
            }
            Expr::Null => VarType::Unknown,
            Expr::Call { callee, .. } => {
                if let Expr::Identifier(name, _) = &**callee {
                    if let Some(ty) = self.func_returns.get(name) {
                        return ty.clone();
                    } else if self.context.struct_layouts.contains_key(name) {
                        return VarType::Struct(name.clone());
                    } else if self.context.class_layouts.contains_key(name) {
                        return VarType::Object(name.clone());
                    } else if name.starts_with("Result_")
                        || name.starts_with("Option_")
                        || name.contains("__Result_")
                        || name.contains("__Option_")
                    {
                        return VarType::Enum(name.clone());
                    } else if let Some(pos) = name.rfind("__") {
                        let base_name = &name[pos + 2..];
                        if base_name.chars().next().is_some_and(|c| c.is_uppercase()) {
                            return VarType::Object(name.clone());
                        }
                    }
                } else if let Expr::MemberAccess {
                    object,
                    property,
                    computed_class: _,
                    is_static_operator: _,
                } = &**callee
                {
                    if let Expr::Identifier(obj_name, _) = &**object {
                        if obj_name.starts_with("Result_") || obj_name.starts_with("Option_") {
                            return VarType::Enum(obj_name.clone());
                        }
                        if self.context.class_layouts.contains_key(obj_name)
                            || self.context.struct_layouts.contains_key(obj_name)
                        {
                            let static_method_name = format!("{}_{}", obj_name, property);
                            if let Some(ty) = self.func_returns.get(&static_method_name) {
                                return ty.clone();
                            }
                        }
                    }
                    let obj_ty = self.get_expr_type(object);
                    if let VarType::Object(obj_name) = obj_ty {
                        let full_name = format!("{}_{}", obj_name, property);
                        if let Some(ty) = self.func_returns.get(&full_name) {
                            return ty.clone();
                        }
                        return VarType::Unknown;
                    } else if let VarType::Struct(obj_name) = obj_ty {
                        let full_name = format!("{}_{}", obj_name, property);
                        if let Some(ty) = self.func_returns.get(&full_name) {
                            return ty.clone();
                        }
                    }
                }
                VarType::Unknown
            }
            Expr::Try(inner) => {
                let inner_ty = self.get_expr_type(inner);
                if let VarType::Enum(name) = inner_ty {
                    if name.starts_with("Result_") {
                        let parts: Vec<&str> = name.split('_').collect();
                        if parts.len() >= 3 {
                            return parse_vartype(
                                parts[1],
                                None,
                                Some(&self.context.struct_layouts),
                                None,
                            );
                        }
                    } else if name.starts_with("Option_") {
                        let parts: Vec<&str> = name.split('_').collect();
                        if parts.len() >= 2 {
                            return parse_vartype(
                                parts[1],
                                None,
                                Some(&self.context.struct_layouts),
                                None,
                            );
                        }
                    }
                }
                VarType::Unknown
            }
            Expr::Await(inner) => {
                let inner_ty = self.get_expr_type(inner);
                if let VarType::Promise(t) = inner_ty {
                    *t
                } else {
                    VarType::Unknown
                }
            }
            Expr::Closure {
                params,
                return_type,
                ..
            } => {
                let mut param_types = Vec::new();
                for (_, ty_ann) in params {
                    param_types.push(parse_type_annotation(
                        ty_ann,
                        None,
                        Some(&self.context.struct_layouts),
                        None,
                    ));
                }
                let ret = if let Some(ret_ann) = return_type {
                    Box::new(parse_type_annotation(
                        ret_ann,
                        None,
                        Some(&self.context.struct_layouts),
                        None,
                    ))
                } else {
                    Box::new(VarType::Unknown)
                };
                VarType::Function(param_types, ret)
            }
            _ => VarType::Unknown,
        }
    }

    pub fn translate_args(&mut self, args: &[Expr]) -> Result<Vec<Value>, CodegenError> {
        let mut arg_vals = Vec::new();
        for arg in args {
            let mut arg_val = self.translate_expr(arg)?;
            let arg_ty = self.get_expr_type(arg);
            if let VarType::Struct(name) = &arg_ty {
                arg_val = self.copy_struct(name, arg_val);
            }
            arg_vals.push(arg_val);
        }
        Ok(arg_vals)
    }

    fn copy_struct(&mut self, name: &str, src_ptr: Value) -> Value {
        let layout = self.context.struct_layouts.get(name).unwrap();
        let ptr_ty = self.context.module.target_config().pointer_type();

        let size = layout.size as u32;
        let slot_data = cranelift::prelude::StackSlotData::new(
            cranelift::prelude::StackSlotKind::ExplicitSlot,
            size,
            0,
        );
        let slot = self.builder.create_sized_stack_slot(slot_data);
        let dst_ptr = self.builder.ins().stack_addr(ptr_ty, slot, 0);

        // Copy each field
        for &(offset, _) in layout.fields.values() {
            let field_val = self.builder.ins().load(
                types::I64,
                cranelift::prelude::MemFlagsData::new(),
                src_ptr,
                offset as i32,
            );
            self.builder.ins().store(
                cranelift::prelude::MemFlagsData::new(),
                field_val,
                dst_ptr,
                offset as i32,
            );
        }

        dst_ptr
    }

    pub fn translate_expr(&mut self, expr: &Expr) -> Result<Value, CodegenError> {
        match expr {
            Expr::IntLiteral(i) => Ok(self.builder.ins().iconst(types::I64, *i)),
            Expr::FloatLiteral(f) => Ok(self.builder.ins().f64const(*f)),
            Expr::StringLiteral(s) => {
                let string_name = if let Some(name) = self.context.string_cache.get(s) {
                    name.clone()
                } else {
                    let id = self.context.string_id;
                    self.context.string_id += 1;
                    let name = format!("__str_const_{}", id);

                    let mut data_ctx = DataDescription::new();
                    let mut bytes = s.clone().into_bytes();
                    bytes.push(0); // Null terminator
                    data_ctx.define(bytes.into_boxed_slice());

                    let data_id = self
                        .context
                        .module
                        .declare_data(&name, Linkage::Local, false, false)
                        .unwrap();
                    self.context.module.define_data(data_id, &data_ctx).unwrap();

                    self.context.string_cache.insert(s.clone(), name.clone());
                    name
                };

                let data_id = self
                    .context
                    .module
                    .declare_data(&string_name, Linkage::Local, false, false)
                    .unwrap();
                let local_id = self
                    .context
                    .module
                    .declare_data_in_func(data_id, self.builder.func);
                let ptr_ty = self.context.module.target_config().pointer_type();
                Ok(self.builder.ins().symbol_value(ptr_ty, local_id))
            }
            Expr::InterpolatedString(parts) => {
                if parts.is_empty() {
                    let mut data_ctx = DataDescription::new();
                    data_ctx.define(vec![0].into_boxed_slice());
                    let id = self.context.string_id;
                    self.context.string_id += 1;
                    let string_name = format!("__empty_str_{}", id);
                    let data_id = self
                        .context
                        .module
                        .declare_data(&string_name, Linkage::Local, false, false)
                        .unwrap();
                    self.context.module.define_data(data_id, &data_ctx).unwrap();
                    let local_id = self
                        .context
                        .module
                        .declare_data_in_func(data_id, self.builder.func);
                    let ptr_ty = self.context.module.target_config().pointer_type();
                    return Ok(self.builder.ins().symbol_value(ptr_ty, local_id));
                }

                let mut current_val = None;
                for part in parts {
                    let part_ty = self.get_expr_type(part);
                    let val = self.translate_expr(part)?;

                    let str_val = if part_ty == VarType::String {
                        val
                    } else if part_ty == VarType::Float {
                        let mut float_val = val;
                        if self.builder.func.dfg.value_type(val) == types::I64 {
                            float_val = self.builder.ins().bitcast(
                                types::F64,
                                cranelift::prelude::MemFlagsData::new(),
                                val,
                            );
                        }
                        let to_str = self.context.module.declare_func_in_func(
                            *self.context.funcs.get("float_to_string").unwrap(),
                            self.builder.func,
                        );
                        let call = self.builder.ins().call(to_str, &[float_val]);
                        self.builder.inst_results(call)[0]
                    } else if part_ty == VarType::Bool {
                        let to_str = self.context.module.declare_func_in_func(
                            *self.context.funcs.get("bool_to_string").unwrap(),
                            self.builder.func,
                        );
                        let call = self.builder.ins().call(to_str, &[val]);
                        self.builder.inst_results(call)[0]
                    } else {
                        // Assume Int
                        let to_str = self.context.module.declare_func_in_func(
                            *self.context.funcs.get("int_to_string").unwrap(),
                            self.builder.func,
                        );
                        let call = self.builder.ins().call(to_str, &[val]);
                        self.builder.inst_results(call)[0]
                    };

                    if let Some(prev) = current_val {
                        let concat = self.context.module.declare_func_in_func(
                            *self.context.funcs.get("concat_strings").unwrap(),
                            self.builder.func,
                        );
                        let call = self.builder.ins().call(concat, &[prev, str_val]);
                        current_val = Some(self.builder.inst_results(call)[0]);
                    } else {
                        current_val = Some(str_val);
                    }
                }

                Ok(current_val.unwrap())
            }
            Expr::BoolLiteral(b) => {
                let val = if *b { 1 } else { 0 };
                Ok(self.builder.ins().iconst(types::I64, val))
            }
            Expr::Identifier(name, _) => {
                if let Some((var, ty)) = self.variables.get(name) {
                    let val = self.builder.use_var(*var);
                    if matches!(ty, VarType::Object(_)) {
                        let retain_id = *self.context.funcs.get("retain").unwrap();
                        let local_retain = self
                            .context
                            .module
                            .declare_func_in_func(retain_id, self.builder.func);
                        self.builder.ins().call(local_retain, &[val]);
                    }
                    Ok(val)
                } else if let Some(&data_id) = self.context.global_vars.get(name) {
                    let ptr_ty = self.context.module.target_config().pointer_type();
                    let local_data = self
                        .context
                        .module
                        .declare_data_in_func(data_id, self.builder.func);
                    let ptr = self.builder.ins().symbol_value(ptr_ty, local_data);
                    Ok(self.builder.ins().load(
                        types::I64,
                        cranelift::prelude::MemFlagsData::new(),
                        ptr,
                        0,
                    ))
                } else {
                    Err(CodegenError {
                        message: format!(
                            "Undefined variable: {} (enum_layouts: {:?})",
                            name,
                            self.context.enum_layouts.keys().collect::<Vec<_>>()
                        ),
                    })
                }
            }
            Expr::Binary { left, op, right } => {
                let lhs = self.translate_expr(left)?;
                let rhs = self.translate_expr(right)?;

                let ty = self.builder.func.dfg.value_type(lhs);
                let is_float = ty == types::F64;

                match op {
                    BinaryOp::Add => {
                        if is_float {
                            Ok(self.builder.ins().fadd(lhs, rhs))
                        } else {
                            Ok(self.builder.ins().iadd(lhs, rhs))
                        }
                    }
                    BinaryOp::Sub => {
                        if is_float {
                            Ok(self.builder.ins().fsub(lhs, rhs))
                        } else {
                            Ok(self.builder.ins().isub(lhs, rhs))
                        }
                    }
                    BinaryOp::Mul => {
                        if is_float {
                            Ok(self.builder.ins().fmul(lhs, rhs))
                        } else {
                            Ok(self.builder.ins().imul(lhs, rhs))
                        }
                    }
                    BinaryOp::Div => {
                        if is_float {
                            Ok(self.builder.ins().fdiv(lhs, rhs))
                        } else {
                            Ok(self.builder.ins().sdiv(lhs, rhs))
                        }
                    }
                    BinaryOp::Mod => {
                        if is_float {
                            // Cranelift doesn't have a native frem instruction, so we'll throw an error or trap.
                            // But for now, we just trap or unimplemented, or for integer just do srem.
                            // Actually since float mod isn't widely used in benchmarks, we'll just panic for float mod.
                            panic!("Float modulo not supported yet");
                        } else {
                            Ok(self.builder.ins().srem(lhs, rhs))
                        }
                    }
                    BinaryOp::Eq => {
                        if is_float {
                            let c = self.builder.ins().fcmp(FloatCC::Equal, lhs, rhs);
                            Ok(self.builder.ins().uextend(types::I64, c))
                        } else {
                            let c = self.builder.ins().icmp(IntCC::Equal, lhs, rhs);
                            Ok(self.builder.ins().uextend(types::I64, c))
                        }
                    }
                    BinaryOp::NotEq => {
                        if is_float {
                            let c = self.builder.ins().fcmp(FloatCC::NotEqual, lhs, rhs);
                            Ok(self.builder.ins().uextend(types::I64, c))
                        } else {
                            let c = self.builder.ins().icmp(IntCC::NotEqual, lhs, rhs);
                            Ok(self.builder.ins().uextend(types::I64, c))
                        }
                    }
                    BinaryOp::Less => {
                        if is_float {
                            let c = self.builder.ins().fcmp(FloatCC::LessThan, lhs, rhs);
                            Ok(self.builder.ins().uextend(types::I64, c))
                        } else {
                            let c = self.builder.ins().icmp(IntCC::SignedLessThan, lhs, rhs);
                            Ok(self.builder.ins().uextend(types::I64, c))
                        }
                    }
                    BinaryOp::LessEq => {
                        if is_float {
                            let c = self.builder.ins().fcmp(FloatCC::LessThanOrEqual, lhs, rhs);
                            Ok(self.builder.ins().uextend(types::I64, c))
                        } else {
                            let c = self
                                .builder
                                .ins()
                                .icmp(IntCC::SignedLessThanOrEqual, lhs, rhs);
                            Ok(self.builder.ins().uextend(types::I64, c))
                        }
                    }
                    BinaryOp::Greater => {
                        if is_float {
                            let c = self.builder.ins().fcmp(FloatCC::GreaterThan, lhs, rhs);
                            Ok(self.builder.ins().uextend(types::I64, c))
                        } else {
                            let c = self.builder.ins().icmp(IntCC::SignedGreaterThan, lhs, rhs);
                            Ok(self.builder.ins().uextend(types::I64, c))
                        }
                    }
                    BinaryOp::GreaterEq => {
                        if is_float {
                            let c = self
                                .builder
                                .ins()
                                .fcmp(FloatCC::GreaterThanOrEqual, lhs, rhs);
                            Ok(self.builder.ins().uextend(types::I64, c))
                        } else {
                            let c =
                                self.builder
                                    .ins()
                                    .icmp(IntCC::SignedGreaterThanOrEqual, lhs, rhs);
                            Ok(self.builder.ins().uextend(types::I64, c))
                        }
                    }
                    BinaryOp::And => {
                        Ok(self.builder.ins().band(lhs, rhs)) // bitwise AND works for booleans represented as 0/1 integers
                    }
                    BinaryOp::Or => {
                        Ok(self.builder.ins().bor(lhs, rhs)) // bitwise OR works for booleans represented as 0/1 integers
                    }
                }
            }
            Expr::Await(inner) => {
                let promise_ptr = self.translate_expr(inner)?;
                let await_id = *self.context.funcs.get("__pace_promise_await").unwrap();
                let local_await = self
                    .context
                    .module
                    .declare_func_in_func(await_id, self.builder.func);
                let call = self.builder.ins().call(local_await, &[promise_ptr]);
                Ok(self.builder.inst_results(call)[0])
            }
            Expr::Try(inner) => {
                let inner_ptr = self.translate_expr(inner)?;

                // Read the tag at offset 8 (0 = Ok/Some, 1 = Err/None usually based on how variants are sorted)
                // Wait, we need to know exactly which tag is Ok/Err.
                // We'll dynamically look up the tag ID of "Ok" or "Some".
                let inner_ty = self.get_expr_type(inner);
                let enum_name = if let VarType::Enum(name) = inner_ty {
                    name
                } else {
                    return Err(CodegenError {
                        message: "? operator used on non-enum".to_string(),
                    });
                };
                let enum_layout = self.context.enum_layouts.get(&enum_name).unwrap();

                // Determine which tags represent the success and failure
                let is_result = enum_name.starts_with("Result_");
                let (success_tag, _) = if is_result {
                    enum_layout.variants.get("Ok").unwrap()
                } else {
                    enum_layout.variants.get("Some").unwrap()
                };

                let tag_val = self.builder.ins().load(
                    types::I64,
                    cranelift::prelude::MemFlagsData::new(),
                    inner_ptr,
                    8,
                );
                let expected_tag = self.builder.ins().iconst(types::I64, *success_tag as i64);
                let is_success = self.builder.ins().icmp(
                    cranelift::codegen::ir::condcodes::IntCC::Equal,
                    tag_val,
                    expected_tag,
                );

                let continue_block = self.builder.create_block();
                let err_block = self.builder.create_block();

                self.builder
                    .ins()
                    .brif(is_success, continue_block, &[], err_block, &[]);

                // Error Block: Return the whole enum from the function
                self.builder.seal_block(err_block);
                self.builder.switch_to_block(err_block);
                self.builder.ins().return_(&[inner_ptr]);

                // Continue Block: Extract the first field of the Ok/Some variant
                self.builder.seal_block(continue_block);
                self.builder.switch_to_block(continue_block);

                // The value of Ok/Some is at offset 16 (since tag is 8, ARC is 0)
                // Note: Only works if Ok/Some has a 64-bit primitive or pointer (which is true for our types right now)
                let val = self.builder.ins().load(
                    types::I64,
                    cranelift::prelude::MemFlagsData::new(),
                    inner_ptr,
                    16,
                );
                Ok(val)
            }
            Expr::Assign { target, value } => {
                let mut val = self.translate_expr(value)?;
                let val_ty = self.get_expr_type(value);
                if let VarType::Struct(name) = &val_ty {
                    val = self.copy_struct(name, val);
                }
                if let Expr::Identifier(name, _) = &**target {
                    if let Some((var, ty)) = self.variables.get(name) {
                        if matches!(ty, VarType::Object(_)) {
                            // Release old value
                            let old_val = self.builder.use_var(*var);
                            let release_id = *self.context.funcs.get("release").unwrap();
                            let local_release = self
                                .context
                                .module
                                .declare_func_in_func(release_id, self.builder.func);
                            self.builder.ins().call(local_release, &[old_val]);

                            // Retain new value for the variable (caller gets the original +1)
                            let retain_id = *self.context.funcs.get("retain").unwrap();
                            let local_retain = self
                                .context
                                .module
                                .declare_func_in_func(retain_id, self.builder.func);
                            self.builder.ins().call(local_retain, &[val]);
                        }
                        self.builder.def_var(*var, val);
                        Ok(val)
                    } else if let Some(&data_id) = self.context.global_vars.get(name) {
                        let ptr_ty = self.context.module.target_config().pointer_type();
                        let local_data = self
                            .context
                            .module
                            .declare_data_in_func(data_id, self.builder.func);
                        let ptr = self.builder.ins().symbol_value(ptr_ty, local_data);
                        self.builder.ins().store(
                            cranelift::prelude::MemFlagsData::new(),
                            val,
                            ptr,
                            0,
                        );
                        Ok(val)
                    } else {
                        Err(CodegenError {
                            message: format!("Variable '{}' not found in JIT environment", name),
                        })
                    }
                } else if let Expr::MemberAccess {
                    object,
                    property,
                    computed_class: _,
                    is_static_operator: _,
                } = &**target
                {
                    if let Expr::Identifier(obj_name, _) = &**object {
                        let maybe_static_field =
                            if let Some(layout) = self.context.class_layouts.get(obj_name) {
                                layout.static_fields.get(property)
                            } else if let Some(layout) = self.context.struct_layouts.get(obj_name) {
                                layout.static_fields.get(property)
                            } else {
                                None
                            };

                        if let Some(&(data_id, ref f_ty)) = maybe_static_field {
                            let ptr_ty = self.context.module.target_config().pointer_type();
                            let data_ref = self
                                .context
                                .module
                                .declare_data_in_func(data_id, self.builder.func);
                            let addr = self.builder.ins().symbol_value(ptr_ty, data_ref);

                            if matches!(f_ty, VarType::Object(_)) {
                                let old_val = self.builder.ins().load(
                                    f_ty.to_cranelift_type(),
                                    cranelift::prelude::MemFlagsData::new(),
                                    addr,
                                    0,
                                );
                                let release_id = *self.context.funcs.get("release").unwrap();
                                let local_release = self
                                    .context
                                    .module
                                    .declare_func_in_func(release_id, self.builder.func);
                                self.builder.ins().call(local_release, &[old_val]);

                                let retain_id = *self.context.funcs.get("retain").unwrap();
                                let local_retain = self
                                    .context
                                    .module
                                    .declare_func_in_func(retain_id, self.builder.func);
                                self.builder.ins().call(local_retain, &[val]);
                            }

                            self.builder.ins().store(
                                cranelift::prelude::MemFlagsData::new(),
                                val,
                                addr,
                                0,
                            );
                            return Ok(val);
                        }
                    }

                    let obj_ptr = self.translate_expr(object)?;

                    let obj_type = self.get_expr_type(object);
                    let (f_offset, f_ty) = match obj_type {
                        VarType::Object(name) => {
                            let layout = self.context.class_layouts.get(&name).unwrap();
                            layout.fields.get(property).unwrap().clone()
                        }
                        VarType::Struct(name) => {
                            let layout = self.context.struct_layouts.get(&name).unwrap();
                            layout.fields.get(property).unwrap().clone()
                        }
                        _ => panic!("MemberAccess assign on non-object type: {:?}", obj_type),
                    };

                    if matches!(f_ty, VarType::Object(_)) {
                        let old_val = self.builder.ins().load(
                            f_ty.to_cranelift_type(),
                            cranelift::prelude::MemFlagsData::new(),
                            obj_ptr,
                            f_offset as i32,
                        );
                        let release_id = *self.context.funcs.get("release").unwrap();
                        let local_release = self
                            .context
                            .module
                            .declare_func_in_func(release_id, self.builder.func);
                        self.builder.ins().call(local_release, &[old_val]);

                        let retain_id = *self.context.funcs.get("retain").unwrap();
                        let local_retain = self
                            .context
                            .module
                            .declare_func_in_func(retain_id, self.builder.func);
                        self.builder.ins().call(local_retain, &[val]);
                    }

                    self.builder.ins().store(
                        cranelift::prelude::MemFlagsData::new(),
                        val,
                        obj_ptr,
                        f_offset as i32,
                    );
                    Ok(val)
                } else {
                    Err(CodegenError {
                        message: "Invalid assignment target".to_string(),
                    })
                }
            }
            Expr::Call { callee, args } => {
                let callee_ty = self.get_expr_type(callee);
                if matches!(callee_ty, VarType::Function(_, _)) {
                    let fat_ptr = self.translate_expr(callee)?;

                    let ptr_ty = self.context.module.target_config().pointer_type();
                    let func_ptr = self.builder.ins().load(
                        ptr_ty,
                        cranelift::prelude::MemFlagsData::new(),
                        fat_ptr,
                        0,
                    );
                    let env_ptr = fat_ptr; // The fat pointer itself is the environment pointer

                    let mut arg_vals = vec![env_ptr];
                    for arg in args {
                        let mut arg_val = self.translate_expr(arg)?;
                        let arg_ty = self.get_expr_type(arg);
                        if let VarType::Struct(name) = &arg_ty {
                            arg_val = self.copy_struct(name, arg_val);
                        }
                        arg_vals.push(arg_val);
                    }

                    let mut sig = self.context.module.make_signature();
                    sig.params.push(AbiParam::new(ptr_ty)); // env
                    for _ in args {
                        sig.params.push(AbiParam::new(types::I64));
                    }
                    sig.returns.push(AbiParam::new(types::I64));

                    let sig_ref = self.builder.import_signature(sig);
                    let call = self
                        .builder
                        .ins()
                        .call_indirect(sig_ref, func_ptr, &arg_vals);

                    let results = self.builder.inst_results(call);
                    if results.is_empty() {
                        return Ok(self.builder.ins().iconst(types::I64, 0));
                    } else {
                        return Ok(results[0]);
                    }
                }

                if let Expr::Identifier(func_name, _) = &**callee {
                    if func_name == "print" {
                        let arg_expr = &args[0];
                        let arg_ty = self.get_expr_type(arg_expr);

                        let arg_val = self.translate_expr(arg_expr)?;
                        let ty = self.builder.func.dfg.value_type(arg_val);

                        let target_name = if ty == types::F64 {
                            "print_float"
                        } else if arg_ty == VarType::String
                            || matches!(arg_ty, VarType::Nullable(ref inner) if **inner == VarType::String)
                        {
                            "print_string"
                        } else {
                            "print_int" // Fallback to int
                        };

                        let func_id = *self.context.funcs.get(target_name).unwrap();
                        let local_func = self
                            .context
                            .module
                            .declare_func_in_func(func_id, self.builder.func);
                        let call = self.builder.ins().call(local_func, &[arg_val]);

                        let results = self.builder.inst_results(call);
                        if results.is_empty() {
                            return Ok(self.builder.ins().iconst(types::I64, 0));
                        } else {
                            return Ok(results[0]);
                        }
                    } else if let Some(&func_id) = self.context.funcs.get(func_name) {
                        let local_func = self
                            .context
                            .module
                            .declare_func_in_func(func_id, self.builder.func);
                        let arg_vals = self.translate_args(args)?;
                        let call = self.builder.ins().call(local_func, &arg_vals);

                        let results = self.builder.inst_results(call);
                        if results.is_empty() {
                            return Ok(self.builder.ins().iconst(types::I64, 0));
                        } else {
                            return Ok(results[0]);
                        }
                    } else if let Some(layout) = self.context.class_layouts.get(func_name) {
                        let ptr_ty = self.context.module.target_config().pointer_type();

                        let malloc_id = *self.context.funcs.get("malloc").unwrap();
                        let local_malloc = self
                            .context
                            .module
                            .declare_func_in_func(malloc_id, self.builder.func);

                        let size = 16 + layout.fields.len() * 8;
                        let size_val = self.builder.ins().iconst(types::I64, size as i64);

                        let call = self.builder.ins().call(local_malloc, &[size_val]);
                        let obj_ptr = self.builder.inst_results(call)[0];

                        // Set ARC count to 1
                        let one = self.builder.ins().iconst(types::I64, 1);
                        self.builder.ins().store(
                            cranelift::prelude::MemFlagsData::new(),
                            one,
                            obj_ptr,
                            0,
                        );

                        // Set VTable
                        let vtable_gv = self
                            .context
                            .module
                            .declare_data_in_func(layout.vtable_id, self.builder.func);
                        let vtable_addr = self.builder.ins().symbol_value(ptr_ty, vtable_gv);
                        self.builder.ins().store(
                            cranelift::prelude::MemFlagsData::new(),
                            vtable_addr,
                            obj_ptr,
                            8,
                        );

                        let zero = self.builder.ins().iconst(types::I64, 0);
                        for (field_name, &(offset, _)) in &layout.fields {
                            if field_name == "__mailbox" {
                                let mb_create_id =
                                    *self.context.funcs.get("__pace_mailbox_create").unwrap();
                                let local_mb_create = self
                                    .context
                                    .module
                                    .declare_func_in_func(mb_create_id, self.builder.func);
                                let mb_call = self.builder.ins().call(local_mb_create, &[]);
                                let mb_ptr = self.builder.inst_results(mb_call)[0];
                                self.builder.ins().store(
                                    cranelift::prelude::MemFlagsData::new(),
                                    mb_ptr,
                                    obj_ptr,
                                    offset as i32,
                                );
                            } else {
                                self.builder.ins().store(
                                    cranelift::prelude::MemFlagsData::new(),
                                    zero,
                                    obj_ptr,
                                    offset as i32,
                                );
                            }
                        }

                        // Call init if it exists
                        let init_name = format!("{}_init", func_name);
                        if let Some(&init_id) = self.context.funcs.get(&init_name) {
                            let local_init = self
                                .context
                                .module
                                .declare_func_in_func(init_id, self.builder.func);
                            let mut arg_vals = vec![obj_ptr];
                            arg_vals.extend(self.translate_args(args)?);
                            self.builder.ins().call(local_init, &arg_vals);
                        }

                        return Ok(obj_ptr);
                    } else if let Some(layout) = self.context.struct_layouts.get(func_name) {
                        let ptr_ty = self.context.module.target_config().pointer_type();
                        let size = layout.size as u32;
                        let slot_data = cranelift::prelude::StackSlotData::new(
                            cranelift::prelude::StackSlotKind::ExplicitSlot,
                            size,
                            0,
                        );
                        let slot = self.builder.create_sized_stack_slot(slot_data);
                        let obj_ptr = self.builder.ins().stack_addr(ptr_ty, slot, 0);

                        // Create a sorted list of fields by offset to map args correctly
                        let mut sorted_fields: Vec<_> = layout
                            .fields
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        sorted_fields.sort_by_key(|(_, (offset, _))| *offset);

                        for (i, arg) in args.iter().enumerate() {
                            if let Some((_, (offset, _))) = sorted_fields.get(i) {
                                let mut arg_val = self.translate_expr(arg)?;
                                let arg_ty = self.get_expr_type(arg);
                                if let VarType::Struct(name) = &arg_ty {
                                    arg_val = self.copy_struct(name, arg_val);
                                }
                                self.builder.ins().store(
                                    cranelift::prelude::MemFlagsData::new(),
                                    arg_val,
                                    obj_ptr,
                                    *offset as i32,
                                );
                            }
                        }

                        return Ok(obj_ptr);
                    }
                } else if let Expr::MemberAccess {
                    object,
                    property,
                    computed_class: _,
                    is_static_operator: _,
                } = &**callee
                {
                    let mut obj_name_opt = None;
                    if let Expr::Identifier(obj_name, _) = &**object {
                        obj_name_opt = Some(obj_name.clone());
                    } else if let Expr::GenericInstantiation { callee, .. } = &**object {
                        if let Expr::Identifier(obj_name, _) = &**callee {
                            obj_name_opt = Some(obj_name.clone());
                        }
                    }

                    if let Some(obj_name) = obj_name_opt {
                        if self.context.enum_layouts.contains_key(&obj_name) {
                            let constructor_name = format!("{}_{}", obj_name, property);
                            let func_id =
                                self.context
                                    .funcs
                                    .get(&constructor_name)
                                    .unwrap_or_else(|| {
                                        panic!("Enum constructor {} not found", constructor_name)
                                    });
                            let local_callee = self
                                .context
                                .module
                                .declare_func_in_func(*func_id, self.builder.func);

                            let arg_vals = self.translate_args(args)?;

                            let call = self.builder.ins().call(local_callee, &arg_vals);
                            return Ok(self.builder.inst_results(call)[0]);
                        } else if self.context.class_layouts.contains_key(&obj_name)
                            || self.context.struct_layouts.contains_key(&obj_name)
                        {
                            // STATIC METHOD CALL!
                            let static_method_name = format!("{}_{}", obj_name, property);
                            let func_id = self
                                .context
                                .funcs
                                .get(&static_method_name)
                                .unwrap_or_else(|| {
                                    panic!("Static method {} not found", static_method_name)
                                });
                            let local_callee = self
                                .context
                                .module
                                .declare_func_in_func(*func_id, self.builder.func);

                            let arg_vals = self.translate_args(args)?;

                            let call = self.builder.ins().call(local_callee, &arg_vals);
                            let results = self.builder.inst_results(call);
                            if results.is_empty() {
                                return Ok(self.builder.ins().iconst(types::I64, 0));
                            } else {
                                return Ok(results[0]);
                            }
                        }
                    }

                    let obj_ptr = self.translate_expr(object)?;
                    let ptr_ty = self.context.module.target_config().pointer_type();

                    let obj_type = self.get_expr_type(object);
                    let (m_offset, is_actor) = if let VarType::Object(type_name) = &obj_type {
                        let layout =
                            self.context
                                .class_layouts
                                .get(type_name)
                                .unwrap_or_else(|| {
                                    panic!("Class or interface {} not found in layouts", type_name)
                                });
                        (
                            *layout.methods.get(property).unwrap_or_else(|| {
                                panic!("Method {} not found in {}", property, type_name)
                            }),
                            layout.fields.contains_key("__mailbox"),
                        )
                    } else {
                        let layout = self
                            .context
                            .class_layouts
                            .values()
                            .find(|l| l.methods.contains_key(property))
                            .unwrap_or_else(|| {
                                panic!("Method {} not found in any class layout", property)
                            });
                        (*layout.methods.get(property).unwrap(), false)
                    };

                    let vtable_ptr = self.builder.ins().load(
                        ptr_ty,
                        cranelift::prelude::MemFlagsData::new(),
                        obj_ptr,
                        8,
                    );
                    let method_ptr = self.builder.ins().load(
                        ptr_ty,
                        cranelift::prelude::MemFlagsData::new(),
                        vtable_ptr,
                        m_offset as i32,
                    );

                    let mut arg_vals = vec![obj_ptr];
                    for arg in args {
                        let mut arg_val = self.translate_expr(arg)?;
                        let arg_ty = self.get_expr_type(arg);
                        if let VarType::Struct(name) = &arg_ty {
                            arg_val = self.copy_struct(name, arg_val);
                        }
                        arg_vals.push(arg_val);
                    }

                    if is_actor {
                        let promise_create_id =
                            *self.context.funcs.get("__pace_promise_create").unwrap();
                        let local_promise_create = self
                            .context
                            .module
                            .declare_func_in_func(promise_create_id, self.builder.func);
                        let promise_call = self.builder.ins().call(local_promise_create, &[]);
                        let promise_ptr = self.builder.inst_results(promise_call)[0];

                        let layout = self
                            .context
                            .class_layouts
                            .get(if let VarType::Object(name) = &obj_type {
                                name
                            } else {
                                unreachable!()
                            })
                            .unwrap();
                        let mb_offset = layout.fields.get("__mailbox").unwrap().0;
                        let mailbox_ptr = self.builder.ins().load(
                            types::I64,
                            cranelift::prelude::MemFlagsData::new(),
                            obj_ptr,
                            mb_offset as i32,
                        );

                        let malloc_id = *self.context.funcs.get("malloc").unwrap();
                        let local_malloc = self
                            .context
                            .module
                            .declare_func_in_func(malloc_id, self.builder.func);
                        let tuple_size = arg_vals.len() * 8;
                        let size_val = self.builder.ins().iconst(types::I64, tuple_size as i64);
                        let malloc_call = self.builder.ins().call(local_malloc, &[size_val]);
                        let tuple_ptr = self.builder.inst_results(malloc_call)[0];

                        for (i, val) in arg_vals.iter().enumerate() {
                            self.builder.ins().store(
                                cranelift::prelude::MemFlagsData::new(),
                                *val,
                                tuple_ptr,
                                (i * 8) as i32,
                            );
                        }

                        let mb_send_id = *self.context.funcs.get("__pace_mailbox_send").unwrap();
                        let local_mb_send = self
                            .context
                            .module
                            .declare_func_in_func(mb_send_id, self.builder.func);
                        self.builder.ins().call(
                            local_mb_send,
                            &[mailbox_ptr, method_ptr, tuple_ptr, promise_ptr],
                        );

                        return Ok(promise_ptr);
                    } else {
                        let mut sig = self.context.module.make_signature();
                        sig.params.push(AbiParam::new(ptr_ty)); // self
                        for _ in args {
                            sig.params.push(AbiParam::new(types::I64));
                        }
                        sig.returns.push(AbiParam::new(types::I64));

                        let sig_ref = self.builder.import_signature(sig);
                        let call = self
                            .builder
                            .ins()
                            .call_indirect(sig_ref, method_ptr, &arg_vals);

                        let results = self.builder.inst_results(call);
                        if results.is_empty() {
                            return Ok(self.builder.ins().iconst(types::I64, 0));
                        } else {
                            return Ok(results[0]);
                        }
                    }
                }
                Err(CodegenError {
                    message: format!("Cannot resolve function call: {:?}", callee),
                })
            }
            Expr::MemberAccess {
                object,
                property,
                computed_class: _,
                is_static_operator: _,
            } => {
                let mut obj_name_opt = None;
                if let Expr::Identifier(obj_name, _) = &**object {
                    obj_name_opt = Some(obj_name.clone());
                } else if let Expr::GenericInstantiation { callee, .. } = &**object {
                    if let Expr::Identifier(obj_name, _) = &**callee {
                        obj_name_opt = Some(obj_name.clone());
                    }
                }

                if let Some(obj_name) = obj_name_opt {
                    if self.context.enum_layouts.contains_key(&obj_name) {
                        let constructor_name = format!("{}_{}", obj_name, property);
                        let func_id =
                            self.context
                                .funcs
                                .get(&constructor_name)
                                .unwrap_or_else(|| {
                                    panic!("Enum constructor {} not found", constructor_name)
                                });
                        let local_callee = self
                            .context
                            .module
                            .declare_func_in_func(*func_id, self.builder.func);

                        let call = self.builder.ins().call(local_callee, &[]);
                        return Ok(self.builder.inst_results(call)[0]);
                    }

                    let maybe_static_field =
                        if let Some(layout) = self.context.class_layouts.get(&obj_name) {
                            layout.static_fields.get(property)
                        } else if let Some(layout) = self.context.struct_layouts.get(&obj_name) {
                            layout.static_fields.get(property)
                        } else {
                            None
                        };

                    if let Some(&(data_id, ref f_ty)) = maybe_static_field {
                        let ptr_ty = self.context.module.target_config().pointer_type();
                        let data_ref = self
                            .context
                            .module
                            .declare_data_in_func(data_id, self.builder.func);
                        let addr = self.builder.ins().symbol_value(ptr_ty, data_ref);
                        let val = self.builder.ins().load(
                            f_ty.to_cranelift_type(),
                            cranelift::prelude::MemFlagsData::new(),
                            addr,
                            0,
                        );

                        if matches!(f_ty, VarType::Object(_)) {
                            let retain_id = *self.context.funcs.get("retain").unwrap();
                            let local_retain = self
                                .context
                                .module
                                .declare_func_in_func(retain_id, self.builder.func);
                            self.builder.ins().call(local_retain, &[val]);
                        }

                        return Ok(val);
                    }
                }

                let obj_ptr = self.translate_expr(object)?;

                let obj_type = self.get_expr_type(object);
                let (f_offset, f_ty) = match obj_type {
                    VarType::Object(name) => {
                        let layout = self.context.class_layouts.get(&name).unwrap();
                        layout.fields.get(property).unwrap().clone()
                    }
                    VarType::Struct(name) => {
                        let layout = self.context.struct_layouts.get(&name).unwrap();
                        layout.fields.get(property).unwrap().clone()
                    }
                    _ => panic!("MemberAccess on non-object type: {:?}", obj_type),
                };

                let val = self.builder.ins().load(
                    f_ty.to_cranelift_type(),
                    cranelift::prelude::MemFlagsData::new(),
                    obj_ptr,
                    f_offset as i32,
                );

                if matches!(f_ty, VarType::Object(_)) {
                    let retain_id = *self.context.funcs.get("retain").unwrap();
                    let local_retain = self
                        .context
                        .module
                        .declare_func_in_func(retain_id, self.builder.func);
                    self.builder.ins().call(local_retain, &[val]);
                }

                Ok(val)
            }
            Expr::Closure { .. } => {
                // 1. Generate unique function name
                let closure_id = self.pending_closures.len();
                let fn_name = format!("__closure_fn_{}", closure_id);

                let mut captured_vars = Vec::new();
                for (name, (_, ty)) in self.variables.iter() {
                    captured_vars.push((name.clone(), ty.clone()));
                }

                captured_vars.sort_by(|a, b| a.0.cmp(&b.0));

                let env_size = 16 + (captured_vars.len() * 8);

                let malloc_id = *self.context.funcs.get("malloc").unwrap();
                let local_malloc = self
                    .context
                    .module
                    .declare_func_in_func(malloc_id, self.builder.func);
                let size_val = self.builder.ins().iconst(types::I64, env_size as i64);
                let malloc_call = self.builder.ins().call(local_malloc, &[size_val]);
                let env_ptr = self.builder.inst_results(malloc_call)[0];

                for (i, (name, _)) in captured_vars.iter().enumerate() {
                    let offset = 16 + (i * 8);
                    let (var, _) = self.variables.get(name).unwrap();
                    let val = self.builder.use_var(*var);
                    self.builder.ins().store(
                        cranelift::prelude::MemFlagsData::new(),
                        val,
                        env_ptr,
                        offset as i32,
                    );
                }

                self.pending_closures
                    .push((fn_name.clone(), expr.clone(), captured_vars));

                let mut sig = self.context.module.make_signature();
                sig.params.push(AbiParam::new(
                    self.context.module.target_config().pointer_type(),
                )); // env

                let num_params = match expr {
                    Expr::Closure { params, .. } => params.len(),
                    _ => 0,
                };

                for _ in 0..num_params {
                    sig.params.push(AbiParam::new(types::I64));
                }
                sig.returns.push(AbiParam::new(types::I64));

                let func_id = self
                    .context
                    .module
                    .declare_function(&fn_name, Linkage::Export, &sig)
                    .unwrap();
                let local_func = self
                    .context
                    .module
                    .declare_func_in_func(func_id, self.builder.func);
                let func_ptr = self.builder.ins().func_addr(
                    self.context.module.target_config().pointer_type(),
                    local_func,
                );

                self.builder.ins().store(
                    cranelift::prelude::MemFlagsData::new(),
                    func_ptr,
                    env_ptr,
                    0,
                );

                Ok(env_ptr)
            }
            Expr::Null => Ok(self.builder.ins().iconst(types::I64, 0)),
            Expr::Unwrap(inner) => {
                let inner_val = self.translate_expr(inner)?;

                let is_null =
                    self.builder
                        .ins()
                        .icmp_imm_u(cranelift::prelude::IntCC::Equal, inner_val, 0);

                // Trap if null
                let trap_block = self.builder.create_block();
                let cont_block = self.builder.create_block();

                self.builder
                    .ins()
                    .brif(is_null, trap_block, &[], cont_block, &[]);

                self.builder.switch_to_block(trap_block);
                self.builder.seal_block(trap_block);
                self.builder
                    .ins()
                    .trap(cranelift::prelude::TrapCode::user(1).unwrap()); // Null pointer dereference

                self.builder.switch_to_block(cont_block);
                self.builder.seal_block(cont_block);

                Ok(inner_val)
            }
            Expr::NullCoalesce { left, right } => {
                let left_val = self.translate_expr(left)?;

                let is_null =
                    self.builder
                        .ins()
                        .icmp_imm_u(cranelift::prelude::IntCC::Equal, left_val, 0);

                let right_block = self.builder.create_block();
                let merge_block = self.builder.create_block();
                self.builder.append_block_param(merge_block, types::I64);

                self.builder.ins().brif(
                    is_null,
                    right_block,
                    &[],
                    merge_block,
                    &[cranelift::codegen::ir::BlockArg::Value(left_val)],
                );

                self.builder.switch_to_block(right_block);
                self.builder.seal_block(right_block);
                let right_val = self.translate_expr(right)?;
                self.builder.ins().jump(
                    merge_block,
                    &[cranelift::codegen::ir::BlockArg::Value(right_val)],
                );

                self.builder.switch_to_block(merge_block);
                self.builder.seal_block(merge_block);

                let result = self.builder.block_params(merge_block)[0];
                Ok(result)
            }
            Expr::OptionalMemberAccess { object, property } => {
                let obj_ptr = self.translate_expr(object)?;

                let is_null =
                    self.builder
                        .ins()
                        .icmp_imm_u(cranelift::prelude::IntCC::Equal, obj_ptr, 0);

                let access_block = self.builder.create_block();
                let merge_block = self.builder.create_block();
                self.builder.append_block_param(merge_block, types::I64);

                let zero_val = self.builder.ins().iconst(types::I64, 0);
                self.builder.ins().brif(
                    is_null,
                    merge_block,
                    &[cranelift::codegen::ir::BlockArg::Value(zero_val)],
                    access_block,
                    &[],
                );

                self.builder.switch_to_block(access_block);
                self.builder.seal_block(access_block);

                let obj_type = self.get_expr_type(object);
                let (f_offset, f_ty) = match obj_type {
                    VarType::Object(name) => {
                        let layout = self.context.class_layouts.get(&name).unwrap();
                        layout.fields.get(property).unwrap().clone()
                    }
                    VarType::Struct(name) => {
                        let layout = self.context.struct_layouts.get(&name).unwrap();
                        layout.fields.get(property).unwrap().clone()
                    }
                    _ => panic!("OptionalMemberAccess on non-object type: {:?}", obj_type),
                };

                let val = self.builder.ins().load(
                    types::I64,
                    cranelift::prelude::MemFlagsData::new(),
                    obj_ptr,
                    f_offset as i32,
                );

                if matches!(f_ty, VarType::Object(_)) {
                    let retain_id = *self.context.funcs.get("retain").unwrap();
                    let local_retain = self
                        .context
                        .module
                        .declare_func_in_func(retain_id, self.builder.func);
                    self.builder.ins().call(local_retain, &[val]);
                }

                self.builder
                    .ins()
                    .jump(merge_block, &[cranelift::codegen::ir::BlockArg::Value(val)]);

                self.builder.switch_to_block(merge_block);
                self.builder.seal_block(merge_block);

                let result = self.builder.block_params(merge_block)[0];
                Ok(result)
            }
            _ => Err(CodegenError {
                message: format!("Cannot translate expression: {:?}", expr),
            }),
        }
    }
}
