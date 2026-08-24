use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Module, FuncId, DataId, Linkage, DataDescription};
use std::collections::HashMap;
use pace_ast::Stmt;
use miette::Diagnostic;
use thiserror::Error;
use crate::translator::VarType;

#[derive(Debug, Clone)]
pub struct StructLayout {
    pub name: String,
    pub fields: HashMap<String, (usize, VarType)>,
    pub size: usize,
}

#[derive(Debug, Clone)]
pub struct ClassLayout {
    pub name: String,
    pub fields: HashMap<String, (usize, VarType)>,
    pub methods: HashMap<String, usize>,
    pub vtable_id: DataId,
}

#[derive(Debug, Clone)]
pub struct InterfaceLayout {
    pub name: String,
    pub methods: HashMap<String, usize>,
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
    struct_layouts: HashMap<String, StructLayout>,
    interface_layouts: HashMap<String, InterfaceLayout>,
}

impl Default for JITCompiler {
    fn default() -> Self {
        Self::new()
    }
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
        builder.symbol("__pace_concat_strings", pace_runtime::__pace_concat_strings as *const u8);
        builder.symbol("__pace_int_to_string", pace_runtime::__pace_int_to_string as *const u8);
        builder.symbol("__pace_float_to_string", pace_runtime::__pace_float_to_string as *const u8);
        builder.symbol("__pace_bool_to_string", pace_runtime::__pace_bool_to_string as *const u8);
        builder.symbol("__pace_malloc", pace_runtime::__pace_malloc as *const u8);
        builder.symbol("__pace_retain", pace_runtime::__pace_retain as *const u8);
        builder.symbol("__pace_release", pace_runtime::__pace_release as *const u8);
        builder.symbol("__pace_free", pace_runtime::__pace_free as *const u8);
        builder.symbol("__pace_ptr_store", pace_runtime::__pace_ptr_store as *const u8);
        builder.symbol("__pace_ptr_load", pace_runtime::__pace_ptr_load as *const u8);
        builder.symbol("__pace_time", pace_runtime::__pace_time as *const u8);
        builder.symbol("__pace_get_year", pace_runtime::__pace_get_year as *const u8);
        
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

        let mut sig_retain = module.make_signature();
        sig_retain.params.push(AbiParam::new(ptr_ty));
        let retain_id = module.declare_function("__pace_retain", Linkage::Import, &sig_retain).unwrap();

        let mut sig_release = module.make_signature();
        sig_release.params.push(AbiParam::new(ptr_ty));
        let release_id = module.declare_function("__pace_release", Linkage::Import, &sig_release).unwrap();

        let mut sig_concat = module.make_signature();
        sig_concat.params.push(AbiParam::new(ptr_ty));
        sig_concat.params.push(AbiParam::new(ptr_ty));
        sig_concat.returns.push(AbiParam::new(ptr_ty));
        let concat_id = module.declare_function("__pace_concat_strings", Linkage::Import, &sig_concat).unwrap();

        let mut sig_int_to_string = module.make_signature();
        sig_int_to_string.params.push(AbiParam::new(types::I64));
        sig_int_to_string.returns.push(AbiParam::new(ptr_ty));
        let int_to_str_id = module.declare_function("__pace_int_to_string", Linkage::Import, &sig_int_to_string).unwrap();

        let mut sig_float_to_string = module.make_signature();
        sig_float_to_string.params.push(AbiParam::new(types::F64));
        sig_float_to_string.returns.push(AbiParam::new(ptr_ty));
        let float_to_str_id = module.declare_function("__pace_float_to_string", Linkage::Import, &sig_float_to_string).unwrap();

        let mut sig_bool_to_string = module.make_signature();
        sig_bool_to_string.params.push(AbiParam::new(types::I64));
        sig_bool_to_string.returns.push(AbiParam::new(ptr_ty));
        let bool_to_str_id = module.declare_function("__pace_bool_to_string", Linkage::Import, &sig_bool_to_string).unwrap();

        let mut sig_free = module.make_signature();
        sig_free.params.push(AbiParam::new(ptr_ty));
        sig_free.params.push(AbiParam::new(types::I64));
        let free_id = module.declare_function("__pace_free", Linkage::Import, &sig_free).unwrap();

        let mut sig_ptr_store = module.make_signature();
        sig_ptr_store.params.push(AbiParam::new(ptr_ty));
        sig_ptr_store.params.push(AbiParam::new(types::I64));
        sig_ptr_store.params.push(AbiParam::new(types::I64));
        let ptr_store_id = module.declare_function("__pace_ptr_store", Linkage::Import, &sig_ptr_store).unwrap();

