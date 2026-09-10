#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Int,
    Float,
    String,
    Error,
    Struct(pace_hir::HirId),
    Class(pace_hir::HirId),
}
