use std::path::Path;
use std::fs::File;
use std::io::Write;

mod translator;


use cranelift_codegen::{
    ir::{types, InstBuilder, AbiParam},
    settings::{self, Configurable},
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{Linkage, Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};
use cranelift_native;

pub struct CraneliftGenerator;

impl CraneliftGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn compile_program(&self, program: &mir::Program, output_file: &Path) -> Result<(), String> {
        let mut flag_builder = settings::builder();
        flag_builder.set("is_pic", "true").unwrap();
        flag_builder.set("opt_level", "speed_and_size").unwrap();
        let isa_builder = cranelift_native::builder().map_err(|e| format!("Failed to create native ISA builder: {}", e))?;
        let isa = isa_builder.finish(settings::Flags::new(flag_builder)).map_err(|e| format!("Failed to finish ISA: {}", e))?;
        
        let builder = ObjectBuilder::new(
            isa,
            "pace_module",
            default_libcall_names(),
        ).map_err(|e| format!("Failed to create ObjectBuilder: {}", e))?;
        
        let mut module = ObjectModule::new(builder);
        
        let mut ctx = module.make_context();
        let mut builder_context = FunctionBuilderContext::new();

        use std::collections::HashMap;
        
        let mut func_ids = HashMap::new();
        
        // 1. Declare all functions
        for (name, func) in &program.functions {
            let mut sig = module.make_signature();
            for _ in &func.parameters {
                sig.params.push(AbiParam::new(types::I64));
            }
            sig.returns.push(AbiParam::new(types::I64));
            
            let func_id = module.declare_function(name, Linkage::Export, &sig)
                .map_err(|e| format!("Failed to declare {}: {}", name, e))?;
            func_ids.insert(name.clone(), func_id);
        }
        
        // Declare runtime functions
        let mut print_sig = module.make_signature();
        print_sig.params.push(AbiParam::new(types::I64));
        print_sig.returns.push(AbiParam::new(types::I64));
        let print_id = module.declare_function("pace_print", Linkage::Import, &print_sig)
            .map_err(|e| format!("Failed to declare pace_print: {}", e))?;
        func_ids.insert("pace_print".to_string(), print_id);

        let mut alloc_sig = module.make_signature();
        alloc_sig.params.push(AbiParam::new(types::I64));
        alloc_sig.returns.push(AbiParam::new(types::I64)); // returns pointer as I64
        let alloc_id = module.declare_function("pace_alloc", Linkage::Import, &alloc_sig)
            .map_err(|e| format!("Failed to declare pace_alloc: {}", e))?;
        func_ids.insert("pace_alloc".to_string(), alloc_id);

        let mut retain_sig = module.make_signature();
        retain_sig.params.push(AbiParam::new(types::I64)); // obj pointer
        let retain_id = module.declare_function("pace_retain", Linkage::Import, &retain_sig)
            .map_err(|e| format!("Failed to declare pace_retain: {}", e))?;
        func_ids.insert("pace_retain".to_string(), retain_id);

        let mut release_sig = module.make_signature();
        release_sig.params.push(AbiParam::new(types::I64)); // obj pointer
        let release_id = module.declare_function("pace_release", Linkage::Import, &release_sig)
            .map_err(|e| format!("Failed to declare pace_release: {}", e))?;
        func_ids.insert("pace_release".to_string(), release_id);

        let mut weak_retain_sig = module.make_signature();
        weak_retain_sig.params.push(AbiParam::new(types::I64));
        let weak_retain_id = module.declare_function("pace_weak_retain", Linkage::Import, &weak_retain_sig).unwrap();
        func_ids.insert("pace_weak_retain".to_string(), weak_retain_id);

        let mut weak_release_sig = module.make_signature();
        weak_release_sig.params.push(AbiParam::new(types::I64));
        let weak_release_id = module.declare_function("pace_weak_release", Linkage::Import, &weak_release_sig).unwrap();
        func_ids.insert("pace_weak_release".to_string(), weak_release_id);

        let mut weak_upgrade_sig = module.make_signature();
        weak_upgrade_sig.params.push(AbiParam::new(types::I64));
        weak_upgrade_sig.returns.push(AbiParam::new(types::I64));
        let weak_upgrade_id = module.declare_function("pace_weak_upgrade", Linkage::Import, &weak_upgrade_sig).unwrap();
        func_ids.insert("pace_weak_upgrade".to_string(), weak_upgrade_id);

        // 2. Define all functions
        for (name, func) in &program.functions {
            ctx.func.signature.clear(module.isa().default_call_conv());
            for _ in &func.parameters {
                ctx.func.signature.params.push(AbiParam::new(types::I64));
            }
            ctx.func.signature.returns.push(AbiParam::new(types::I64));
            
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
            
            let mut translator = crate::translator::Translator::new(&mut builder, &mut module, program, &func_ids);
            translator.translate(func)?;
            
            builder.finalize(module.target_config());
            
            let func_id = *func_ids.get(name).unwrap();
            module.define_function(func_id, &mut ctx)
                .map_err(|e| format!("Failed to define {}: {}", name, e))?;
                
            module.clear_context(&mut ctx);
        }

        let object_product = module.finish();
        let bytes = object_product.emit().map_err(|e| format!("Failed to emit object: {}", e))?;
        
        let mut file = File::create(output_file).map_err(|e| format!("Failed to create object file: {}", e))?;
        file.write_all(&bytes).map_err(|e| format!("Failed to write object file: {}", e))?;

        Ok(())
    }
}

