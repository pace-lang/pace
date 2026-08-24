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
    Nullable(Box<VarType>),
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
        _ => VarType::Object(base_name.to_string()),
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
        builder: &mut FunctionBuilder,
        stmt: &Stmt,
        variables: &mut HashMap<String, (Variable, VarType)>,
        var_index: &mut usize,
        func_returns: &HashMap<String, VarType>,
    ) -> Result<(Value, bool), CodegenError> {
        match stmt {
            Stmt::VarDecl { name, initializer, .. } => {
                let mut var_ty = VarType::Unknown;
                let val = if let Some(expr) = initializer {
                    var_ty = Self::get_expr_type(expr, variables, func_returns, struct_layouts);
                    let mut val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, expr, variables, var_index, func_returns)?;
                    
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
                let ty = Self::get_expr_type(expr, variables, func_returns, struct_layouts);
                let val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, expr, variables, var_index, func_returns)?;
                if matches!(ty, VarType::Object(_)) {
                    let release_id = *funcs.get("release").unwrap();
                    let local_release = module.declare_func_in_func(release_id, builder.func);
                    builder.ins().call(local_release, &[val]);
                }
                Ok((val, false))
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let cond_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, condition, variables, var_index, func_returns)?;
                
                let then_block = builder.create_block();
                let else_block = builder.create_block();
                let merge_block = builder.create_block();
                
                builder.ins().brif(cond_val, then_block, &[], else_block, &[]);
                
                // Then
                builder.switch_to_block(then_block);
                builder.seal_block(then_block);
                let (then_res, then_term) = Self::translate_stmt(module, funcs, class_layouts, struct_layouts, builder, then_branch, variables, var_index, func_returns)?;
                if !then_term {
                    builder.ins().jump(merge_block, &[]);
                }
                
                // Else
                builder.switch_to_block(else_block);
                builder.seal_block(else_block);
                let (_else_res, else_term) = if let Some(elb) = else_branch {
                    Self::translate_stmt(module, funcs, class_layouts, struct_layouts, builder, elb, variables, var_index, func_returns)?
                } else {
                    (builder.ins().iconst(types::I64, 0), false)
                };
                if !else_term {
                    builder.ins().jump(merge_block, &[]);
                }
                
                // Merge
                builder.switch_to_block(merge_block);
                builder.seal_block(merge_block);
                
                Ok((then_res, then_term && else_term))
            }
            Stmt::While { condition, body } => {
                let cond_block = builder.create_block();
                let body_block = builder.create_block();
                let exit_block = builder.create_block();
                
                builder.ins().jump(cond_block, &[]);
                builder.switch_to_block(cond_block);
                
                let cond_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, condition, variables, var_index, func_returns)?;
                builder.ins().brif(cond_val, body_block, &[], exit_block, &[]);
                
                builder.switch_to_block(body_block);
                builder.seal_block(body_block);
                
                let (_, body_term) = Self::translate_stmt(module, funcs, class_layouts, struct_layouts, builder, body, variables, var_index, func_returns)?;
                if !body_term {
                    builder.ins().jump(cond_block, &[]);
                }
                
                builder.seal_block(cond_block);
                
                builder.switch_to_block(exit_block);
                builder.seal_block(exit_block);
                
                Ok((builder.ins().iconst(types::I64, 0), false))
            }
            Stmt::Loop { body } => {
                let body_block = builder.create_block();
                
                builder.ins().jump(body_block, &[]);
                builder.switch_to_block(body_block);
                
                let (_, body_term) = Self::translate_stmt(module, funcs, class_layouts, struct_layouts, builder, body, variables, var_index, func_returns)?;
                if !body_term {
                    builder.ins().jump(body_block, &[]);
                }
                
                builder.seal_block(body_block);
                
                let exit = builder.create_block();
                builder.switch_to_block(exit);
                builder.seal_block(exit);
                Ok((builder.ins().iconst(types::I64, 0), false))
            }
            Stmt::Module { body, .. } | Stmt::Block(body) => {
                let initial_vars: Vec<String> = variables.keys().cloned().collect();
                
                let mut last_val = builder.ins().iconst(types::I64, 0);
                let mut terminated = false;
                for s in body {
                    let (val, term) = Self::translate_stmt(module, funcs, class_layouts, struct_layouts, builder, s, variables, var_index, func_returns)?;
                    last_val = val;
                    if term {
                        terminated = true;
                        break;
                    }
                }
                
                // Release local object variables
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
                
                Ok((last_val, terminated))
            }
            Stmt::Return(expr_opt) => {
                let ret_val = if let Some(expr) = expr_opt {
                    let mut val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, expr, variables, var_index, func_returns)?;
                    let val_ty = Self::get_expr_type(expr, variables, func_returns, struct_layouts);
                    if let VarType::Struct(name) = &val_ty {
                        val = Self::copy_struct(module, struct_layouts, builder, name, val);
                    }
                    val
                } else {
                    builder.ins().iconst(types::I64, 0)
                };
                builder.ins().return_(&[ret_val]);
                Ok((ret_val, true))
            }
            _ => Ok((builder.ins().iconst(types::I64, 0), false))
        }
    }
    
    pub fn get_expr_type(expr: &Expr, variables: &HashMap<String, (Variable, VarType)>, func_returns: &HashMap<String, VarType>, struct_layouts: &HashMap<String, crate::compiler::StructLayout>) -> VarType {
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
            Expr::Binary { left, .. } => Self::get_expr_type(left, variables, func_returns, struct_layouts), // simplified
            Expr::Call { callee, .. } => {
                if let Expr::Identifier(name) = &**callee {
                    if let Some(ty) = func_returns.get(name) {
                        return ty.clone();
                    } else if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                        if struct_layouts.contains_key(name) {
                            return VarType::Struct(name.clone());
                        }
                        return VarType::Object(name.clone());
                    }
                } else if let Expr::MemberAccess { object, property, .. } = &**callee {
                    let obj_ty = Self::get_expr_type(object, variables, func_returns, struct_layouts);
                    if let VarType::Object(obj_name) = obj_ty {
                        let full_name = format!("{}_{}", obj_name, property);
                        if let Some(ty) = func_returns.get(&full_name) {
                            return ty.clone();
                        }
                    } else if let VarType::Struct(obj_name) = obj_ty {
                        let full_name = format!("{}_{}", obj_name, property);
                        if let Some(ty) = func_returns.get(&full_name) {
                            return ty.clone();
                        }
                    }
                }
                VarType::Unknown
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
        class_layouts: &HashMap<String, ClassLayout>,
        struct_layouts: &HashMap<String, crate::compiler::StructLayout>,
        builder: &mut FunctionBuilder,
        expr: &Expr,
        variables: &mut HashMap<String, (Variable, VarType)>,
        var_index: &mut usize,
        func_returns: &HashMap<String, VarType>,
    ) -> Result<Value, CodegenError> {
        match expr {
            Expr::IntLiteral(i) => Ok(builder.ins().iconst(types::I64, *i)),
            Expr::FloatLiteral(f) => Ok(builder.ins().f64const(*f)),
            Expr::StringLiteral(s) => {
                let mut data_ctx = DataDescription::new();
                let mut bytes = s.clone().into_bytes();
                bytes.push(0); // Null terminator
                data_ctx.define(bytes.into_boxed_slice());
                
                static STRING_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                let id = STRING_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let string_name = format!("__str_const_{}", id);
                
                let data_id = module.declare_data(&string_name, Linkage::Local, false, false).unwrap();
                module.define_data(data_id, &data_ctx).unwrap();
                
                let local_id = module.declare_data_in_func(data_id, builder.func);
                let ptr_ty = module.target_config().pointer_type();
                Ok(builder.ins().symbol_value(ptr_ty, local_id))
            }
            Expr::InterpolatedString(parts) => {
                if parts.is_empty() {
                    let mut data_ctx = DataDescription::new();
                    data_ctx.define(vec![0].into_boxed_slice());
                    static EMPTY_STRING_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                    let id = EMPTY_STRING_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let string_name = format!("__empty_str_{}", id);
                    let data_id = module.declare_data(&string_name, Linkage::Local, false, false).unwrap();
                    module.define_data(data_id, &data_ctx).unwrap();
                    let local_id = module.declare_data_in_func(data_id, builder.func);
                    let ptr_ty = module.target_config().pointer_type();
                    return Ok(builder.ins().symbol_value(ptr_ty, local_id));
                }
                
                let mut current_val = None;
                for part in parts {
                    let part_ty = Self::get_expr_type(part, variables, func_returns, struct_layouts);
                    let val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, part, variables, var_index, func_returns)?;
                    
                    let str_val = if part_ty == VarType::String {
                        val
                    } else if part_ty == VarType::Float {
                        let to_str = module.declare_func_in_func(*funcs.get("float_to_string").unwrap(), builder.func);
                        let call = builder.ins().call(to_str, &[val]);
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
                    Err(CodegenError { message: format!("Undefined variable: {}", name) })
                }
            }
            Expr::Binary { left, op, right } => {
                let lhs = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, left, variables, var_index, func_returns)?;
                let rhs = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, right, variables, var_index, func_returns)?;
                
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
            Expr::Assign { target, value } => {
                let mut val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, value, variables, var_index, func_returns)?;
                let val_ty = Self::get_expr_type(value, variables, func_returns, struct_layouts);
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
                    let obj_ptr = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, object, variables, var_index, func_returns)?;
                    
                    let obj_type = Self::get_expr_type(object, variables, func_returns, struct_layouts);
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
                        let arg_ty = Self::get_expr_type(arg_expr, variables, func_returns, struct_layouts);
                        
                        let arg_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, arg_expr, variables, var_index, func_returns)?;
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
                            let mut arg_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, arg, variables, var_index, func_returns)?;
                            let arg_ty = Self::get_expr_type(arg, variables, func_returns, struct_layouts);
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
                        for &(offset, _) in layout.fields.values() {
                            builder.ins().store(cranelift::prelude::MemFlagsData::new(), zero, obj_ptr, offset as i32);
                        }
                        
                        // Call init if it exists
                        let init_name = format!("{}_init", func_name);
                        if let Some(&init_id) = funcs.get(&init_name) {
                            let local_init = module.declare_func_in_func(init_id, builder.func);
                            let mut arg_vals = vec![obj_ptr];
                            for arg in args {
                                arg_vals.push(Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, arg, variables, var_index, func_returns)?);
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
                                let mut arg_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, arg, variables, var_index, func_returns)?;
                                let arg_ty = Self::get_expr_type(arg, variables, func_returns, struct_layouts);
                                if let VarType::Struct(name) = &arg_ty {
                                    arg_val = Self::copy_struct(module, struct_layouts, builder, name, arg_val);
                                }
                                builder.ins().store(cranelift::prelude::MemFlagsData::new(), arg_val, obj_ptr, *offset as i32);
                            }
                        }
                        
                        return Ok(obj_ptr);
                    }
                } else if let Expr::MemberAccess { object, property, .. } = &**callee {
                    let obj_ptr = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, object, variables, var_index, func_returns)?;
                    let ptr_ty = module.target_config().pointer_type();
                    
                    let obj_type = Self::get_expr_type(object, variables, func_returns, struct_layouts);
                    let m_offset = if let VarType::Object(type_name) = &obj_type {
                        let layout = class_layouts.get(type_name)
                            .unwrap_or_else(|| panic!("Class or interface {} not found in layouts", type_name));
                        *layout.methods.get(property).unwrap_or_else(|| panic!("Method {} not found in {}", property, type_name))
                    } else {
                        let layout = class_layouts.values().find(|l| l.methods.contains_key(property))
                            .unwrap_or_else(|| panic!("Method {} not found in any class layout", property));
                        *layout.methods.get(property).unwrap()
                    };
                    
                    let vtable_ptr = builder.ins().load(ptr_ty, cranelift::prelude::MemFlagsData::new(), obj_ptr, 8);
                    let method_ptr = builder.ins().load(ptr_ty, cranelift::prelude::MemFlagsData::new(), vtable_ptr, m_offset as i32);
                    
                    let mut sig = module.make_signature();
                    sig.params.push(AbiParam::new(ptr_ty)); // self
                    for _ in args {
                        sig.params.push(AbiParam::new(types::I64));
                    }
                    sig.returns.push(AbiParam::new(types::I64));
                    
                    let mut arg_vals = vec![obj_ptr];
                    for arg in args {
                        let mut arg_val = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, arg, variables, var_index, func_returns)?;
                        let arg_ty = Self::get_expr_type(arg, variables, func_returns, struct_layouts);
                        if let VarType::Struct(name) = &arg_ty {
                            arg_val = Self::copy_struct(module, struct_layouts, builder, name, arg_val);
                        }
                        arg_vals.push(arg_val);
                    }
                    
                    let sig_ref = builder.import_signature(sig);
                    let call = builder.ins().call_indirect(sig_ref, method_ptr, &arg_vals);
                    
                    let results = builder.inst_results(call);
                    if results.is_empty() {
                        return Ok(builder.ins().iconst(types::I64, 0));
                    } else {
                        return Ok(results[0]);
                    }
                }
                Err(CodegenError { message: format!("Cannot resolve function call: {:?}", callee) })
            }
            Expr::MemberAccess { object, property, .. } => {
                let obj_ptr = Self::translate_expr(module, funcs, class_layouts, struct_layouts, builder, object, variables, var_index, func_returns)?;
                
                let obj_type = Self::get_expr_type(object, variables, func_returns, struct_layouts);
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
            _ => Err(CodegenError { message: format!("Cannot translate expression: {:?}", expr) })
        }
    }
}
