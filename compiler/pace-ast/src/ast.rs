use pace_span::{FileId, Span, Symbol};

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub name: Ident,
    pub ty: Type,
    pub default_value: Option<Expr>,
    pub is_mut: bool,
    pub is_private: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticFieldDef {
    pub name: Ident,
    pub ty: Type,
    pub value: Expr,
    pub is_private: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstFieldDef {
    pub name: Ident,
    pub ty: Type,
    pub value: Expr,
    pub is_private: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallArg {
    pub label: Option<Ident>,
    pub expr: Expr,
}

use std::collections::HashMap;

/// Represents a complete parsed Pace program consisting of multiple modules.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub modules: HashMap<Symbol, Module>,
    pub module_order: Vec<Symbol>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub name: Symbol,
    pub file_id: FileId,
    pub declarations: Vec<Decl>,
    pub comments: Vec<(Span, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ident {
    pub name: Symbol,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenericParam {
    pub name: Ident,
    pub trait_bounds: Vec<Type>,
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
        is_private: bool,
        span: Span,
    },
    Var {
        name: Ident,
        ty: Option<Type>,
        value: Option<Expr>,
        is_private: bool,
        span: Span,
    },
    Const {
        name: Ident,
        ty: Option<Type>,
        value: Expr,
        is_private: bool,
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
        is_private: bool,
        span: Span,
    },
    Struct {
        name: Ident,
        generic_params: Option<Vec<GenericParam>>,
        with: Vec<Ident>,
        fields: Vec<FieldDef>, // name, type, default, is_optional, is_private
        static_fields: Vec<StaticFieldDef>, // name, type, default, is_private
        const_fields: Vec<ConstFieldDef>,
        methods: Vec<Decl>, // Expects Decl::Function
        is_private: bool,
        span: Span,
    },
    Class {
        name: Ident,
        generic_params: Option<Vec<GenericParam>>,
        extends: Option<Ident>,
        with: Vec<Ident>,
        fields: Vec<FieldDef>,
        static_fields: Vec<StaticFieldDef>,
        const_fields: Vec<ConstFieldDef>,
        methods: Vec<Decl>, // Expects Decl::Function
        is_private: bool,
        span: Span,
    },
    Trait {
        name: Ident,
        generic_params: Option<Vec<GenericParam>>,
        methods: Vec<Decl>, // Expects Decl::Function
        is_private: bool,
        span: Span,
    },
    Enum {
        name: Ident,
        generic_params: Option<Vec<GenericParam>>,
        variants: Vec<EnumVariant>,
        is_private: bool,
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
    Closure(Vec<Type>, Box<Type>, Span),
}

impl Type {
    pub fn span(&self) -> Span {
        match self {
            Type::Named(ident) => ident.span,
            Type::Generic(_, _, span) => *span,
            Type::Optional(_, span) => *span,
            Type::Closure(_, _, span) => *span,
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
    InterpolatedString(Vec<Expr>, Span),
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
        args: Vec<CallArg>,
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
    Closure {
        params: Vec<ClosureParam>,
        return_type: Option<Type>,
        body: ClosureBody,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClosureBody {
    Expr(Box<Expr>),
    Block(Block),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClosureParam {
    pub name: Ident,
    pub ty: Option<Type>,
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
            Expr::InterpolatedString(_, span) => *span,
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
            Expr::Closure { span, .. } => *span,
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

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let { span, .. } => *span,
            Stmt::Var { span, .. } => *span,
            Stmt::ExprStmt(_, span) => *span,
            Stmt::Return(_, span) => *span,
        }
    }
}
