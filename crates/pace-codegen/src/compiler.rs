use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Module, Linkage};
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
        }
    }

    pub fn compile_and_run(&mut self, stmts: &[Stmt]) -> Result<(), CodegenError> {
        // For simplicity in Phase 3 bootstrap, we'll compile everything into a `main` function and run it.
        
        self.ctx.func.signature.returns.push(AbiParam::new(types::I64));

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        // Very basic variable tracking
        let mut variables = HashMap::new();
        let mut var_index = 0;

        let mut last_val = None;

        for stmt in stmts {
            match stmt {
                Stmt::VarDecl { name, initializer, .. } => {
                    let val = if let Some(expr) = initializer {
                        Self::translate_expr(&mut builder, expr, &variables)?
                    } else {
                        builder.ins().iconst(types::I64, 0) // default 0
                    };
                    
                    let var = Variable::new(var_index);
                    builder.declare_var(var, types::I64);
                    builder.def_var(var, val);
                    variables.insert(name.clone(), var);
                    var_index += 1;
                }
                Stmt::Expr(expr) => {
                    last_val = Some(Self::translate_expr(&mut builder, expr, &variables)?);
                }
                Stmt::FuncDecl { .. } => {
                    // Ignore functions for the very first basic JIT test
                }
                Stmt::ClassDecl { .. } | Stmt::InterfaceDecl { .. } | Stmt::StructDecl { .. } => {
                    // Ignore for now
                }
                _ => {}
            }
        }

        let ret_val = last_val.unwrap_or_else(|| builder.ins().iconst(types::I64, 0));
        builder.ins().return_(&[ret_val]);
        builder.finalize();

        let id = self.module
            .declare_function("main", Linkage::Export, &self.ctx.func.signature)
            .map_err(|e| CodegenError { message: e.to_string() })?;

        self.module
            .define_function(id, &mut self.ctx)
            .map_err(|e| CodegenError { message: e.to_string() })?;

        self.module.clear_context(&mut self.ctx);
        self.module.finalize_definitions().unwrap();

        let code = self.module.get_finalized_function(id);
        
        // Execute the code
        let main_func: fn() -> i64 = unsafe { std::mem::transmute(code) };
        let result = main_func();
        
        println!("Execution returned: {}", result);

        Ok(())
    }

    fn translate_expr(
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
                let lhs = Self::translate_expr(builder, left, variables)?;
                let rhs = Self::translate_expr(builder, right, variables)?;
                match op {
                    BinaryOp::Add => Ok(builder.ins().iadd(lhs, rhs)),
                    BinaryOp::Sub => Ok(builder.ins().isub(lhs, rhs)),
                    BinaryOp::Mul => Ok(builder.ins().imul(lhs, rhs)),
                    BinaryOp::Div => Ok(builder.ins().sdiv(lhs, rhs)),
                    _ => Err(CodegenError { message: "Binary operator not supported yet".into() })
                }
            }
            _ => Err(CodegenError { message: format!("Expression type not supported yet: {:?}", expr) })
        }
    }
}
