mod decl;
mod expr;
mod generics;
mod program;
mod stmt;
mod types;

pub use generics::substitute_type;

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedField {
    pub name: String,
    pub ty: crate::ty::Ty,
    pub is_mut: bool,
    pub is_private: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VTableEntry {
    pub name: String,
    pub ty: crate::ty::Ty,
    pub mangled_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedMethod {
    pub name: String,
    pub ty: crate::ty::Ty,
    pub mangled_name: String,
    pub is_override: bool,
    pub is_private: bool,
    pub span: pace_span::Span,
}

use pace_hir::{Decl, HirId};
use std::collections::{HashMap, HashSet};

use crate::ty::Ty;

use pace_errors::Reporter;

#[derive(Debug, Clone, Default)]
pub struct ModuleScope {
    pub visible_symbols: HashMap<String, String>, // e.g. "test_operators" -> "operators::test_operators"
    pub private_symbols: HashSet<String>,
    pub aliases: HashMap<String, String>,         // e.g. "op" -> "operators"
    pub wildcard_imports: Vec<String>,
}

pub struct TypeChecker {
    pub env: HashMap<HirId, Ty>,
    pub mutability_env: HashMap<HirId, bool>, // true = mutable, false = immutable
    pub loop_depth: usize,
    pub next_id: u32,
    pub methods_env: HashMap<String, Ty>,
    pub methods_privacy: HashMap<String, bool>,
    pub global_functions: HashMap<String, Ty>,
    pub global_vars: HashMap<String, Ty>,
    pub struct_defs: HashMap<HirId, Vec<ResolvedField>>,
    pub class_defs: HashMap<HirId, Vec<ResolvedField>>,
    pub class_parents: HashMap<HirId, HirId>,
    pub class_vtables: HashMap<HirId, Vec<VTableEntry>>,
    pub enum_defs: HashMap<HirId, Vec<pace_hir::EnumVariant>>,
    pub trait_defs: HashMap<String, Decl>,
    pub static_fields_env: HashMap<String, Ty>,
    pub static_fields_privacy: HashMap<String, bool>, // format: "{class_name}_{field_name}"
    pub const_env: HashMap<String, Ty>,
    pub named_types: HashMap<String, (HirId, u8)>, // maps mangled name to (HirId, 0=struct, 1=class, 2=enum)
    pub display_names: HashMap<String, String>,
    pub generic_templates: HashMap<String, Decl>,
    pub generic_templates_by_id: HashMap<HirId, String>,
    pub implemented_traits: HashMap<HirId, Vec<String>>,
    pub current_expected_ty: Option<Ty>,
    pub current_return_ty: Option<Ty>,
    pub current_fn_name: Option<String>,
    pub current_module: Option<String>,
    pub module_scopes: HashMap<String, ModuleScope>,
    pub resolved_global_names: HashMap<HirId, String>,
    pub declared_bindings: Vec<(HirId, String, pace_span::Span)>,
    pub used_bindings: std::collections::HashSet<HirId>,
    pub used_bindings_by_name: std::collections::HashSet<String>,
    pub initialized_bindings: std::collections::HashSet<HirId>,
    pub local_types: HashMap<HirId, Ty>, // Persisted types of all variables
    pub instantiated_generics: Vec<Decl>,
    pub symbol_references: HashMap<HirId, Vec<pace_span::Span>>,
    pub inlay_hints: Vec<(pace_span::Span, String)>,
    pub function_calls: Vec<(pace_span::Span, Ty)>,
    pub hover_info: HashMap<pace_span::Span, String>,
    pub definition_info: HashMap<pace_span::Span, pace_span::Span>,
    pub global_spans: HashMap<String, pace_span::Span>,
    pub reporter: Reporter,
    pub is_lib: bool,
    pub in_async_context: bool,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
            mutability_env: HashMap::new(),
            methods_env: HashMap::new(),
            methods_privacy: HashMap::new(),
            global_functions: HashMap::new(),
            global_vars: HashMap::new(),
            struct_defs: HashMap::new(),
            class_defs: HashMap::new(),
            class_parents: HashMap::new(),
            class_vtables: HashMap::new(),
            enum_defs: HashMap::new(),
            trait_defs: HashMap::new(),
            static_fields_env: HashMap::new(),
            static_fields_privacy: HashMap::new(),
            const_env: HashMap::new(),
            named_types: HashMap::new(),
            display_names: HashMap::new(),
            generic_templates: HashMap::new(),
            generic_templates_by_id: HashMap::new(),
            implemented_traits: HashMap::new(),
            current_expected_ty: None,
            current_fn_name: None,
            current_return_ty: None,
            current_module: None,
            module_scopes: HashMap::new(),
            resolved_global_names: HashMap::new(),
            declared_bindings: Vec::new(),
            used_bindings: std::collections::HashSet::new(),
            used_bindings_by_name: std::collections::HashSet::new(),
            initialized_bindings: std::collections::HashSet::new(),
            local_types: HashMap::new(),
            instantiated_generics: Vec::new(),
            symbol_references: HashMap::new(),
            inlay_hints: Vec::new(),
            function_calls: Vec::new(),
            hover_info: HashMap::new(),
            definition_info: HashMap::new(),
            global_spans: HashMap::new(),
            loop_depth: 0,
            next_id: 1000000,
            reporter: Reporter::new(),
            is_lib: false,
            in_async_context: false,
        }
    }

    pub fn generate_id(&mut self) -> pace_hir::HirId {
        let id = self.next_id;
        self.next_id += 1;
        pace_hir::HirId(id)
    }

}
