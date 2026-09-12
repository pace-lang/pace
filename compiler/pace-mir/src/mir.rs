use pace_ast::BinaryOp;
use pace_ty::Ty;

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
pub enum Lvalue {
    Local(Local),
    FieldAccess(Local, String),
    EnumFieldAccess(Local, String, String), // Local, variant_name, field_name
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Assign(Lvalue, Rvalue),
    Retain(Lvalue),
    Release(Lvalue),
    GlobalWrite(String, Local),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Rvalue {
    Use(Local),
    BinaryOp(BinaryOp, Local, Local),
    IntConstant(String),
    FloatConstant(String),
    BoolConstant(bool),
    StringConstant(String),
    Call(Local, Vec<Local>),
    BuiltinCall(String, Vec<Local>),
    GlobalCall(String, Vec<Local>),
    VirtualCall(usize, Local, Vec<Local>),
    GlobalRead(String),
    FieldAccess(Local, String),
    EnumFieldAccess(Local, String, String),
    Instantiate(pace_ty::Ty, Vec<Local>),
    InstantiateEnum(pace_hir::HirId, String, Vec<Local>),
    EnumTag(Local),
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
    pub locals: Vec<pace_ty::Ty>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirFunction {
    pub name: String,
    pub params: Vec<Local>,
    pub return_type: Ty,
    pub body: MirBody,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirProgram {
    pub functions: Vec<MirFunction>,
    pub main_body: MirBody,
    pub struct_defs: std::collections::HashMap<pace_hir::HirId, Vec<(String, pace_ty::Ty, bool)>>,
    pub class_defs: std::collections::HashMap<pace_hir::HirId, Vec<(String, pace_ty::Ty, bool)>>,
    pub class_vtables: std::collections::HashMap<pace_hir::HirId, Vec<(String, Ty, String)>>,
    pub enum_defs: std::collections::HashMap<
        pace_hir::HirId,
        Vec<(String, Option<Vec<(String, pace_ty::Ty)>>)>,
    >,
    pub global_vars: Vec<(String, pace_ty::Ty)>,
}