        let mut sig_ptr_load = module.make_signature();
        sig_ptr_load.params.push(AbiParam::new(ptr_ty));
        sig_ptr_load.params.push(AbiParam::new(types::I64));
        sig_ptr_load.returns.push(AbiParam::new(types::I64));
        let ptr_load_id = module.declare_function("__pace_ptr_load", Linkage::Import, &sig_ptr_load).unwrap();

        let mut sig_time = module.make_signature();
        sig_time.params.push(AbiParam::new(types::I64));
        sig_time.returns.push(AbiParam::new(types::I64));
        let time_id = module.declare_function("__pace_time", Linkage::Import, &sig_time).unwrap();

        let mut sig_get_year = module.make_signature();
        sig_get_year.params.push(AbiParam::new(types::I64));
        sig_get_year.returns.push(AbiParam::new(types::I64));
        let get_year_id = module.declare_function("__pace_get_year", Linkage::Import, &sig_get_year).unwrap();

        let mut funcs = HashMap::new();
        funcs.insert("print_int".to_string(), print_int_id);
        funcs.insert("print_float".to_string(), print_float_id);
        funcs.insert("print_string".to_string(), print_string_id);
        funcs.insert("malloc".to_string(), malloc_id);
        funcs.insert("retain".to_string(), retain_id);
        funcs.insert("release".to_string(), release_id);
        funcs.insert("concat_strings".to_string(), concat_id);
        funcs.insert("int_to_string".to_string(), int_to_str_id);
        funcs.insert("float_to_string".to_string(), float_to_str_id);
        funcs.insert("bool_to_string".to_string(), bool_to_str_id);
        funcs.insert("free".to_string(), free_id);
        funcs.insert("ptr_store".to_string(), ptr_store_id);
        funcs.insert("ptr_load".to_string(), ptr_load_id);
        funcs.insert("time".to_string(), time_id);
        funcs.insert("get_year".to_string(), get_year_id);

        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
            funcs,
            class_layouts: HashMap::new(),
            struct_layouts: HashMap::new(),
            interface_layouts: HashMap::new(),
        }
    }

    fn register_interfaces(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            if let Stmt::InterfaceDecl { name: interface_name, methods, generic_params: _ } = stmt {
                let mut method_map = HashMap::new();
                let mut m_offset = 16; // 0: drop, 8: size
                
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name: method_name, params: _, return_type: _, .. } = method_stmt {
                        method_map.insert(method_name.clone(), m_offset);
                        m_offset += 8;
                    }
                }
                
                let layout = InterfaceLayout {
                    name: interface_name.clone(),
                    methods: method_map.clone(),
                };
                self.interface_layouts.insert(interface_name.clone(), layout);

                // Insert a dummy ClassLayout for the interface so translate_expr can find its methods by type_name
                let dummy_vtable_name = format!("__iface_vtable_{}", interface_name);
                let dummy_vtable_id = self.module.declare_data(&dummy_vtable_name, Linkage::Local, false, false).unwrap();
                let dummy_class_layout = ClassLayout {
                    name: interface_name.clone(),
                    fields: HashMap::new(),
                    methods: method_map,
                    vtable_id: dummy_vtable_id,
                };
                self.class_layouts.insert(interface_name.clone(), dummy_class_layout);
            }
        }
    }

    fn register_classes(&mut self, stmts: &[Stmt]) -> Result<(), CodegenError> {
        let _ptr_ty = self.module.target_config().pointer_type();
        
        for stmt in stmts {
            if let Stmt::ClassDecl { name: class_name, fields, methods, implements, generic_params: _ } = stmt {
                let mut field_map = HashMap::new();
                let mut offset = 16; // 8 bytes for ARC, 8 bytes for vtable pointer
                for field in fields {
                    if let Stmt::VarDecl { name: field_name, type_annotation, .. } = field {
                        let ty_str = type_annotation.as_ref().map(|t| t.name.as_str()).unwrap_or("Unknown");
                        let field_ty = crate::translator::parse_vartype(ty_str, Some(&class_name));
                        field_map.insert(field_name.clone(), (offset, field_ty));
                        offset += 8;
                    }
                }
                
                let mut method_map = HashMap::new();
                let mut m_offset = 16;
                let mut vtable_funcs: HashMap<String, cranelift_module::FuncId> = HashMap::new();
                
                // Seed methods from interface if implemented
                if let Some(iface_annotation) = implements
                    && let Some(iface_layout) = self.interface_layouts.get(&iface_annotation.name) {
                        for (m_name, m_off) in &iface_layout.methods {
                            method_map.insert(m_name.clone(), *m_off);
                            if *m_off >= m_offset {
                                m_offset = *m_off + 8;
                            }
                        }
                    }
                
                let ptr_ty = self.module.target_config().pointer_type();
                
                let drop_name = format!("__drop_{}", class_name);
                let mut drop_sig = self.module.make_signature();
                drop_sig.params.push(AbiParam::new(ptr_ty)); // obj ptr
                let drop_id = self.module.declare_function(&drop_name, Linkage::Local, &drop_sig)
                    .map_err(|e| CodegenError { message: e.to_string() })?;
                self.funcs.insert(drop_name.clone(), drop_id);
                
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name: method_name, params, return_type: _, .. } = method_stmt {
                        if !method_map.contains_key(method_name) && method_name != "init" {
                            method_map.insert(method_name.clone(), m_offset);
                            m_offset += 8;
                        }
                        

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
                        
                        if method_name != "init" {
                            vtable_funcs.insert(method_name.clone(), id);
                        }
                    }
                }
                
                let vtable_name = format!("__vtable_{}", class_name);
                let vtable_id = self.module.declare_data(&vtable_name, Linkage::Local, false, false)
                    .map_err(|e| CodegenError { message: e.to_string() })?;
                    
                let mut data_ctx = DataDescription::new();
                let size = (16 + fields.len() * 8) as u64;
                let mut vtable_bytes = vec![0u8; m_offset];
                vtable_bytes[8..16].copy_from_slice(&size.to_ne_bytes());
                data_ctx.define(vtable_bytes.into_boxed_slice());
                
                let drop_ref = self.module.declare_func_in_data(drop_id, &mut data_ctx);
                data_ctx.write_function_addr(0, drop_ref);
                
                for (m_name, func_id) in &vtable_funcs {
                    let byte_offset = *method_map.get(m_name).unwrap();
                    let func_ref = self.module.declare_func_in_data(*func_id, &mut data_ctx);
                    data_ctx.write_function_addr(byte_offset as u32, func_ref);
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
            } else if let Stmt::StructDecl { name: struct_name, fields, generic_params: _ } = stmt {
                let mut field_map = HashMap::new();
                let mut offset = 0; // Structs have no header (0 bytes for ARC/VTable)
                for field in fields {
                    if let Stmt::VarDecl { name: field_name, type_annotation, .. } = field {
                        let ty_str = type_annotation.as_ref().map(|t| t.name.as_str()).unwrap_or("Unknown");
                        let field_ty = crate::translator::parse_vartype(ty_str, Some(&struct_name));
                        field_map.insert(field_name.clone(), (offset, field_ty));
                        offset += 8; // All fields are currently 8 bytes (i64/f64/ptr)
                    }
                }
                
                let layout = StructLayout {
                    name: struct_name.clone(),
                    fields: field_map,
                    size: offset,
                };
                self.struct_layouts.insert(struct_name.clone(), layout);
            }
        }
        Ok(())
    }

    pub fn compile_and_run(&mut self, stmts: &[Stmt]) -> Result<(), CodegenError> {
        // Run Monomorphization Pass
        let mut mono = crate::monomorphize::MonomorphizationPass::new();
        mono.process(stmts);
        let final_stmts = &mono.final_stmts;

        self.register_interfaces(final_stmts);
        self.register_classes(final_stmts)?;

        // Pass 1: Declare all functions
        for stmt in final_stmts {
            if let Stmt::FuncDecl { name, params, return_type: _, .. } = stmt {
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

        let mut func_returns = HashMap::new();
        for stmt in final_stmts {
            if let Stmt::FuncDecl { name, return_type, .. } = stmt {
                let ret = return_type.as_ref().map(|t| t.name.as_str()).unwrap_or("Int");
                func_returns.insert(name.clone(), crate::translator::parse_vartype(ret, None));
            } else if let Stmt::ClassDecl { name: class_name, methods, .. } = stmt {
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name, params: _, return_type, .. } = method_stmt {
                        let ret = return_type.as_ref().map(|t| t.name.as_str()).unwrap_or("Int");
                        let full_name = format!("{}_{}", class_name, name);
                        func_returns.insert(full_name, crate::translator::parse_vartype(ret, Some(&class_name)));
                    }
                }
            } else if let Stmt::InterfaceDecl { name: interface_name, methods, generic_params: _ } = stmt {
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name, params: _, return_type, .. } = method_stmt {
                        let ret = return_type.as_ref().map(|t| t.name.as_str()).unwrap_or("Int");
                        let full_name = format!("{}_{}", interface_name, name);
                        func_returns.insert(full_name, crate::translator::parse_vartype(ret, Some(&interface_name)));
                    }
                }
            }
        }

        // Pass 2: Define all functions and class methods
        for stmt in final_stmts {
            if let Stmt::FuncDecl { name, params, body, return_type: _, .. } = stmt {
                let id = *self.funcs.get(name).unwrap();
                self.compile_function(name, params, body, id, &func_returns, None)?;
            } else if let Stmt::ClassDecl { name: class_name, methods, .. } = stmt {
                self.generate_drop_function(class_name)?;
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name, params, body, return_type: _, .. } = method_stmt {
                        let full_name = format!("{}_{}", class_name, name);
                        let id = *self.funcs.get(&full_name).unwrap();
                        
                        let mut new_params = vec![pace_ast::Param {
                            name: "self".to_string(),
                            type_annotation: pace_ast::TypeAnnotation {
                                name: class_name.clone(),
                                args: vec![],
                                is_nullable: false,
                            },
                        }];
                        new_params.extend(params.clone());
                        
                        self.compile_function(&full_name, &new_params, body, id, &func_returns, Some(&class_name))?;
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
        
        let mut func_returns = HashMap::new();
        for stmt in final_stmts {
            if let Stmt::FuncDecl { name, return_type, .. } = stmt {
                let ret = return_type.as_ref().map(|t| t.name.as_str()).unwrap_or("Int");
                func_returns.insert(name.clone(), crate::translator::parse_vartype(ret, None));
            }
        }

        for stmt in stmts {
            match stmt {
                Stmt::VarDecl { .. } | Stmt::Expr(_) | Stmt::If { .. } | Stmt::While { .. } | Stmt::Loop { .. } => {
                    let (val, _) = crate::translator::Translator::translate_stmt(&mut self.module, &self.funcs, &self.class_layouts, &self.struct_layouts, &mut builder, stmt, &mut variables, &mut var_index, &func_returns)?;
                    last_val = Some(val);
                }
                _ => {}
            }
        }

        // Call main if it exists
        if let Some(&main_id) = self.funcs.get("main") {
            let local_func = self.module.declare_func_in_func(main_id, builder.func);
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

    fn generate_drop_function(&mut self, class_name: &str) -> Result<(), CodegenError> {
        let layout = self.class_layouts.get(class_name).unwrap().clone();
        let drop_name = format!("__drop_{}", class_name);
        let func_id = *self.funcs.get(&drop_name).unwrap();
        
        self.ctx.func.signature.params.push(AbiParam::new(self.module.target_config().pointer_type()));
        
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);
        
        let obj_ptr = builder.block_params(entry_block)[0];
        
        for &(offset, ref ty) in layout.fields.values() {
            if matches!(ty, VarType::Object(_)) {
                let val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), obj_ptr, offset as i32);
                let release_id = *self.funcs.get("release").unwrap();
                let local_release = self.module.declare_func_in_func(release_id, builder.func);
                builder.ins().call(local_release, &[val]);
            }
        }
        
        builder.ins().return_(&[]);
        builder.finalize(self.module.target_config());
        
        self.module.define_function(func_id, &mut self.ctx)
            .map_err(|e| CodegenError { message: e.to_string() })?;
            
        self.module.clear_context(&mut self.ctx);
        Ok(())
    }

    fn compile_function(
        &mut self,
        _name: &str,
        params: &[pace_ast::Param],
        body: &[Stmt],
        func_id: FuncId,
        func_returns: &HashMap<String, crate::translator::VarType>,
        current_class: Option<&str>,
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
            
            let param_ty = crate::translator::parse_vartype(&param.type_annotation.name, current_class);
            variables.insert(param.name.clone(), (var, param_ty));
            var_index += 1;
        }

        let mut last_val = None;
        let mut terminated = false;
        for stmt in body {
            let (val, term) = crate::translator::Translator::translate_stmt(&mut self.module, &self.funcs, &self.class_layouts, &self.struct_layouts, &mut builder, stmt, &mut variables, &mut var_index, func_returns)?;
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
            .map_err(|e| {
                println!("Cranelift Verifier Error in function {}: {:?}", _name, e);
                CodegenError { message: e.to_string() }
            })?;
        
        self.module.clear_context(&mut self.ctx);
        Ok(())
    }
}
