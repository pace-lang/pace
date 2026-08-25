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
}

impl Default for AotCompiler {
    fn default() -> Self {
        Self::new()
    }
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

        let mut sig_hash = module.make_signature();
        sig_hash.params.push(AbiParam::new(types::I64));
        sig_hash.returns.push(AbiParam::new(types::I64));
        let hash_id = module.declare_function("__pace_hash", Linkage::Import, &sig_hash).unwrap();

        let mut sig_sb_new = module.make_signature();
        sig_sb_new.returns.push(AbiParam::new(ptr_ty));
        let sb_new_id = module.declare_function("__pace_sb_new", Linkage::Import, &sig_sb_new).unwrap();

        let mut sig_sb_append = module.make_signature();
        sig_sb_append.params.push(AbiParam::new(ptr_ty));
        sig_sb_append.params.push(AbiParam::new(ptr_ty));
        let sb_append_id = module.declare_function("__pace_sb_append", Linkage::Import, &sig_sb_append).unwrap();

        let mut sig_sb_build = module.make_signature();
        sig_sb_build.params.push(AbiParam::new(ptr_ty));
        sig_sb_build.returns.push(AbiParam::new(ptr_ty));
        let sb_build_id = module.declare_function("__pace_sb_build", Linkage::Import, &sig_sb_build).unwrap();

        let mut sig_sb_free = module.make_signature();
        sig_sb_free.params.push(AbiParam::new(ptr_ty));
        let sb_free_id = module.declare_function("__pace_sb_free", Linkage::Import, &sig_sb_free).unwrap();

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
        funcs.insert("hash".to_string(), hash_id);
        funcs.insert("sb_new".to_string(), sb_new_id);
        funcs.insert("sb_append".to_string(), sb_append_id);
        funcs.insert("sb_build".to_string(), sb_build_id);
        funcs.insert("sb_free".to_string(), sb_free_id);

        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
            funcs,
            class_layouts: HashMap::new(),
            struct_layouts: HashMap::new(),
            interface_layouts: HashMap::new(),
            enum_layouts: HashMap::new(),
        }
    }

    fn register_interfaces(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            if let Stmt::InterfaceDecl { name: interface_name, methods, generic_params: _ } = stmt {
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
                        let field_ty = crate::translator::parse_vartype(ty_str, Some(class_name));
                        field_map.insert(field_name.clone(), (offset, field_ty));
                        offset += 8;
                    }
                }
                
                let mut method_map = HashMap::new();
                let mut m_offset = 16;
                
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
                
                let mut vtable_funcs = std::collections::HashMap::new();
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name: method_name, params, .. } = method_stmt {
                        if method_name != "init" {
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
            } else if let Stmt::EnumDecl { name: enum_name, variants, generic_params: _ } = stmt {
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
                            let field_var_type = crate::translator::parse_vartype(&field_ty.name, Some(enum_name));
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
                
            } else if let Stmt::StructDecl { name: struct_name, fields, generic_params: _ } = stmt {
                let mut field_map = HashMap::new();
                let mut offset = 0; // Structs have no header
                for field in fields {
                    if let Stmt::VarDecl { name: field_name, type_annotation, .. } = field {
                        let ty_str = type_annotation.as_ref().map(|t| t.name.as_str()).unwrap_or("Unknown");
                        let field_ty = crate::translator::parse_vartype(ty_str, Some(struct_name));
                        field_map.insert(field_name.clone(), (offset, field_ty));
                        offset += 8;
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
                func_returns.insert(name.clone(), crate::translator::parse_vartype(ret, None));
            } else if let Stmt::ClassDecl { name: class_name, methods, .. } = stmt {
                for method in methods {
                    if let Stmt::FuncDecl { name: method_name, params: _, return_type, .. } = method {
                        let ret = return_type.as_ref().map(|t| t.name.as_str()).unwrap_or("Int");
                        let full_name = format!("{}_{}", class_name, method_name);
                        func_returns.insert(full_name, crate::translator::parse_vartype(ret, Some(class_name)));
                    }
                }
            } else if let Stmt::InterfaceDecl { name: interface_name, methods, generic_params: _ } = stmt {
                for method in methods {
                    if let Stmt::FuncDecl { name: method_name, params: _, return_type, .. } = method {
                        let ret = return_type.as_ref().map(|t| t.name.as_str()).unwrap_or("Int");
                        let full_name = format!("{}_{}", interface_name, method_name);
                        func_returns.insert(full_name, crate::translator::parse_vartype(ret, Some(interface_name)));
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
            } else if let Stmt::ClassDecl { name: class_name, methods, .. } = stmt {
                self.generate_drop_function(class_name)?;
                for method_stmt in methods {
                    if let Stmt::FuncDecl { name: method_name, params, body, return_type, .. } = method_stmt {
                        let full_name = format!("{}_{}", class_name, method_name);
                        let id = *self.funcs.get(&full_name).unwrap();
                        
                        let self_param = pace_ast::Param {
                            name: "self".to_string(),
                            type_annotation: pace_ast::TypeAnnotation {
                                module_prefix: None,
                                name: class_name.clone(),
                                args: vec![],
                                is_nullable: false,
                            },
                        };
                        let mut all_params = vec![self_param];
                        all_params.extend(params.clone());
                        let ret = return_type.as_ref().map(|t| t.name.as_str()).unwrap_or("Int");
                        
                        self.compile_function(&full_name, &all_params, body, id, &func_returns, ret, Some(class_name))?;
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
        let ret_ty = crate::translator::parse_vartype(ret_type_str, current_class);
        self.ctx.func.signature.returns.push(AbiParam::new(ret_ty.to_cranelift_type()));
        for param in params {
            let ty = crate::translator::parse_vartype(&param.type_annotation.name, current_class);
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
            
            let param_ty = crate::translator::parse_vartype(&param.type_annotation.name, current_class);
            variables.insert(param.name.clone(), (var, param_ty));
            var_index += 1;
        }

        let mut last_val = None;
        let mut terminated = false;
        for stmt in body {
            let (val, term) = Translator::translate_stmt(&mut self.module, &self.funcs, &self.class_layouts, &self.struct_layouts, &self.enum_layouts, &mut builder, stmt, &mut variables, &mut var_index, func_returns)?;
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
        Ok(())
    }
}
