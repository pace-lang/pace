use super::{Translator, VarType};
use crate::layouts::CodegenError;
use cranelift::prelude::*;
use cranelift_module::{DataDescription, Linkage, Module};
use pace_ast::{BinaryOp, Expr};

impl<'a, 'b, M: Module> Translator<'a, 'b, M> {
    pub fn translate_expr(
        &mut self,
        expr_id: pace_ast::arena::ExprId,
    ) -> Result<Value, CodegenError> {
        let expr = self.arena.get_expr(expr_id);
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
                    let mut bytes: Vec<u8> = s.as_str().bytes().collect();
                    bytes.push(0); // Null terminator
                    data_ctx.define(bytes.into_boxed_slice());

                    let data_id = self
                        .context
                        .module
                        .declare_data(&name, Linkage::Local, false, false)
                        .unwrap();
                    self.context.module.define_data(data_id, &data_ctx).unwrap();

                    self.context.string_cache.insert(*s, name.clone());
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
                    let part_ty = self.get_expr_type(*part);
                    let val = self.translate_expr(*part)?;

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
                            *self
                                .context
                                .funcs
                                .get(&ustr::Ustr::from("float_to_string"))
                                .unwrap(),
                            self.builder.func,
                        );
                        let call = self.builder.ins().call(to_str, &[float_val]);
                        self.builder.inst_results(call)[0]
                    } else if part_ty == VarType::Bool {
                        let to_str = self.context.module.declare_func_in_func(
                            *self
                                .context
                                .funcs
                                .get(&ustr::Ustr::from("bool_to_string"))
                                .unwrap(),
                            self.builder.func,
                        );
                        let call = self.builder.ins().call(to_str, &[val]);
                        self.builder.inst_results(call)[0]
                    } else {
                        // Assume Int
                        let to_str = self.context.module.declare_func_in_func(
                            *self
                                .context
                                .funcs
                                .get(&ustr::Ustr::from("int_to_string"))
                                .unwrap(),
                            self.builder.func,
                        );
                        let call = self.builder.ins().call(to_str, &[val]);
                        self.builder.inst_results(call)[0]
                    };

                    if let Some(prev) = current_val {
                        let concat = self.context.module.declare_func_in_func(
                            *self
                                .context
                                .funcs
                                .get(&ustr::Ustr::from("concat_strings"))
                                .unwrap(),
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
                        let retain_id =
                            *self.context.funcs.get(&ustr::Ustr::from("retain")).unwrap();
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
            Expr::Unary {
                op,
                expr: inner_expr,
            } => {
                let inner_val = self.translate_expr(*inner_expr)?;
                let ty = self.builder.func.dfg.value_type(inner_val);
                let is_float = ty == types::F64;
                match op {
                    pace_ast::UnaryOp::Not => {
                        let c = self.builder.ins().icmp_imm_u(IntCC::Equal, inner_val, 0);
                        Ok(self.builder.ins().uextend(types::I64, c))
                    }
                    pace_ast::UnaryOp::Neg => {
                        if is_float {
                            Ok(self.builder.ins().fneg(inner_val))
                        } else {
                            Ok(self.builder.ins().ineg(inner_val))
                        }
                    }
                    pace_ast::UnaryOp::BitNot => Ok(self.builder.ins().bnot(inner_val)),
                }
            }
            Expr::Binary { left, op, right } => {
                let lhs = self.translate_expr(*left)?;
                let rhs = self.translate_expr(*right)?;

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
                let promise_ptr = self.translate_expr(*inner)?;
                let await_id = *self
                    .context
                    .funcs
                    .get(&ustr::Ustr::from("__pace_promise_await"))
                    .unwrap();
                let local_await = self
                    .context
                    .module
                    .declare_func_in_func(await_id, self.builder.func);
                let call = self.builder.ins().call(local_await, &[promise_ptr]);
                Ok(self.builder.inst_results(call)[0])
            }
            Expr::Try(inner) => {
                let inner_ptr = self.translate_expr(*inner)?;

                // Read the tag at offset 8 (0 = Ok/Some, 1 = Err/None usually based on how variants are sorted)
                // Wait, we need to know exactly which tag is Ok/Err.
                // We'll dynamically look up the tag ID of "Ok" or "Some".
                let inner_ty = self.get_expr_type(*inner);
                let enum_name = if let VarType::Enum(name) = inner_ty {
                    name
                } else {
                    return Err(CodegenError {
                        message: "? operator used on non-enum".to_string(),
                    });
                };
                let enum_layout = self
                    .context
                    .enum_layouts
                    .get(&ustr::Ustr::from(&enum_name))
                    .unwrap();

                // Determine which tags represent the success and failure
                let is_result = enum_name.starts_with("Result_");
                let (success_tag, _) = if is_result {
                    enum_layout.variants.get(&ustr::Ustr::from("Ok")).unwrap()
                } else {
                    enum_layout.variants.get(&ustr::Ustr::from("Some")).unwrap()
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
                let mut val = self.translate_expr(*value)?;
                let val_ty = self.get_expr_type(*value);
                if let VarType::Struct(name) = &val_ty {
                    val = self.copy_struct(name, val);
                }
                if let Expr::Identifier(name, _) = self.arena.get_expr(*target) {
                    if let Some((var, ty)) = self.variables.get(name) {
                        if matches!(ty, VarType::Object(_)) {
                            // Release old value
                            let old_val = self.builder.use_var(*var);
                            let release_id = *self
                                .context
                                .funcs
                                .get(&ustr::Ustr::from("release"))
                                .unwrap();
                            let local_release = self
                                .context
                                .module
                                .declare_func_in_func(release_id, self.builder.func);
                            self.builder.ins().call(local_release, &[old_val]);

                            // Retain new value for the variable (caller gets the original +1)
                            let retain_id =
                                *self.context.funcs.get(&ustr::Ustr::from("retain")).unwrap();
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
                } = self.arena.get_expr(*target)
                {
                    if let Expr::Identifier(obj_name, _) = self.arena.get_expr(*object) {
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
                                let release_id = *self
                                    .context
                                    .funcs
                                    .get(&ustr::Ustr::from("release"))
                                    .unwrap();
                                let local_release = self
                                    .context
                                    .module
                                    .declare_func_in_func(release_id, self.builder.func);
                                self.builder.ins().call(local_release, &[old_val]);

                                let retain_id =
                                    *self.context.funcs.get(&ustr::Ustr::from("retain")).unwrap();
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

                    let obj_ptr = self.translate_expr(*object)?;

                    let obj_type = self.get_expr_type(*object);
                    let (f_offset, f_ty) = match obj_type {
                        VarType::Object(name) => {
                            let layout = self
                                .context
                                .class_layouts
                                .get(&ustr::Ustr::from(name.as_ref()))
                                .unwrap();
                            layout.fields.get(property).unwrap().clone()
                        }
                        VarType::Struct(name) => {
                            let layout = self
                                .context
                                .struct_layouts
                                .get(&ustr::Ustr::from(name.as_ref()))
                                .unwrap();
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
                        let release_id = *self
                            .context
                            .funcs
                            .get(&ustr::Ustr::from("release"))
                            .unwrap();
                        let local_release = self
                            .context
                            .module
                            .declare_func_in_func(release_id, self.builder.func);
                        self.builder.ins().call(local_release, &[old_val]);

                        let retain_id =
                            *self.context.funcs.get(&ustr::Ustr::from("retain")).unwrap();
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
                let callee_ty = self.get_expr_type(*callee);
                if matches!(callee_ty, VarType::Function(_, _)) {
                    let fat_ptr = self.translate_expr(*callee)?;

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
                        let mut arg_val = self.translate_expr(*arg)?;
                        let arg_ty = self.get_expr_type(*arg);
                        if let VarType::Struct(name) = &arg_ty {
                            arg_val = self.copy_struct(name, arg_val);
                        }
                        arg_vals.push(arg_val);
                    }

                    let mut sig = self.context.module.make_signature();
                    sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
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

                if let Expr::Identifier(func_name, _) = self.arena.get_expr(*callee) {
                    if func_name == "print" {
                        let arg_expr = &args[0];
                        let arg_ty = self.get_expr_type(*arg_expr);

                        let arg_val = self.translate_expr(*arg_expr)?;
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

                        let func_id = *self
                            .context
                            .funcs
                            .get(&ustr::Ustr::from(target_name))
                            .unwrap();
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

                        let malloc_id =
                            *self.context.funcs.get(&ustr::Ustr::from("malloc")).unwrap();
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
                                let mb_create_id = *self
                                    .context
                                    .funcs
                                    .get(&ustr::Ustr::from("__pace_mailbox_create"))
                                    .unwrap();
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
                        if let Some(&init_id) =
                            self.context.funcs.get(&ustr::Ustr::from(&init_name))
                        {
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
                        let mut sorted_fields: Vec<_> =
                            layout.fields.iter().map(|(k, v)| (*k, v.clone())).collect();
                        sorted_fields.sort_by_key(|(_, (offset, _))| *offset);

                        for (i, arg) in args.iter().enumerate() {
                            if let Some((_, (offset, _))) = sorted_fields.get(i) {
                                let mut arg_val = self.translate_expr(*arg)?;
                                let arg_ty = self.get_expr_type(*arg);
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
                } = self.arena.get_expr(*callee)
                {
                    let mut obj_name_opt = None;
                    if let Expr::Identifier(obj_name, _) = self.arena.get_expr(*object) {
                        obj_name_opt = Some(*obj_name);
                    } else if let Expr::GenericInstantiation { callee, .. } =
                        self.arena.get_expr(*object)
                        && let Expr::Identifier(obj_name, _) = self.arena.get_expr(*callee)
                    {
                        obj_name_opt = Some(*obj_name);
                    }

                    if let Some(obj_name) = obj_name_opt {
                        if self.context.enum_layouts.contains_key(&obj_name) {
                            let constructor_name = format!("{}_{}", obj_name, property);
                            let func_id = self
                                .context
                                .funcs
                                .get(&ustr::Ustr::from(&constructor_name))
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
                                .get(&ustr::Ustr::from(&static_method_name))
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

                    let obj_ptr = self.translate_expr(*object)?;
                    let ptr_ty = self.context.module.target_config().pointer_type();

                    let obj_type = self.get_expr_type(*object);
                    let (m_offset, is_actor) = if let VarType::Object(type_name) = &obj_type {
                        let layout = self
                            .context
                            .class_layouts
                            .get(&ustr::Ustr::from(type_name))
                            .unwrap_or_else(|| {
                                panic!("Class or interface {} not found in layouts", type_name)
                            });
                        (
                            *layout.methods.get(property).unwrap_or_else(|| {
                                panic!("Method {} not found in {}", property, type_name)
                            }),
                            layout.fields.contains_key(&ustr::Ustr::from("__mailbox")),
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
                        let mut arg_val = self.translate_expr(*arg)?;
                        let arg_ty = self.get_expr_type(*arg);
                        if let VarType::Struct(name) = &arg_ty {
                            arg_val = self.copy_struct(name, arg_val);
                        }
                        arg_vals.push(arg_val);
                    }

                    if is_actor {
                        let promise_create_id = *self
                            .context
                            .funcs
                            .get(&ustr::Ustr::from("__pace_promise_create"))
                            .unwrap();
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
                        let mb_offset =
                            layout.fields.get(&ustr::Ustr::from("__mailbox")).unwrap().0;
                        let mailbox_ptr = self.builder.ins().load(
                            types::I64,
                            cranelift::prelude::MemFlagsData::new(),
                            obj_ptr,
                            mb_offset as i32,
                        );

                        let malloc_id =
                            *self.context.funcs.get(&ustr::Ustr::from("malloc")).unwrap();
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

                        let mb_send_id = *self
                            .context
                            .funcs
                            .get(&ustr::Ustr::from("__pace_mailbox_send"))
                            .unwrap();
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
                        sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
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
                if let Expr::Identifier(obj_name, _) = self.arena.get_expr(*object) {
                    obj_name_opt = Some(*obj_name);
                } else if let Expr::GenericInstantiation { callee, .. } =
                    self.arena.get_expr(*object)
                    && let Expr::Identifier(obj_name, _) = self.arena.get_expr(*callee)
                {
                    obj_name_opt = Some(*obj_name);
                }

                if let Some(obj_name) = obj_name_opt {
                    if self.context.enum_layouts.contains_key(&obj_name) {
                        let constructor_name = format!("{}_{}", obj_name, property);
                        let func_id = self
                            .context
                            .funcs
                            .get(&ustr::Ustr::from(&constructor_name))
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

                    let maybe_static_field = if let Some(layout) =
                        self.context.class_layouts.get(&ustr::Ustr::from(&obj_name))
                    {
                        layout.static_fields.get(property)
                    } else if let Some(layout) = self
                        .context
                        .struct_layouts
                        .get(&ustr::Ustr::from(&obj_name))
                    {
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
                            let retain_id =
                                *self.context.funcs.get(&ustr::Ustr::from("retain")).unwrap();
                            let local_retain = self
                                .context
                                .module
                                .declare_func_in_func(retain_id, self.builder.func);
                            self.builder.ins().call(local_retain, &[val]);
                        }

                        return Ok(val);
                    }
                }

                let obj_ptr = self.translate_expr(*object)?;

                let obj_type = self.get_expr_type(*object);
                let (f_offset, f_ty) = match obj_type {
                    VarType::Object(name) => {
                        let layout = self
                            .context
                            .class_layouts
                            .get(&ustr::Ustr::from(name.as_ref()))
                            .unwrap();
                        layout.fields.get(property).unwrap().clone()
                    }
                    VarType::Struct(name) => {
                        let layout = self
                            .context
                            .struct_layouts
                            .get(&ustr::Ustr::from(name.as_ref()))
                            .unwrap();
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
                    let retain_id = *self.context.funcs.get(&ustr::Ustr::from("retain")).unwrap();
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
                let closure_id = self.context.closure_id;
                self.context.closure_id += 1;
                let fn_name = format!("__closure_fn_{}", closure_id);

                let mut captured_vars = Vec::new();
                for (name, (_, ty)) in self.variables.iter() {
                    captured_vars.push((*name, ty.clone()));
                }

                captured_vars.sort_by_key(|a| a.0);

                let env_size = 16 + (captured_vars.len() * 8);

                let malloc_id = *self.context.funcs.get(&ustr::Ustr::from("malloc")).unwrap();
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
                    .push((fn_name.clone().into(), expr.clone(), captured_vars));

                let mut sig = self.context.module.make_signature();
                sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
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
                let inner_val = self.translate_expr(*inner)?;

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
                let left_val = self.translate_expr(*left)?;

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
                let right_val = self.translate_expr(*right)?;
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
                let obj_ptr = self.translate_expr(*object)?;

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

                let obj_type = self.get_expr_type(*object);
                let (f_offset, f_ty) = match obj_type {
                    VarType::Object(name) => {
                        let layout = self
                            .context
                            .class_layouts
                            .get(&ustr::Ustr::from(name.as_ref()))
                            .unwrap();
                        layout.fields.get(property).unwrap().clone()
                    }
                    VarType::Struct(name) => {
                        let layout = self
                            .context
                            .struct_layouts
                            .get(&ustr::Ustr::from(name.as_ref()))
                            .unwrap();
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
                    let retain_id = *self.context.funcs.get(&ustr::Ustr::from("retain")).unwrap();
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
