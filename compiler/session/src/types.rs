use crate::Symbol;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    String,
    Boolean,
    Void,
    Error,
    Any,
    BuiltinFunc,
    OverloadedFunction(Vec<(Symbol, TypeId)>),
    Function(Vec<Symbol>, Vec<TypeId>, TypeId),
    EnumVariantConstructor(Symbol, Symbol, Vec<Symbol>, Vec<TypeId>, TypeId),
    Class(Symbol, Vec<Symbol>),
    Struct(Symbol, Vec<Symbol>),
    Enum(Symbol, Vec<Symbol>),
    Instance(Symbol),
    Generic(Symbol),
    GenericInstance(Symbol, Vec<TypeId>),
    Interface(Symbol),
    Optional(TypeId),
    Array(TypeId),
    Range,
    Null,
    // FFI Types
    CInt,
    CUInt,
    CChar,
    CSize,
    Pointer(TypeId),
}

pub struct TypeArena {
    types: Vec<Type>,
    map: HashMap<Type, TypeId>,
}

impl Default for TypeArena {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeArena {
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            map: HashMap::new(),
        }
    }

    pub fn intern(&mut self, ty: Type) -> TypeId {
        if let Some(&id) = self.map.get(&ty) {
            return id;
        }
        let id = TypeId(self.types.len() as u32);
        self.types.push(ty.clone());
        self.map.insert(ty, id);
        id
    }

    pub fn get(&self, id: TypeId) -> &Type {
        &self.types[id.0 as usize]
    }
}
