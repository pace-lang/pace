use cranelift_module::{Module, FuncId};
use std::collections::HashMap;
use crate::layouts::{ClassLayout, StructLayout, InterfaceLayout, EnumLayout};

pub struct CodegenContext<M: Module> {
    pub module: M,
    pub funcs: HashMap<String, FuncId>,
    pub class_layouts: HashMap<String, ClassLayout>,
    pub struct_layouts: HashMap<String, StructLayout>,
    pub interface_layouts: HashMap<String, InterfaceLayout>,
    pub enum_layouts: HashMap<String, EnumLayout>,
    pub string_cache: HashMap<String, String>,
    pub string_id: usize,
    pub global_vars: HashMap<String, cranelift_module::DataId>,
}

impl<M: Module> CodegenContext<M> {
    pub fn new(mut module: M) -> Self {
        let ptr_ty = module.target_config().pointer_type();
        let funcs = crate::runtime::declare_runtime_functions(&mut module, ptr_ty);
        
        Self {
            module,
            funcs,
            class_layouts: HashMap::new(),
            struct_layouts: HashMap::new(),
            interface_layouts: HashMap::new(),
            enum_layouts: HashMap::new(),
            string_cache: HashMap::new(),
            string_id: 0,
            global_vars: HashMap::new(),
        }
    }
}
