use crate::context::CodegenContext;
use cranelift::prelude::*;
use cranelift_module::Module;
use cranelift_object::{ObjectBuilder, ObjectModule};

pub struct AotCompiler {
    pub context: CodegenContext<ObjectModule>,
    pub builder_context: cranelift::prelude::FunctionBuilderContext,
    pub ctx: cranelift::codegen::Context,
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
        flag_builder
            .set("preserve_frame_pointers", "false")
            .unwrap();
        flag_builder.set("opt_level", &opt_level).unwrap();
        flag_builder.set("is_pic", "true").unwrap(); // Need PIC for AOT compilation
        
        if opt_level == "speed_and_size" || opt_level == "speed" {
            let _ = flag_builder.set("enable_alias_analysis", "true");
            let _ = flag_builder.set("enable_simd", "true");
            let _ = flag_builder.set("enable_llvm_abi_extensions", "true");
            let _ = flag_builder.set("enable_safepoints", "false");
            let _ = flag_builder.set("unwind_info", "false");
        }

        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });

        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .unwrap();

        let builder = ObjectBuilder::new(
            isa,
            "pace_module",
            cranelift_module::default_libcall_names(),
        )
        .unwrap();

        let module = ObjectModule::new(builder);

        Self {
            context: CodegenContext::new(module),
            builder_context: cranelift::prelude::FunctionBuilderContext::new(),
            ctx: cranelift::codegen::Context::new(),
        }
    }

    pub fn compile_mir_to_object(
        mut self,
        mir: &pace_mir::MirProgram,
    ) -> Result<Vec<u8>, crate::layouts::CodegenError> {
        crate::translator::mir::compile_mir_program(&mut self.context, &mut self.builder_context, &mut self.ctx, mir)?;
        
        let product = self.context.module.finish();
        let bytes = product.emit().map_err(|e| crate::layouts::CodegenError {
            message: e.to_string(),
        })?;
        Ok(bytes)
    }
}
