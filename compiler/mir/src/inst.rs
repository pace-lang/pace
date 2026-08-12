use ast::{BinaryOp, UnaryOp};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    Object(usize),
    Array(Vec<Value>),
    EnumVariant(String, usize, Vec<Value>),
    Void,
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RValue {
    Use(Value),
    BinaryOp(BinaryOp, Value, Value),
    UnaryOp(UnaryOp, Value),
    Call(String, Vec<Value>),
    MethodCall(Value, String, Vec<Value>),
    AllocateObject(String),
    GetProperty(Value, String),
    WeakUpgrade(Value),
    ForceUnwrap(Value),
    Array(Vec<Value>, bool),
    ArrayRepeat(Value, Value, bool),
    ArrayLength(Value),
    IndexGet(Value, Value),
    ConstructVariant(String, usize, Vec<Value>),
    ExtractPayload(Value, usize, usize),
    GetVariantTag(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inst {
    Assign(Place, RValue),
    SetProperty(Value, String, Value),
    IndexSet(Value, Value, Value),

    Retain(Value),
    Release(Value),
    WeakRetain(Value),
    WeakRelease(Value),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub usize);

#[derive(Debug, Clone, PartialEq)]
pub enum Terminator {
    Jump(BlockId),
    Branch { cond: Value, then_block: BlockId, else_block: BlockId },
    Switch { cond: Value, cases: Vec<(usize, BlockId)>, default: Option<BlockId> },
    Return(Option<Value>),
}
