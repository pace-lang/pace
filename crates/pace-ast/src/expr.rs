use crate::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A literal integer (e.g., 42)
    IntLiteral(i64),
    /// A literal float (e.g., 3.14)
    FloatLiteral(f64),
    /// A literal string (e.g., "hello")
    StringLiteral(String),
    /// An interpolated string (e.g., "hello ${name}")
    InterpolatedString(Vec<Expr>),
    /// A boolean literal (true / false)
    BoolLiteral(bool),
    /// A null literal
    Null,
    /// An identifier (e.g., my_var)
    Identifier(String, Span),
    /// A generic instantiation (e.g., Box<Int> or first<String>)
    GenericInstantiation {
        callee: Box<Expr>,
        generic_args: Vec<crate::stmt::TypeAnnotation>,
    },
    /// A binary operation (e.g., a + b)
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    /// An assignment operation (e.g., x = 5 or foo.bar = 10)
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
    },
    /// A member access (e.g., foo.bar or Class::static_method)
    MemberAccess {
        object: Box<Expr>,
        property: String,
        computed_class: Option<String>,
        is_static_operator: bool,
    },
    /// A forced unwrap (e.g., foo!)
    Unwrap(Box<Expr>),
    /// An optional member access (e.g., foo?.bar)
    OptionalMemberAccess {
        object: Box<Expr>,
        property: String,
    },
    /// A null coalesce operation (e.g., a ?? b)
    NullCoalesce {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// A try operator (e.g., foo?)
    Try(Box<Expr>),
    /// An await expression (e.g., await foo)
    Await(Box<Expr>),
    /// A closure (anonymous function)
    Closure {
        params: Vec<(String, crate::stmt::TypeAnnotation)>,
        return_type: Option<crate::stmt::TypeAnnotation>,
        body: Box<Expr>, // Using Expr for both implicit return expressions and blocks (Expr::Block)
    },

    /// A block expression
    Block(Vec<crate::stmt::Stmt>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    And,
    Or,
}
