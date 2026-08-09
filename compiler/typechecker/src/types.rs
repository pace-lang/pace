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
    Class(String),
    Instance(String),
} // Used to prevent cascading type errors during invalid parsing/resolution

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
            Type::Class(name) => write!(f, "Class({})", name),
            Type::Instance(name) => write!(f, "{}", name),
        }
    }
}
