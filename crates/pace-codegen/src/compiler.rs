use crate::context::CodegenContext;
use crate::layouts::{ClassLayout, CodegenError, EnumLayout, InterfaceLayout, StructLayout};
use crate::translator::Translator;
use crate::translator::VarType;
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, FuncId, Linkage, Module};
use pace_ast::Stmt;
use std::collections::HashMap;

pub struct JITCompiler {
    pub context: CodegenContext<JITModule>,
    pub builder_context: cranelift::prelude::FunctionBuilderContext,
    pub ctx: cranelift::codegen::Context,
}

impl Default for JITCompiler {
    fn default() -> Self {
        Self::new("none".to_string())
    }
}

impl JITCompiler {
    pub fn new(opt_level: String) -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("preserve_frame_pointers", "false").unwrap();
        flag_builder.set("opt_level", &opt_level).unwrap();
        flag_builder.set("is_pic", "false").unwrap();

        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });

        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .unwrap();

        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        // Expose pace-runtime to the JIT explicitly
        builder.symbol(
            "__pace_print_int",
            pace_runtime::__pace_print_int as *const u8,
        );
        builder.symbol(
            "__pace_print_float",
            pace_runtime::__pace_print_float as *const u8,
        );
        builder.symbol(
            "__pace_print_string",
            pace_runtime::__pace_print_string as *const u8,
        );
        builder.symbol(
            "__pace_concat_strings",
            pace_runtime::__pace_concat_strings as *const u8,
        );
        builder.symbol(
            "__pace_int_to_string",
            pace_runtime::__pace_int_to_string as *const u8,
        );
        builder.symbol(
            "__pace_float_to_string",
            pace_runtime::__pace_float_to_string as *const u8,
        );
        builder.symbol(
            "__pace_bool_to_string",
            pace_runtime::__pace_bool_to_string as *const u8,
        );
        builder.symbol("__pace_malloc", pace_runtime::__pace_malloc as *const u8);
        builder.symbol("__pace_noop", pace_runtime::__pace_noop as *const u8);
        builder.symbol("__pace_retain", pace_runtime::__pace_retain as *const u8);
        builder.symbol("__pace_release", pace_runtime::__pace_release as *const u8);
        builder.symbol("__pace_free", pace_runtime::__pace_free as *const u8);
        builder.symbol(
            "__pace_ptr_store",
            pace_runtime::__pace_ptr_store as *const u8,
        );
        builder.symbol(
            "__pace_ptr_load",
            pace_runtime::__pace_ptr_load as *const u8,
        );
        builder.symbol("__pace_time", pace_runtime::__pace_time as *const u8);
        builder.symbol(
            "__pace_get_year",
            pace_runtime::__pace_get_year as *const u8,
        );
        builder.symbol("__pace_hash", pace_runtime::__pace_hash as *const u8);
        builder.symbol("__pace_sb_new", pace_runtime::__pace_sb_new as *const u8);
        builder.symbol(
            "__pace_sb_append",
            pace_runtime::__pace_sb_append as *const u8,
        );
        builder.symbol(
            "__pace_sb_build",
            pace_runtime::__pace_sb_build as *const u8,
        );
        builder.symbol("__pace_sb_free", pace_runtime::__pace_sb_free as *const u8);
        builder.symbol(
            "__pace_mailbox_create",
            pace_runtime::__pace_mailbox_create as *const u8,
        );
        builder.symbol(
            "__pace_mailbox_send",
            pace_runtime::__pace_mailbox_send as *const u8,
        );
        builder.symbol(
            "__pace_mailbox_destroy",
            pace_runtime::__pace_mailbox_destroy as *const u8,
        );
        builder.symbol(
            "__pace_promise_create",
            pace_runtime::__pace_promise_create as *const u8,
        );
        builder.symbol(
            "__pace_promise_resolve",
            pace_runtime::__pace_promise_resolve as *const u8,
        );
        builder.symbol(
            "__pace_promise_await",
            pace_runtime::__pace_promise_await as *const u8,
        );

        // Add FS, OS, Process, HTTP, and String symbols
        builder.symbol("fsWriteText", pace_runtime::__pace_fs_write as *const u8);
        builder.symbol("fsExists", pace_runtime::__pace_fs_exists as *const u8);
        builder.symbol("fsReadText", pace_runtime::__pace_fs_read as *const u8);
        builder.symbol("fsDeleteFile", pace_runtime::__pace_fs_delete as *const u8);
        builder.symbol("fsMakeDir", pace_runtime::__pace_fs_mkdir as *const u8);
        builder.symbol(
            "fsDirExists",
            pace_runtime::__pace_fs_dir_exists as *const u8,
        );

        builder.symbol("osGetEnv", pace_runtime::__pace_os_getenv as *const u8);
        builder.symbol("osName", pace_runtime::__pace_os_name as *const u8);

        builder.symbol("processRun", pace_runtime::__pace_process_run as *const u8);
        builder.symbol(
            "processExit",
            pace_runtime::__pace_process_exit as *const u8,
        );

        builder.symbol("httpGet", pace_runtime::__pace_http_get as *const u8);
        builder.symbol("httpPost", pace_runtime::__pace_http_post as *const u8);
        builder.symbol("httpPut", pace_runtime::__pace_http_put as *const u8);
        builder.symbol("httpDelete", pace_runtime::__pace_http_delete as *const u8);

        builder.symbol(
            "getLastError",
            pace_runtime::__pace_get_last_error as *const u8,
        );
        builder.symbol(
            "__pace_string_split",
            pace_runtime::__pace_string_split as *const u8,
        );
        builder.symbol(
            "__pace_string_replace",
            pace_runtime::__pace_string_replace as *const u8,
        );
        builder.symbol(
            "__pace_string_substring",
            pace_runtime::__pace_string_substring as *const u8,
        );
        builder.symbol("__pace_string_trim", pace_runtime::__pace_string_trim as *const u8);
        builder.symbol(
            "__pace_string_index_of",
            pace_runtime::__pace_string_index_of as *const u8,
        );
        builder.symbol(
            "__pace_string_starts_with",
            pace_runtime::__pace_string_starts_with as *const u8,
        );

        let module = JITModule::new(builder);

        Self {
            context: CodegenContext::new(module),
            builder_context: cranelift::prelude::FunctionBuilderContext::new(),
            ctx: cranelift::codegen::Context::new(),
        }
    }

    fn register_interfaces(&mut self, arena: &pace_ast::arena::AstArena, stmts: &[pace_ast::arena::StmtId]) {
        for stmt_id in stmts {
            let stmt = arena.get_stmt(*stmt_id).clone();
            if let Stmt::InterfaceDecl {
                name: interface_name,
                methods,
                ..
            } = stmt
            {
                let mut method_map = HashMap::new();
                let mut m_offset = 16; // 0: drop, 8: size

                for method_stmt_id in methods {
                    let method_stmt = arena.get_stmt(method_stmt_id).clone();
                    if let Stmt::FuncDecl {
                        name: method_name, ..
                    } = method_stmt
                        && method_name != "init" {
                            method_map.insert(method_name, m_offset);
                            m_offset += 8;
                        }
                }

                let layout = InterfaceLayout {
                    name: interface_name,
                    methods: method_map.clone(),
                };
                self.context
                    .interface_layouts
                    .insert(interface_name, layout);

                // Insert a dummy ClassLayout for the interface so translate_expr can find its methods by type_name
                let dummy_vtable_name = format!("__iface_vtable_{}", interface_name);
                let dummy_vtable_id = self
                    .context
                    .module
                    .declare_data(&dummy_vtable_name, Linkage::Local, false, false)
                    .unwrap();

                let mut data_ctx = DataDescription::new();
                let vtable_bytes = vec![0u8; 16];
                data_ctx.define(vtable_bytes.into_boxed_slice());
                self.context
                    .module
                    .define_data(dummy_vtable_id, &data_ctx)
                    .unwrap();

                let dummy_class_layout = ClassLayout {
                    name: interface_name,
                    fields: HashMap::new(),
                    static_fields: HashMap::new(),
                    methods: method_map,
                    vtable_id: dummy_vtable_id,
                };
                self.context
                    .class_layouts
                    .insert(interface_name, dummy_class_layout);
            }
        }
    }

    fn register_classes(&mut self, arena: &pace_ast::arena::AstArena, stmts: &[pace_ast::arena::StmtId]) -> Result<(), CodegenError> {
        let _ptr_ty = self.context.module.target_config().pointer_type();

        for stmt_id in stmts {
            let stmt = arena.get_stmt(*stmt_id).clone();
            if let Stmt::ClassDecl {
                name: class_name,
                fields,
                methods,
                implements,
                ..
            }
            | Stmt::ActorDecl {
                name: class_name,
                fields,
                methods,
                implements,
                ..
            } = stmt
            {
                let is_actor = matches!(arena.get_stmt(*stmt_id), Stmt::ActorDecl { .. });
                let mut field_map = HashMap::new();
                let mut offset = 16; // 8 bytes for ARC, 8 bytes for vtable pointer

                if is_actor {
                    field_map.insert("__mailbox".to_string().into(), (offset, VarType::Unknown)); // Internal pointer
                    offset += 8;
                }

                let mut static_fields = HashMap::new();
                for field_id in &fields {
                    let field = arena.get_stmt(*field_id).clone();
                    if let Stmt::VarDecl {
                        name: field_name,
                        type_annotation,
                        is_static,
                        initializer,
                        ..
                    } = field
                    {
                        let ty_str = type_annotation
                            .as_ref()
                            .map(|t| t.name.as_str())
                            .unwrap_or("Unknown");
                        let field_ty = crate::translator::parse_vartype(
                            ty_str,
                            Some(class_name.as_str()),
                            Some(&self.context.struct_layouts),
                            Some(&self.context.enum_layouts),
                        );
                        if is_static {
                            let global_name = format!("{}_{}", class_name, field_name);
                            let data_id = self
                                .context
                                .module
                                .declare_data(&global_name, Linkage::Export, true, false)
                                .expect("Failed to declare static field");
                            let mut data_ctx = DataDescription::new();

                            let mut init_bytes = vec![0u8; 8];
                            if let Some(init_expr) = initializer {
                                use pace_ast::Expr;
                                match arena.get_expr(init_expr) {
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
                            self.context
                                .module
                                .define_data(data_id, &data_ctx)
                                .expect("Failed to define static field data");
                            static_fields.insert(field_name, (data_id, field_ty));
                        } else {
                            field_map.insert(field_name.to_string().into(), (offset, field_ty));
                            offset += 8;
                        }
                    }
                }

                let mut method_map = HashMap::new();
                let mut m_offset = 16;
                let mut vtable_funcs: HashMap<ustr::Ustr, cranelift_module::FuncId> = HashMap::new();

                // Seed methods from interface if implemented
                if let Some(iface_annotation) = implements
                    && let Some(iface_layout) =
                        self.context.interface_layouts.get(&iface_annotation.name)
                {
                    for (m_name, m_off) in &iface_layout.methods {
                        method_map.insert(*m_name, *m_off);
                        if *m_off >= m_offset {
                            m_offset = *m_off + 8;
                        }
                    }
                }

                let ptr_ty = self.context.module.target_config().pointer_type();

                let drop_name = format!("__drop_{}", class_name);
                let mut drop_sig = self.context.module.make_signature();
                drop_sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
                drop_sig.params.push(AbiParam::new(ptr_ty)); // obj ptr
                let drop_id = self
                    .context
                    .module
                    .declare_function(&drop_name, Linkage::Local, &drop_sig)
                    .map_err(|e| CodegenError {
                        message: e.to_string(),
                    })?;
                self.context.funcs.insert(drop_name.clone().into(), drop_id);

                for method_stmt_id in methods {
                    let method_stmt = arena.get_stmt(method_stmt_id).clone();
                    if let Stmt::FuncDecl {
                        name: method_name,
                        params,
                        is_static,
                        ..
                    } = method_stmt
                    {
                        if !is_static
                            && !method_map.contains_key(&method_name)
                            && method_name != "init"
                        {
                            method_map.insert(method_name, m_offset);
                            m_offset += 8;
                        }

                        let full_name = format!("{}_{}", class_name, method_name);
                        let mut sig = self.context.module.make_signature();
                        sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
                        if !is_static {
                            sig.params.push(AbiParam::new(ptr_ty)); // self
                        }
                        for _ in params {
                            sig.params.push(AbiParam::new(types::I64));
                        }
                        sig.returns.push(AbiParam::new(types::I64));

                        let id = self
                            .context
                            .module
                            .declare_function(&full_name, Linkage::Local, &sig)
                            .map_err(|e| CodegenError {
                                message: e.to_string(),
                            })?;
                        self.context.funcs.insert(full_name.clone().into(), id);

                        if !is_static && method_name != "init" {
                            if is_actor {
                                let async_name = format!("__async_{}_{}", class_name, method_name);
                                let mut async_sig = self.context.module.make_signature();
                                async_sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
                                async_sig.params.push(AbiParam::new(types::I64));
                                async_sig.returns.push(AbiParam::new(types::I64));
                                let async_id = self
                                    .context
                                    .module
                                    .declare_function(&async_name, Linkage::Local, &async_sig)
                                    .map_err(|e| CodegenError {
                                        message: e.to_string(),
                                    })?;
                                self.context.funcs.insert(async_name.clone().into(), async_id);
                                vtable_funcs.insert(method_name, async_id);
                            } else {
                                vtable_funcs.insert(method_name, id);
                            }
                        }
                    }
                }

                let vtable_name = format!("__vtable_{}", class_name);
                let vtable_id = self
                    .context
                    .module
                    .declare_data(&vtable_name, Linkage::Local, false, false)
                    .map_err(|e| CodegenError {
                        message: e.to_string(),
                    })?;

                let mut data_ctx = DataDescription::new();
                let size = (16 + fields.len() * 8) as u64;
                let mut vtable_bytes = vec![0u8; m_offset];
                vtable_bytes[8..16].copy_from_slice(&size.to_ne_bytes());
                data_ctx.define(vtable_bytes.into_boxed_slice());

                let drop_ref = self
                    .context
                    .module
                    .declare_func_in_data(drop_id, &mut data_ctx);
                data_ctx.write_function_addr(0, drop_ref);

                for (m_name, func_id) in &vtable_funcs {
                    let byte_offset = *method_map.get(m_name).unwrap();
                    let func_ref = self
                        .context
                        .module
                        .declare_func_in_data(*func_id, &mut data_ctx);
                    data_ctx.write_function_addr(byte_offset as u32, func_ref);
                }

                self.context
                    .module
                    .define_data(vtable_id, &data_ctx)
                    .map_err(|e| CodegenError {
                        message: e.to_string(),
                    })?;

                let layout = ClassLayout {
                    name: class_name,
                    fields: field_map,
                    methods: method_map,
                    static_fields,
                    vtable_id,
                };
                self.context
                    .class_layouts
                    .insert(class_name, layout);
            } else if let Stmt::EnumDecl {
                name: enum_name,
                variants,
                ..
            } = stmt
            {
                let mut max_size = 16; // 8 for ARC, 8 for Tag
                let mut variant_map = HashMap::new();

                let ptr_ty = self.context.module.target_config().pointer_type();

                let drop_name = format!("__drop_{}", enum_name);
                let mut drop_sig = self.context.module.make_signature();
                drop_sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
                drop_sig.params.push(AbiParam::new(ptr_ty)); // obj ptr
                let drop_id = self
                    .context
                    .module
                    .declare_function(&drop_name, Linkage::Local, &drop_sig)
                    .map_err(|e| CodegenError {
                        message: e.to_string(),
                    })?;
                self.context.funcs.insert(drop_name.clone().into(), drop_id);

                for (tag_id, variant) in variants.iter().enumerate() {
                    let mut variant_types = Vec::new();
                    let mut variant_size = 16;

                    if let Some(fields) = &variant.fields {
                        for field_ty in fields {
                            let field_var_type = crate::translator::parse_vartype(
                                &field_ty.name,
                                Some(enum_name.as_str()),
                                Some(&self.context.struct_layouts),
                                Some(&self.context.enum_layouts),
                            );
                            variant_types.push(field_var_type);
                            variant_size += 8;
                        }
                    }

                    if variant_size > max_size {
                        max_size = variant_size;
                    }

                    variant_map
                        .insert(variant.name, (tag_id as u64, variant_types.clone()));

                    // Generate Constructor Signature: e.g. Result_Ok(T) -> ResultPtr
                    let constructor_name = format!("{}_{}", enum_name, variant.name);
                    let mut sig = self.context.module.make_signature();
                    sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
                    for _ in 0..variant_types.len() {
                        sig.params.push(AbiParam::new(types::I64));
                    }
                    sig.returns.push(AbiParam::new(types::I64));

                    let constructor_id = self
                        .context
                        .module
                        .declare_function(&constructor_name, Linkage::Local, &sig)
                        .map_err(|e| CodegenError {
                            message: e.to_string(),
                        })?;
                    self.context.funcs.insert(constructor_name.into(), constructor_id);
                }

                let layout = EnumLayout {
                    name: enum_name,
                    max_size,
                    variants: variant_map,
                    drop_func_id: drop_id,
                };
                self.context.enum_layouts.insert(enum_name, layout);
            } else if let Stmt::StructDecl {
                name: struct_name,
                fields,
                ..
            } = stmt
            {
                let mut field_map = HashMap::new();
                let mut offset = 0; // Structs have no header (0 bytes for ARC/VTable)
                let mut static_fields = HashMap::new();
                for field_id in &fields {
                    let field = arena.get_stmt(*field_id).clone();
                    if let Stmt::VarDecl {
                        name: field_name,
                        type_annotation,
                        is_static,
                        initializer,
                        ..
                    } = field
                    {
                        let ty_str = type_annotation
                            .as_ref()
                            .map(|t| t.name.as_str())
                            .unwrap_or("Unknown");
                        let field_ty = crate::translator::parse_vartype(
                            ty_str,
                            Some(struct_name.as_str()),
                            Some(&self.context.struct_layouts),
                            Some(&self.context.enum_layouts),
                        );
                        if is_static {
                            let global_name = format!("{}_{}", struct_name, field_name);
                            let data_id = self
                                .context
                                .module
                                .declare_data(&global_name, Linkage::Export, true, false)
                                .expect("Failed to declare static field");
                            let mut data_ctx = DataDescription::new();

                            let mut init_bytes = vec![0u8; 8];
                            if let Some(init_expr) = initializer {
                                use pace_ast::Expr;
                                match arena.get_expr(init_expr) {
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
                            self.context
                                .module
                                .define_data(data_id, &data_ctx)
                                .expect("Failed to define static field data");
                            static_fields.insert(field_name, (data_id, field_ty));
                        } else {
                            field_map.insert(field_name, (offset, field_ty));
                            offset += 8; // All fields are currently 8 bytes (i64/f64/ptr)
                        }
                    }
                }

                let layout = StructLayout {
                    name: struct_name,
                    fields: field_map,
                    static_fields,
                    size: offset,
                };
                self.context
                    .struct_layouts
                    .insert(struct_name, layout);
            }
        }
        Ok(())
    }

    pub fn compile_and_run(&mut self, arena: &mut pace_ast::arena::AstArena, stmts: &[pace_ast::arena::StmtId]) -> Result<(), CodegenError> {
        let flat_stmts = crate::flatten_ast(arena, stmts);

        // Run Monomorphization Pass
        let mut mono = crate::monomorphize::MonomorphizationPass::new(arena);
        mono.process(&flat_stmts);
        let final_stmts = &mono.final_stmts;

        self.register_interfaces(arena, final_stmts);
        self.register_classes(arena, final_stmts)?;

        // Pass 1: Declare all functions
        for stmt_id in final_stmts {
            let stmt = arena.get_stmt(*stmt_id).clone();
            if let Stmt::FuncDecl { name, params, .. } = stmt {
                let mut sig = self.context.module.make_signature();
                sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
                for _ in params {
                    sig.params.push(AbiParam::new(types::I64)); // Assume I64 for now
                }
                sig.returns.push(AbiParam::new(types::I64)); // Assume I64 return

                let id = self
                    .context
                    .module
                    .declare_function(name.as_str(), Linkage::Local, &sig)
                    .map_err(|e| CodegenError {
                        message: e.to_string(),
                    })?;
                self.context.funcs.insert(name, id);
            }
        }

        // Pass 1.5: Declare global variables
        for stmt_id in final_stmts {
            let stmt = arena.get_stmt(*stmt_id).clone();
            if let Stmt::VarDecl { name, .. } = stmt {
                let id = self
                    .context
                    .module
                    .declare_data(name.as_str(), Linkage::Export, true, false)
                    .map_err(|e| CodegenError {
                        message: e.to_string(),
                    })?;
                self.context.global_vars.insert(name, id);

                let mut data = DataDescription::new();
                data.define_zeroinit(8); // Allocate 8 bytes for an I64/ptr
                self.context
                    .module
                    .define_data(id, &data)
                    .map_err(|e| CodegenError {
                        message: e.to_string(),
                    })?;
            }
        }

        let mut func_returns = HashMap::new();
        for stmt_id in final_stmts {
            let stmt = arena.get_stmt(*stmt_id).clone();
            if let Stmt::FuncDecl {
                name, return_type, ..
            } = stmt
            {
                let ret = return_type
                    .as_ref()
                    .map(|t| t.name.as_str())
                    .unwrap_or("Int");
                func_returns.insert(
                    name,
                    crate::translator::parse_vartype(
                        ret,
                        None,
                        Some(&self.context.struct_layouts),
                        Some(&self.context.enum_layouts),
                    ),
                );
            } else if let Stmt::ClassDecl {
                name: class_name,
                methods,
                ..
            }
            | Stmt::ActorDecl {
                name: class_name,
                methods,
                ..
            } = stmt
            {
                for method_stmt_id in methods {
                    let method_stmt = arena.get_stmt(method_stmt_id).clone();
                    if let Stmt::FuncDecl {
                        name,
                        params: _,
                        return_type,
                        ..
                    } = method_stmt
                    {
                        let ret = return_type
                            .as_ref()
                            .map(|t| t.name.as_str())
                            .unwrap_or("Int");
                        let full_name = format!("{}_{}", class_name, name);
                        func_returns.insert(
                            full_name.into(),
                            crate::translator::parse_vartype(
                                ret,
                                Some(class_name.as_str()),
                                Some(&self.context.struct_layouts),
                                Some(&self.context.enum_layouts),
                            ),
                        );
                    }
                }
            } else if let Stmt::InterfaceDecl {
                name: interface_name,
                methods,
                ..
            } = stmt
            {
                for method_stmt_id in methods {
                    let method_stmt = arena.get_stmt(method_stmt_id).clone();
                    if let Stmt::FuncDecl {
                        name,
                        params: _,
                        return_type,
                        ..
                    } = method_stmt
                    {
                        let ret = return_type
                            .as_ref()
                            .map(|t| t.name.as_str())
                            .unwrap_or("Int");
                        let full_name = format!("{}_{}", interface_name, name);
                        func_returns.insert(
                            full_name.into(),
                            crate::translator::parse_vartype(
                                ret,
                                Some(interface_name.as_str()),
                                Some(&self.context.struct_layouts),
                                Some(&self.context.enum_layouts),
                            ),
                        );
                    }
                }
            }
        }

        // Pass 2: Define all functions and class methods
        for stmt_id in final_stmts {
            let stmt = arena.get_stmt(*stmt_id).clone();
            if let Stmt::FuncDecl {
                name, params, body, ..
            } = stmt
            {
                let id = *self.context.funcs.get(&name).unwrap();
                self.compile_function(arena, name.as_str(), &params, &body, id, &func_returns, None)?;
            } else if let Stmt::ClassDecl {
                name: class_name,
                methods,
                ..
            }
            | Stmt::ActorDecl {
                name: class_name,
                methods,
                ..
            } = stmt
            {
                let is_actor = matches!(arena.get_stmt(*stmt_id), Stmt::ActorDecl { .. });
                self.generate_drop_function(class_name.as_str())?;
                for method_stmt_id in methods {
                    let method_stmt = arena.get_stmt(method_stmt_id).clone();
                    if let Stmt::FuncDecl {
                        name,
                        params,
                        body,
                        is_static,
                        ..
                    } = method_stmt
                    {
                        let full_name = format!("{}_{}", class_name, name);
                        let id = *self.context.funcs.get(&ustr::Ustr::from(&full_name)).unwrap();

                        let mut new_params = vec![];
                        if !is_static {
                            new_params.push(pace_ast::Param {
                                name: "self".to_string().into(),
                                type_annotation: pace_ast::TypeAnnotation {
                                    module_prefix: None,
                                    name: class_name,
                                    args: vec![],
                                    is_nullable: false,
                                    is_function: false,
                                    function_params: None,
                                    function_return: None,
                                },
                            });
                        }
                        new_params.extend(params.clone());

                        self.compile_function(
                            arena,
                            full_name.as_str(),
                            &new_params,
                            &body,
                            id,
                            &func_returns,
                            Some(class_name.as_str()),
                        )?;

                        if !is_static && name != "init" && is_actor {
                            self.generate_async_wrapper(class_name.as_str(), name.as_str(), params.len())?;
                        }
                    }
                }
            } else if let Stmt::EnumDecl {
                name: enum_name,
                variants,
                ..
            } = stmt
            {
                self.generate_enum_drop_function(enum_name.as_str())?;
                self.generate_enum_constructors(enum_name.as_str(), &variants)?;
            }
        }

        // Pass 3: Compile implicit `__entry__` that executes top-level code and calls `main` if it exists.
        self.ctx
            .func
            .signature
            .returns
            .push(AbiParam::new(types::I64));
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let mut variables: std::collections::HashMap<ustr::Ustr, (cranelift::prelude::Variable, crate::translator::VarType)> = std::collections::HashMap::new();
        let mut var_index = 0;
        let mut last_val = None;

        let mut func_returns = HashMap::new();
        for stmt_id in final_stmts {
            let stmt = arena.get_stmt(*stmt_id).clone();
            if let Stmt::FuncDecl {
                name, return_type, ..
            } = stmt
            {
                let ret = return_type
                    .as_ref()
                    .map(|t| t.name.as_str())
                    .unwrap_or("Int");
                func_returns.insert(
                    name,
                    crate::translator::parse_vartype(
                        ret,
                        None,
                        Some(&self.context.struct_layouts),
                        Some(&self.context.enum_layouts),
                    ),
                );
            }
        }

        let mut pending_closures: Vec<(
            ustr::Ustr,
            pace_ast::Expr,
            Vec<(ustr::Ustr, crate::translator::VarType)>,
        )> = Vec::new();
        let mut translator = Translator {
            arena,
            context: &mut self.context,
            builder: &mut builder,
            variables: &mut variables,
            var_index: &mut var_index,
            func_returns: &func_returns,
            pending_closures: &mut pending_closures,
            is_global_context: true,
        };
        for stmt_id in final_stmts {
            let stmt = arena.get_stmt(*stmt_id).clone();
            match stmt {
                Stmt::VarDecl { .. }
                | Stmt::Expr(_)
                | Stmt::If { .. }
                | Stmt::While { .. }
                | Stmt::Loop { .. }
                | Stmt::Match { .. } => {
                    let (val, _) = translator.translate_stmt(*stmt_id)?;
                    last_val = Some(val);
                }
                _ => {}
            }
        }

        // Call main if it exists
        if let Some(&main_id) = self.context.funcs.get(&ustr::Ustr::from("main")) {
            let local_func = self
                .context
                .module
                .declare_func_in_func(main_id, builder.func);
            let call = builder.ins().call(local_func, &[]);
            let res = builder.inst_results(call)[0];
            last_val = Some(res);
        }

        let ret_val = last_val.unwrap_or_else(|| builder.ins().iconst(types::I64, 0));
        builder.ins().return_(&[ret_val]);
        builder.finalize(self.context.module.target_config());

        let id = self
            .context
            .module
            .declare_function("__entry__", Linkage::Export, &self.ctx.func.signature)
            .map_err(|e| CodegenError {
                message: e.to_string(),
            })?;

        self.context
            .module
            .define_function(id, &mut self.ctx)
            .map_err(|e| CodegenError {
                message: e.to_string(),
            })?;

        self.context.module.clear_context(&mut self.ctx);

        for (fn_name, expr, captured_vars) in pending_closures.into_iter() {
            self.compile_closure(arena, &fn_name, expr, captured_vars, &func_returns, None)?;
        }
        self.context.module.finalize_definitions().unwrap();

        let code = self.context.module.get_finalized_function(id);

        // Execute the code
        let entry_func: fn() -> i64 = unsafe { std::mem::transmute(code) };
        let _result = entry_func();

        Ok(())
    }

    fn generate_drop_function(&mut self, class_name: &str) -> Result<(), CodegenError> {
        let layout = self.context.class_layouts.get(&ustr::Ustr::from(class_name)).unwrap().clone();
        let drop_name = format!("__drop_{}", class_name);
        let func_id = *self.context.funcs.get(&ustr::Ustr::from(&drop_name)).unwrap();

        self.ctx.func.signature.params.push(AbiParam::new(
            self.context.module.target_config().pointer_type(),
        ));

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let obj_ptr = builder.block_params(entry_block)[0];

        for &(offset, ref ty) in layout.fields.values() {
            if matches!(ty, VarType::Object(_)) {
                let val = builder.ins().load(
                    types::I64,
                    cranelift::prelude::MemFlagsData::new(),
                    obj_ptr,
                    offset as i32,
                );
                let release_id = *self.context.funcs.get(&ustr::Ustr::from("release")).unwrap();
                let local_release = self
                    .context
                    .module
                    .declare_func_in_func(release_id, builder.func);
                builder.ins().call(local_release, &[val]);
            }
        }

        builder.ins().return_(&[]);
        builder.finalize(self.context.module.target_config());

        self.context
            .module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| CodegenError {
                message: e.to_string(),
            })?;

        self.context.module.clear_context(&mut self.ctx);
        Ok(())
    }

    fn generate_async_wrapper(
        &mut self,
        class_name: &str,
        method_name: &str,
        num_args: usize,
    ) -> Result<(), CodegenError> {
        let async_name = format!("__async_{}_{}", class_name, method_name);
        let id = *self.context.funcs.get(&ustr::Ustr::from(&async_name)).unwrap();
        let target_id = *self
            .context
            .funcs
            .get(&ustr::Ustr::from(&format!("{}_{}", class_name, method_name)))
            .unwrap();

        self.ctx
            .func
            .signature
            .params
            .push(AbiParam::new(types::I64)); // arg_ptr
        self.ctx
            .func
            .signature
            .returns
            .push(AbiParam::new(types::I64));

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let arg_ptr = builder.block_params(entry_block)[0];

        let mut call_args = Vec::new();
        for i in 0..=num_args {
            // self + args
            let val = builder.ins().load(
                types::I64,
                cranelift::prelude::MemFlagsData::new(),
                arg_ptr,
                (i * 8) as i32,
            );
            call_args.push(val);
        }

        // Call the real method
        let local_target = self
            .context
            .module
            .declare_func_in_func(target_id, builder.func);
        let call = builder.ins().call(local_target, &call_args);

        let results = builder.inst_results(call);
        let ret_val = if results.is_empty() {
            builder.ins().iconst(types::I64, 0)
        } else {
            results[0]
        };

        // Free the tuple allocated by the caller
        let free_id = *self.context.funcs.get(&ustr::Ustr::from("free")).unwrap();
        let local_free = self
            .context
            .module
            .declare_func_in_func(free_id, builder.func);
        let size_val = builder
            .ins()
            .iconst(types::I64, ((num_args + 1) * 8) as i64);
        builder.ins().call(local_free, &[arg_ptr, size_val]);

        builder.ins().return_(&[ret_val]);
        builder.finalize(self.context.module.target_config());

        self.context
            .module
            .define_function(id, &mut self.ctx)
            .map_err(|e| CodegenError {
                message: e.to_string(),
            })?;
        self.context.module.clear_context(&mut self.ctx);
        Ok(())
    }

    fn generate_enum_drop_function(&mut self, enum_name: &str) -> Result<(), CodegenError> {
        let layout = self.context.enum_layouts.get(&ustr::Ustr::from(enum_name)).unwrap().clone();
        let func_id = layout.drop_func_id;

        self.ctx.func.signature.params.push(AbiParam::new(
            self.context.module.target_config().pointer_type(),
        ));

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);

        let obj_ptr = builder.block_params(entry_block)[0];

        let tag_val = builder.ins().load(
            types::I64,
            cranelift::prelude::MemFlagsData::new(),
            obj_ptr,
            8,
        );

        // We create a switch table manually using basic blocks
        let mut blocks = Vec::new();
        for _ in 0..layout.variants.len() {
            blocks.push(builder.create_block());
        }
        let end_block = builder.create_block();

        // Build the switch statement
        for (tag_id, _) in layout.variants.values() {
            let next_check = builder.create_block();
            let expected_tag = builder.ins().iconst(types::I64, *tag_id as i64);
            let is_match = builder.ins().icmp(
                cranelift::codegen::ir::condcodes::IntCC::Equal,
                tag_val,
                expected_tag,
            );
            builder
                .ins()
                .brif(is_match, blocks[*tag_id as usize], &[], next_check, &[]);

            builder.seal_block(next_check);
            builder.switch_to_block(next_check);
        }
        builder.ins().jump(end_block, &[]); // Fallback

        // Build the variant blocks
        for (tag_id, fields) in layout.variants.values() {
            let block = blocks[*tag_id as usize];
            builder.seal_block(block);
            builder.switch_to_block(block);

            let mut offset = 16;
            for ty in fields {
                if matches!(ty, VarType::Object(_)) {
                    let val = builder.ins().load(
                        types::I64,
                        cranelift::prelude::MemFlagsData::new(),
                        obj_ptr,
                        offset,
                    );
                    let release_id = *self.context.funcs.get(&ustr::Ustr::from("release")).unwrap();
                    let local_release = self
                        .context
                        .module
                        .declare_func_in_func(release_id, builder.func);
                    builder.ins().call(local_release, &[val]);
                }
                offset += 8;
            }

            builder.ins().jump(end_block, &[]);
        }

        builder.seal_block(end_block);
        builder.switch_to_block(end_block);
        builder.ins().return_(&[]);

        builder.seal_block(entry_block); // Seal the initial block too

        builder.finalize(self.context.module.target_config());

        self.context
            .module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| CodegenError {
                message: e.to_string(),
            })?;

        self.context.module.clear_context(&mut self.ctx);
        Ok(())
    }

    fn generate_enum_constructors(
        &mut self,
        enum_name: &str,
        variants: &[pace_ast::EnumVariant],
    ) -> Result<(), CodegenError> {
        let layout = self.context.enum_layouts.get(&ustr::Ustr::from(enum_name)).unwrap().clone();

        for variant in variants {
            let constructor_name = format!("{}_{}", enum_name, variant.name);
            let func_id = *self.context.funcs.get(&ustr::Ustr::from(&constructor_name)).unwrap();
            let (tag_id, fields) = layout.variants.get(&variant.name).unwrap();

            for _ in 0..fields.len() {
                self.ctx
                    .func
                    .signature
                    .params
                    .push(AbiParam::new(types::I64));
            }
            self.ctx
                .func
                .signature
                .returns
                .push(AbiParam::new(types::I64));

            let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            // Call malloc
            let malloc_id = *self.context.funcs.get(&ustr::Ustr::from("malloc")).unwrap();
            let local_malloc = self
                .context
                .module
                .declare_func_in_func(malloc_id, builder.func);
            let size_val = builder.ins().iconst(types::I64, layout.max_size as i64);
            let call = builder.ins().call(local_malloc, &[size_val]);
            let obj_ptr = builder.inst_results(call)[0];

            // Set ARC counter to 1
            let ref_count = builder.ins().iconst(types::I64, 1);
            builder.ins().store(
                cranelift::prelude::MemFlagsData::new(),
                ref_count,
                obj_ptr,
                0,
            );

            // Set Tag
            let tag_val = builder.ins().iconst(types::I64, *tag_id as i64);
            builder
                .ins()
                .store(cranelift::prelude::MemFlagsData::new(), tag_val, obj_ptr, 8);

            // Set fields and increment their ARC
            let mut offset = 16;
            for (i, field_ty) in fields.iter().enumerate() {
                let arg_val = builder.block_params(entry_block)[i];
                builder.ins().store(
                    cranelift::prelude::MemFlagsData::new(),
                    arg_val,
                    obj_ptr,
                    offset,
                );

                if matches!(field_ty, VarType::Object(_)) {
                    let retain_id = *self.context.funcs.get(&ustr::Ustr::from("retain")).unwrap();
                    let local_retain = self
                        .context
                        .module
                        .declare_func_in_func(retain_id, builder.func);
                    builder.ins().call(local_retain, &[arg_val]);
                }

                offset += 8;
            }

            builder.ins().return_(&[obj_ptr]);
            builder.finalize(self.context.module.target_config());

            self.context
                .module
                .define_function(func_id, &mut self.ctx)
                .map_err(|e| CodegenError {
                    message: e.to_string(),
                })?;

            self.context.module.clear_context(&mut self.ctx);
        }
        Ok(())
    }

    fn compile_function(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        _name: &str,
        params: &[pace_ast::Param],
        body: &[pace_ast::arena::StmtId],
        func_id: FuncId,
        func_returns: &HashMap<ustr::Ustr, crate::translator::VarType>,
        current_class: Option<&str>,
    ) -> Result<(), CodegenError> {
        self.ctx
            .func
            .signature
            .returns
            .push(AbiParam::new(types::I64));
        for _ in params {
            self.ctx
                .func
                .signature
                .params
                .push(AbiParam::new(types::I64));
        }

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let mut variables: std::collections::HashMap<ustr::Ustr, (cranelift::prelude::Variable, crate::translator::VarType)> = std::collections::HashMap::new();
        let mut var_index = 0;

        // Declare parameters as variables
        for (i, param) in params.iter().enumerate() {
            let val = builder.block_params(entry_block)[i];
            let var = builder.declare_var(types::I64);
            builder.def_var(var, val);

            let param_ty = crate::translator::parse_type_annotation(
                &param.type_annotation,
                current_class,
                Some(&self.context.struct_layouts),
                Some(&self.context.enum_layouts),
            );
            variables.insert(param.name, (var, param_ty));
            var_index += 1;
        }

        let mut last_val = None;
        let mut terminated = false;
        let mut pending_closures: Vec<(
            ustr::Ustr,
            pace_ast::Expr,
            Vec<(ustr::Ustr, crate::translator::VarType)>,
        )> = Vec::new();
        let mut translator = Translator {
            arena,
            context: &mut self.context,
            builder: &mut builder,
            variables: &mut variables,
            var_index: &mut var_index,
            func_returns,
            pending_closures: &mut pending_closures,
            is_global_context: false,
        };
        for stmt_id in body {
            let (val, term) = translator.translate_stmt(*stmt_id)?;
            last_val = Some(val);
            if term {
                terminated = true;
                break;
            }
        }

        // Implicit return if block isn't terminated
        if !terminated {
            let ret = last_val.unwrap_or_else(|| builder.ins().iconst(types::I64, 0));

            // Release all active local object variables
            for (var, ty) in variables.values() {
                if matches!(ty, crate::translator::VarType::Object(_)) {
                    let obj_val = builder.use_var(*var);
                    let release_id = *self
                        .context
                        .funcs
                        .get(&ustr::Ustr::from("release"))
                        .unwrap_or_else(|| panic!("release not found"));
                    let local_release = self
                        .context
                        .module
                        .declare_func_in_func(release_id, builder.func);
                    builder.ins().call(local_release, &[obj_val]);
                }
            }

            builder.ins().return_(&[ret]);
        }

        builder.finalize(self.context.module.target_config());

        self.context
            .module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| {
                println!("Cranelift Verifier Error in function {}: {:?}", _name, e);
                std::fs::write("ir.txt", self.ctx.func.display().to_string()).unwrap();
                CodegenError {
                    message: e.to_string(),
                }
            })?;

        self.context.module.clear_context(&mut self.ctx);

        for (fn_name, expr, captured_vars) in pending_closures.into_iter() {
            self.compile_closure(arena, &fn_name, expr, captured_vars, func_returns, current_class)?;
        }

        Ok(())
    }

    fn compile_closure(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        fn_name: &str,
        expr: pace_ast::Expr,
        captured_vars: Vec<(ustr::Ustr, crate::translator::VarType)>,
        func_returns: &HashMap<ustr::Ustr, crate::translator::VarType>,
        current_class: Option<&str>,
    ) -> Result<(), CodegenError> {
        let (params, body) = match &expr {
            pace_ast::Expr::Closure { params, body, .. } => (params.clone(), *body),
            _ => {
                return Err(CodegenError {
                    message: "Invalid closure expression".to_string(),
                });
            }
        };

        let mut sig = self.context.module.make_signature();
        sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
        sig.params.push(AbiParam::new(
            self.context.module.target_config().pointer_type(),
        )); // env pointer
        for _ in &params {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I64)); // Assume returning I64 for now

        let func_id = self
            .context
            .module
            .declare_function(fn_name, Linkage::Export, &sig)
            .unwrap();

        self.ctx.func.signature = sig;

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let env_ptr = builder.block_params(entry_block)[0];

        let mut variables: std::collections::HashMap<ustr::Ustr, (cranelift::prelude::Variable, crate::translator::VarType)> = std::collections::HashMap::new();
        let mut var_index = 0;

        // Load captured variables from environment
        for (i, (name, ty)) in captured_vars.iter().enumerate() {
            let offset = 16 + (i * 8);
            let val = builder.ins().load(
                types::I64,
                cranelift::prelude::MemFlagsData::new(),
                env_ptr,
                offset as i32,
            );
            let var = builder.declare_var(types::I64);
            builder.def_var(var, val);
            variables.insert(*name, (var, ty.clone()));
            var_index += 1;
        }

        // Declare closure parameters as variables
        for (i, param) in params.iter().enumerate() {
            let val = builder.block_params(entry_block)[i + 1]; // +1 because env_ptr is at 0
            let var = builder.declare_var(types::I64);
            builder.def_var(var, val);
            let param_ty = crate::translator::parse_type_annotation(
                &param.1,
                current_class,
                Some(&self.context.struct_layouts),
                Some(&self.context.enum_layouts),
            );
            variables.insert(ustr::Ustr::from(&param.0), (var, param_ty));
            var_index += 1;
        }

        let mut terminated = false;
        let mut pending_closures: Vec<(
            ustr::Ustr,
            pace_ast::Expr,
            Vec<(ustr::Ustr, crate::translator::VarType)>,
        )> = Vec::new();
        let body_stmt_id = arena.alloc_stmt(pace_ast::Stmt::Expr(body));
        
        let mut translator = Translator {
            arena,
            context: &mut self.context,
            builder: &mut builder,
            variables: &mut variables,
            var_index: &mut var_index,
            func_returns,
            pending_closures: &mut pending_closures,
            is_global_context: false,
        };

        let (val, term) = translator.translate_stmt(body_stmt_id)?;
        let last_val = Some(val);
        if term {
            terminated = true;
        }

        if !terminated {
            let ret = last_val.unwrap_or_else(|| builder.ins().iconst(types::I64, 0));
            builder.ins().return_(&[ret]);
        }

        builder.finalize(self.context.module.target_config());
        self.context
            .module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| CodegenError {
                message: e.to_string(),
            })?;
        self.context.module.clear_context(&mut self.ctx);

        // Recursively compile any nested closures
        for (nested_fn, nested_expr, nested_captured) in pending_closures {
            self.compile_closure(
                arena,
                &nested_fn,
                nested_expr,
                nested_captured,
                func_returns,
                current_class,
            )?;
        }

        Ok(())
    }
}
