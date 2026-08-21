use cranelift_codegen::ir::{AbiParam, types};
use cranelift_module::{Linkage, Module};
use cranelift_object::ObjectModule;
use mir::ForeignAbiType;
use std::collections::HashMap;

pub fn translate_abi_type(ty: &ForeignAbiType) -> cranelift_codegen::ir::Type {
    match ty {
        ForeignAbiType::I8 => types::I8,
        ForeignAbiType::I16 => types::I16,
        ForeignAbiType::I32 => types::I32,
        ForeignAbiType::I64 => types::I64,
        ForeignAbiType::F32 => types::F32,
        ForeignAbiType::F64 => types::F64,
        ForeignAbiType::Pointer => types::I64,
    }
}

pub fn declare_all_functions(
    module: &mut ObjectModule,
    program: &mir::Program,
) -> Result<HashMap<String, cranelift_module::FuncId>, String> {
    let mut func_ids = HashMap::new();

    // 1. Declare all user functions
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

    // 2. Declare User FFI functions
    let intrinsics = [
        "hash", "equals", "sizeof", "ptrRead", "ptrWrite", "arrayGet", "arraySet", "arrayLen", "paceNullPointer",
        "bitwiseAnd", "bitwiseOr", "bitwiseXor", "bitwiseNot", "bitwiseShl", "bitwiseShr",
        "paceRetainRef", "paceReleaseRef"
    ];

    for (name, foreign_func) in &program.foreign_functions {
        let symbol_name = foreign_func.symbol.as_str();
        let is_intrinsic = intrinsics.iter().any(|i| symbol_name == *i || symbol_name.starts_with(&format!("{}_", i)));
        if is_intrinsic {
            continue;
        }

        let mut sig = module.make_signature();
        for param_ty in &foreign_func.param_types {
            sig.params.push(AbiParam::new(translate_abi_type(param_ty)));
        }
        if let Some(ret_ty) = &foreign_func.return_type {
            sig.returns.push(AbiParam::new(translate_abi_type(ret_ty)));
        }

        let func_id = module
            .declare_function(&foreign_func.symbol, Linkage::Import, &sig)
            .map_err(|e| format!("Failed to declare foreign func {}: {}", name, e))?;
        func_ids.insert(name.clone(), func_id);
    }

    // 3. Declare Runtime Functions
    declare_runtime_functions(module, &mut func_ids)?;

    Ok(func_ids)
}

