use crate::arena::{ExprId, StmtId};
use pace_common::{BinaryOp, Span, UnaryOp};

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A literal integer (e.g., 42)
    IntLiteral(i64),
    /// A literal float (e.g., 3.14)
    FloatLiteral(f64),
    /// A literal string (e.g., "hello")
    StringLiteral(ustr::Ustr),
    /// An interpolated string (e.g., "hello ${name}")
    InterpolatedString(Vec<ExprId>),
    /// A boolean literal (true / false)
    BoolLiteral(bool),
    /// A null literal
    Null,
    /// An identifier (e.g., my_var)
    Identifier(ustr::Ustr, Span),
    /// A generic instantiation (e.g., Box<Int> or first<String>)
    GenericInstantiation {
        callee: ExprId,
        generic_args: Vec<pace_common::TypeAnnotation>,
    },
    /// A unary operation (e.g. !a, -5)
    Unary {
        op: UnaryOp,
        expr: ExprId,
    },
    /// A binary operation (e.g., a + b)
    Binary {
        left: ExprId,
        op: BinaryOp,
        right: ExprId,
    },
    Call {
        callee: ExprId,
        args: Vec<ExprId>,
    },
    /// An assignment operation (e.g., x = 5 or foo.bar = 10)
    Assign {
        target: ExprId,
        value: ExprId,
    },
    /// A member access (e.g., foo.bar or Class::static_method)
    MemberAccess {
        object: ExprId,
        property: ustr::Ustr,
        computed_class: Option<ustr::Ustr>,
        is_static_operator: bool,
    },
    /// A forced unwrap (e.g., foo!)
    Unwrap(ExprId),
    /// An optional member access (e.g., foo?.bar)
    OptionalMemberAccess {
        object: ExprId,
        property: ustr::Ustr,
    },
    /// A null coalesce operation (e.g., a ?? b)
    NullCoalesce {
        left: ExprId,
        right: ExprId,
    },
    /// A try operator (e.g., foo?)
    Try(ExprId),
    /// An await expression (e.g., await foo)
    Await(ExprId),
    /// A closure (anonymous function)
    Closure {
        params: Vec<(ustr::Ustr, pace_common::TypeAnnotation)>,
        return_type: Option<pace_common::TypeAnnotation>,
        body: ExprId, // Using Expr for both implicit return expressions and blocks (Expr::Block)
    },

    /// A block expression
    Block(Vec<StmtId>),
}
