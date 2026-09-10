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
    Function {
        name: Ident,
        params: Vec<(Ident, Type)>,
        return_type: Option<Type>,
        body: Block,
        span: Span,
    },
    Struct {
        name: Ident,
        fields: Vec<(Ident, Type)>,
        span: Span,
    },
    Class {
        name: Ident,
        fields: Vec<(Ident, Type)>,
        methods: Vec<Decl>, // Expects Decl::Function
        span: Span,
    },
    Expr(Expr, Span),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Named(Ident),
    Optional(Box<Type>, Span),
}

impl Type {
    pub fn span(&self) -> Span {
        match self {
            Type::Named(ident) => ident.span,
            Type::Optional(_, span) => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        name: Ident,
        value: Expr,
        span: Span,
    },
    ExprStmt(Expr, Span),
    Return(Option<Expr>, Span),
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
    MemberAccess {
        object: Box<Expr>,
        member: Ident,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
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
        target: Ident,
        value: Box<Expr>,
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
    Gt,
    Lt,
    GtEq,
    LtEq,
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLiteral(_, s) => *s,
            Expr::FloatLiteral(_, s) => *s,
            Expr::StringLiteral(_, s) => *s,
            Expr::Ident(id) => id.span,
            Expr::Binary { span, .. } => *span,
            Expr::MemberAccess { span, .. } => *span,
            Expr::Call { span, .. } => *span,
            Expr::If { span, .. } => *span,
            Expr::While { span, .. } => *span,
            Expr::Assign { span, .. } => *span,
        }
    }
}

impl Decl {
    pub fn span(&self) -> Span {
        match self {
            Decl::Let { span, .. } => *span,
            Decl::Const { span, .. } => *span,
            Decl::Function { span, .. } => *span,
            Decl::Struct { span, .. } => *span,
            Decl::Class { span, .. } => *span,
            Decl::Expr(_, span) => *span,
        }
    }
}
