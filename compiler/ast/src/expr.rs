use crate::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Negate,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// A literal integer like `10`
    Integer(i64),
    /// A literal float like `3.14`
    Float(f64),
    /// A literal string like `"hello"`
    String(String),
    /// A boolean literal like `true` or `false`
    Boolean(bool),
    /// A variable reference like `count`
    Variable(String),
    /// A binary operation like `a + b`
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    /// A unary operation like `-a`
    Unary(UnaryOp, Box<Expr>),
    /// A grouped expression like `(a + b)`
    Grouping(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}
