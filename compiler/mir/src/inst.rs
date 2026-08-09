use ast::{BinaryOp, UnaryOp};

#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    Var(String),
    Temp(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Place(Place),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RValue {
    Use(Value),
    BinaryOp(BinaryOp, Value, Value),
    UnaryOp(UnaryOp, Value),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inst {
    Assign(Place, RValue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub usize);

#[derive(Debug, Clone, PartialEq)]
pub enum Terminator {
    Jump(BlockId),
    Branch { cond: Value, then_block: BlockId, else_block: BlockId },
    Return(Option<Value>),
}
