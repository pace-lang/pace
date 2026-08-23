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
    Identifier(String),
    /// A binary operation (e.g., a + b)
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    /// A function call (e.g., foo(x))
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    /// A member access (e.g., foo.bar)
    MemberAccess {
        object: Box<Expr>,
        property: String,
        computed_class: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    NotEq,
}
