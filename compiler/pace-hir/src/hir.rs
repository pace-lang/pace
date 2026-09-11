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
        value: Option<Expr>,
        span: Span,
    },
    Var {
        id: HirId,
        name: String,
        ty: Option<Type>,
        value: Option<Expr>,
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
        value: Option<Expr>,
        span: Span,
    },
    Var {
        id: HirId,
        name: String,
        ty: Option<Type>,
        value: Option<Expr>,
        span: Span,
    },
    Const {
        id: HirId,
        name: String,
        ty: Option<Type>,
        value: Expr,
        span: Span,
    },
    Struct {
        id: HirId,
        name: String,
        generic_params: Option<Vec<(String, Option<Type>)>>,
        fields: Vec<(String, Type, Option<Expr>)>,
        static_fields: Vec<(String, Type, Expr)>,
        const_fields: Vec<(String, Type, Expr)>,
        methods: Vec<Decl>, // Lowered to global functions anyway, but kept for namespacing if needed
        span: Span,
    },
    Class {
        id: HirId,
        name: String,
        generic_params: Option<Vec<(String, Option<Type>)>>,
        fields: Vec<(String, Type, Option<Expr>)>,
        static_fields: Vec<(String, Type, Expr)>,
        const_fields: Vec<(String, Type, Expr)>,
        methods: Vec<Decl>,
        span: Span,
    },
    Enum {
        id: HirId,
        name: String,
        generic_params: Option<Vec<(String, Option<Type>)>>,
        variants: Vec<EnumVariant>,
        span: Span,
    },
    Function {
        id: HirId,
        name: String,
        generic_params: Option<Vec<(String, Option<Type>)>>,
        params: Vec<(HirId, String, Type)>,
        return_type: Option<Type>,
        body: Block,
        is_static: bool,
        span: Span,
    },
    Expr(Expr, Span),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub id: HirId,
    pub fields: Option<Vec<(String, Type)>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLiteral(String, Span),
    FloatLiteral(String, Span),
    BoolLiteral(bool, Span),
    StringLiteral(String, Span),
    Ident(HirId, String, Span),
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
    Ident(HirId, String, Span),
    Variant {
        name: String,
        fields: Option<Vec<(HirId, String, Span)>>,
        span: Span,
    },
    CatchAll(Span),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLiteral(_, span) => *span,
            Expr::FloatLiteral(_, span) => *span,
            Expr::BoolLiteral(_, span) => *span,
            Expr::StringLiteral(_, span) => *span,
            Expr::Ident(_, _, s) => *s,
            Expr::Binary { span, .. } => *span,
            Expr::MemberAccess { span, .. } => *span,
            Expr::Call { span, .. } => *span,
            Expr::BuiltinCall(_, _, span) => *span,
            Expr::If { span, .. } => *span,
            Expr::While { span, .. } => *span,
            Expr::Assign { span, .. } => *span,
            Expr::Match { span, .. } => *span,
        }
    }
}
