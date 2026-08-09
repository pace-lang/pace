use crate::span::Span;
use crate::expr::Expr;

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// A let declaration: `let name = expression;`
    Let {
        name: String,
        initializer: Expr,
    },
    /// A var declaration: `var name = expression;`
    Var {
        name: String,
        initializer: Expr,
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
    /// A function declaration: `func name(params) -> return_type { body }`
    Func {
        name: String,
        params: Vec<(String, String)>, // (name, type)
        return_type: Option<String>,
        body: Box<Stmt>,
    },
    /// A return statement: `return value;`
    Return {
        value: Option<Expr>,
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
