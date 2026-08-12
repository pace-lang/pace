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
    String(String),
    InterpolatedString(Vec<TypedExpr>),
    Boolean(bool),
    Null,
    Variable(String),
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
        name: String,
    },
    Set {
        object: Box<TypedExpr>,
        name: String,
        value: Box<TypedExpr>,
    },
    Assign {
        name: String,
        value: Box<TypedExpr>,
    },
    SelfRef,
    ForceUnwrap(Box<TypedExpr>),
    OptionalGet {
        object: Box<TypedExpr>,
        name: String,
    },
    Array(Vec<TypedExpr>),
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
        enum_name: String,
        variant_name: String,
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
        name: String,
        type_annotation: Option<TypeExpr>,
        initializer: Option<TypedExpr>,
    },
    Var {
        name: String,
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
        item_name: String,
        iterator: TypedExpr,
        body: Box<TypedStmt>,
    },
    Func {
        name: String,
        type_params: Vec<String>,
        params: Vec<(String, TypeExpr)>,
        return_type: Option<TypeExpr>,
        body: Box<TypedStmt>,
    },
    ForeignFunc {
        name: String,
        params: Vec<(String, TypeExpr)>,
        return_type: Option<TypeExpr>,
    },
    Return {
        value: Option<TypedExpr>,
    },
    Class {
        name: String,
        type_params: Vec<String>,
        implements: Vec<String>,
        methods: Vec<TypedStmt>,
        fields: Vec<TypedStmt>,
    },
    Interface {
        name: String,
        methods: Vec<TypedStmt>,
    },
    Enum {
        name: String,
        type_params: Vec<String>,
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
