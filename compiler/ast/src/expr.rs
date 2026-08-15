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
        path: Vec<session::Symbol>,
        bindings: Option<Vec<session::Symbol>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm<'a> {
    pub pattern: Pattern,
    pub body: &'a Expr<'a>, // Using Expr for fat arrow block or single expr
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind<'a> {
    /// A match expression: `match expr { ... }`
    Match {
        value: &'a Expr<'a>,
        arms: Vec<MatchArm<'a>>,
    },
    /// A literal integer like `10`
    Integer(i64),
    /// A literal float like `3.14`
    Float(f64),
    /// A literal string like `"hello"`
    String(session::Symbol),
    InterpolatedString(Vec<Expr<'a>>),
    /// A boolean literal like `true` or `false`
    Boolean(bool),
    /// A null literal `null`
    Null,
    /// A variable reference like `count`
    Variable(session::Symbol),
    Range {
        start: &'a Expr<'a>,
        end: &'a Expr<'a>,
    },
    /// A binary operation like `a + b`
    Binary(&'a Expr<'a>, BinaryOp, &'a Expr<'a>),
    /// A unary operation like `-a`
    Unary(UnaryOp, &'a Expr<'a>),
    /// A grouped expression like `(a + b)`
    Grouping(&'a Expr<'a>),
    /// A function call like `f<T>(a, b)`
    Call {
        callee: &'a Expr<'a>,
        type_args: Vec<TypeExpr<'a>>,
        arguments: Vec<Expr<'a>>,
    },
    /// Property access: `object.name`
    Get {
        object: &'a Expr<'a>,
        name: session::Symbol,
    },
    /// Property assignment: `object.name = value`
    Set {
        object: &'a Expr<'a>,
        name: session::Symbol,
        value: &'a Expr<'a>,
    },
    /// Variable assignment: `name = value`
    Assign {
        name: session::Symbol,
        value: &'a Expr<'a>,
    },
    /// Self reference: `self`
    SelfRef,
    /// Force unwrap: `expr!`
    ForceUnwrap(&'a Expr<'a>),
    /// Optional property access: `object?.name`
    OptionalGet {
        object: &'a Expr<'a>,
        name: session::Symbol,
    },
    /// Null Coalesce: `left ?? right`
    NullCoalesce {
        left: &'a Expr<'a>,
        right: &'a Expr<'a>,
    },
    /// Null Coalesce Assignment: `left ??= right`
    NullCoalesceAssign {
        left: &'a Expr<'a>,
        right: &'a Expr<'a>,
    },
    /// Array literal: `[1, 2, 3]`
    Array(Vec<Expr<'a>>),
    /// List comprehension: `[expr for item in iterator]`
    ListComprehension {
        expr: &'a Expr<'a>,
        item_name: session::Symbol,
        iterator: &'a Expr<'a>,
    },
    /// Array repeat initialization: `[0; 10]`
    ArrayRepeat {
        value: &'a Expr<'a>,
        count: &'a Expr<'a>,
    },
    /// Index access: `arr[i]`
    IndexGet {
        object: &'a Expr<'a>,
        index: &'a Expr<'a>,
    },
    /// Index assignment: `arr[i] = value`
    IndexSet {
        object: &'a Expr<'a>,
        index: &'a Expr<'a>,
        value: &'a Expr<'a>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr<'a> {
    pub kind: ExprKind<'a>,
    pub span: Span,
}

impl<'a> Expr<'a> {
    pub fn new(kind: ExprKind<'a>, span: Span) -> Self {
        Self { kind, span }
    }
}
