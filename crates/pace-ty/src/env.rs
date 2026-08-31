use pace_ast::Visibility;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// Poison type for graceful error recovery
    Error,
    Int,
    Float,
    String,
    Bool,
    Null,
    /// A nullable wrapper around a type (e.g. `Int?`)
    Nullable(Box<Type>),
    /// A reference type (heap-allocated, ARC)
    Class(ustr::Ustr),
    /// An actor type (isolated state, async messages)
    Actor(ustr::Ustr),
    /// A value type (stack-allocated)
    Struct(ustr::Ustr),
    /// An enum type
    Enum(ustr::Ustr),
    /// A function type
    Function {
        generic_params: Option<Vec<ustr::Ustr>>,
        params: Vec<Type>,
        return_type: Box<Type>,
    },
    Unknown, // Used for auto-inference before resolution or error state
    Void,    // Used for functions that don't return anything
    Any,     // Used for built-ins like print that take multiple types
    GenericParameter(ustr::Ustr),
    GenericInstance {
        base: Box<Type>,
        args: Vec<Type>,
    },
    /// An asynchronous value that resolves to the inner type
    /// An asynchronous value that resolves to the inner type
    Promise(Box<Type>),
}

impl Type {
    pub fn resolve_generics(&self, substitutions: &HashMap<ustr::Ustr, Type>) -> Type {
        match self {
            Type::GenericParameter(name) => {
                if let Some(subst) = substitutions.get(name) {
                    subst.clone()
                } else {
                    self.clone()
                }
            }
            Type::GenericInstance { base, args } => Type::GenericInstance {
                base: Box::new(base.resolve_generics(substitutions)),
                args: args
                    .iter()
                    .map(|arg| arg.resolve_generics(substitutions))
                    .collect(),
            },
            Type::Nullable(inner) => {
                Type::Nullable(Box::new(inner.resolve_generics(substitutions)))
            }
            Type::Function {
                generic_params,
                params,
                return_type,
            } => Type::Function {
                generic_params: generic_params.clone(),
                params: params
                    .iter()
                    .map(|p| p.resolve_generics(substitutions))
                    .collect(),
                return_type: Box::new(return_type.resolve_generics(substitutions)),
            },
            Type::Promise(inner) => Type::Promise(Box::new(inner.resolve_generics(substitutions))),
            _ => self.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnumSignature {
    pub generic_params: Option<Vec<ustr::Ustr>>,
    pub variants: HashMap<ustr::Ustr, Option<Vec<Type>>>,
}

#[derive(Debug, Clone)]
pub struct GlobalVariableSignature {
    pub ty: Type,
    pub is_mutable: bool,
    pub visibility: Visibility,
    pub module: ustr::Ustr,
    pub span: pace_ast::Span,
}

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub params: Vec<Type>,
    pub return_type: Type,
    pub span: pace_ast::Span,
    pub is_used: bool,
    pub visibility: Visibility,
    pub module: ustr::Ustr,
    pub generic_params: Option<Vec<ustr::Ustr>>,
    pub is_static: bool,
}

#[derive(Debug, Clone)]
pub struct ActorSignature {
    pub generic_params: Option<Vec<ustr::Ustr>>,
    pub fields: HashMap<ustr::Ustr, Type>,
    pub static_fields: HashMap<ustr::Ustr, Type>,
    pub methods: HashMap<ustr::Ustr, FunctionSignature>,
}

#[derive(Debug, Clone)]
pub struct ClassSignature {
    pub generic_params: Option<Vec<ustr::Ustr>>,
    pub fields: HashMap<ustr::Ustr, Type>,
    pub static_fields: HashMap<ustr::Ustr, Type>,
    pub methods: HashMap<ustr::Ustr, FunctionSignature>,
}

#[derive(Debug, Clone)]
pub struct VarInfo {
    pub ty: Type,
    pub span: pace_ast::Span,
    pub is_used: bool,
    pub is_mutable: bool,
}

#[derive(Clone, Default)]
pub struct Environment {
    scopes: Vec<HashMap<ustr::Ustr, VarInfo>>,
    pub functions: HashMap<ustr::Ustr, FunctionSignature>,
    pub classes: HashMap<ustr::Ustr, ClassSignature>,
    pub structs: HashMap<ustr::Ustr, ClassSignature>,
    pub enums: HashMap<ustr::Ustr, EnumSignature>,
    pub actors: HashMap<ustr::Ustr, ActorSignature>,
    pub symbol_types: HashMap<ustr::Ustr, Type>,
    pub global_vars: HashMap<ustr::Ustr, GlobalVariableSignature>,
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
            symbol_types: HashMap::new(),
            global_vars: HashMap::new(),
        };
        e.inject_prelude();
        e
    }

