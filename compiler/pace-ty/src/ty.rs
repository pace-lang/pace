#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Int,
    Float,
    String,
    Bool,
    Struct(pace_hir::HirId),
    Class(pace_hir::HirId),
    Enum(pace_hir::HirId),
    Function(Vec<Ty>, Box<Ty>),
    Closure(Vec<Ty>, Box<Ty>),
    Optional(Box<Ty>),
    Void,
}
