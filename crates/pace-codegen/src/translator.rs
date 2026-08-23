use cranelift::prelude::*;
use cranelift_module::{Module, FuncId};
use pace_ast::{Expr, Stmt, BinaryOp};
use std::collections::HashMap;
use crate::compiler::CodegenError;

pub struct Translator;

impl Translator {
    pub fn translate_stmt(
        module: &mut impl Module,
        funcs: &HashMap<String, FuncId>,
        builder: &mut FunctionBuilder,
        stmt: &Stmt,
        variables: &mut HashMap<String, Variable>,
        var_index: &mut usize,
    ) -> Result<(Value, bool), CodegenError> {
        match stmt {
            Stmt::VarDecl { name, initializer, .. } => {
                let val = if let Some(expr) = initializer {
                    Self::translate_expr(module, funcs, builder, expr, variables)?
                } else {
                    builder.ins().iconst(types::I64, 0)
                };
                let var = Variable::new(*var_index);
                builder.declare_var(var, types::I64);
                builder.def_var(var, val);
                variables.insert(name.clone(), var);
                *var_index += 1;
                Ok((val, false))
            }
            Stmt::Expr(expr) => {
                let val = Self::translate_expr(module, funcs, builder, expr, variables)?;
                Ok((val, false))
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let cond_val = Self::translate_expr(module, funcs, builder, condition, variables)?;
                
                let then_block = builder.create_block();
                let else_block = builder.create_block();
                let merge_block = builder.create_block();
                
                builder.ins().brif(cond_val, then_block, &[], else_block, &[]);
                
                // Then
                builder.switch_to_block(then_block);
                builder.seal_block(then_block);
                let (then_res, then_term) = Self::translate_stmt(module, funcs, builder, then_branch, variables, var_index)?;
                if !then_term {
                    builder.ins().jump(merge_block, &[]);
                }
                
                // Else
                builder.switch_to_block(else_block);
                builder.seal_block(else_block);
                let (else_res, else_term) = if let Some(elb) = else_branch {
                    Self::translate_stmt(module, funcs, builder, elb, variables, var_index)?
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
                let header_block = builder.create_block();
                let body_block = builder.create_block();
                let exit_block = builder.create_block();
                
                builder.ins().jump(header_block, &[]);
                builder.switch_to_block(header_block);
                
                let cond_val = Self::translate_expr(module, funcs, builder, condition, variables)?;
                builder.ins().brif(cond_val, body_block, &[], exit_block, &[]);
                
                builder.switch_to_block(body_block);
                builder.seal_block(body_block);
                let (_, body_term) = Self::translate_stmt(module, funcs, builder, body, variables, var_index)?;
                if !body_term {
                    builder.ins().jump(header_block, &[]);
                }
                
                builder.seal_block(header_block);
                
                builder.switch_to_block(exit_block);
                builder.seal_block(exit_block);
                
                Ok((builder.ins().iconst(types::I64, 0), false))
            }
            Stmt::Loop { body } => {
                let body_block = builder.create_block();
                
                builder.ins().jump(body_block, &[]);
                builder.switch_to_block(body_block);
                
                let (_, body_term) = Self::translate_stmt(module, funcs, builder, body, variables, var_index)?;
                if !body_term {
                    builder.ins().jump(body_block, &[]);
                }
                
                builder.seal_block(body_block);
                
                let exit = builder.create_block();
                builder.switch_to_block(exit);
                builder.seal_block(exit);
                Ok((builder.ins().iconst(types::I64, 0), false))
            }
            Stmt::Block(stmts) => {
                let mut last_val = builder.ins().iconst(types::I64, 0);
                let mut terminated = false;
                for s in stmts {
                    let (val, term) = Self::translate_stmt(module, funcs, builder, s, variables, var_index)?;
                    last_val = val;
                    if term {
                        terminated = true;
                        break;
                    }
                }
                Ok((last_val, terminated))
            }
            Stmt::Return(expr_opt) => {
                let ret_val = if let Some(expr) = expr_opt {
                    Self::translate_expr(module, funcs, builder, expr, variables)?
                } else {
                    builder.ins().iconst(types::I64, 0)
                };
                builder.ins().return_(&[ret_val]);
                Ok((ret_val, true))
            }
            _ => Ok((builder.ins().iconst(types::I64, 0), false))
        }
    }

    pub fn translate_expr(
        module: &mut impl Module,
        funcs: &HashMap<String, FuncId>,
        builder: &mut FunctionBuilder,
        expr: &Expr,
        variables: &HashMap<String, Variable>,
    ) -> Result<Value, CodegenError> {
        match expr {
            Expr::IntLiteral(i) => Ok(builder.ins().iconst(types::I64, *i)),
            Expr::FloatLiteral(_) => Err(CodegenError { message: "Floats not supported yet".into() }),
            Expr::StringLiteral(_) => Err(CodegenError { message: "Strings not supported yet".into() }),
            Expr::BoolLiteral(b) => {
                let val = if *b { 1 } else { 0 };
                Ok(builder.ins().iconst(types::I64, val))
            }
            Expr::Identifier(name) => {
                if let Some(var) = variables.get(name) {
                    Ok(builder.use_var(*var))
                } else {
                    Err(CodegenError { message: format!("Variable '{}' not found in JIT environment", name) })
                }
            }
            Expr::Binary { left, op, right } => {
                let lhs = Self::translate_expr(module, funcs, builder, left, variables)?;
                let rhs = Self::translate_expr(module, funcs, builder, right, variables)?;
                
                match op {
                    BinaryOp::Add => Ok(builder.ins().iadd(lhs, rhs)),
                    BinaryOp::Sub => Ok(builder.ins().isub(lhs, rhs)),
                    BinaryOp::Mul => Ok(builder.ins().imul(lhs, rhs)),
                    BinaryOp::Div => Ok(builder.ins().sdiv(lhs, rhs)),
                    BinaryOp::Eq => {
                        let c = builder.ins().icmp(IntCC::Equal, lhs, rhs);
                        Ok(builder.ins().uextend(types::I64, c))
                    }
                    BinaryOp::NotEq => {
                        let c = builder.ins().icmp(IntCC::NotEqual, lhs, rhs);
                        Ok(builder.ins().uextend(types::I64, c))
                    }
                }
            }
            Expr::Call { callee, args } => {
                if let Expr::Identifier(func_name) = &**callee {
                    if let Some(&func_id) = funcs.get(func_name) {
                        let local_func = module.declare_func_in_func(func_id, &mut builder.func);
                        let mut arg_vals = Vec::new();
                        for arg in args {
                            arg_vals.push(Self::translate_expr(module, funcs, builder, arg, variables)?);
                        }
                        let call = builder.ins().call(local_func, &arg_vals);
                        return Ok(builder.inst_results(call)[0]);
                    }
                }
                Err(CodegenError { message: format!("Cannot resolve function call: {:?}", callee) })
            }
            _ => Err(CodegenError { message: format!("Expression type not supported yet: {:?}", expr) })
        }
    }
}
