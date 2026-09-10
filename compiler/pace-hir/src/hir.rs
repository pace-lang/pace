use pace_ast::BinaryOp;
use pace_span::Span;

/// A unique ID for variables and definitions across the entire program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HirId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub declarations: Vec<Decl>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    Let {
        id: HirId,
        name: String,
        value: Expr,
        span: Span,
    },
    Struct {
        id: HirId,
        name: String,
        span: Span,
    },
    Class {
        id: HirId,
        name: String,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLiteral(String, Span),
    StringLiteral(String, Span),
    Ident(HirId, Span),
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: Span,
    },
    MemberAccess {
        object: Box<Expr>,
        member: String,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLiteral(_, s) => *s,
            Expr::StringLiteral(_, s) => *s,
            Expr::Ident(_, s) => *s,
            Expr::Binary { span, .. } => *span,
            Expr::MemberAccess { span, .. } => *span,
        }
    }
}
