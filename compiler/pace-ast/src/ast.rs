use pace_span::{FileId, Span};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub modules: HashMap<String, Module>,
    pub module_order: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub name: String,
    pub file_id: FileId,
    pub declarations: Vec<Decl>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenericParam {
    pub name: Ident,
    pub default: Option<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    Import {
        path: Vec<Ident>,
        alias: Option<Ident>,
        span: Span,
    },
    Let {
        name: Ident,
        ty: Option<Type>,
        value: Option<Expr>,
        span: Span,
    },
    Var {
        name: Ident,
        ty: Option<Type>,
        value: Option<Expr>,
        span: Span,
    },
    Const {
        name: Ident,
        ty: Option<Type>,
        value: Expr,
        span: Span,
    },
    Function {
        name: Ident,
        generic_params: Option<Vec<GenericParam>>,
        params: Vec<(Ident, Type)>,
        return_type: Option<Type>,
        body: Block,
        is_static: bool,
        is_override: bool,
        span: Span,
    },
    Struct {
        name: Ident,
        generic_params: Option<Vec<GenericParam>>,
        with: Vec<Ident>,
        fields: Vec<(Ident, Type, Option<Expr>, bool)>,
        static_fields: Vec<(Ident, Type, Expr)>,
        const_fields: Vec<(Ident, Type, Expr)>,
        methods: Vec<Decl>, // Expects Decl::Function
        span: Span,
    },
    Class {
        name: Ident,
        generic_params: Option<Vec<GenericParam>>,
        extends: Option<Ident>,
        with: Vec<Ident>,
        fields: Vec<(Ident, Type, Option<Expr>, bool)>,
        static_fields: Vec<(Ident, Type, Expr)>,
        const_fields: Vec<(Ident, Type, Expr)>,
        methods: Vec<Decl>, // Expects Decl::Function
        span: Span,
    },
    Trait {
        name: Ident,
        generic_params: Option<Vec<GenericParam>>,
        methods: Vec<Decl>, // Expects Decl::Function
        span: Span,
    },
    Enum {
        name: Ident,
        generic_params: Option<Vec<GenericParam>>,
        variants: Vec<EnumVariant>,
        span: Span,
    },
    Expr(Expr, Span),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: Ident,
    pub fields: Option<Vec<(Ident, Type)>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Named(Ident),
    Generic(Box<Type>, Vec<Type>, Span),
    Optional(Box<Type>, Span),
}

impl Type {
    pub fn span(&self) -> Span {
        match self {
            Type::Named(ident) => ident.span,
            Type::Generic(_, _, span) => *span,
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
        ty: Option<Type>,
        value: Option<Expr>,
        span: Span,
    },
    Var {
        name: Ident,
        ty: Option<Type>,
        value: Option<Expr>,
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
    Null(Span),
    Ident(Ident, Option<Vec<Type>>),
    Super(Span),
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
    OptionalMemberAccess {
        object: Box<Expr>,
        member: Ident,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<(Option<Ident>, Expr)>,
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
        target: Box<Expr>,
        value: Box<Expr>,
        span: Span,
    },
    Match {
        subject: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Ident(Ident),
    Variant {
        name: Ident,
        fields: Option<Vec<Ident>>,
        span: Span,
    },
    CatchAll(Span),
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
    NullCoalesce,
    And,
    Or,
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLiteral(_, span) => *span,
            Expr::FloatLiteral(_, span) => *span,
            Expr::StringLiteral(_, span) => *span,
            Expr::Null(span) => *span,
            Expr::Ident(ident, _) => ident.span,
            Expr::Super(span) => *span,
            Expr::Binary { span, .. } => *span,
            Expr::MemberAccess { span, .. } => *span,
            Expr::OptionalMemberAccess { span, .. } => *span,
            Expr::Call { span, .. } => *span,
            Expr::If { span, .. } => *span,
            Expr::While { span, .. } => *span,
            Expr::Assign { span, .. } => *span,
            Expr::Match { span, .. } => *span,
        }
    }
}

impl Decl {
    pub fn span(&self) -> Span {
        match self {
            Decl::Import { span, .. } => *span,
            Decl::Let { span, .. } => *span,
            Decl::Var { span, .. } => *span,
            Decl::Const { span, .. } => *span,
            Decl::Function { span, .. } => *span,
            Decl::Struct { span, .. } => *span,
            Decl::Class { span, .. } => *span,
            Decl::Trait { span, .. } => *span,
            Decl::Enum { span, .. } => *span,
            Decl::Expr(_, span) => *span,
        }
    }
}
