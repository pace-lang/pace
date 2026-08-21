use std::fs::File;
use std::io::Write;
use std::path::Path;

mod translator;
mod declarations;
mod metadata;

use cranelift_codegen::{
    ir::{AbiParam, types},
    settings::{self, Configurable},
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};

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

        // 1. Declare all functions and FFIs
        let func_ids = declarations::declare_all_functions(&mut module, program)?;

        // 2. Declare Data / Metadata sections
        let class_metadata_ids = metadata::declare_class_metadata(&mut module, program, &func_ids)?;
        let enum_metadata_ids = metadata::declare_enum_metadata(&mut module, program)?;

        // 3. Define all functions
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
