pub mod inst;
pub mod block;
pub mod function;
pub mod program;

pub use function::Function;
pub use inst::{Inst, RValue, Place, Value, BlockId, Terminator};
pub use block::BasicBlock;
pub use program::{Program, ClassDef, EnumDef, EnumVariantDef, ForeignFunction, ForeignAbiType};
