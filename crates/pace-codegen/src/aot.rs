use cranelift::prelude::*;
use cranelift_object::{ObjectBuilder, ObjectModule};
use cranelift_module::{Module, Linkage, FuncId};
use pace_ast::{Stmt, Expr};
use std::collections::HashMap;
use crate::compiler::CodegenError;
use crate::translator::Translator;

pub struct AotCompiler {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    module: ObjectModule,
    funcs: HashMap<String, FuncId>,
}

impl AotCompiler {
    pub fn new() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "true").unwrap(); // Need PIC for AOT compilation
        
        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });
        
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .unwrap();
            
        let builder = ObjectBuilder::new(
            isa, 
            "pace_module",
            cranelift_module::default_libcall_names()
        ).unwrap();
        
        let module = ObjectModule::new(builder);

        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
            funcs: HashMap::new(),
        }
    }

    pub fn compile_to_object(mut self, stmts: &[Stmt]) -> Result<Vec<u8>, CodegenError> {
        // Pass 1: Declare all functions
        for stmt in stmts {
            if let Stmt::FuncDecl { name, params, .. } = stmt {
                let mut sig = self.module.make_signature();
                for _ in params {
                    sig.params.push(AbiParam::new(types::I64));
                }
                sig.returns.push(AbiParam::new(types::I64));
                
                // If the function is `main`, export it so C linker can find it!
                let linkage = if name == "main" { Linkage::Export } else { Linkage::Local };
                
                let id = self.module.declare_function(name, linkage, &sig)
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

        let product = self.module.finish();
        let bytes = product.emit().map_err(|e| CodegenError { message: e.to_string() })?;
        
        Ok(bytes)
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
            let (val, term) = Translator::translate_stmt(&mut self.module, &self.funcs, &mut builder, stmt, &mut variables, &mut var_index)?;
            last_val = Some(val);
            if term {
                terminated = true;
                break;
            }
        }

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
}
