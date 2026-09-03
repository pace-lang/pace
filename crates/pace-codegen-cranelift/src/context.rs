use cranelift_module::{FuncId, Module};
use std::collections::HashMap;

pub struct CodegenContext<M: Module> {
    pub module: M,
    pub funcs: HashMap<ustr::Ustr, FuncId>,
    pub string_cache: HashMap<ustr::Ustr, String>,
    pub string_id: usize,
    pub global_vars: HashMap<ustr::Ustr, cranelift_module::DataId>,
    pub closure_id: usize,
}

impl<M: Module> CodegenContext<M> {
    pub fn new(mut module: M) -> Self {
        let ptr_ty = module.target_config().pointer_type();
        let funcs = crate::runtime::declare_runtime_functions(&mut module, ptr_ty);

        Self {
            module,
            funcs,
            string_cache: HashMap::new(),
            string_id: 0,
            global_vars: HashMap::new(),
            closure_id: 0,
        }
    }
}
