use crate::arena::{ExprId, StmtId};
use crate::{Span, TypeAnnotation, Visibility};

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// An expression evaluated for its side effects
    Expr(ExprId),
    /// A variable declaration (e.g., let x: Int = 5;)
    VarDecl {
        name: ustr::Ustr,
        is_mutable: bool,
        type_annotation: Option<TypeAnnotation>,
        is_static: bool,
        is_weak: bool,
        visibility: Visibility,
        initializer: Option<ExprId>,
        span: Span,
    },
    /// A block of statements (e.g., { ... })
    Block(Vec<StmtId>),
    /// A return statement (e.g., return 42;)
    Return(Option<ExprId>),
    /// An if statement
    If {
        condition: ExprId,
        then_branch: StmtId,
        else_branch: Option<StmtId>,
    },
    /// A function declaration
    FuncDecl {
        name: ustr::Ustr,
        generic_params: Option<Vec<ustr::Ustr>>,
        params: Vec<Param>,
        return_type: Option<TypeAnnotation>,
        body: Vec<StmtId>,
        is_async: bool,
        is_static: bool,
        is_extern: bool,
        visibility: Visibility,
        span: Span,
    },
    ClassDecl {
        name: ustr::Ustr,
        generic_params: Option<Vec<ustr::Ustr>>,
        fields: Vec<StmtId>,  // VarDecl
        methods: Vec<StmtId>, // FuncDecl
        implements: Option<TypeAnnotation>,
    },
    /// An actor declaration
    ActorDecl {
        name: ustr::Ustr,
        generic_params: Option<Vec<ustr::Ustr>>,
        fields: Vec<StmtId>,  // VarDecl
        methods: Vec<StmtId>, // FuncDecl
        implements: Option<TypeAnnotation>,
    },
    /// An interface declaration
    InterfaceDecl {
        name: ustr::Ustr,
        generic_params: Option<Vec<ustr::Ustr>>,
        methods: Vec<StmtId>, // FuncDecl without body
    },
    /// A struct declaration
    StructDecl {
        name: ustr::Ustr,
        generic_params: Option<Vec<ustr::Ustr>>,
        fields: Vec<StmtId>, // VarDecl
    },
    /// An enum declaration
    EnumDecl {
        name: ustr::Ustr,
        generic_params: Option<Vec<ustr::Ustr>>,
        variants: Vec<EnumVariant>,
    },
    /// A while loop
    While { condition: ExprId, body: StmtId },
    /// An infinite loop
    Loop { body: StmtId },
    /// A for-in loop
    ForIn {
        item: ustr::Ustr,
        iterable: ExprId,
        body: StmtId,
    },
    /// A pattern matching statement
    Match {
        expr: ExprId,
        arms: Vec<(Pattern, StmtId)>,
    },
    /// An import statement
    Import {
        path: ustr::Ustr,
        alias: Option<ustr::Ustr>,
        show: Option<Vec<ustr::Ustr>>,
        hide: Option<Vec<ustr::Ustr>>,
    },
    /// An export statement
    Export { path: ustr::Ustr },
    /// A module containing statements
    Module { name: ustr::Ustr, body: Vec<StmtId> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: ustr::Ustr,
    pub type_annotation: TypeAnnotation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: ustr::Ustr,
    pub fields: Option<Vec<TypeAnnotation>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// A catch-all pattern (e.g. `_`)
    Wildcard,
    /// A literal match (e.g. `5`, `"hello"`)
    Literal(ExprId),
    /// A variable binding (e.g. `x`) with a span
    Variable(ustr::Ustr, Span),
    /// An enum variant pattern (e.g. `Some(x)`)
    Variant {
        enum_name: Option<ustr::Ustr>,
        variant_name: ustr::Ustr,
        fields: Option<Vec<Pattern>>,
        generic_args: Option<Vec<TypeAnnotation>>,
    },
}