fn declare_runtime_functions(
    module: &mut ObjectModule,
    func_ids: &mut HashMap<String, cranelift_module::FuncId>,
) -> Result<(), String> {
    let mut alloc_sig = module.make_signature();
    alloc_sig.params.push(AbiParam::new(types::I64));
    alloc_sig.params.push(AbiParam::new(types::I64));
    alloc_sig.returns.push(AbiParam::new(types::I64));
    let alloc_id = module.declare_function("pace_alloc", Linkage::Import, &alloc_sig).map_err(|e| e.to_string())?;
    func_ids.insert("pace_alloc".to_string(), alloc_id);

    let mut alloc_repeat_sig = module.make_signature();
    alloc_repeat_sig.params.push(AbiParam::new(types::I64));
    alloc_repeat_sig.params.push(AbiParam::new(types::I64));
    alloc_repeat_sig.params.push(AbiParam::new(types::I64));
    alloc_repeat_sig.returns.push(AbiParam::new(types::I64));
    let alloc_repeat_id = module.declare_function("pace_alloc_array_repeat", Linkage::Import, &alloc_repeat_sig).map_err(|e| e.to_string())?;
    func_ids.insert("pace_alloc_array_repeat".to_string(), alloc_repeat_id);

    let mut spawn_sig = module.make_signature();
    spawn_sig.params.push(AbiParam::new(types::I64));
    let spawn_id = module.declare_function("pace_spawn_task", Linkage::Import, &spawn_sig).unwrap();
    func_ids.insert("pace_spawn_task".to_string(), spawn_id);

    let mut actor_mb_create_sig = module.make_signature();
    actor_mb_create_sig.params.push(AbiParam::new(types::I64));
    actor_mb_create_sig.returns.push(AbiParam::new(types::I64));
    let actor_mb_create_id = module.declare_function("pace_actor_mailbox_create", Linkage::Import, &actor_mb_create_sig).unwrap();
    func_ids.insert("pace_actor_mailbox_create".to_string(), actor_mb_create_id);

    let mut actor_mb_push_sig = module.make_signature();
    actor_mb_push_sig.params.push(AbiParam::new(types::I64));
    actor_mb_push_sig.params.push(AbiParam::new(types::I64));
    let actor_mb_push_id = module.declare_function("pace_actor_mailbox_push", Linkage::Import, &actor_mb_push_sig).unwrap();
    func_ids.insert("pace_actor_mailbox_push".to_string(), actor_mb_push_id);

    let mut panic_sig = module.make_signature();
    panic_sig.params.push(AbiParam::new(types::I64));
    let panic_id = module.declare_function("pacePanic", Linkage::Import, &panic_sig).map_err(|e| e.to_string())?;
    func_ids.insert("pacePanic".to_string(), panic_id);

    let mut debug_ptr_sig = module.make_signature();
    debug_ptr_sig.params.push(AbiParam::new(types::I64));
    let debug_ptr_id = module.declare_function("debug_ptr", Linkage::Import, &debug_ptr_sig).map_err(|e| e.to_string())?;
    func_ids.insert("debug_ptr".to_string(), debug_ptr_id);

    let mut hash_str_sig = module.make_signature();
    hash_str_sig.params.push(AbiParam::new(types::I64));
    hash_str_sig.returns.push(AbiParam::new(types::I64));
    let hash_str_id = module.declare_function("hash_String", Linkage::Import, &hash_str_sig).unwrap();
    func_ids.insert("hash_String".to_string(), hash_str_id);

    let mut equals_str_sig = module.make_signature();
    equals_str_sig.params.push(AbiParam::new(types::I64));
    equals_str_sig.params.push(AbiParam::new(types::I64));
    equals_str_sig.returns.push(AbiParam::new(types::I64));
    let equals_str_id = module.declare_function("equals_String", Linkage::Import, &equals_str_sig).unwrap();
    func_ids.insert("equals_String".to_string(), equals_str_id);

    let mut retain_sig = module.make_signature();
    retain_sig.params.push(AbiParam::new(types::I64));
    let retain_id = module.declare_function("pace_retain", Linkage::Import, &retain_sig).map_err(|e| e.to_string())?;
    func_ids.insert("pace_retain".to_string(), retain_id);

    let mut release_sig = module.make_signature();
    release_sig.params.push(AbiParam::new(types::I64));
    let release_id = module.declare_function("pace_release", Linkage::Import, &release_sig).map_err(|e| e.to_string())?;
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

    let mut str_concat_sig = module.make_signature();
    str_concat_sig.params.push(AbiParam::new(types::I64));
    str_concat_sig.params.push(AbiParam::new(types::I64));
    str_concat_sig.returns.push(AbiParam::new(types::I64));
    let str_concat_id = module.declare_function("pace_string_concat", Linkage::Import, &str_concat_sig).unwrap();
    func_ids.insert("pace_string_concat".to_string(), str_concat_id);

    let mut int_to_string_sig = module.make_signature();
    int_to_string_sig.params.push(AbiParam::new(types::I64));
    int_to_string_sig.returns.push(AbiParam::new(types::I64));
    let int_to_string_id = module.declare_function("pace_int_to_string", Linkage::Import, &int_to_string_sig).unwrap();
    func_ids.insert("pace_int_to_string".to_string(), int_to_string_id);

    let mut float_to_string_sig = module.make_signature();
    float_to_string_sig.params.push(AbiParam::new(types::F64));
    float_to_string_sig.returns.push(AbiParam::new(types::I64));
    let float_to_string_id = module.declare_function("pace_float_to_string", Linkage::Import, &float_to_string_sig).unwrap();
    func_ids.insert("pace_float_to_string".to_string(), float_to_string_id);

    let mut bool_to_string_sig = module.make_signature();
    bool_to_string_sig.params.push(AbiParam::new(types::I64));
    bool_to_string_sig.returns.push(AbiParam::new(types::I64));
    let bool_to_string_id = module.declare_function("pace_bool_to_string", Linkage::Import, &bool_to_string_sig).unwrap();
    func_ids.insert("pace_bool_to_string".to_string(), bool_to_string_id);

    Ok(())
}
