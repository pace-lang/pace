use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    Null,
    /// A custom type like a Class or Struct (e.g. `UserService`)
    Custom(String),
    Unknown, // Used for auto-inference before resolution or error state
    Void,    // Used for functions that don't return anything
    Any,     // Used for built-ins like print that take multiple types
}

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub params: Vec<Type>,
    pub return_type: Type,
}

#[derive(Debug, Clone)]
pub struct ClassSignature {
    pub fields: HashMap<String, Type>,
    pub methods: HashMap<String, FunctionSignature>,
}

#[derive(Clone, Default)]
pub struct Environment {
    scopes: Vec<HashMap<String, Type>>,
    pub functions: HashMap<String, FunctionSignature>,
    pub classes: HashMap<String, ClassSignature>,
}

impl Environment {
    pub fn new() -> Self {
        let mut e = Self {
            scopes: vec![HashMap::new()], // Start with a global scope
            functions: HashMap::new(),
            classes: HashMap::new(),
        };
        e.inject_prelude();
        e
    }

    fn inject_prelude(&mut self) {
        // Inject built-in print function
        self.register_function(
            "print".to_string(),
            FunctionSignature {
                params: vec![Type::Any], // Accept any type
                return_type: Type::Void,
            },
        );
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn define(&mut self, name: String, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    pub fn get(&self, name: &str) -> Option<&Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }
    
    pub fn register_function(&mut self, name: String, sig: FunctionSignature) {
        self.functions.insert(name.clone(), sig);
        self.define(name, Type::Custom("Function".to_string()));
    }
    
    pub fn register_class(&mut self, name: String, sig: ClassSignature) {
        self.classes.insert(name.clone(), sig);
        self.define(name.clone(), Type::Custom(name));
    }
}
