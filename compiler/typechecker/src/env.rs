use std::collections::HashMap;
use ast::types::Type;
use session::Symbol;

#[derive(Debug, Clone)]
pub struct Binding {
    pub ty: Type,
    pub is_mutable: bool,
}

#[derive(Debug)]
pub struct TypeEnvironment {
    bindings: HashMap<Symbol, Vec<Binding>>,
    scope_decls: Vec<Vec<Symbol>>,
}

impl Default for TypeEnvironment {
    fn default() -> Self {
        panic!("TypeEnvironment requires a global scope to be initialized manually. Use TypeEnvironment::new(global_print_symbol)");
    }
}

impl TypeEnvironment {
    pub fn new(global_print_sym: Symbol) -> Self {
        let mut env = Self {
            bindings: HashMap::new(),
            scope_decls: vec![Vec::new()], // Global scope
        };
        // Insert global print function
        env.declare(global_print_sym, Type::BuiltinFunc);
        env
    }

    pub fn push_scope(&mut self) {
        self.scope_decls.push(Vec::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scope_decls.len() > 1 {
            let decls = self.scope_decls.pop().unwrap();
            for sym in decls {
                if let Some(stack) = self.bindings.get_mut(&sym) {
                    stack.pop();
                    if stack.is_empty() {
                        self.bindings.remove(&sym);
                    }
                }
            }
        } else {
            panic!("Cannot pop the global scope.");
        }
    }

    pub fn declare(&mut self, name: Symbol, ty: Type) {
        self.declare_var(name, ty, false);
    }

    pub fn declare_var(&mut self, name: Symbol, ty: Type, is_mutable: bool) {
        self.bindings.entry(name).or_default().push(Binding { ty, is_mutable });
        self.scope_decls.last_mut().unwrap().push(name);
    }

    pub fn resolve(&self, name: Symbol) -> Option<Type> {
        self.bindings.get(&name).and_then(|stack| stack.last()).map(|b| b.ty.clone())
    }

    pub fn is_mutable(&self, name: Symbol) -> bool {
        self.bindings.get(&name).and_then(|stack| stack.last()).map(|b| b.is_mutable).unwrap_or(false)
    }
}
