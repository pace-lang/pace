use cranelift_module::{DataDescription, Linkage, Module};
use cranelift_object::ObjectModule;
use std::collections::HashMap;

pub fn declare_class_metadata(
    module: &mut ObjectModule,
    program: &mir::Program,
    func_ids: &HashMap<String, cranelift_module::FuncId>,
) -> Result<HashMap<String, cranelift_module::DataId>, String> {
    let mut class_metadata_ids = HashMap::new();

    for (class_name, class_def) in &program.classes {
        let mut data_ctx = DataDescription::new();
        data_ctx.set_align(8);
        let mut metadata_bytes = Vec::new();

        // 0: deinit_fn (8 bytes, set via relocation later if exists)
        metadata_bytes.extend_from_slice(&0u64.to_le_bytes());

        // 8: mailbox_offset (uint64_t)
        let mailbox_offset: u64 = if class_def.is_actor {
            let mb_idx = class_def.fields.iter().position(|f| f == "__mailbox").unwrap();
            24 + mb_idx as u64 * 8
        } else {
            0
        };
        metadata_bytes.extend_from_slice(&mailbox_offset.to_le_bytes());

        // 16: field_count (uint64_t)
        metadata_bytes
            .extend_from_slice(&(class_def.reference_fields.len() as u64).to_le_bytes());

        // 24+: field_offsets (uint64_t[])
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
            .map_err(|e| format!("Failed to declare class metadata for {}: {}", class_name, e))?;

        module.define_data(data_id, &data_ctx).map_err(|e| e.to_string())?;
        class_metadata_ids.insert(class_name.clone(), data_id);

        for static_field in &class_def.static_fields {
            let mut static_ctx = DataDescription::new();
            static_ctx.set_align(8);
            static_ctx.define(Box::new([0u8; 8]));
            let static_id = module
                .declare_data(
                    &format!("_pace_static_{}_{}", class_name, static_field),
                    Linkage::Export,
                    true,
                    false,
                )
                .map_err(|e| e.to_string())?;
            module.define_data(static_id, &static_ctx).map_err(|e| e.to_string())?;
        }
    }

    Ok(class_metadata_ids)
}

pub fn declare_enum_metadata(
    module: &mut ObjectModule,
    program: &mir::Program,
) -> Result<HashMap<(String, usize), cranelift_module::DataId>, String> {
    let mut enum_metadata_ids = HashMap::new();
    for (enum_name, enum_def) in &program.enums {
        for (variant_idx, variant_def) in enum_def.variants.iter().enumerate() {
            let mut data_ctx = DataDescription::new();
            data_ctx.set_align(8);
            let mut metadata_bytes = Vec::new();

            // 0: deinit_fn (Enums don't have deinit, so it's always 0)
            metadata_bytes.extend_from_slice(&0u64.to_le_bytes());

            let mut all_refs: std::collections::HashSet<usize> = variant_def.reference_payloads.clone();
            for &idx in variant_def.struct_payloads.keys() {
                all_refs.insert(idx);
            }
            
            // 8: mailbox_offset (uint64_t) - Enums are never actors
            metadata_bytes.extend_from_slice(&0u64.to_le_bytes());

            // 16: field_count (uint64_t)
            metadata_bytes.extend_from_slice(
                &(all_refs.len() as u64).to_le_bytes(),
            );

            let mut sorted_refs: Vec<usize> = all_refs.into_iter().collect();
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
                .map_err(|e| e.to_string())?;

            module.define_data(data_id, &data_ctx).map_err(|e| e.to_string())?;
            enum_metadata_ids.insert((enum_name.clone(), variant_idx), data_id);
        }
    }
    
    Ok(enum_metadata_ids)
}
