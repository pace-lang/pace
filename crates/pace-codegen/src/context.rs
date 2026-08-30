use crate::layouts::{ClassLayout, EnumLayout, InterfaceLayout, StructLayout};
use cranelift_module::{FuncId, Module};
use std::collections::HashMap;

pub struct CodegenContext<M: Module> {
    pub module: M,
    pub funcs: HashMap<ustr::Ustr, FuncId>,
    pub class_layouts: HashMap<ustr::Ustr, ClassLayout>,
    pub struct_layouts: HashMap<ustr::Ustr, StructLayout>,
    pub interface_layouts: HashMap<ustr::Ustr, InterfaceLayout>,
    pub enum_layouts: HashMap<ustr::Ustr, EnumLayout>,
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
            class_layouts: HashMap::new(),
            struct_layouts: HashMap::new(),
            interface_layouts: HashMap::new(),
            enum_layouts: HashMap::new(),
            string_cache: HashMap::new(),
            string_id: 0,
            global_vars: HashMap::new(),
            closure_id: 0,
        }
    }
}
