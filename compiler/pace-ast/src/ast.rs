use pace_span::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub declarations: Vec<Decl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    Let {
        name: Ident,
        value: Expr,
        span: Span,
    },
    Const {
        name: Ident,
        value: Expr,
        span: Span,
    },
    // Further declarations (Function, Class) will be added as we expand the parser.
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLiteral(String, Span),
    FloatLiteral(String, Span),
    StringLiteral(String, Span),
    Ident(Ident),
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    EqEq,
    NotEq,
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLiteral(_, s) => *s,
            Expr::FloatLiteral(_, s) => *s,
            Expr::StringLiteral(_, s) => *s,
            Expr::Ident(id) => id.span,
            Expr::Binary { span, .. } => *span,
        }
    }
}
