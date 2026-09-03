pub mod basic_block;
pub mod body;
pub mod lower;
pub mod statement;
pub mod opt;

pub use basic_block::{BasicBlock, BasicBlockData, SwitchTargets, Terminator};
pub use body::{LocalDecl, LocalKind, MirBody, Mutability};
pub use lower::{MirBuilder, MirProgram};
pub use statement::{AggregateKind, BorrowKind, Constant, Local, Operand, Place, PlaceBase, ProjectionElem, Rvalue, Statement, UnaryOp};
pub use opt::optimize;
