pub mod inst;
pub mod block;
pub mod function;

pub use inst::{Place, Value, RValue, Inst, Terminator, BlockId};
pub use block::BasicBlock;
pub use function::Function;
