use diagnostics::Span;
use crate::expr::{BinaryOp, UnaryOp, Pattern};
use crate::stmt::{TypeExpr, EnumVariant};
use crate::types::Type;

#[derive(Debug, Clone, PartialEq)]
pub struct TypedMatchArm {
    pub pattern: Pattern,
    pub body: Box<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedExprKind {
    Integer(i64),
    Float(f64),
    String(session::Symbol),
    InterpolatedString(Vec<TypedExpr>),
    Boolean(bool),
    Null,
    Variable(session::Symbol),
    Range {
        start: Box<TypedExpr>,
        end: Box<TypedExpr>,
    },
    Binary(Box<TypedExpr>, BinaryOp, Box<TypedExpr>),
    Unary(UnaryOp, Box<TypedExpr>),
    Grouping(Box<TypedExpr>),
    Call {
        callee: Box<TypedExpr>,
        type_args: Vec<TypeExpr>,
        arguments: Vec<TypedExpr>,
    },
    Get {
        object: Box<TypedExpr>,
        name: session::Symbol,
    },
    Set {
        object: Box<TypedExpr>,
        name: session::Symbol,
        value: Box<TypedExpr>,
    },
    Assign {
        name: session::Symbol,
        value: Box<TypedExpr>,
    },
    SelfRef,
    ForceUnwrap(Box<TypedExpr>),
    OptionalGet {
        object: Box<TypedExpr>,
        name: session::Symbol,
    },
    NullCoalesce {
        left: Box<TypedExpr>,
        right: Box<TypedExpr>,
    },
    NullCoalesceAssign {
        left: Box<TypedExpr>,
        right: Box<TypedExpr>,
    },
    Array(Vec<TypedExpr>),
    ListComprehension {
        expr: Box<TypedExpr>,
        item_name: session::Symbol,
        iterator: Box<TypedExpr>,
    },
    ArrayRepeat {
        value: Box<TypedExpr>,
        count: Box<TypedExpr>,
    },
    IndexGet {
        object: Box<TypedExpr>,
        index: Box<TypedExpr>,
    },
    IndexSet {
        object: Box<TypedExpr>,
        index: Box<TypedExpr>,
        value: Box<TypedExpr>,
    },
    Match {
        value: Box<TypedExpr>,
        arms: Vec<TypedMatchArm>,
    },
    EnumVariant {
        enum_name: session::Symbol,
        variant_name: session::Symbol,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    pub ty: Type,
    pub span: Span,
}

impl TypedExpr {
    pub fn new(kind: TypedExprKind, ty: Type, span: Span) -> Self {
        Self { kind, ty, span }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedStmtKind {
    Let {
        name: session::Symbol,
        type_annotation: Option<TypeExpr>,
        initializer: Option<TypedExpr>,
    },
    Var {
        name: session::Symbol,
        type_annotation: Option<TypeExpr>,
        initializer: Option<TypedExpr>,
        is_weak: bool,
    },
    Expression(TypedExpr),
    Block(Vec<TypedStmt>),
    If {
        condition: TypedExpr,
        then_branch: Box<TypedStmt>,
        else_branch: Option<Box<TypedStmt>>,
    },
    While {
        condition: TypedExpr,
        body: Box<TypedStmt>,
    },
    For {
        item_name: session::Symbol,
        iterator: TypedExpr,
        body: Box<TypedStmt>,
    },
    Func {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        params: Vec<(session::Symbol, TypeExpr)>,
        return_type: Option<TypeExpr>,
        body: Box<TypedStmt>,
    },
    ForeignFunc {
        name: session::Symbol,
        params: Vec<(session::Symbol, TypeExpr)>,
        return_type: Option<TypeExpr>,
    },
    Return {
        value: Option<TypedExpr>,
    },
    Class {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        implements: Vec<session::Symbol>,
        methods: Vec<TypedStmt>,
        fields: Vec<TypedStmt>,
    },
    Interface {
        name: session::Symbol,
        methods: Vec<TypedStmt>,
    },
    Enum {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        variants: Vec<EnumVariant>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedStmt {
    pub kind: TypedStmtKind,
    pub span: Span,
}

impl TypedStmt {
    pub fn new(kind: TypedStmtKind, span: Span) -> Self {
        Self { kind, span }
    }
}
