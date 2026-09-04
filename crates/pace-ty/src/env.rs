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
    /// An interface type
    Interface(ustr::Ustr),
    /// An actor type (isolated state, async messages)
    Actor(ustr::Ustr),
    /// A value type (stack-allocated)
    Struct(ustr::Ustr),
    /// An enum type
    Enum(ustr::Ustr),
    /// A function type
    Function {
        generic_params: Option<Vec<pace_ast::GenericParam>>,
        params: Vec<Type>,
        return_type: Box<Type>,
    },
    Unknown, // Used for auto-inference before resolution or error state
    Void,    // Used for functions that don't return anything
    Any,     // Used for built-ins like print that take multiple types
    GenericParameter(ustr::Ustr, Option<Box<Type>>),
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
            Type::GenericParameter(name, bound) => {
                if let Some(subst) = substitutions.get(name) {
                    subst.clone()
                } else {
                    let new_bound = bound.as_ref().map(|b| Box::new(b.resolve_generics(substitutions)));
                    Type::GenericParameter(*name, new_bound)
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
    pub generic_params: Option<Vec<pace_ast::GenericParam>>,
    pub variants: HashMap<ustr::Ustr, Option<Vec<Type>>>,
    pub span: pace_span::Span,
}

#[derive(Debug, Clone)]
pub struct GlobalVariableSignature {
    pub ty: Type,
    pub is_mutable: bool,
    pub visibility: Visibility,
    pub module: ustr::Ustr,
    pub span: pace_span::Span,
}

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub params: Vec<Type>,
    pub return_type: Type,
    pub span: pace_span::Span,
    pub is_used: bool,
    pub visibility: Visibility,
    pub module: ustr::Ustr,
    pub generic_params: Option<Vec<pace_ast::GenericParam>>,
    pub is_static: bool,
}

#[derive(Debug, Clone)]
pub struct ActorSignature {
    pub generic_params: Option<Vec<pace_ast::GenericParam>>,
    pub implements: Option<Type>,
    pub fields: HashMap<ustr::Ustr, Type>,
    pub static_fields: HashMap<ustr::Ustr, Type>,
    pub methods: HashMap<ustr::Ustr, FunctionSignature>,
    pub span: pace_span::Span,
}

#[derive(Debug, Clone)]
pub struct ClassSignature {
    pub generic_params: Option<Vec<pace_ast::GenericParam>>,
    pub implements: Option<Type>,
    pub fields: HashMap<ustr::Ustr, Type>,
    pub static_fields: HashMap<ustr::Ustr, Type>,
    pub methods: HashMap<ustr::Ustr, FunctionSignature>,
    pub span: pace_span::Span,
}

#[derive(Debug, Clone)]
pub struct InterfaceSignature {
    pub generic_params: Option<Vec<pace_ast::GenericParam>>,
    pub methods: HashMap<ustr::Ustr, FunctionSignature>,
    pub span: pace_span::Span,
}

#[derive(Debug, Clone)]
pub struct VarInfo {
    pub ty: Type,
    pub span: pace_span::Span,
    pub is_used: bool,
    pub is_mutable: bool,
}

#[derive(Clone, Default)]
pub struct Environment {
    scopes: Vec<HashMap<ustr::Ustr, VarInfo>>,
    pub functions: HashMap<ustr::Ustr, FunctionSignature>,
    pub classes: HashMap<ustr::Ustr, ClassSignature>,
    pub interfaces: HashMap<ustr::Ustr, InterfaceSignature>,
    pub structs: HashMap<ustr::Ustr, ClassSignature>,
    pub enums: HashMap<ustr::Ustr, EnumSignature>,
    pub actors: HashMap<ustr::Ustr, ActorSignature>,
    pub symbol_types: HashMap<ustr::Ustr, Type>,
    pub global_vars: HashMap<ustr::Ustr, GlobalVariableSignature>,
    pub node_types: HashMap<pace_hir::arena::ExprId, Type>,
    pub node_definitions: HashMap<pace_hir::arena::ExprId, pace_span::Span>,
}

impl Environment {
    pub fn new() -> Self {
        let mut e = Self {
            scopes: vec![HashMap::new()], // Start with a global scope
            functions: HashMap::new(),
            classes: HashMap::new(),
            interfaces: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            actors: HashMap::new(),
            symbol_types: HashMap::new(),
            global_vars: HashMap::new(),
            node_types: HashMap::new(),
            node_definitions: HashMap::new(),
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
                span: pace_span::Span::default(),
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
                span: pace_span::Span::default(),
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
                span: pace_span::Span::default(),
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
                span: pace_span::Span::default(),
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
                span: pace_span::Span::default(),
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
                span: pace_span::Span::default(),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: Some(vec![pace_ast::GenericParam { name: "T".into(), bound: None }]),
                is_static: false,
            },
        );
        self.register_function(
            "__pace_release_generic".into(),
            FunctionSignature {
                params: vec![Type::Any],
                return_type: Type::Void,
                span: pace_span::Span::default(),
                is_used: true,
                visibility: Visibility::Public,
                module: "std".into(),
                generic_params: Some(vec![pace_ast::GenericParam { name: "T".into(), bound: None }]),
                is_static: false,
            },
        );
        self.register_function(
            "__pace_noop".into(),
            FunctionSignature {
                params: vec![Type::Any],
                return_type: Type::Void,
                span: pace_span::Span::default(),
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
                span: pace_span::Span::default(),
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
                span: pace_span::Span::default(),
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
                span: pace_span::Span::default(),
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
                span: pace_span::Span::default(),
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
                span: pace_span::Span::default(),
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
        span: pace_span::Span,
        is_mutable: bool,
    ) -> Result<(), pace_span::Span> {
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

    pub fn get_definition_span(&self, name: ustr::Ustr) -> Option<pace_span::Span> {
        if let Some(var_info) = self.get_var_info(name) {
            return Some(var_info.span);
        }
        if let Some(func) = self.functions.get(&name) {
            return Some(func.span);
        }
        if let Some(cls) = self.classes.get(&name) {
            return Some(cls.span);
        }
        if let Some(strct) = self.structs.get(&name) {
            return Some(strct.span);
        }
        if let Some(enm) = self.enums.get(&name) {
            return Some(enm.span);
        }
        if let Some(act) = self.actors.get(&name) {
            return Some(act.span);
        }
        if let Some(glob) = self.global_vars.get(&name) {
            return Some(glob.span);
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
        let _ = self.define(name, Type::Class(name), pace_span::Span::default(), false);
    }

    pub fn register_interface(&mut self, name: ustr::Ustr, sig: InterfaceSignature) {
        self.interfaces.insert(name, sig);
        let _ = self.define(name, Type::Interface(name), pace_span::Span::default(), false);
    }

    pub fn register_struct(&mut self, name: ustr::Ustr, sig: ClassSignature) {
        self.structs.insert(name, sig);
        let _ = self.define(name, Type::Struct(name), pace_span::Span::default(), false);
    }

    pub fn register_enum(&mut self, name: ustr::Ustr, sig: EnumSignature) {
        self.enums.insert(name, sig);
        let _ = self.define(name, Type::Enum(name), pace_span::Span::default(), false);
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
