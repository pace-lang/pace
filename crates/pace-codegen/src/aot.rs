use cranelift::prelude::*;
use cranelift_object::{ObjectBuilder, ObjectModule};
use cranelift_module::{Module, FuncId, Linkage, DataDescription};
use pace_ast::Stmt;
use std::collections::HashMap;
use crate::translator::VarType;
use crate::compiler::{CodegenError, ClassLayout, InterfaceLayout, StructLayout, EnumLayout};
use crate::translator::Translator;

pub struct AotCompiler {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    module: ObjectModule,
    funcs: HashMap<String, FuncId>,
    class_layouts: HashMap<String, ClassLayout>,
    struct_layouts: HashMap<String, StructLayout>,
    interface_layouts: HashMap<String, InterfaceLayout>,
    enum_layouts: HashMap<String, EnumLayout>,
    string_cache: HashMap<String, String>,
    string_id: usize,
}

impl Default for AotCompiler {
    fn default() -> Self {
        Self::new("none".to_string())
    }
}

impl AotCompiler {
    pub fn new(opt_level: String) -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("opt_level", &opt_level).unwrap();
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

        let ptr_ty = module.target_config().pointer_type();
        let funcs = crate::runtime::declare_runtime_functions(&mut module, ptr_ty);


        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
            funcs,
            class_layouts: HashMap::new(),
            struct_layouts: HashMap::new(),
            interface_layouts: HashMap::new(),
            enum_layouts: HashMap::new(),
            string_cache: HashMap::new(),
            string_id: 0,
        }
    }

    fn register_interfaces(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            if let Stmt::InterfaceDecl { name: interface_name, methods, generic_params: _, .. } = stmt {
                let mut method_map = HashMap::new();
                let mut m_offset = 16; // 0: drop, 8: size
                
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name: method_name, .. } = method_stmt {
                        if method_name != "init" {
                            method_map.insert(method_name.clone(), m_offset);
                            m_offset += 8;
                        }
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
                
                let mut data_ctx = DataDescription::new();
                let vtable_bytes = vec![0u8; 16];
                data_ctx.define(vtable_bytes.into_boxed_slice());
                self.module.define_data(dummy_vtable_id, &data_ctx).unwrap();
                
                let dummy_class_layout = ClassLayout {
                    name: interface_name.clone(),
                    fields: HashMap::new(),
                    static_fields: HashMap::new(),
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
            if let Stmt::ClassDecl { name: class_name, fields, methods, implements, generic_params: _, .. } | Stmt::ActorDecl { name: class_name, fields, methods, implements, generic_params: _, .. } = stmt {
                let is_actor = matches!(stmt, Stmt::ActorDecl { .. });
                let mut field_map = HashMap::new();
                let mut offset = 16; // 8 bytes for ARC, 8 bytes for vtable pointer
                
                if is_actor {
                    field_map.insert("__mailbox".to_string(), (offset, crate::translator::VarType::Unknown)); // Internal pointer
                    offset += 8;
                }
                
                let mut static_fields = HashMap::new();
                for field in fields {
                    if let Stmt::VarDecl { name: field_name, type_annotation, is_static, initializer, .. } = field {
                        let ty_str = type_annotation.as_ref().map(|t| t.name.as_str()).unwrap_or("Unknown");
                        let field_ty = crate::translator::parse_vartype(ty_str, Some(class_name), Some(&self.struct_layouts), Some(&self.enum_layouts));
                        if *is_static {
                            let global_name = format!("{}_{}", class_name, field_name);
                            let data_id = self.module.declare_data(&global_name, Linkage::Export, true, false)
                                .expect("Failed to declare static field");
                            let mut data_ctx = DataDescription::new();
                            
                            let mut init_bytes = vec![0u8; 8];
                            if let Some(init_expr) = initializer {
                                use pace_ast::Expr;
                                match init_expr {
                                    Expr::IntLiteral(i) => {
                                        init_bytes.copy_from_slice(&i.to_ne_bytes());
                                    }
                                    Expr::FloatLiteral(f) => {
                                        init_bytes.copy_from_slice(&f.to_bits().to_ne_bytes());
                                    }
                                    Expr::BoolLiteral(b) => {
                                        let val: i64 = if *b { 1 } else { 0 };
                                        init_bytes.copy_from_slice(&val.to_ne_bytes());
                                    }
                                    _ => {} // Default to 0 for complex expressions
                                }
                            }
                            
                            data_ctx.define(init_bytes.into_boxed_slice());
                            self.module.define_data(data_id, &data_ctx)
                                .expect("Failed to define static field data");
                            static_fields.insert(field_name.clone(), (data_id, field_ty));
                        } else {
                            field_map.insert(field_name.clone(), (offset, field_ty));
                            offset += 8;
                        }
                    }
                }
                
                let mut method_map = HashMap::new();
                let mut m_offset = 16;
                let mut vtable_funcs = HashMap::new();
                
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
                    if let Stmt::FuncDecl { name: method_name, params, is_static, .. } = method_stmt {
                        if !is_static && !method_map.contains_key(method_name) && method_name != "init" {
                            method_map.insert(method_name.clone(), m_offset);
                            m_offset += 8;
                        }
                        
                        let full_name = format!("{}_{}", class_name, method_name);
                        let mut sig = self.module.make_signature();
                        if !is_static {
                            sig.params.push(AbiParam::new(ptr_ty)); // self
                        }
                        for _ in params {
                            sig.params.push(AbiParam::new(types::I64));
                        }
                        sig.returns.push(AbiParam::new(types::I64));
                        
                        let id = self.module.declare_function(&full_name, Linkage::Local, &sig)
                            .map_err(|e| CodegenError { message: e.to_string() })?;
                        self.funcs.insert(full_name.clone(), id);
                        
                        if !is_static && method_name != "init" {
                            if is_actor {
                                let async_name = format!("__async_{}_{}", class_name, method_name);
                                let mut async_sig = self.module.make_signature();
                                async_sig.params.push(AbiParam::new(types::I64));
                                async_sig.returns.push(AbiParam::new(types::I64));
                                let async_id = self.module.declare_function(&async_name, Linkage::Local, &async_sig)
                                    .map_err(|e| CodegenError { message: e.to_string() })?;
                                self.funcs.insert(async_name.clone(), async_id);
                                vtable_funcs.insert(method_name.clone(), async_id);
                            } else {
                                vtable_funcs.insert(method_name.clone(), id);
                            }
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
                    static_fields,
                    methods: method_map,
                    vtable_id,
                };
                self.class_layouts.insert(class_name.clone(), layout);
            } else if let Stmt::EnumDecl { name: enum_name, variants, generic_params: _, .. } = stmt {
                let mut max_size = 16; // 8 for ARC, 8 for Tag
                let mut variant_map = HashMap::new();
                
                let ptr_ty = self.module.target_config().pointer_type();
                
                let drop_name = format!("__drop_{}", enum_name);
                let mut drop_sig = self.module.make_signature();
                drop_sig.params.push(AbiParam::new(ptr_ty)); // obj ptr
                let drop_id = self.module.declare_function(&drop_name, Linkage::Local, &drop_sig)
                    .map_err(|e| CodegenError { message: e.to_string() })?;
                self.funcs.insert(drop_name.clone(), drop_id);
                
                for (tag_idx, variant) in variants.iter().enumerate() {
                    let mut variant_types = Vec::new();
                    let mut variant_size = 16;
                    
                    if let Some(fields) = &variant.fields {
                        for field_ty in fields {
                            let field_var_type = crate::translator::parse_vartype(&field_ty.name, Some(enum_name), Some(&self.struct_layouts), Some(&self.enum_layouts));
                            variant_types.push(field_var_type);
                            variant_size += 8;
                        }
                    }
                    
                    if variant_size > max_size {
                        max_size = variant_size;
                    }
                    
                    variant_map.insert(variant.name.clone(), (tag_idx as u64, variant_types.clone()));
                    
                    let constructor_name = format!("{}_{}", enum_name, variant.name);
                    let mut sig = self.module.make_signature();
                    for _ in 0..variant_types.len() {
                        sig.params.push(AbiParam::new(types::I64));
                    }
                    sig.returns.push(AbiParam::new(ptr_ty));
                    
                    let constructor_id = self.module.declare_function(&constructor_name, Linkage::Local, &sig)
                        .map_err(|e| CodegenError { message: e.to_string() })?;
                    self.funcs.insert(constructor_name, constructor_id);
                }
                
                let layout = EnumLayout {
                    name: enum_name.clone(),
                    max_size,
                    variants: variant_map,
                    drop_func_id: drop_id,
                };
                self.enum_layouts.insert(enum_name.clone(), layout);
                
            } else if let Stmt::StructDecl { name: struct_name, fields, generic_params: _, .. } = stmt {
                let mut field_map = HashMap::new();
                let mut offset = 0; // Structs have no header
                for field in fields {
                    if let Stmt::VarDecl { name: field_name, type_annotation, .. } = field {
                        let ty_str = type_annotation.as_ref().map(|t| t.name.as_str()).unwrap_or("Unknown");
                        let field_ty = crate::translator::parse_vartype(ty_str, Some(struct_name), Some(&self.struct_layouts), Some(&self.enum_layouts));
                        field_map.insert(field_name.clone(), (offset, field_ty));
                        offset += 8;
                    }
                }
                
                let layout = StructLayout {
                    name: struct_name.clone(),
                    fields: field_map,
                    static_fields: HashMap::new(),
                    size: offset,
                };
                self.struct_layouts.insert(struct_name.clone(), layout);
            }
        }
        Ok(())
    }

    pub fn compile_to_object(mut self, stmts: &[Stmt]) -> Result<Vec<u8>, CodegenError> {
        // Run Monomorphization Pass
        let mut mono = crate::monomorphize::MonomorphizationPass::new();
        mono.process(stmts);
        let final_stmts = &mono.final_stmts;

        // Register layouts for classes and interfaces
        self.register_interfaces(final_stmts);
        self.register_classes(final_stmts)?;

        // Pass 1: Declare all functions
        for stmt in final_stmts {
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
        for stmt in final_stmts {
            if let Stmt::FuncDecl { name, return_type, .. } = stmt {
                let ret = return_type.as_ref().map(|t| t.name.as_str()).unwrap_or("Int");
                func_returns.insert(name.clone(), crate::translator::parse_vartype(ret, None, Some(&self.struct_layouts), Some(&self.enum_layouts)));
            } else if let Stmt::ClassDecl { name: class_name, methods, .. } | Stmt::ActorDecl { name: class_name, methods, .. } = stmt {
                for method in methods {
                    if let Stmt::FuncDecl { name: method_name, params: _, return_type, .. } = method {
                        let ret = return_type.as_ref().map(|t| t.name.as_str()).unwrap_or("Int");
                        let full_name = format!("{}_{}", class_name, method_name);
                        func_returns.insert(full_name, crate::translator::parse_vartype(ret, Some(class_name), Some(&self.struct_layouts), Some(&self.enum_layouts)));
                    }
                }
            } else if let Stmt::InterfaceDecl { name: interface_name, methods, generic_params: _, .. } = stmt {
                for method in methods {
                    if let Stmt::FuncDecl { name: method_name, params: _, return_type, .. } = method {
                        let ret = return_type.as_ref().map(|t| t.name.as_str()).unwrap_or("Int");
                        let full_name = format!("{}_{}", interface_name, method_name);
                        func_returns.insert(full_name, crate::translator::parse_vartype(ret, Some(interface_name), Some(&self.struct_layouts), Some(&self.enum_layouts)));
                    }
                }
            }
        }

                // Pass 2: Define all functions and class methods
        for stmt in final_stmts {
            if let Stmt::FuncDecl { name, params, body, return_type, .. } = stmt {
                let id = *self.funcs.get(name).unwrap();
                let ret = return_type.as_ref().map(|t| t.name.as_str()).unwrap_or("Int");
                self.compile_function(name, params, body, id, &func_returns, ret, None)?;
            } else if let Stmt::ClassDecl { name: class_name, methods, .. } | Stmt::ActorDecl { name: class_name, methods, .. } = stmt {
                let is_actor = matches!(stmt, Stmt::ActorDecl { .. });
                self.generate_drop_function(class_name)?;
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name: method_name, params, body, return_type, is_static, .. } = method_stmt {
                        let full_name = format!("{}_{}", class_name, method_name);
                        let id = *self.funcs.get(&full_name).unwrap();
                        
                        let mut all_params = vec![];
                        if !is_static {
                            all_params.push(pace_ast::Param {
                                name: "self".to_string(),
                                type_annotation: pace_ast::TypeAnnotation {
                                    module_prefix: None,
                                    name: class_name.clone(),
                                    args: vec![],
                                    is_nullable: false,
                is_function: false,
                function_params: None,
                function_return: None
            },
                            });
                        }
                        all_params.extend(params.clone());
                        let ret = return_type.as_ref().map(|t| t.name.as_str()).unwrap_or("Int");
                        
                        self.compile_function(&full_name, &all_params, body, id, &func_returns, ret, Some(class_name))?;
                        
                        if !is_static && method_name != "init" && is_actor {
                            self.generate_async_wrapper(class_name, method_name, params.len())?;
                        }
                    }
                }
            } else if let Stmt::EnumDecl { name: enum_name, variants, .. } = stmt {
                self.generate_enum_drop_function(enum_name)?;
                self.generate_enum_constructors(enum_name, variants)?;
            }
        }

        let product = self.module.finish();
        let bytes = product.emit().map_err(|e| CodegenError { message: e.to_string() })?;
        
        Ok(bytes)
    }

    fn generate_async_wrapper(&mut self, class_name: &str, method_name: &str, num_args: usize) -> Result<(), CodegenError> {
        let async_name = format!("__async_{}_{}", class_name, method_name);
        let id = *self.funcs.get(&async_name).unwrap();
        let target_id = *self.funcs.get(&format!("{}_{}", class_name, method_name)).unwrap();
        
        self.ctx.func.signature.params.push(AbiParam::new(types::I64)); // arg_ptr
        self.ctx.func.signature.returns.push(AbiParam::new(types::I64));
        
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);
        
        let arg_ptr = builder.block_params(entry_block)[0];
        
        let mut call_args = Vec::new();
        for i in 0..=num_args { // self + args
            let val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), arg_ptr, (i * 8) as i32);
            call_args.push(val);
        }
        
        // Call the real method
        let local_target = self.module.declare_func_in_func(target_id, builder.func);
        let call = builder.ins().call(local_target, &call_args);
        
        let results = builder.inst_results(call);
        let ret_val = if results.is_empty() {
            builder.ins().iconst(types::I64, 0)
        } else {
            results[0]
        };
        
        // Free the tuple allocated by the caller
        let free_id = *self.funcs.get("free").unwrap();
        let local_free = self.module.declare_func_in_func(free_id, builder.func);
        let size_val = builder.ins().iconst(types::I64, ((num_args + 1) * 8) as i64);
        builder.ins().call(local_free, &[arg_ptr, size_val]);
        
        builder.ins().return_(&[ret_val]);
        builder.finalize(self.module.target_config());
        
        self.module.define_function(id, &mut self.ctx).map_err(|e| CodegenError { message: e.to_string() })?;
        self.module.clear_context(&mut self.ctx);
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
        
        for (field_name, &(offset, ref ty)) in &layout.fields {
            if field_name == "__mailbox" {
                let val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), obj_ptr, offset as i32);
                let destroy_id = *self.funcs.get("__pace_mailbox_destroy").unwrap();
                let local_destroy = self.module.declare_func_in_func(destroy_id, builder.func);
                builder.ins().call(local_destroy, &[val]);
            } else if matches!(ty, VarType::Object(_)) {
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

    fn generate_enum_drop_function(&mut self, enum_name: &str) -> Result<(), CodegenError> {
        let layout = self.enum_layouts.get(enum_name).unwrap().clone();
        let func_id = layout.drop_func_id;
        
        self.ctx.func.signature.params.push(AbiParam::new(self.module.target_config().pointer_type()));
        
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        
        let obj_ptr = builder.block_params(entry_block)[0];
        
        let tag_val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), obj_ptr, 8);
        
        let mut blocks = Vec::new();
        for _ in 0..layout.variants.len() {
            blocks.push(builder.create_block());
        }
        let end_block = builder.create_block();
        
        for (tag_id, _) in layout.variants.values() {
            let next_check = builder.create_block();
            let expected_tag = builder.ins().iconst(types::I64, *tag_id as i64);
            let is_match = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::Equal, tag_val, expected_tag);
            builder.ins().brif(is_match, blocks[*tag_id as usize], &[], next_check, &[]);
            
            builder.seal_block(next_check);
            builder.switch_to_block(next_check);
        }
        builder.ins().jump(end_block, &[]); // Fallback
        
        for (tag_id, fields) in layout.variants.values() {
            let block = blocks[*tag_id as usize];
            builder.seal_block(block);
            builder.switch_to_block(block);
            
            let mut offset = 16;
            for ty in fields {
                if matches!(ty, VarType::Object(_)) {
                    let val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), obj_ptr, offset);
                    let release_id = *self.funcs.get("release").unwrap();
                    let local_release = self.module.declare_func_in_func(release_id, builder.func);
                    builder.ins().call(local_release, &[val]);
                }
                offset += 8;
            }
            
            builder.ins().jump(end_block, &[]);
        }
        
        builder.seal_block(end_block);
        builder.switch_to_block(end_block);
        builder.ins().return_(&[]);
        
        builder.seal_block(entry_block);
        
        builder.finalize(self.module.target_config());
        
        self.module.define_function(func_id, &mut self.ctx)
            .map_err(|e| CodegenError { message: e.to_string() })?;
            
        self.module.clear_context(&mut self.ctx);
        Ok(())
    }

    fn generate_enum_constructors(&mut self, enum_name: &str, variants: &[pace_ast::EnumVariant]) -> Result<(), CodegenError> {
        let layout = self.enum_layouts.get(enum_name).unwrap().clone();
        
        for variant in variants {
            let constructor_name = format!("{}_{}", enum_name, variant.name);
            let func_id = *self.funcs.get(&constructor_name).unwrap();
            let (tag_id, fields) = layout.variants.get(&variant.name).unwrap();
            
            for _ in 0..fields.len() {
                self.ctx.func.signature.params.push(AbiParam::new(types::I64));
            }
            self.ctx.func.signature.returns.push(AbiParam::new(types::I64));
            
            let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);
            
            let malloc_id = *self.funcs.get("malloc").unwrap();
            let local_malloc = self.module.declare_func_in_func(malloc_id, builder.func);
            let size_val = builder.ins().iconst(types::I64, layout.max_size as i64);
            let call = builder.ins().call(local_malloc, &[size_val]);
            let obj_ptr = builder.inst_results(call)[0];
            
            let ref_count = builder.ins().iconst(types::I64, 1);
            builder.ins().store(cranelift::prelude::MemFlagsData::new(), ref_count, obj_ptr, 0);
            
            let tag_val = builder.ins().iconst(types::I64, *tag_id as i64);
            builder.ins().store(cranelift::prelude::MemFlagsData::new(), tag_val, obj_ptr, 8);
            
            let mut offset = 16;
            for (i, field_ty) in fields.iter().enumerate() {
                let arg_val = builder.block_params(entry_block)[i];
                builder.ins().store(cranelift::prelude::MemFlagsData::new(), arg_val, obj_ptr, offset);
                
                if matches!(field_ty, VarType::Object(_)) {
                    let retain_id = *self.funcs.get("retain").unwrap();
                    let local_retain = self.module.declare_func_in_func(retain_id, builder.func);
                    builder.ins().call(local_retain, &[arg_val]);
                }
                
                offset += 8;
            }
            
            builder.ins().return_(&[obj_ptr]);
            builder.finalize(self.module.target_config());
            
            self.module.define_function(func_id, &mut self.ctx)
                .map_err(|e| CodegenError { message: e.to_string() })?;
                
            self.module.clear_context(&mut self.ctx);
        }
        Ok(())
    }

    fn compile_function(
        &mut self,
        _name: &str,
        params: &[pace_ast::Param],
        body: &[Stmt],
        func_id: FuncId,
        func_returns: &HashMap<String, crate::translator::VarType>,
        ret_type_str: &str,
        current_class: Option<&str>,
    ) -> Result<(), CodegenError> {
        let ret_ty = crate::translator::parse_vartype(ret_type_str, current_class, Some(&self.struct_layouts), Some(&self.enum_layouts));
        self.ctx.func.signature.returns.push(AbiParam::new(ret_ty.to_cranelift_type()));
        for param in params {
            let ty = crate::translator::parse_type_annotation(&param.type_annotation, current_class, Some(&self.struct_layouts), Some(&self.enum_layouts));
            self.ctx.func.signature.params.push(AbiParam::new(ty.to_cranelift_type()));
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
            
            let param_ty = crate::translator::parse_type_annotation(&param.type_annotation, current_class, Some(&self.struct_layouts), Some(&self.enum_layouts));
            variables.insert(param.name.clone(), (var, param_ty));
            var_index += 1;
        }

        let mut last_val = None;
        let mut terminated = false;
        let mut pending_closures = Vec::new();
        for stmt in body {
            let (val, term) = Translator::translate_stmt(&mut self.module, &self.funcs, &self.class_layouts, &self.struct_layouts, &self.enum_layouts, &mut builder, stmt, &mut variables, &mut var_index, func_returns, &mut self.string_cache, &mut self.string_id, &mut pending_closures)?;
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
            .map_err(|e| {
                println!("Cranelift Verifier Error in function {}: {:?}", _name, e);
                CodegenError { message: e.to_string() }
            })?;
        
        self.module.clear_context(&mut self.ctx);
        
        for (fn_name, expr, captured_vars) in pending_closures {
            self.compile_closure(&fn_name, expr, captured_vars, func_returns, current_class)?;
        }
        
        Ok(())
    }
    
    fn compile_closure(
        &mut self,
        fn_name: &str,
        expr: pace_ast::Expr,
        captured_vars: Vec<(String, crate::translator::VarType)>,
        func_returns: &HashMap<String, crate::translator::VarType>,
        current_class: Option<&str>,
    ) -> Result<(), CodegenError> {
        let (params, body) = match expr {
            pace_ast::Expr::Closure { params, body, .. } => (params, body),
            _ => return Err(CodegenError { message: "Invalid closure expression".to_string() }),
        };
        
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(self.module.target_config().pointer_type())); // env pointer
        for _ in &params {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I64)); // Assume returning I64 for now
        
        let func_id = self.module.declare_function(fn_name, Linkage::Export, &sig).unwrap();
        
        self.ctx.func.signature = sig;
        
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);
        
        let env_ptr = builder.block_params(entry_block)[0];
        
        let mut variables = HashMap::new();
        let mut var_index = 0;
        
        // Load captured variables from environment
        for (i, (name, ty)) in captured_vars.iter().enumerate() {
            let offset = 16 + (i * 8);
            let val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), env_ptr, offset as i32);
            let var = builder.declare_var(types::I64);
            builder.def_var(var, val);
            variables.insert(name.clone(), (var, ty.clone()));
            var_index += 1;
        }
        
        // Declare closure parameters as variables
        for (i, param) in params.iter().enumerate() {
            let val = builder.block_params(entry_block)[i + 1]; // +1 because env_ptr is at 0
            let var = builder.declare_var(types::I64);
            builder.def_var(var, val);
            let param_ty = crate::translator::parse_type_annotation(&param.1, current_class, Some(&self.struct_layouts), Some(&self.enum_layouts));
            variables.insert(param.0.clone(), (var, param_ty));
            var_index += 1;
        }
        
        let mut terminated = false;
        let mut pending_closures = Vec::new();
        
        let body_stmt = pace_ast::Stmt::Expr(*body);
        let (val, term) = crate::translator::Translator::translate_stmt(&mut self.module, &self.funcs, &self.class_layouts, &self.struct_layouts, &self.enum_layouts, &mut builder, &body_stmt, &mut variables, &mut var_index, func_returns, &mut self.string_cache, &mut self.string_id, &mut pending_closures)?;
        let last_val = Some(val);
        if term {
            terminated = true;
        }
        
        if !terminated {
            let ret = last_val.unwrap_or_else(|| builder.ins().iconst(types::I64, 0));
            builder.ins().return_(&[ret]);
        }
        
        builder.finalize(self.module.target_config());
        self.module.define_function(func_id, &mut self.ctx)
            .map_err(|e| CodegenError { message: e.to_string() })?;
        self.module.clear_context(&mut self.ctx);
        
        // Recursively compile any nested closures
        for (nested_fn, nested_expr, nested_captured) in pending_closures {
            self.compile_closure(&nested_fn, nested_expr, nested_captured, func_returns, current_class)?;
        }
        
        Ok(())
    }
}
