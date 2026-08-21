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
    Bool(bool),
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
    AllocateTask(String),
    AllocateStruct(String),
    GetProperty(Value, String, String),
    GetStaticProperty(String, String),
    WeakUpgrade(Value),
    ForceUnwrap(Value),
    Array(Vec<Value>, bool),
    ArrayRepeat(Value, Value, bool),
    ArrayLength(Value),
    IndexGet(Value, Value),
    ConstructVariant(String, usize, Vec<Value>),
    ExtractPayload(String, Value, usize, usize, bool),
    GetVariantTag(Value),
    Await(Value),
    GetTaskResult(Value),
    Spawn(Value),
    ActorMailboxPush(Value, String, Vec<Value>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inst {
    Assign(Place, RValue),
    SetProperty(Value, String, String, Value, bool),
    SetStaticProperty(String, String, Value, bool),
    IndexSet(Value, Value, Value),

    Retain(Value),
    Release(Value),
    WeakRetain(Value),
    WeakRelease(Value),

    MemCopy(Value, Value, String),
    RegisterWaker(Value, Value),
    DropStruct(Value, String), // ptr, struct_name
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub usize);

#[derive(Debug, Clone, PartialEq)]
pub enum Terminator {
    Jump(BlockId),
    Branch {
        cond: Value,
        then_block: BlockId,
        else_block: BlockId,
    },
    Switch {
        cond: Value,
        cases: Vec<(usize, BlockId)>,
        default: Option<BlockId>,
    },
    Return(Option<Value>),
}
