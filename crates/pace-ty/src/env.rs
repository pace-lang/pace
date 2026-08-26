use pace_ast::Visibility;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    Null,
    /// A nullable wrapper around a type (e.g. `Int?`)
    Nullable(Box<Type>),
    /// A reference type (heap-allocated, ARC)
    Class(String),
    /// An actor type (isolated state, async messages)
    Actor(String),
    /// A value type (stack-allocated)
    Struct(String),
    /// An enum type
    Enum(String),
    /// A function type
    Function,
    Unknown, // Used for auto-inference before resolution or error state
    Void,    // Used for functions that don't return anything
    Any,     // Used for built-ins like print that take multiple types
    GenericParameter(String),
    GenericInstance { base: Box<Type>, args: Vec<Type> },
    /// An asynchronous value that resolves to the inner type
    Promise(Box<Type>),
}

#[derive(Debug, Clone)]
pub struct EnumSignature {
    pub generic_params: Option<Vec<String>>,
    pub variants: HashMap<String, Option<Vec<Type>>>,
}

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub params: Vec<Type>,
    pub return_type: Type,
    pub span: (usize, usize),
    pub is_used: bool,
    pub visibility: Visibility,
    pub module: String,
    pub generic_params: Option<Vec<String>>,
    pub is_static: bool,
}

#[derive(Debug, Clone)]
pub struct ActorSignature {
    pub generic_params: Option<Vec<String>>,
    pub fields: HashMap<String, Type>,
    pub static_fields: HashMap<String, Type>,
    pub methods: HashMap<String, FunctionSignature>,
}

#[derive(Debug, Clone)]
pub struct ClassSignature {
    pub generic_params: Option<Vec<String>>,
    pub fields: HashMap<String, Type>,
    pub static_fields: HashMap<String, Type>,
    pub methods: HashMap<String, FunctionSignature>,
}

#[derive(Debug, Clone)]
pub struct VarInfo {
    pub ty: Type,
    pub span: (usize, usize),
    pub is_used: bool,
    pub is_mutable: bool,
}

#[derive(Clone, Default)]
pub struct Environment {
    scopes: Vec<HashMap<String, VarInfo>>,
    pub functions: HashMap<String, FunctionSignature>,
    pub classes: HashMap<String, ClassSignature>,
    pub structs: HashMap<String, ClassSignature>,
    pub enums: HashMap<String, EnumSignature>,
    pub actors: HashMap<String, ActorSignature>,
}