    fn inject_prelude(&mut self) {
        // Inject built-in print function
        self.register_function(
            "print".into(),
            FunctionSignature {
                params: vec![Type::Any], // Accept any type
                return_type: Type::Void,
                span: pace_ast::Span::default(),
                is_used: true, // Always consider built-ins used
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: None,
                is_static: false,
            },
        );
        // Inject built-in hash function
        self.register_function(
            "hash".into(),
            FunctionSignature {
                params: vec![Type::Any], // Accept any type
                return_type: Type::Int,
                span: pace_ast::Span::default(),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "malloc".into(),
            FunctionSignature {
                params: vec![Type::Int],
                return_type: Type::Int,
                span: pace_ast::Span::default(),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "free".into(),
            FunctionSignature {
                params: vec![Type::Int, Type::Int],
                return_type: Type::Void,
                span: pace_ast::Span::default(),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "ptrStore".into(),
            FunctionSignature {
                params: vec![Type::Int, Type::Int, Type::Any],
                return_type: Type::Void,
                span: pace_ast::Span::default(),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "__pace_retain_generic".into(),
            FunctionSignature {
                params: vec![Type::Any],
                return_type: Type::Void,
                span: pace_ast::Span::default(),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: Some(vec!["T".into()]),
                is_static: false,
            },
        );
        self.register_function(
            "__pace_release_generic".into(),
            FunctionSignature {
                params: vec![Type::Any],
                return_type: Type::Void,
                span: pace_ast::Span::default(),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: Some(vec!["T".into()]),
                is_static: false,
            },
        );
        self.register_function(
            "__pace_noop".into(),
            FunctionSignature {
                params: vec![Type::Any],
                return_type: Type::Void,
                span: pace_ast::Span::default(),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "retain".into(),
            FunctionSignature {
                params: vec![Type::Any],
                return_type: Type::Void,
                span: pace_ast::Span::default(),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "release".into(),
            FunctionSignature {
                params: vec![Type::Any],
                return_type: Type::Void,
                span: pace_ast::Span::default(),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "ptrLoad".into(),
            FunctionSignature {
                params: vec![Type::Int, Type::Int],
                return_type: Type::Any,
                span: pace_ast::Span::default(),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "time".into(),
            FunctionSignature {
                params: vec![Type::Int],
                return_type: Type::Int,
                span: pace_ast::Span::default(),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: None,
                is_static: false,
            },
        );
        self.register_function(
            "getYear".into(),
            FunctionSignature {
                params: vec![Type::Int],
                return_type: Type::Int,
                span: pace_ast::Span::default(),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: None,
                is_static: false,
            },
        );
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) -> Vec<(ustr::Ustr, VarInfo)> {
        if self.scopes.len() > 1 {
            let scope = self.scopes.pop().unwrap();
            scope.into_iter().collect()
        } else {
            Vec::new()
        }
    }

    pub fn define(
        &mut self,
        name: ustr::Ustr,
        ty: Type,
        span: pace_ast::Span,
        is_mutable: bool,
    ) -> Result<(), pace_ast::Span> {
        if let Some(scope) = self.scopes.last_mut() {
            if let Some(existing) = scope.get(&name) {
                return Err(existing.span);
            }
            scope.insert(
                name,
                VarInfo {
                    ty: ty.clone(),
                    span,
                    is_used: false,
                    is_mutable,
                },
            );
        }
        self.symbol_types.insert(name, ty);
        Ok(())
    }

    pub fn get_mut(&mut self, name: ustr::Ustr) -> Option<&mut VarInfo> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(var_info) = scope.get_mut(&name) {
                return Some(var_info);
            }
        }
        None
    }

    pub fn get(&self, name: ustr::Ustr) -> Option<&Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(var_info) = scope.get(&name) {
                return Some(&var_info.ty);
            }
        }
        None
    }

    pub fn get_var_info(&self, name: ustr::Ustr) -> Option<&VarInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(var_info) = scope.get(&name) {
                return Some(var_info);
            }
        }
        None
    }

    pub fn find_closest_variable(&self, name: ustr::Ustr) -> Option<ustr::Ustr> {
        let mut closest = None;
        let mut min_distance = usize::MAX;

        for scope in self.scopes.iter().rev() {
            for var_name in scope.keys() {
                let dist = levenshtein(name.as_str(), var_name.as_str());
                if dist <= 2 && dist < min_distance {
                    min_distance = dist;
                    closest = Some(*var_name);
                }
            }
        }
        closest
    }

    pub fn has(&self, name: ustr::Ustr) -> bool {
        for scope in self.scopes.iter().rev() {
            if scope.contains_key(&name) {
                return true;
            }
        }
        false
    }

    pub fn is_local(&self, name: ustr::Ustr) -> bool {
        for scope in self.scopes.iter().skip(1).rev() {
            if scope.contains_key(&name) {
                return true;
            }
        }
        false
    }

    pub fn register_actor(&mut self, name: ustr::Ustr, sig: ActorSignature) {
        self.actors.insert(name, sig);
    }

    pub fn register_global_var(&mut self, name: ustr::Ustr, sig: GlobalVariableSignature) {
        self.global_vars.insert(name, sig);
    }

    pub fn register_function(&mut self, name: ustr::Ustr, sig: FunctionSignature) {
        let span = sig.span;
        let fn_type = Type::Function {
            generic_params: sig.generic_params.clone(),
            params: sig.params.clone(),
            return_type: Box::new(sig.return_type.clone()),
        };
        self.functions.insert(name, sig);
        let _ = self.define(name, fn_type, span, false);
    }

    pub fn register_class(&mut self, name: ustr::Ustr, sig: ClassSignature) {
        self.classes.insert(name, sig);
        let _ = self.define(name, Type::Class(name), pace_ast::Span::default(), false);
    }

    pub fn register_struct(&mut self, name: ustr::Ustr, sig: ClassSignature) {
        self.structs.insert(name, sig);
        let _ = self.define(name, Type::Struct(name), pace_ast::Span::default(), false);
    }

    pub fn register_enum(&mut self, name: ustr::Ustr, sig: EnumSignature) {
        self.enums.insert(name, sig);
        let _ = self.define(name, Type::Enum(name), pace_ast::Span::default(), false);
    }
}

pub fn levenshtein(a: &str, b: &str) -> usize {
    let mut matrix = vec![vec![0; b.len() + 1]; a.len() + 1];
    for i in 0..=a.len() {
        matrix[i][0] = i;
    }
    for j in 0..=b.len() {
        matrix[0][j] = j;
    }
    for (i, ca) in a.chars().enumerate() {
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            matrix[i + 1][j + 1] = std::cmp::min(
                std::cmp::min(matrix[i][j + 1] + 1, matrix[i + 1][j] + 1),
                matrix[i][j] + cost,
            );
        }
    }
    matrix[a.len()][b.len()]
}
