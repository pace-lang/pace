use cranelift::prelude::*;
use cranelift_object::{ObjectBuilder, ObjectModule};
use cranelift_module::{Module, FuncId, DataId, Linkage, DataDescription};
use pace_ast::Stmt;
use miette::Diagnostic;
use thiserror::Error;
use std::collections::HashMap;
use crate::translator::VarType;
use crate::compiler::{CodegenError, ClassLayout, InterfaceLayout};
use crate::translator::Translator;

pub struct AotCompiler {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    module: ObjectModule,
    funcs: HashMap<String, FuncId>,
    class_layouts: HashMap<String, ClassLayout>,
    interface_layouts: HashMap<String, InterfaceLayout>,
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
        
        let mut module = ObjectModule::new(builder);

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

        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
            funcs,
            class_layouts: HashMap::new(),
            interface_layouts: HashMap::new(),
        }
    }

    fn register_interfaces(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            if let Stmt::InterfaceDecl { name: interface_name, methods } = stmt {
                let mut method_map = HashMap::new();
                let mut m_offset = 16; // 0: drop, 8: size
                
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name: method_name, .. } = method_stmt {
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
        let ptr_ty = self.module.target_config().pointer_type();
        
        for stmt in stmts {
            if let Stmt::ClassDecl { name: class_name, fields, methods, .. } = stmt {
                let mut field_map = HashMap::new();
                let mut offset = 16; // 8 bytes for ARC, 8 bytes for vtable pointer
                for field in fields {
                    if let Stmt::VarDecl { name: field_name, type_annotation, .. } = field {
                        let ty_str = type_annotation.as_deref().unwrap_or("Unknown");
                        let field_ty = crate::translator::parse_vartype(ty_str);
                        field_map.insert(field_name.clone(), (offset, field_ty));
                        offset += 8;
                    }
                }
                
                let mut method_map = HashMap::new();
                let mut m_offset = 16;
                let mut vtable_funcs = Vec::new();
                
                // Seed methods from interface if implemented
                if let Stmt::ClassDecl { implements: Some(iface_name), .. } = stmt {
                    if let Some(iface_layout) = self.interface_layouts.get(iface_name) {
                        for (m_name, m_off) in &iface_layout.methods {
                            method_map.insert(m_name.clone(), *m_off);
                            if *m_off >= m_offset {
                                m_offset = *m_off + 8;
                            }
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
                    if let Stmt::FuncDecl { name: method_name, params, .. } = method_stmt {
                        if !method_map.contains_key(method_name) {
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
                        vtable_funcs.push(id);
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
                
                let mut current_offset = 16;
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

    pub fn compile_to_object(mut self, stmts: &[Stmt]) -> Result<Vec<u8>, CodegenError> {
        self.register_interfaces(stmts);
        self.register_classes(stmts)?;

        // Pass 1: Declare all functions
        for stmt in stmts {
            if let Stmt::FuncDecl { name, params, .. } = stmt {
                let mut sig = self.module.make_signature();
                for _ in params {
                    sig.params.push(AbiParam::new(types::I64));
                }
                sig.returns.push(AbiParam::new(types::I64));
                
                // main must be exported, others can be local
                let linkage = if name == "main" { Linkage::Export } else { Linkage::Local };
                
                let id = self.module.declare_function(name, linkage, &sig)
                    .map_err(|e| CodegenError { message: e.to_string() })?;
                self.funcs.insert(name.clone(), id);
            }
        }

        let mut func_returns = HashMap::new();
        for stmt in stmts {
            if let Stmt::FuncDecl { name, return_type, .. } = stmt {
                let ret = return_type.as_deref().unwrap_or("Int");
                func_returns.insert(name.clone(), crate::translator::parse_vartype(ret));
            } else if let Stmt::ClassDecl { name: class_name, methods, .. } = stmt {
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name, return_type, .. } = method_stmt {
                        let ret = return_type.as_deref().unwrap_or("Int");
                        let full_name = format!("{}_{}", class_name, name);
                        func_returns.insert(full_name, crate::translator::parse_vartype(ret));
                    }
                }
            } else if let Stmt::InterfaceDecl { name: interface_name, methods } = stmt {
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name, return_type, .. } = method_stmt {
                        let ret = return_type.as_deref().unwrap_or("Int");
                        let full_name = format!("{}_{}", interface_name, name);
                        func_returns.insert(full_name, crate::translator::parse_vartype(ret));
                    }
                }
            }
        }

        // Pass 2: Define all functions and class methods
        for stmt in stmts {
            if let Stmt::FuncDecl { name, params, body, .. } = stmt {
                let id = *self.funcs.get(name).unwrap();
                self.compile_function(name, params, body, id, &func_returns)?;
            } else if let Stmt::ClassDecl { name: class_name, methods, .. } = stmt {
                self.generate_drop_function(class_name)?;
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name, params, body, .. } = method_stmt {
                        let full_name = format!("{}_{}", class_name, name);
                        let id = *self.funcs.get(&full_name).unwrap();
                        
                        let mut new_params = vec![pace_ast::Param {
                            name: "self".to_string(),
                            type_annotation: class_name.clone(),
                        }];
                        new_params.extend(params.clone());
                        
                        self.compile_function(&full_name, &new_params, body, id, &func_returns)?;
                    }
                }
            }
        }

        let product = self.module.finish();
        let bytes = product.emit().map_err(|e| CodegenError { message: e.to_string() })?;
        
        Ok(bytes)
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
                let local_release = self.module.declare_func_in_func(release_id, &mut builder.func);
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
            let var = builder.declare_var(types::I64);
            builder.def_var(var, val);
            
            let param_ty = crate::translator::parse_vartype(&param.type_annotation);
            variables.insert(param.name.clone(), (var, param_ty));
            var_index += 1;
        }

        let mut last_val = None;
        let mut terminated = false;
        for stmt in body {
            let (val, term) = Translator::translate_stmt(&mut self.module, &self.funcs, &self.class_layouts, &mut builder, stmt, &mut variables, &mut var_index, func_returns)?;
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
        
        builder.finalize(self.module.target_config());
        
        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| CodegenError { message: e.to_string() })?;
        
        self.module.clear_context(&mut self.ctx);
        Ok(())
    }
}