impl Environment {
    pub fn new() -> Self {
        let mut e = Self {
            scopes: vec![HashMap::new()], // Start with a global scope
            functions: HashMap::new(),
            classes: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            actors: HashMap::new(),
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
                span: (0, 0),
                is_used: true, // Always consider built-ins used
                visibility: Visibility::Public,
                module: "std".to_string(),
                generic_params: None,
                is_static: false,
            },
        );
        // Inject built-in hash function
        self.register_function(
            "hash".to_string(),
            FunctionSignature {
                params: vec![Type::Any], // Accept any type
                return_type: Type::Int,
                span: (0, 0),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".to_string(),
                generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "malloc".to_string(),
            FunctionSignature {
                params: vec![Type::Int],
                return_type: Type::Int,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "free".to_string(),
            FunctionSignature {
                params: vec![Type::Int, Type::Int],
                return_type: Type::Void,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "ptr_store".to_string(),
            FunctionSignature {
                params: vec![Type::Int, Type::Int, Type::Any],
                return_type: Type::Void,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "ptr_load".to_string(),
            FunctionSignature {
                params: vec![Type::Int, Type::Int],
                return_type: Type::Any,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "time".to_string(),
            FunctionSignature {
                params: vec![Type::Int],
                return_type: Type::Int,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "sb_new".to_string(),
            FunctionSignature {
                params: vec![],
                return_type: Type::Int,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "sb_append".to_string(),
            FunctionSignature {
                params: vec![Type::Int, Type::String],
                return_type: Type::Void,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "sb_build".to_string(),
            FunctionSignature {
                params: vec![Type::Int],
                return_type: Type::String,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "sb_free".to_string(),
            FunctionSignature {
                params: vec![Type::Int],
                return_type: Type::Void,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None,
                is_static: false,
            },
        );
        // FS and HTTP FFI functions
        self.register_function(
            "fs_writeText".to_string(),
            FunctionSignature {
                params: vec![Type::String, Type::String],
                return_type: Type::Int, // Returns 1 on success, 0 on failure
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "fs_exists".to_string(),
            FunctionSignature {
                params: vec![Type::String],
                return_type: Type::Int,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "fs_readText".to_string(),
            FunctionSignature {
                params: vec![Type::String],
                return_type: Type::Nullable(Box::new(Type::String)),
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "http_get".to_string(),
            FunctionSignature {
                params: vec![Type::String],
                return_type: Type::Nullable(Box::new(Type::String)),
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "string_split".to_string(),
            FunctionSignature {
                params: vec![Type::String, Type::String],
                return_type: Type::Int, // Actually it returns Int pointer
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None, is_static: false,
            }
        );
        self.register_function(
            "string_replace".to_string(),
            FunctionSignature {
                params: vec![Type::String, Type::String, Type::String],
                return_type: Type::String,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None, is_static: false,
            }
        );
        self.register_function(
            "string_substring".to_string(),
            FunctionSignature {
                params: vec![Type::String, Type::Int, Type::Int],
                return_type: Type::String,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None, is_static: false,
            }
        );
        self.register_function(
            "string_trim".to_string(),
            FunctionSignature {
                params: vec![Type::String],
                return_type: Type::String,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None, is_static: false,
            }
        );
        self.register_function(
            "string_index_of".to_string(),
            FunctionSignature {
                params: vec![Type::String, Type::String],
                return_type: Type::Int,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None, is_static: false,
            }
        );
        self.register_function(
            "string_starts_with".to_string(),
            FunctionSignature {
                params: vec![Type::String, Type::String],
                return_type: Type::Int,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None, is_static: false,
            }
        );
        self.register_function(
            "get_last_error".to_string(),
            FunctionSignature {
                params: vec![],
                return_type: Type::String,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None, is_static: false,
            }
        );
        self.register_function(
            "get_year".to_string(),
            FunctionSignature {
                params: vec![Type::Int],
                return_type: Type::Int,
                span: (0, 0), is_used: true, visibility: Visibility::Public, module: "std".to_string(), generic_params: None,
                is_static: false,
            },
        );
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) -> Vec<(String, VarInfo)> {
        if self.scopes.len() > 1 {
            let scope = self.scopes.pop().unwrap();
            scope.into_iter().collect()
        } else {
            Vec::new()
        }
    }

    pub fn define(&mut self, name: String, ty: Type, span: (usize, usize), is_mutable: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, VarInfo { ty, span, is_used: false, is_mutable });
        }
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut VarInfo> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(var_info) = scope.get_mut(name) {
                return Some(var_info);
            }
        }
        None
    }

    pub fn get(&self, name: &str) -> Option<&Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(var_info) = scope.get(name) {
                return Some(&var_info.ty);
            }
        }
        None
    }

    pub fn has(&self, name: &str) -> bool {
        for scope in self.scopes.iter().rev() {
            if scope.contains_key(name) {
                return true;
            }
        }
        false
    }

    pub fn is_local(&self, name: &str) -> bool {
        for scope in self.scopes.iter().skip(1).rev() {
            if scope.contains_key(name) {
                return true;
            }
        }
        false
    }
    
    pub fn register_actor(&mut self, name: String, sig: ActorSignature) {
        self.actors.insert(name, sig);
    }

    pub fn register_function(&mut self, name: String, sig: FunctionSignature) {
        let span = sig.span;
        self.functions.insert(name.clone(), sig);
        self.define(name, Type::Function, span, false);
    }
    
    pub fn register_class(&mut self, name: String, sig: ClassSignature) {
        self.classes.insert(name.clone(), sig);
        self.define(name.clone(), Type::Class(name), (0, 0), false);
    }

    pub fn register_struct(&mut self, name: String, sig: ClassSignature) {
        self.structs.insert(name.clone(), sig);
        self.define(name.clone(), Type::Struct(name), (0, 0), false);
    }
    
    pub fn register_enum(&mut self, name: String, sig: EnumSignature) {
        self.enums.insert(name.clone(), sig);
        self.define(name.clone(), Type::Enum(name), (0, 0), false);
    }
}
