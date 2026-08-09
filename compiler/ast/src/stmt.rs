use crate::span::Span;
use crate::expr::Expr;

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// A let declaration: `let name = expression;`
    Let {
        name: String,
        initializer: Expr,
    },
    /// A var declaration: `var name = expression;`
    Var {
        name: String,
        initializer: Expr,
    },
    /// An expression evaluated for side effects: `10 + 20;`
    Expression(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

impl Stmt {
    pub fn new(kind: StmtKind, span: Span) -> Self {
        Self { kind, span }
    }
}
