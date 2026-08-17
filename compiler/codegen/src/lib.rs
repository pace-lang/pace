use std::fs::File;
use std::io::Write;
use std::path::Path;

mod translator;

use cranelift_codegen::{
    ir::{AbiParam, types},
    settings::{self, Configurable},
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{DataDescription, Linkage, Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};
use mir::ForeignAbiType;

pub struct CraneliftGenerator;

impl Default for CraneliftGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CraneliftGenerator {
    pub fn new() -> Self {
        Self
    }

    fn translate_abi_type(ty: &ForeignAbiType) -> cranelift_codegen::ir::Type {
        match ty {
            ForeignAbiType::I8 => types::I8,
            ForeignAbiType::I16 => types::I16,
            ForeignAbiType::I32 => types::I32,
            ForeignAbiType::I64 => types::I64,
            ForeignAbiType::F32 => types::F32,
            ForeignAbiType::F64 => types::F64,
            ForeignAbiType::Pointer => types::I64, // Pointers are 64-bit on our target
        }
    }

    pub fn compile_program(
        &self,
        program: &mir::Program,
        output_file: &Path,
        release: bool,
    ) -> Result<(), String> {
        let mut flag_builder = settings::builder();
        flag_builder.set("is_pic", "true").unwrap();

        if release {
            flag_builder.set("opt_level", "speed_and_size").unwrap();
        } else {
            flag_builder.set("opt_level", "none").unwrap();
        }
        let isa_builder = cranelift_native::builder()
            .map_err(|e| format!("Failed to create native ISA builder: {}", e))?;
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| format!("Failed to finish ISA: {}", e))?;

        let builder = ObjectBuilder::new(isa, "pace_module", default_libcall_names())
            .map_err(|e| format!("Failed to create ObjectBuilder: {}", e))?;

        let mut module = ObjectModule::new(builder);

        let mut ctx = module.make_context();
        let mut builder_context = FunctionBuilderContext::new();

        use std::collections::HashMap;

        let mut func_ids = HashMap::new();

        // 1. Declare all functions
        let mut sorted_functions: Vec<_> = program.functions.iter().collect();
        sorted_functions.sort_by_key(|(name, _)| *name);

        for (name, func) in sorted_functions {
            let mut sig = module.make_signature();
            for _ in &func.parameters {
                sig.params.push(AbiParam::new(types::I64));
            }
            sig.returns.push(AbiParam::new(types::I64));

            let symbol_name = if name == "main" {
                name.clone()
            } else {
                format!("pace_func_{}", name)
            };

            let func_id = module
                .declare_function(&symbol_name, Linkage::Export, &sig)
                .map_err(|e| format!("Failed to declare {}: {}", name, e))?;
            func_ids.insert(name.clone(), func_id);
        }

        // Declare User FFI functions
        for (name, foreign_func) in &program.foreign_functions {
            let mut sig = module.make_signature();
            for param_ty in &foreign_func.param_types {
                sig.params
                    .push(AbiParam::new(Self::translate_abi_type(param_ty)));
            }
            if let Some(ret_ty) = &foreign_func.return_type {
                sig.returns
                    .push(AbiParam::new(Self::translate_abi_type(ret_ty)));
            }

            let func_id = module
                .declare_function(&foreign_func.symbol, Linkage::Import, &sig)
                .map_err(|e| format!("Failed to declare foreign func {}: {}", name, e))?;
            func_ids.insert(name.clone(), func_id);
        }

        // Declare runtime functions

        let mut alloc_sig = module.make_signature();
        alloc_sig.params.push(AbiParam::new(types::I64));
        alloc_sig.params.push(AbiParam::new(types::I64)); // metadata_ptr
        alloc_sig.returns.push(AbiParam::new(types::I64)); // returns pointer as I64
        let alloc_id = module
            .declare_function("pace_alloc", Linkage::Import, &alloc_sig)
            .map_err(|e| format!("Failed to declare pace_alloc: {}", e))?;
        func_ids.insert("pace_alloc".to_string(), alloc_id);

        let mut alloc_repeat_sig = module.make_signature();
        alloc_repeat_sig.params.push(AbiParam::new(types::I64)); // count
        alloc_repeat_sig.params.push(AbiParam::new(types::I64)); // val
        alloc_repeat_sig.params.push(AbiParam::new(types::I64)); // metadata
        alloc_repeat_sig.returns.push(AbiParam::new(types::I64));
        let alloc_repeat_id = module
            .declare_function(
                "pace_alloc_array_repeat",
                Linkage::Import,
                &alloc_repeat_sig,
            )
            .map_err(|e| format!("Failed to declare pace_alloc_array_repeat: {}", e))?;
        func_ids.insert("pace_alloc_array_repeat".to_string(), alloc_repeat_id);

        let mut panic_sig = module.make_signature();
        panic_sig.params.push(AbiParam::new(types::I64));
        let panic_id = module
            .declare_function("pacePanic", Linkage::Import, &panic_sig)
            .map_err(|e| format!("Failed to declare pacePanic: {}", e))?;
        func_ids.insert("pacePanic".to_string(), panic_id);

        let mut retain_sig = module.make_signature();
        retain_sig.params.push(AbiParam::new(types::I64)); // obj pointer
        let retain_id = module
            .declare_function("pace_retain", Linkage::Import, &retain_sig)
            .map_err(|e| format!("Failed to declare pace_retain: {}", e))?;
        func_ids.insert("pace_retain".to_string(), retain_id);

        let mut release_sig = module.make_signature();
        release_sig.params.push(AbiParam::new(types::I64)); // obj pointer
        let release_id = module
            .declare_function("pace_release", Linkage::Import, &release_sig)
            .map_err(|e| format!("Failed to declare pace_release: {}", e))?;
        func_ids.insert("pace_release".to_string(), release_id);

        let mut weak_retain_sig = module.make_signature();
        weak_retain_sig.params.push(AbiParam::new(types::I64));
        let weak_retain_id = module
            .declare_function("pace_weak_retain", Linkage::Import, &weak_retain_sig)
            .unwrap();
        func_ids.insert("pace_weak_retain".to_string(), weak_retain_id);

        let mut weak_release_sig = module.make_signature();
        weak_release_sig.params.push(AbiParam::new(types::I64));
        let weak_release_id = module
            .declare_function("pace_weak_release", Linkage::Import, &weak_release_sig)
            .unwrap();
        func_ids.insert("pace_weak_release".to_string(), weak_release_id);

        let mut weak_upgrade_sig = module.make_signature();
        weak_upgrade_sig.params.push(AbiParam::new(types::I64));
        weak_upgrade_sig.returns.push(AbiParam::new(types::I64));
        let weak_upgrade_id = module
            .declare_function("pace_weak_upgrade", Linkage::Import, &weak_upgrade_sig)
            .unwrap();
        func_ids.insert("pace_weak_upgrade".to_string(), weak_upgrade_id);



        let mut str_concat_sig = module.make_signature();
        str_concat_sig.params.push(AbiParam::new(types::I64));
        str_concat_sig.params.push(AbiParam::new(types::I64));
        str_concat_sig.returns.push(AbiParam::new(types::I64));
        let str_concat_id = module
            .declare_function("pace_string_concat", Linkage::Import, &str_concat_sig)
            .unwrap();
        func_ids.insert("pace_string_concat".to_string(), str_concat_id);

        let mut int_to_string_sig = module.make_signature();
        int_to_string_sig.params.push(AbiParam::new(types::I64));
        int_to_string_sig.returns.push(AbiParam::new(types::I64));
        let int_to_string_id = module
            .declare_function("pace_int_to_string", Linkage::Import, &int_to_string_sig)
            .unwrap();
        func_ids.insert("pace_int_to_string".to_string(), int_to_string_id);

        let mut float_to_string_sig = module.make_signature();
        float_to_string_sig.params.push(AbiParam::new(types::F64));
        float_to_string_sig.returns.push(AbiParam::new(types::I64));
        let float_to_string_id = module
            .declare_function(
                "pace_float_to_string",
                Linkage::Import,
                &float_to_string_sig,
            )
            .unwrap();
        func_ids.insert("pace_float_to_string".to_string(), float_to_string_id);

        let mut bool_to_string_sig = module.make_signature();
        bool_to_string_sig.params.push(AbiParam::new(types::I64));
        bool_to_string_sig.returns.push(AbiParam::new(types::I64));
        let bool_to_string_id = module
            .declare_function("pace_bool_to_string", Linkage::Import, &bool_to_string_sig)
            .unwrap();
        func_ids.insert("pace_bool_to_string".to_string(), bool_to_string_id);

        let mut class_metadata_ids = HashMap::new();

        for (class_name, class_def) in &program.classes {
            let mut data_ctx = DataDescription::new();
            data_ctx.set_align(8);
            let mut metadata_bytes = Vec::new();

            // 0: deinit_fn (8 bytes, set via relocation later if exists)
            metadata_bytes.extend_from_slice(&0u64.to_le_bytes());

            // 8: field_count (uint64_t)
            metadata_bytes
                .extend_from_slice(&(class_def.reference_fields.len() as u64).to_le_bytes());

            // 16+: field_offsets (uint64_t[])
            for (idx, field_name) in class_def.fields.iter().enumerate() {
                if class_def.reference_fields.contains(field_name) {
                    let offset = 24 + idx * 8; // 24 bytes header + index * 8
                    metadata_bytes.extend_from_slice(&(offset as u64).to_le_bytes());
                }
            }

            data_ctx.define(metadata_bytes.into_boxed_slice());

            let deinit_name = format!("{}::deinit", class_name);
            if let Some(func_id) = func_ids.get(&deinit_name) {
                let func_ref = module.declare_func_in_data(*func_id, &mut data_ctx);
                data_ctx.write_function_addr(0, func_ref);
            }

            let data_id = module
                .declare_data(
                    &format!("_pace_metadata_{}", class_name),
                    Linkage::Local,
                    true,
                    false,
                )
                .unwrap();

            module.define_data(data_id, &data_ctx).unwrap();
            class_metadata_ids.insert(class_name.clone(), data_id);
        }

        let mut enum_metadata_ids = HashMap::new();
        for (enum_name, enum_def) in &program.enums {
            for (variant_idx, variant_def) in enum_def.variants.iter().enumerate() {
                let mut data_ctx = DataDescription::new();
                data_ctx.set_align(8);
                let mut metadata_bytes = Vec::new();

                // 0: deinit_fn (Enums don't have deinit, so it's always 0)
                metadata_bytes.extend_from_slice(&0u64.to_le_bytes());

                // 8: field_count (uint64_t)
                metadata_bytes.extend_from_slice(
                    &(variant_def.reference_payloads.len() as u64).to_le_bytes(),
                );

                let mut sorted_refs: Vec<usize> =
                    variant_def.reference_payloads.iter().cloned().collect();
                sorted_refs.sort();

                for idx in sorted_refs {
                    let offset = 32 + idx * 8; // 24 bytes header + 8 bytes tag + idx * 8
                    metadata_bytes.extend_from_slice(&(offset as u64).to_le_bytes());
                }

                data_ctx.define(metadata_bytes.into_boxed_slice());

                let data_id = module
                    .declare_data(
                        &format!("_pace_metadata_{}_{}", enum_name, variant_def.name),
                        Linkage::Local,
                        true,
                        false,
                    )
                    .unwrap();

                module.define_data(data_id, &data_ctx).unwrap();
                enum_metadata_ids.insert((enum_name.clone(), variant_idx), data_id);
            }
        }

        // 2. Define all functions
        let mut sorted_functions_def: Vec<_> = program.functions.iter().collect();
        sorted_functions_def.sort_by_key(|(name, _)| *name);

        for (name, func) in sorted_functions_def {
            ctx.func.signature.clear(module.isa().default_call_conv());
            for _ in &func.parameters {
                ctx.func.signature.params.push(AbiParam::new(types::I64));
            }
            ctx.func.signature.returns.push(AbiParam::new(types::I64));

            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);

            let mut translator = crate::translator::Translator::new(
                &mut builder,
                &mut module,
                program,
                &func_ids,
                &class_metadata_ids,
                &enum_metadata_ids,
            );
            translator.translate(func)?;

            builder.finalize(module.target_config());

            let func_id = *func_ids.get(name).unwrap();
            if let Err(e) = module.define_function(func_id, &mut ctx) {
                println!("Cranelift IR for {}:\n{}", name, ctx.func.display());
                return Err(format!("Failed to define {}: {}", name, e));
            }

            module.clear_context(&mut ctx);
        }

        let object_product = module.finish();
        let bytes = object_product
            .emit()
            .map_err(|e| format!("Failed to emit object: {}", e))?;

        let mut file = File::create(output_file)
            .map_err(|e| format!("Failed to create object file: {}", e))?;
        file.write_all(&bytes)
            .map_err(|e| format!("Failed to write object file: {}", e))?;

        Ok(())
    }
}
