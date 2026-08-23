use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Module, FuncId, DataId, Linkage, DataDescription};
use pace_ast::Stmt;
use std::collections::HashMap;
use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ClassLayout {
    pub name: String,
    pub fields: HashMap<String, usize>,
    pub methods: HashMap<String, usize>,
    pub vtable_id: DataId,
}

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
    class_layouts: HashMap<String, ClassLayout>,
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

        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        
        // Expose pace-runtime to the JIT explicitly
        builder.symbol("__pace_print_int", pace_runtime::__pace_print_int as *const u8);
        builder.symbol("__pace_print_float", pace_runtime::__pace_print_float as *const u8);
        builder.symbol("__pace_print_string", pace_runtime::__pace_print_string as *const u8);
        builder.symbol("__pace_malloc", pace_runtime::__pace_malloc as *const u8);
        
        let mut module = JITModule::new(builder);

        let mut sig_int = module.make_signature();
        sig_int.params.push(AbiParam::new(types::I64));
        let print_int_id = module.declare_function("__pace_print_int", Linkage::Import, &sig_int).unwrap();
        
        let mut sig_float = module.make_signature();
        sig_float.params.push(AbiParam::new(types::F64));
        let print_float_id = module.declare_function("__pace_print_float", Linkage::Import, &sig_float).unwrap();
        
        let ptr_ty = module.target_config().pointer_type();
        let mut sig_string = module.make_signature();
        sig_string.params.push(AbiParam::new(ptr_ty));
        let print_string_id = module.declare_function("__pace_print_string", Linkage::Import, &sig_string).unwrap();

        let mut sig_malloc = module.make_signature();
        sig_malloc.params.push(AbiParam::new(types::I64));
        sig_malloc.returns.push(AbiParam::new(ptr_ty));
        let malloc_id = module.declare_function("__pace_malloc", Linkage::Import, &sig_malloc).unwrap();

        let mut funcs = HashMap::new();
        funcs.insert("print_int".to_string(), print_int_id);
        funcs.insert("print_float".to_string(), print_float_id);
        funcs.insert("print_string".to_string(), print_string_id);
        funcs.insert("malloc".to_string(), malloc_id);

        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
            funcs,
            class_layouts: HashMap::new(),
        }
    }

    fn register_classes(&mut self, stmts: &[Stmt]) -> Result<(), CodegenError> {
        let ptr_ty = self.module.target_config().pointer_type();
        
        for stmt in stmts {
            if let Stmt::ClassDecl { name: class_name, fields, methods, .. } = stmt {
                let mut field_map = HashMap::new();
                let mut offset = 8; // Offset 0 is VTable ptr
                for field in fields {
                    if let Stmt::VarDecl { name: field_name, .. } = field {
                        field_map.insert(field_name.clone(), offset);
                        offset += 8;
                    }
                }
                
                let mut method_map = HashMap::new();
                let mut m_offset = 0;
                let mut vtable_funcs = Vec::new();
                
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name: method_name, params, .. } = method_stmt {
                        method_map.insert(method_name.clone(), m_offset);
                        m_offset += 8;
                        
                        let full_name = format!("{}_{}", class_name, method_name);
                        let mut sig = self.module.make_signature();
                        sig.params.push(AbiParam::new(ptr_ty)); // self
                        for _ in params {
                            sig.params.push(AbiParam::new(types::I64));
                        }
                        sig.returns.push(AbiParam::new(types::I64));
                        
                        let id = self.module.declare_function(&full_name, Linkage::Local, &sig)
                            .map_err(|e| CodegenError { message: e.to_string() })?;
                        self.funcs.insert(full_name.clone(), id);
                        vtable_funcs.push(id);
                    }
                }
                
                let vtable_name = format!("__vtable_{}", class_name);
                let vtable_id = self.module.declare_data(&vtable_name, Linkage::Local, false, false)
                    .map_err(|e| CodegenError { message: e.to_string() })?;
                    
                let mut data_ctx = DataDescription::new();
                data_ctx.define_zeroinit(m_offset);
                
                let mut current_offset = 0;
                for &func_id in &vtable_funcs {
                    let func_ref = self.module.declare_func_in_data(func_id, &mut data_ctx);
                    data_ctx.write_function_addr(current_offset as u32, func_ref);
                    current_offset += 8;
                }
                
                self.module.define_data(vtable_id, &data_ctx)
                    .map_err(|e| CodegenError { message: e.to_string() })?;
                    
                let layout = ClassLayout {
                    name: class_name.clone(),
                    fields: field_map,
                    methods: method_map,
                    vtable_id,
                };
                self.class_layouts.insert(class_name.clone(), layout);
            }
        }
        Ok(())
    }

    pub fn compile_and_run(&mut self, stmts: &[Stmt]) -> Result<(), CodegenError> {
        self.register_classes(stmts)?;

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

        // Pass 2: Define all functions and class methods
        for stmt in stmts {
            if let Stmt::FuncDecl { name, params, body, .. } = stmt {
                let id = *self.funcs.get(name).unwrap();
                self.compile_function(name, params, body, id)?;
            } else if let Stmt::ClassDecl { name: class_name, methods, .. } = stmt {
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name, params, body, .. } = method_stmt {
                        let full_name = format!("{}_{}", class_name, name);
                        let id = *self.funcs.get(&full_name).unwrap();
                        
                        // We need to inject 'self' into params for compilation, but `compile_function` expects `&[pace_ast::stmt::Param]`.
                        // For simplicity, we'll just let `compile_function` handle it, but wait, `compile_function` uses `params` directly.
                        // Let's create a new params vec.
                        let mut new_params = vec![pace_ast::stmt::Param {
                            name: "self".to_string(),
                            type_annotation: class_name.clone(),
                        }];
                        new_params.extend(params.clone());
                        
                        self.compile_function(&full_name, &new_params, body, id)?;
                    }
                }
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
                    let (val, _) = crate::translator::Translator::translate_stmt(&mut self.module, &self.funcs, &self.class_layouts, &mut builder, stmt, &mut variables, &mut var_index)?;
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
        builder.finalize(self.module.target_config());

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
        let _result = entry_func();

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
            let var = builder.declare_var(types::I64);
            builder.def_var(var, val);
            variables.insert(param.name.clone(), var);
            var_index += 1;
        }

        let mut last_val = None;
        let mut terminated = false;
        for stmt in body {
            let (val, term) = crate::translator::Translator::translate_stmt(&mut self.module, &self.funcs, &self.class_layouts, &mut builder, stmt, &mut variables, &mut var_index)?;
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
        
        builder.finalize(self.module.target_config());
        
        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| CodegenError { message: e.to_string() })?;
        
        self.module.clear_context(&mut self.ctx);
        Ok(())
    }
}
