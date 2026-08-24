use crate::expr::Expr;

#[derive(Debug, Clone, PartialEq)]
pub struct TypeAnnotation {
    pub name: String,
    pub args: Vec<TypeAnnotation>,
    pub is_nullable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// An expression evaluated for its side effects
    Expr(Expr),
    /// A variable declaration (e.g., let x: Int = 5;)
    VarDecl {
        name: String,
        is_mutable: bool,
        type_annotation: Option<TypeAnnotation>,
        initializer: Option<Expr>,
        span: (usize, usize),
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
        name: String,
        generic_params: Option<Vec<String>>,
        params: Vec<Param>,
        return_type: Option<TypeAnnotation>,
        body: Vec<Stmt>,
        is_async: bool,
        visibility: Visibility,
        span: (usize, usize),
    },
    /// A class declaration
    ClassDecl {
        name: String,
        generic_params: Option<Vec<String>>,
        fields: Vec<Stmt>, // VarDecl
        methods: Vec<Stmt>, // FuncDecl
        implements: Option<TypeAnnotation>,
    },
    /// An interface declaration
    InterfaceDecl {
        name: String,
        generic_params: Option<Vec<String>>,
        methods: Vec<Stmt>, // FuncDecl without body
    },
    /// A struct declaration
    StructDecl {
        name: String,
        generic_params: Option<Vec<String>>,
        fields: Vec<Stmt>, // VarDecl
    },
    /// A while loop
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
    /// An infinite loop
    Loop {
        body: Box<Stmt>,
    },
    /// A for-in loop
    ForIn {
        item: String,
        iterable: Expr,
        body: Box<Stmt>,
    },
    /// A pattern matching statement
    Match {
        expr: Expr,
        arms: Vec<(Expr, Box<Stmt>)>,
    },
    /// An import statement
    Import {
        path: String,
        items: Option<Vec<String>>,
    },
    /// A module containing statements
    Module {
        name: String,
        body: Vec<Stmt>,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub type_annotation: TypeAnnotation,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Visibility {
    #[default]
    Private,
    Public,
}
