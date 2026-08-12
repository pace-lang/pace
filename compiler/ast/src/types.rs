use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    String,
    Boolean,
    Void,
    Error,
    Any,
    BuiltinFunc,
    OverloadedFunction(Vec<(String, Type)>),
    Function(Vec<String>, Vec<Type>, Box<Type>),
    EnumVariantConstructor(String, String, Vec<String>, Vec<Type>, Box<Type>),
    Class(String, Vec<String>),
    Enum(String, Vec<String>),
    Instance(String),
    Generic(String),
    GenericInstance(String, Vec<Type>),
    Interface(String),
    Optional(Box<Type>),
    Array(Box<Type>),
    Range,
    Null,
    // FFI Types
    CInt,
    CUInt,
    CChar,
    CSize,
    Pointer(Box<Type>),
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "Int"),
            Type::Float => write!(f, "Float"),
            Type::String => write!(f, "String"),
            Type::Boolean => write!(f, "Boolean"),
            Type::Void => write!(f, "Void"),
            Type::Error => write!(f, "<ErrorType>"),
            Type::Any => write!(f, "Any"),
            Type::Range => write!(f, "Range"),
            Type::BuiltinFunc => write!(f, "<BuiltinFunc>"),
            Type::OverloadedFunction(funcs) => write!(f, "<OverloadedFunction({} variants)>", funcs.len()),
            Type::Function(type_params, params, ret) => {
                write!(f, "Func")?;
                if !type_params.is_empty() {
                    write!(f, "<")?;
                    for (i, p) in type_params.iter().enumerate() {
                        if i > 0 { write!(f, ", ")?; }
                        write!(f, "{}", p)?;
                    }
                    write!(f, ">")?;
                }
                write!(f, "(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", p)?;
                }
                write!(f, ") -> {}", ret)
            }
            Type::EnumVariantConstructor(enum_name, variant_name, type_params, params, ret) => {
                write!(f, "VariantConstructor({}::{})", enum_name, variant_name)?;
                if !type_params.is_empty() {
                    write!(f, "<")?;
                    for (i, p) in type_params.iter().enumerate() {
                        if i > 0 { write!(f, ", ")?; }
                        write!(f, "{}", p)?;
                    }
                    write!(f, ">")?;
                }
                write!(f, "(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", p)?;
                }
                write!(f, ") -> {}", ret)
            }
            Type::Class(name, type_params) => {
                write!(f, "Class({})", name)?;
                if !type_params.is_empty() {
                    write!(f, "<")?;
                    for (i, p) in type_params.iter().enumerate() {
                        if i > 0 { write!(f, ", ")?; }
                        write!(f, "{}", p)?;
                    }
                    write!(f, ">")?;
                }
                Ok(())
            }
            Type::Enum(name, type_params) => {
                write!(f, "Enum({})", name)?;
                if !type_params.is_empty() {
                    write!(f, "<")?;
                    for (i, p) in type_params.iter().enumerate() {
                        if i > 0 { write!(f, ", ")?; }
                        write!(f, "{}", p)?;
                    }
                    write!(f, ">")?;
                }
                Ok(())
            }
            Type::Instance(name) => write!(f, "{}", name),
            Type::Generic(name) => write!(f, "{}", name),
            Type::GenericInstance(name, args) => {
                write!(f, "{}<", name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", arg)?;
                }
                write!(f, ">")
            }
            Type::Interface(name) => write!(f, "Interface({})", name),
            Type::Optional(inner) => write!(f, "{}?", inner),
            Type::Array(inner) => write!(f, "[{}]", inner),
            Type::Null => write!(f, "Null"),
            Type::CInt => write!(f, "CInt"),
            Type::CUInt => write!(f, "CUInt"),
            Type::CChar => write!(f, "CChar"),
            Type::CSize => write!(f, "CSize"),
            Type::Pointer(inner) => write!(f, "Pointer<{}>", inner),
        }
    }
}
