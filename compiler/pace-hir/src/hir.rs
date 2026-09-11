use pace_ast::{BinaryOp, Type};
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
        ty: Option<Type>,
        value: Expr,
        span: Span,
    },
    Var {
        id: HirId,
        name: String,
        ty: Option<Type>,
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
        ty: Option<Type>,
        value: Expr,
        span: Span,
    },
    Var {
        id: HirId,
        name: String,
        ty: Option<Type>,
        value: Expr,
        span: Span,
    },
    Struct {
        id: HirId,
        name: String,
        fields: Vec<(String, Type)>,
        methods: Vec<Decl>, // Lowered to global functions anyway, but kept for namespacing if needed
        span: Span,
    },
    Class {
        id: HirId,
        name: String,
        fields: Vec<(String, Type)>,
        methods: Vec<Decl>,
        span: Span,
    },
    Function {
        id: HirId,
        name: String,
        params: Vec<(HirId, String, Type)>,
        return_type: Option<Type>,
        body: Block,
        span: Span,
    },
    Expr(Expr, Span),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLiteral(String, Span),
    FloatLiteral(String, Span),
    BoolLiteral(bool, Span),
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
        args: Vec<(Option<String>, Expr)>,
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
        target: Box<Expr>,
        value: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLiteral(_, span) => *span,
            Expr::FloatLiteral(_, span) => *span,
            Expr::BoolLiteral(_, span) => *span,
            Expr::StringLiteral(_, span) => *span,
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
