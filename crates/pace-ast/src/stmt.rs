use crate::expr::Expr;
use crate::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct TypeAnnotation {
    pub module_prefix: Option<ustr::Ustr>,
    pub name: ustr::Ustr,
    pub args: Vec<TypeAnnotation>,
    pub is_nullable: bool,
    // Function type support
    pub is_function: bool,
    pub function_params: Option<Vec<TypeAnnotation>>,
    pub function_return: Option<Box<TypeAnnotation>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// An expression evaluated for its side effects
    Expr(Expr),
    /// A variable declaration (e.g., let x: Int = 5;)
    VarDecl {
        name: ustr::Ustr,
        is_mutable: bool,
        type_annotation: Option<TypeAnnotation>,
        is_static: bool,
        visibility: Visibility,
        initializer: Option<Expr>,
        span: Span,
    },
    /// A block of statements (e.g., { ... })
    Block(Vec<Stmt>),
    /// A return statement (e.g., return 42;)
    Return(Option<Expr>),
    /// An if statement
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    /// A function declaration
    FuncDecl {
        name: ustr::Ustr,
        generic_params: Option<Vec<ustr::Ustr>>,
        params: Vec<Param>,
        return_type: Option<TypeAnnotation>,
        body: Vec<Stmt>,
        is_async: bool,
        is_static: bool,
        visibility: Visibility,
        doc_comment: Option<ustr::Ustr>,
        span: Span,
    },
    ClassDecl {
        name: ustr::Ustr,
        generic_params: Option<Vec<ustr::Ustr>>,
        fields: Vec<Stmt>,  // VarDecl
        methods: Vec<Stmt>, // FuncDecl
        implements: Option<TypeAnnotation>,
        doc_comment: Option<ustr::Ustr>,
    },
    /// An actor declaration
    ActorDecl {
        name: ustr::Ustr,
        generic_params: Option<Vec<ustr::Ustr>>,
        fields: Vec<Stmt>,  // VarDecl
        methods: Vec<Stmt>, // FuncDecl
        implements: Option<TypeAnnotation>,
        doc_comment: Option<ustr::Ustr>,
    },
    /// An interface declaration
    InterfaceDecl {
        name: ustr::Ustr,
        generic_params: Option<Vec<ustr::Ustr>>,
        methods: Vec<Stmt>, // FuncDecl without body
        doc_comment: Option<ustr::Ustr>,
    },
    /// A struct declaration
    StructDecl {
        name: ustr::Ustr,
        generic_params: Option<Vec<ustr::Ustr>>,
        fields: Vec<Stmt>, // VarDecl
        doc_comment: Option<ustr::Ustr>,
    },
    /// An enum declaration
    EnumDecl {
        name: ustr::Ustr,
        generic_params: Option<Vec<ustr::Ustr>>,
        variants: Vec<EnumVariant>,
        doc_comment: Option<ustr::Ustr>,
    },
    /// A while loop
    While { condition: Expr, body: Box<Stmt> },
    /// An infinite loop
    Loop { body: Box<Stmt> },
    /// A for-in loop
    ForIn {
        item: ustr::Ustr,
        iterable: Expr,
        body: Box<Stmt>,
    },
    /// A pattern matching statement
    Match {
        expr: Expr,
        arms: Vec<(Pattern, Box<Stmt>)>,
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
    Module { name: ustr::Ustr, body: Vec<Stmt> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: ustr::Ustr,
    pub type_annotation: TypeAnnotation,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Visibility {
    #[default]
    Private,
    Public,
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
    Literal(Expr),
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
