use pace_ast::BinaryOp;
use pace_span::Span;

/// A unique ID for variables and definitions across the entire program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HirId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        id: HirId,
        name: String,
        value: Expr,
        span: Span,
    },
    ExprStmt(Expr, Span),
    Return(Option<Expr>, Span),
}

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
    Expr(Expr, Span),
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
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    BuiltinCall(String, Vec<Expr>, Span),
    If {
        cond: Box<Expr>,
        then_block: Block,
        else_block: Option<Block>,
        span: Span,
    },
    While {
        cond: Box<Expr>,
        body: Block,
        span: Span,
    },
    Assign {
        target: HirId,
        value: Box<Expr>,
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
            Expr::Call { span, .. } => *span,
            Expr::BuiltinCall(_, _, span) => *span,
            Expr::If { span, .. } => *span,
            Expr::While { span, .. } => *span,
            Expr::Assign { span, .. } => *span,
        }
    }
}
