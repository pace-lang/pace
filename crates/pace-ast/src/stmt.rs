use crate::expr::Expr;

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// An expression evaluated for its side effects
    Expr(Expr),
    /// A variable declaration (e.g., let x: Int = 5;)
    VarDecl {
        name: String,
        is_mutable: bool,
        type_annotation: Option<String>,
        initializer: Option<Expr>,
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
        params: Vec<Param>,
        return_type: Option<String>,
        body: Vec<Stmt>,
        is_async: bool,
        visibility: Visibility,
    },
    /// A class declaration
    ClassDecl {
        name: String,
        fields: Vec<Stmt>, // VarDecl
        methods: Vec<Stmt>, // FuncDecl
        implements: Option<String>,
    },
    /// An interface declaration
    InterfaceDecl {
        name: String,
        methods: Vec<Stmt>, // FuncDecl without body
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub type_annotation: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Visibility {
    #[default]
    Private,
    Public,
}
