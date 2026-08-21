use crate::expr::{BinaryOp, Pattern, UnaryOp};
use crate::stmt::{EnumVariant, TypeExpr};
use diagnostics::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct TypedMatchArm<'a> {
    pub pattern: Pattern,
    pub body: &'a TypedExpr<'a>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedExprKind<'a> {
    Integer(i64),
    Float(f64),
    String(session::Symbol),
    InterpolatedString(Vec<TypedExpr<'a>>),
    Boolean(bool),
    Null,
    Variable(session::Symbol),
    Range {
        start: &'a TypedExpr<'a>,
        end: &'a TypedExpr<'a>,
    },
    Binary(&'a TypedExpr<'a>, BinaryOp, &'a TypedExpr<'a>),
    Unary(UnaryOp, &'a TypedExpr<'a>),
    Grouping(&'a TypedExpr<'a>),
    Call {
        callee: &'a TypedExpr<'a>,
        type_args: Vec<TypeExpr<'a>>,
        arguments: Vec<TypedExpr<'a>>,
    },
    Get {
        object: &'a TypedExpr<'a>,
        name: session::Symbol,
        is_static: bool,
    },
    Set {
        object: &'a TypedExpr<'a>,
        name: session::Symbol,
        value: &'a TypedExpr<'a>,
        is_static: bool,
    },
    Assign {
        name: session::Symbol,
        value: &'a TypedExpr<'a>,
    },
    SelfRef,
    ForceUnwrap(&'a TypedExpr<'a>),
    PostfixTry(&'a TypedExpr<'a>),
    OptionalGet {
        object: &'a TypedExpr<'a>,
        name: session::Symbol,
    },
    NullCoalesce {
        left: &'a TypedExpr<'a>,
        right: &'a TypedExpr<'a>,
    },
    NullCoalesceAssign {
        left: &'a TypedExpr<'a>,
        right: &'a TypedExpr<'a>,
    },
    Ternary {
        condition: &'a TypedExpr<'a>,
        true_expr: &'a TypedExpr<'a>,
        false_expr: &'a TypedExpr<'a>,
    },
    Array(Vec<TypedExpr<'a>>),
    ListComprehension {
        expr: &'a TypedExpr<'a>,
        item_name: session::Symbol,
        iterator: &'a TypedExpr<'a>,
    },
    ArrayRepeat {
        value: &'a TypedExpr<'a>,
        count: &'a TypedExpr<'a>,
    },
    IndexGet {
        object: &'a TypedExpr<'a>,
        index: &'a TypedExpr<'a>,
    },
    IndexSet {
        object: &'a TypedExpr<'a>,
        index: &'a TypedExpr<'a>,
        value: &'a TypedExpr<'a>,
    },
    Match {
        value: &'a TypedExpr<'a>,
        arms: Vec<TypedMatchArm<'a>>,
    },
    EnumVariant {
        enum_name: session::Symbol,
        variant_name: session::Symbol,
    },
    Await(&'a TypedExpr<'a>),
    Spawn(&'a TypedExpr<'a>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedExpr<'a> {
    pub kind: TypedExprKind<'a>,
    pub ty: session::TypeId,
    pub span: Span,
}

impl<'a> TypedExpr<'a> {
    pub fn new(kind: TypedExprKind<'a>, ty: session::TypeId, span: Span) -> Self {
        Self { kind, ty, span }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedStmtKind<'a> {
    Let {
        name: session::Symbol,
        type_annotation: Option<TypeExpr<'a>>,
        initializer: Option<&'a TypedExpr<'a>>,
        is_static: bool,
    },
    Var {
        name: session::Symbol,
        type_annotation: Option<TypeExpr<'a>>,
        initializer: Option<&'a TypedExpr<'a>>,
        is_weak: bool,
        is_static: bool,
    },
    Expression(&'a TypedExpr<'a>),
    Block(Vec<TypedStmt<'a>>),
    If {
        condition: &'a TypedExpr<'a>,
        then_branch: &'a TypedStmt<'a>,
        else_branch: Option<&'a TypedStmt<'a>>,
    },
    While {
        condition: &'a TypedExpr<'a>,
        body: &'a TypedStmt<'a>,
    },
    For {
        item_name: session::Symbol,
        iterator: &'a TypedExpr<'a>,
        body: &'a TypedStmt<'a>,
        item_ty: session::TypeId,
    },
    Func {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        params: Vec<(session::Symbol, TypeExpr<'a>)>,
        return_type: Option<TypeExpr<'a>>,
        body: &'a TypedStmt<'a>,
        is_async: bool,
        is_static: bool,
    },
    ForeignFunc {
        name: session::Symbol,
        base_name: session::Symbol,
        params: Vec<(session::Symbol, TypeExpr<'a>)>,
        return_type: Option<TypeExpr<'a>>,
        is_static: bool,
    },
    Return {
        value: Option<&'a TypedExpr<'a>>,
    },
    Class {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        implements: Vec<TypeExpr<'a>>,
        methods: Vec<TypedStmt<'a>>,
        fields: Vec<TypedStmt<'a>>,
        is_actor: bool,
    },
    Struct {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        methods: Vec<TypedStmt<'a>>,
        fields: Vec<TypedStmt<'a>>,
    },
    TypeAlias {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        target_type: TypeExpr<'a>,
    },
    Interface {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        methods: Vec<TypedStmt<'a>>,
    },
    Enum {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        variants: Vec<EnumVariant<'a>>,
        methods: Vec<TypedStmt<'a>>,
    },
    Extension {
        target_type: session::TypeId,
        methods: Vec<TypedStmt<'a>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedStmt<'a> {
    pub kind: TypedStmtKind<'a>,
    pub span: Span,
}

impl<'a> TypedStmt<'a> {
    pub fn new(kind: TypedStmtKind<'a>, span: Span) -> Self {
        Self { kind, span }
    }
}
