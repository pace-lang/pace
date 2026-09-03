use crate::context::CodegenContext;
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};

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
        flag_builder
            .set("preserve_frame_pointers", "false")
            .unwrap();
        flag_builder.set("opt_level", &opt_level).unwrap();
        flag_builder.set("is_pic", "false").unwrap();

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

        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        crate::runtime_bindings::RuntimeBindings::register_all(&mut builder);

        let module = JITModule::new(builder);

        Self {
            context: CodegenContext::new(module),
            builder_context: cranelift::prelude::FunctionBuilderContext::new(),
            ctx: cranelift::codegen::Context::new(),
        }
    }

    pub fn compile_and_run_mir(
        &mut self,
        mir: &pace_mir::MirProgram,
    ) -> Result<(), crate::CodegenError> {
        crate::translator::mir::compile_mir_program(&mut self.context, &mut self.builder_context, &mut self.ctx, mir)?;

        self.context.module.finalize_definitions().unwrap();

        // Execute main block
        if let Some(main_id) = self.context.funcs.get(&ustr::Ustr::from("main")) {
            let code_ptr = self.context.module.get_finalized_function(*main_id);
            let main_func: extern "C" fn() -> i64 = unsafe { std::mem::transmute(code_ptr) };
            main_func();
        }

        Ok(())
    }
}
