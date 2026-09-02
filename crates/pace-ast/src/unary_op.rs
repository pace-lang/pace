#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// Logical NOT (e.g. `!true`)
    Not,
    /// Negation (e.g. `-5`)
    Neg,
    /// Bitwise NOT (e.g. `~x`)
    BitNot,
}
