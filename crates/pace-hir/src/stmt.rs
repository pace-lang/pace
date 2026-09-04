use crate::arena::{ExprId, StmtId};
use pace_ast::{TypeAnnotation, Visibility};
use ustr::Ustr;

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// An expression evaluated for its side effects
    Expr(ExprId),
    /// A variable declaration (e.g., let x: Int = 5;)
    VarDecl {
        name: Ustr,
        is_mutable: bool,
        type_annotation: Option<TypeAnnotation>,
        is_static: bool,
        visibility: Visibility,
        initializer: Option<ExprId>,
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
        name: Ustr,
        generic_params: Option<Vec<pace_ast::GenericParam>>,
        params: Vec<Param>,
        return_type: Option<TypeAnnotation>,
        body: Vec<StmtId>,
        is_async: bool,
        is_static: bool,
        is_extern: bool,
        visibility: Visibility,
    },
    ClassDecl {
        name: Ustr,
        generic_params: Option<Vec<pace_ast::GenericParam>>,
        fields: Vec<StmtId>,  // VarDecl
        methods: Vec<StmtId>, // FuncDecl
        implements: Option<TypeAnnotation>,
        visibility: Visibility,
    },
    /// An actor declaration
    ActorDecl {
        name: Ustr,
        generic_params: Option<Vec<pace_ast::GenericParam>>,
        fields: Vec<StmtId>,  // VarDecl
        methods: Vec<StmtId>, // FuncDecl
        implements: Option<TypeAnnotation>,
        visibility: Visibility,
    },
    /// An interface declaration
    InterfaceDecl {
        name: Ustr,
        generic_params: Option<Vec<pace_ast::GenericParam>>,
        methods: Vec<StmtId>, // FuncDecl without body
        visibility: Visibility,
    },
    /// A struct declaration
    StructDecl {
        name: Ustr,
        generic_params: Option<Vec<pace_ast::GenericParam>>,
        fields: Vec<StmtId>, // VarDecl
        visibility: Visibility,
    },
    /// An enum declaration
    EnumDecl {
        name: Ustr,
        generic_params: Option<Vec<pace_ast::GenericParam>>,
        variants: Vec<EnumVariant>,
        visibility: Visibility,
    },
    /// A while loop
    While { condition: ExprId, body: StmtId },
    /// An infinite loop
    Loop { body: StmtId },
    /// A for-in loop
    ForIn {
        item: Ustr,
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
        path: Ustr,
        alias: Option<Ustr>,
        show: Option<Vec<Ustr>>,
        hide: Option<Vec<Ustr>>,
    },
    /// An export statement
    Export { path: Ustr },
    /// A module containing statements
    Module { name: Ustr, body: Vec<StmtId> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: Ustr,
    pub type_annotation: TypeAnnotation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: Ustr,
    pub fields: Option<Vec<TypeAnnotation>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// A catch-all pattern (e.g. `_`)
    Wildcard,
    /// A literal match (e.g. `5`, `"hello"`)
    Literal(ExprId),
    /// A variable binding (e.g. `x`)
    Variable(Ustr),
    /// An enum variant pattern (e.g. `Some(x)`)
    Variant {
        enum_name: Option<Ustr>,
        variant_name: Ustr,
        fields: Option<Vec<Pattern>>,
        generic_args: Option<Vec<TypeAnnotation>>,
    },
}
