pub mod interner;
pub mod types;

pub use interner::{Interner, Symbol};
pub use types::{TypeId, TypeArena};

use std::cell::RefCell;

pub struct CompilerSession {
    pub interner: RefCell<Interner>,
    pub types: RefCell<TypeArena>,

    pub ast_arena: bumpalo::Bump,
}

impl CompilerSession {
    pub fn new() -> Self {
        Self {
            interner: RefCell::new(Interner::new()),
            types: RefCell::new(TypeArena::new()),
            ast_arena: bumpalo::Bump::new(),
        }
    }

    pub fn format_type(&self, id: TypeId) -> String {
        self.format_type_internal(id)
    }

    fn format_type_internal(&self, id: TypeId) -> String {
        use types::Type;
        match self.types.borrow().get(id) {
            Type::Int => "Int".to_string(),
            Type::Float => "Float".to_string(),
            Type::String => "String".to_string(),
            Type::Boolean => "Boolean".to_string(),
            Type::Void => "Void".to_string(),
            Type::Error => "<ErrorType>".to_string(),
            Type::Any => "Any".to_string(),
            Type::Range => "Range".to_string(),
            Type::BuiltinFunc => "<BuiltinFunc>".to_string(),
            Type::OverloadedFunction(funcs) => format!("<OverloadedFunction({} variants)>", funcs.len()),
            Type::Function(type_params, params, ret) => {
                let mut s = "Func".to_string();
                if !type_params.is_empty() {
                    s.push('<');
                    s.push_str(&type_params.iter().map(|p| self.interner.borrow().lookup(*p).to_string()).collect::<Vec<String>>().join(", "));
                    s.push('>');
                }
                s.push('(');
                s.push_str(&params.iter().map(|p| self.format_type_internal(*p)).collect::<Vec<_>>().join(", "));
                s.push_str(") -> ");
                s.push_str(&self.format_type_internal(*ret));
                s
            }
            Type::EnumVariantConstructor(enum_name, variant_name, type_params, params, ret) => {
                let mut s = format!("VariantConstructor({}::{})", self.interner.borrow().lookup(*enum_name), self.interner.borrow().lookup(*variant_name));
                if !type_params.is_empty() {
                    s.push('<');
                    s.push_str(&type_params.iter().map(|p| self.interner.borrow().lookup(*p).to_string()).collect::<Vec<String>>().join(", "));
                    s.push('>');
                }
                s.push('(');
                s.push_str(&params.iter().map(|p| self.format_type_internal(*p)).collect::<Vec<_>>().join(", "));
                s.push_str(") -> ");
                s.push_str(&self.format_type_internal(*ret));
                s
            }
            Type::Class(name, type_params) => {
                let mut s = format!("Class({})", self.interner.borrow().lookup(*name));
                if !type_params.is_empty() {
                    s.push('<');
                    s.push_str(&type_params.iter().map(|p| self.interner.borrow().lookup(*p).to_string()).collect::<Vec<String>>().join(", "));
                    s.push('>');
                }
                s
            }
            Type::Enum(name, type_params) => {
                let mut s = format!("Enum({})", self.interner.borrow().lookup(*name));
                if !type_params.is_empty() {
                    s.push('<');
                    s.push_str(&type_params.iter().map(|p| self.interner.borrow().lookup(*p).to_string()).collect::<Vec<String>>().join(", "));
                    s.push('>');
                }
                s
            }
            Type::Instance(name) => self.interner.borrow().lookup(*name).to_string(),
            Type::Generic(name) => self.interner.borrow().lookup(*name).to_string(),
            Type::GenericInstance(name, args) => {
                let mut s = format!("{}<", self.interner.borrow().lookup(*name));
                s.push_str(&args.iter().map(|a| self.format_type_internal(*a)).collect::<Vec<_>>().join(", "));
                s.push('>');
                s
            }
            Type::Interface(name) => format!("Interface({})", self.interner.borrow().lookup(*name)),
            Type::Optional(inner) => format!("{}?", self.format_type_internal(*inner)),
            Type::Array(inner) => format!("[{}]", self.format_type_internal(*inner)),
            Type::Null => "Null".to_string(),
            Type::CInt => "CInt".to_string(),
            Type::CUInt => "CUInt".to_string(),
            Type::CChar => "CChar".to_string(),
            Type::CSize => "CSize".to_string(),
            Type::Pointer(inner) => format!("Pointer<{}>", self.format_type_internal(*inner)),
        }
    }
}

impl Default for CompilerSession {
    fn default() -> Self {
        Self::new()
    }
}
