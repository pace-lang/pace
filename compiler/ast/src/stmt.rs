use diagnostics::Span;
use crate::expr::Expr;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Named(String),
    GenericInstance(String, Vec<TypeExpr>),
    Optional(Box<TypeExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// A let declaration: `let name = expression;`
    Let {
        name: String,
        type_annotation: Option<TypeExpr>,
        initializer: Option<Expr>,
    },
    /// A var declaration: `[weak] var name: type = expression;`
    Var {
        name: String,
        type_annotation: Option<TypeExpr>,
        initializer: Option<Expr>,
        is_weak: bool,
    },
    /// An expression evaluated for side effects: `10 + 20;`
    Expression(Expr),
    /// A block of statements: `{ ... }`
    Block(Vec<Stmt>),
    /// An if statement: `if condition { then_branch } else { else_branch }`
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    /// A while loop: `while condition { body }`
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
    /// A for loop: `for item in iterator { body }`
    For {
        item_name: String,
        iterator: Expr,
        body: Box<Stmt>,
    },
    /// A function declaration: `func name<T>(params) -> return_type { body }`
    Func {
        name: String,
        type_params: Vec<String>,
        params: Vec<(String, TypeExpr)>, // (name, type)
        return_type: Option<TypeExpr>,
        body: Box<Stmt>,
    },
    /// A return statement: `return value;`
    Return {
        value: Option<Expr>,
    },
    /// A class declaration: `class name<T> { fields; methods; }`
    Class {
        name: String,
        type_params: Vec<String>,
        implements: Vec<String>,
        methods: Vec<Stmt>,
        fields: Vec<Stmt>,
    },
    /// An interface declaration: `interface name { methods; }`
    Interface {
        name: String,
        methods: Vec<Stmt>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

impl Stmt {
    pub fn new(kind: StmtKind, span: Span) -> Self {
        Self { kind, span }
    }
}
