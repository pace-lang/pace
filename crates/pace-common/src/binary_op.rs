#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    And,
    Or,
}
