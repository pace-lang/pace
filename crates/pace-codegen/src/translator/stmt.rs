use super::{parse_type_annotation, parse_vartype};
use crate::layouts::CodegenError;
use cranelift::prelude::*;
use cranelift_module::Module;
use pace_ast::{Expr, Stmt};
use super::{VarType, Translator};

impl<'a, 'b, M: Module> Translator<'a, 'b, M> {
    pub fn translate_stmt(&mut self, stmt_id: pace_ast::arena::StmtId) -> Result<(Value, bool), CodegenError> {
        let stmt = self.arena.get_stmt(stmt_id);
        match stmt {
            Stmt::VarDecl {
                name, initializer, ..
            } => {
                let mut var_ty = VarType::Unknown;
                let val = if let Some(expr) = initializer {
                    var_ty = self.get_expr_type(*expr);
                    let mut val = self.translate_expr(*expr)?;

                    if let VarType::Struct(name) = &var_ty {
                        val = self.copy_struct(name, val);
                    }
                    val
                } else {
                    self.builder.ins().iconst(types::I64, 0)
                };

                if self.is_global_context
                    && let Some(&data_id) = self.context.global_vars.get(name) {
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

                let val_ty = self.builder.func.dfg.value_type(val);
                let var = self.builder.declare_var(val_ty);
                self.builder.def_var(var, val);
                self.variables.insert(*name, (var, var_ty));
                *self.var_index += 1;
                Ok((val, false))
            }
            Stmt::Expr(expr) => {
                let ty = self.get_expr_type(*expr);
                let val = self.translate_expr(*expr)?;
                if matches!(ty, VarType::Object(_)) {
                    let release_id = *self.context.funcs.get(&ustr::Ustr::from("release")).unwrap();
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
                let cond_val = self.translate_expr(*condition)?;

                let then_block = self.builder.create_block();
                let else_block = self.builder.create_block();
                let merge_block = self.builder.create_block();

                self.builder
                    .ins()
                    .brif(cond_val, then_block, &[], else_block, &[]);

                // Then
                self.builder.switch_to_block(then_block);
                self.builder.seal_block(then_block);
                let (_then_res, then_term) = self.translate_stmt(*then_branch)?;
                if !then_term {
                    self.builder.ins().jump(merge_block, &[]);
                }

                // Else
                self.builder.switch_to_block(else_block);
                self.builder.seal_block(else_block);
                let (_else_res, else_term) = if let Some(elb) = else_branch {
                    self.translate_stmt(*elb)?
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
                let body_block = self.builder.create_block();
                let check_block = self.builder.create_block();
                let exit_block = self.builder.create_block();

                // 1. Jump directly to check block first time
                self.builder.ins().jump(check_block, &[]);

                // 2. Body Block
                self.builder.switch_to_block(body_block);
                let (_, body_term) = self.translate_stmt(*body)?;
                if !body_term {
                    self.builder.ins().jump(check_block, &[]);
                }

                // 3. Check Block
                self.builder.switch_to_block(check_block);
                let cond_val = self.translate_expr(*condition)?;
                self.builder
                    .ins()
                    .brif(cond_val, body_block, &[], exit_block, &[]);
                
                // Now that all branches to body_block and check_block have been generated:
                self.builder.seal_block(body_block);
                self.builder.seal_block(check_block);

                // 4. Exit Block
                self.builder.switch_to_block(exit_block);
                self.builder.seal_block(exit_block);

                Ok((self.builder.ins().iconst(types::I64, 0), false))
            }
            Stmt::Match { expr, arms } => {
                let expr_ty = self.get_expr_type(*expr);
                let enum_name = if let VarType::Object(name) = expr_ty {
                    name
                } else if let VarType::Enum(name) = expr_ty {
                    name
                } else {
                    return Err(CodegenError {
                        message: "Match expression must be an Enum type".to_string(),
                    });
                };
                let enum_layout = self.context.enum_layouts.get(&ustr::Ustr::from(&enum_name)).unwrap().clone();

                let obj_ptr = self.translate_expr(*expr)?;

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
                                        .insert(*var_name, (var, variant_types[j].clone()));
                                    *self.var_index += 1;
                                }
                                offset += 8;
                            }
                        }
                    }

                    let (_, term) = self.translate_stmt(*body)?;
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

                let (_, body_term) = self.translate_stmt(*body)?;
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
                let iter_val = self.translate_expr(*iterable)?;
                let iter_ty = self.get_expr_type(*iterable);

                let (length_offset, get_offset) = if let VarType::Object(type_name) = &iter_ty {
                    let layout = self.context.class_layouts.get(&ustr::Ustr::from(type_name)).unwrap();
                    let l = *layout.methods.get(&ustr::Ustr::from("length")).unwrap();
                    let g = *layout.methods.get(&ustr::Ustr::from("get")).unwrap();
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
                    .insert(*item, (item_var, VarType::Unknown));
                *self.var_index += 1;

                let (_, body_term) = self.translate_stmt(*body)?;
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
                let initial_vars: Vec<ustr::Ustr> = self.variables.keys().cloned().collect();
                let mut last_val = self.builder.ins().iconst(types::I64, 0);
                let mut terminated = false;
                for s in body {
                    if !terminated {
                        let (val, term) = self.translate_stmt(*s)?;
                        last_val = val;
                        terminated = term;
                    }
                }

                // Release local object variables
                if !terminated {
                    let current_vars: Vec<ustr::Ustr> = self.variables.keys().cloned().collect();
                    for var_name in current_vars {
                        if !initial_vars.contains(&var_name) {
                            let (var, ty) = self.variables.get(&ustr::Ustr::from(&var_name)).unwrap().clone();
                            if matches!(ty, VarType::Object(_)) {
                                let obj_val = self.builder.use_var(var);
                                let release_id = *self
                                    .context
                                    .funcs
                                    .get(&ustr::Ustr::from("release"))
                                    .unwrap_or_else(|| panic!("release not found"));
                                let local_release = self
                                    .context
                                    .module
                                    .declare_func_in_func(release_id, self.builder.func);
                                self.builder.ins().call(local_release, &[obj_val]);
                            }
                            self.variables.remove(&ustr::Ustr::from(&var_name));
                        }
                    }
                } else {
                    // Just remove from scope without emitting instructions, since block is filled.
                    // This causes a memory leak on early return, but prevents Cranelift panic.
                    // Proper fix requires a unified return block.
                    let current_vars: Vec<ustr::Ustr> = self.variables.keys().cloned().collect();
                    for var_name in current_vars {
                        if !initial_vars.contains(&var_name) {
                            self.variables.remove(&ustr::Ustr::from(&var_name));
                        }
                    }
                }

                Ok((last_val, terminated))
            }
            Stmt::Return(expr_opt) => {
                let ret_val = if let Some(expr) = expr_opt {
                    let mut val = self.translate_expr(*expr)?;
                    let val_ty = self.get_expr_type(*expr);
                    if let VarType::Struct(name) = &val_ty {
                        val = self.copy_struct(name, val);
                    }
                    val
                } else {
                    self.builder.ins().iconst(types::I64, 0)
                };

                // Release all active local object variables
                for (var, ty) in self.variables.values() {
                    if matches!(ty, VarType::Object(_)) {
                        let obj_val = self.builder.use_var(*var);
                        let release_id = *self
                            .context
                            .funcs
                            .get(&ustr::Ustr::from("release"))
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

    pub fn get_expr_type(&self, expr_id: pace_ast::arena::ExprId) -> VarType {
        let expr = self.arena.get_expr(expr_id);
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
            Expr::Binary { left, .. } => self.get_expr_type(*left), // simplified
            Expr::MemberAccess {
                object,
                property,
                computed_class: _,
                is_static_operator: _,
            } => {
                if let Expr::Identifier(obj_name, _) = self.arena.get_expr(*object) {
                    if let Some(layout) = self.context.class_layouts.get(obj_name) {
                        if let Some((_, f_ty)) = layout.static_fields.get(property) {
                            return f_ty.clone();
                        }
                    } else if let Some(layout) = self.context.struct_layouts.get(obj_name)
                        && let Some((_, f_ty)) = layout.static_fields.get(property) {
                            return f_ty.clone();
                        }
                }

                let obj_ty = self.get_expr_type(*object);
                if let VarType::Object(obj_name) = obj_ty {
                    if let Some(layout) = self.context.class_layouts.get(&ustr::Ustr::from(&obj_name))
                        && let Some(f_ty) = layout.fields.get(property)
                    {
                        return f_ty.1.clone();
                    }
                } else if let VarType::Struct(obj_name) = obj_ty
                    && let Some(layout) = self.context.struct_layouts.get(&ustr::Ustr::from(&obj_name))
                    && let Some(f_ty) = layout.fields.get(property)
                {
                    return f_ty.1.clone();
                }
                VarType::Unknown
            }
            Expr::OptionalMemberAccess { object, property } => {
                let obj_ty = self.get_expr_type(*object);
                if let VarType::Object(obj_name) = obj_ty {
                    if let Some(layout) = self.context.class_layouts.get(&ustr::Ustr::from(&obj_name))
                        && let Some(f_ty) = layout.fields.get(property)
                    {
                        return f_ty.1.clone();
                    }
                } else if let VarType::Struct(obj_name) = obj_ty
                    && let Some(layout) = self.context.struct_layouts.get(&ustr::Ustr::from(&obj_name))
                    && let Some(f_ty) = layout.fields.get(property)
                {
                    return f_ty.1.clone();
                }
                VarType::Unknown
            }
            Expr::Unwrap(inner) => {
                let inner_ty = self.get_expr_type(*inner);
                if let VarType::Nullable(nested) = inner_ty {
                    *nested
                } else {
                    inner_ty
                }
            }
            Expr::NullCoalesce { left, right } => {
                let left_ty = self.get_expr_type(*left);
                if matches!(left_ty, VarType::Unknown) {
                    self.get_expr_type(*right)
                } else {
                    left_ty
                }
            }
            Expr::Null => VarType::Unknown,
            Expr::Call { callee, .. } => {
                if let Expr::Identifier(name, _) = self.arena.get_expr(*callee) {
                    if let Some(ty) = self.func_returns.get(name) {
                        return ty.clone();
                    } else if self.context.struct_layouts.contains_key(name) {
                        return VarType::Struct(name.to_string());
                    } else if self.context.class_layouts.contains_key(name) {
                        return VarType::Object(ustr::Ustr::from(name));
                    } else if name.starts_with("Result_")
                        || name.starts_with("Option_")
                        || name.contains("__Result_")
                        || name.contains("__Option_")
                    {
                        return VarType::Enum(ustr::Ustr::from(name));
                    } else if let Some(pos) = name.rfind("__") {
                        let base_name = &name[pos + 2..];
                        if base_name.chars().next().is_some_and(|c| c.is_uppercase()) {
                            return VarType::Object(ustr::Ustr::from(name));
                        }
                    }
                } else if let Expr::MemberAccess {
                    object,
                    property,
                    computed_class: _,
                    is_static_operator: _,
                } = self.arena.get_expr(*callee)
                {
                    if let Expr::Identifier(obj_name, _) = self.arena.get_expr(*object) {
                        if obj_name.starts_with("Result_") || obj_name.starts_with("Option_") {
                            return VarType::Enum(ustr::Ustr::from(obj_name));
                        }
                        if self.context.class_layouts.contains_key(obj_name)
                            || self.context.struct_layouts.contains_key(obj_name)
                        {
                            let static_method_name = format!("{}_{}", obj_name, property);
                            if let Some(ty) = self.func_returns.get(&ustr::Ustr::from(&static_method_name)) {
                                return ty.clone();
                            }
                        }
                    }
                    let obj_ty = self.get_expr_type(*object);
                    if let VarType::Object(obj_name) = obj_ty {
                        let full_name = format!("{}_{}", obj_name, property);
                        if let Some(ty) = self.func_returns.get(&ustr::Ustr::from(&full_name)) {
                            return ty.clone();
                        }
                        return VarType::Unknown;
                    } else if let VarType::Struct(obj_name) = obj_ty {
                        let full_name = format!("{}_{}", obj_name, property);
                        if let Some(ty) = self.func_returns.get(&ustr::Ustr::from(&full_name)) {
                            return ty.clone();
                        }
                    }
                }
                VarType::Unknown
            }
            Expr::Try(inner) => {
                let inner_ty = self.get_expr_type(*inner);
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
                let inner_ty = self.get_expr_type(*inner);
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

    pub fn translate_args(&mut self, args: &[pace_ast::arena::ExprId]) -> Result<Vec<Value>, CodegenError> {
        let mut arg_vals = Vec::new();
        for arg in args {
            let mut arg_val = self.translate_expr(*arg)?;
            let arg_ty = self.get_expr_type(*arg);
            if let VarType::Struct(name) = &arg_ty {
                arg_val = self.copy_struct(name, arg_val);
            }
            arg_vals.push(arg_val);
        }
        Ok(arg_vals)
    }

    pub(crate) fn copy_struct(&mut self, name: &str, src_ptr: Value) -> Value {
        let layout = self.context.struct_layouts.get(&ustr::Ustr::from(name)).unwrap();
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

}
