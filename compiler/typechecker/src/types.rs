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
    Function(Vec<Type>, Box<Type>),
    Class(String, Vec<String>),
    Instance(String),
    Generic(String),
    GenericInstance(String, Vec<Type>),
    Interface(String),
    Optional(Box<Type>),
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
            Type::BuiltinFunc => write!(f, "<BuiltinFunc>"),
            Type::Function(params, ret) => {
                write!(f, "Func(")?;
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
        }
    }
}
