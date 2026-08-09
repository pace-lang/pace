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
        }
    }
}
