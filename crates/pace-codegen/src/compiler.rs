use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Module, Linkage, FuncId};
use pace_ast::{Expr, Stmt, BinaryOp};
use std::collections::HashMap;
use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
#[error("Codegen error: {message}")]
#[diagnostic(code(pace::codegen_error))]
pub struct CodegenError {
    pub message: String,
}

pub struct JITCompiler {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    module: JITModule,
    funcs: HashMap<String, FuncId>,
}

impl JITCompiler {
    pub fn new() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        
        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });
        
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .unwrap();

        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        let module = JITModule::new(builder);

        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
            funcs: HashMap::new(),
        }
    }

    pub fn compile_and_run(&mut self, stmts: &[Stmt]) -> Result<(), CodegenError> {
        // Pass 1: Declare all functions
        for stmt in stmts {
            if let Stmt::FuncDecl { name, params, .. } = stmt {
                let mut sig = self.module.make_signature();
                for _ in params {
                    sig.params.push(AbiParam::new(types::I64)); // Assume I64 for now
                }
                sig.returns.push(AbiParam::new(types::I64)); // Assume I64 return
                
                let id = self.module.declare_function(name, Linkage::Local, &sig)
                    .map_err(|e| CodegenError { message: e.to_string() })?;
                self.funcs.insert(name.clone(), id);
            }
        }

        // Pass 2: Define all functions
        for stmt in stmts {
            if let Stmt::FuncDecl { name, params, body, .. } = stmt {
                let id = *self.funcs.get(name).unwrap();
                self.compile_function(name, params, body, id)?;
            }
        }

        // Pass 3: Compile implicit `__entry__` that executes top-level code and calls `main` if it exists.
        self.ctx.func.signature.returns.push(AbiParam::new(types::I64));
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let mut variables = HashMap::new();
        let mut var_index = 0;
        let mut last_val = None;

        for stmt in stmts {
            match stmt {
                Stmt::VarDecl { .. } | Stmt::Expr(_) | Stmt::If { .. } | Stmt::While { .. } | Stmt::Loop { .. } => {
                    let (val, _) = Self::translate_stmt(&mut self.module, &self.funcs, &mut builder, stmt, &mut variables, &mut var_index)?;
                    last_val = Some(val);
                }
                _ => {}
            }
        }

        // Call main if it exists
        if let Some(&main_id) = self.funcs.get("main") {
            let local_func = self.module.declare_func_in_func(main_id, &mut builder.func);
            let call = builder.ins().call(local_func, &[]);
            let res = builder.inst_results(call)[0];
            last_val = Some(res);
        }

        let ret_val = last_val.unwrap_or_else(|| builder.ins().iconst(types::I64, 0));
        builder.ins().return_(&[ret_val]);
        builder.finalize();

        let id = self.module
            .declare_function("__entry__", Linkage::Export, &self.ctx.func.signature)
            .map_err(|e| CodegenError { message: e.to_string() })?;

        self.module
            .define_function(id, &mut self.ctx)
            .map_err(|e| CodegenError { message: e.to_string() })?;

        self.module.clear_context(&mut self.ctx);
        self.module.finalize_definitions().unwrap();

        let code = self.module.get_finalized_function(id);
        
        // Execute the code
        let entry_func: fn() -> i64 = unsafe { std::mem::transmute(code) };
        let result = entry_func();
        
        println!("Execution returned: {}", result);

        Ok(())
    }

    fn compile_function(
        &mut self,
        _name: &str,
        params: &[pace_ast::Param],
        body: &[Stmt],
        func_id: FuncId,
    ) -> Result<(), CodegenError> {
        self.ctx.func.signature.returns.push(AbiParam::new(types::I64));
        for _ in params {
            self.ctx.func.signature.params.push(AbiParam::new(types::I64));
        }

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let mut variables = HashMap::new();
        let mut var_index = 0;

        // Declare parameters as variables
        for (i, param) in params.iter().enumerate() {
            let val = builder.block_params(entry_block)[i];
            let var = Variable::new(var_index);
            builder.declare_var(var, types::I64);
            builder.def_var(var, val);
            variables.insert(param.name.clone(), var);
            var_index += 1;
        }

        let mut last_val = None;
        let mut terminated = false;
        for stmt in body {
            let (val, term) = Self::translate_stmt(&mut self.module, &self.funcs, &mut builder, stmt, &mut variables, &mut var_index)?;
            last_val = Some(val);
            if term {
                terminated = true;
                break;
            }
        }

        // Implicit return if block isn't terminated
        if !terminated {
            let ret = last_val.unwrap_or_else(|| builder.ins().iconst(types::I64, 0));
            builder.ins().return_(&[ret]);
        }
        
        builder.finalize();
        
        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| CodegenError { message: e.to_string() })?;
        
        self.module.clear_context(&mut self.ctx);
        Ok(())
    }

    fn translate_stmt(
        module: &mut JITModule,
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
                
                // For simplicity, we just return the else_res or then_res, but actual PHI nodes are needed if it's used as expression.
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
                
                // Header block is sealed after all jumps to it are created
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
                
                // Dead code after infinite loop, but Cranelift needs a current block
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

    fn translate_expr(
        module: &mut JITModule,
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
                
                // Compare operations need to be cast to i64 (0 or 1)
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
