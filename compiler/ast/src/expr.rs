use diagnostics::Span;
use crate::stmt::TypeExpr;

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Negate,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_`
    Wildcard,
    /// `Ok(x)` or `Move(x, y)` or `Quit`
    Variant {
        // Can be just "Quit" or "Message.Quit"
        path: Vec<String>,
        bindings: Option<Vec<String>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Box<Expr>, // Using Expr for fat arrow block or single expr
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// A match expression: `match expr { ... }`
    Match {
        value: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    /// A literal integer like `10`
    Integer(i64),
    /// A literal float like `3.14`
    Float(f64),
    /// A literal string like `"hello"`
    String(String),
    InterpolatedString(Vec<Expr>),
    /// A boolean literal like `true` or `false`
    Boolean(bool),
    /// A null literal `null`
    Null,
    /// A variable reference like `count`
    Variable(String),
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
    },
    /// A binary operation like `a + b`
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    /// A unary operation like `-a`
    Unary(UnaryOp, Box<Expr>),
    /// A grouped expression like `(a + b)`
    Grouping(Box<Expr>),
    /// A function call like `f<T>(a, b)`
    Call {
        callee: Box<Expr>,
        type_args: Vec<TypeExpr>,
        arguments: Vec<Expr>,
    },
    /// Property access: `object.name`
    Get {
        object: Box<Expr>,
        name: String,
    },
    /// Property assignment: `object.name = value`
    Set {
        object: Box<Expr>,
        name: String,
        value: Box<Expr>,
    },
    /// Variable assignment: `name = value`
    Assign {
        name: String,
        value: Box<Expr>,
    },
    /// Self reference: `self`
    SelfRef,
    /// Force unwrap: `expr!`
    ForceUnwrap(Box<Expr>),
    /// Optional property access: `object?.name`
    OptionalGet {
        object: Box<Expr>,
        name: String,
    },
    /// Array literal: `[1, 2, 3]`
    Array(Vec<Expr>),
    /// Array repeat initialization: `[0; 10]`
    ArrayRepeat {
        value: Box<Expr>,
        count: Box<Expr>,
    },
    /// Index access: `arr[i]`
    IndexGet {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    /// Index assignment: `arr[i] = value`
    IndexSet {
        object: Box<Expr>,
        index: Box<Expr>,
        value: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}
