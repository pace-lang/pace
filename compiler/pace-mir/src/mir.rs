use pace_ast::BinaryOp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Local(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BasicBlockId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct BasicBlock {
    pub statements: Vec<Statement>,
    pub terminator: Option<Terminator>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Assign(Local, Rvalue),
    Retain(Local),
    Release(Local),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Rvalue {
    Use(Local),
    BinaryOp(BinaryOp, Local, Local),
    IntConstant(String),
    StringConstant(String),
    Call(Local, Vec<Local>),
    BuiltinCall(String, Vec<Local>),
    GlobalCall(String, Vec<Local>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Terminator {
    Return(Local),
    Goto(BasicBlockId),
    Branch {
        cond: Local,
        then_block: BasicBlockId,
        else_block: BasicBlockId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirBody {
    pub blocks: Vec<BasicBlock>,
    pub locals: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirFunction {
    pub name: String,
    pub params: Vec<Local>,
    pub body: MirBody,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirProgram {
    pub functions: Vec<MirFunction>,
    pub main_body: MirBody,
}
