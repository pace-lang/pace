pub mod block;
pub mod function;
pub mod inst;
pub mod program;

pub use block::BasicBlock;
pub use function::Function;
pub use inst::{BlockId, Inst, Place, RValue, Terminator, Value};
pub use program::{ClassDef, EnumDef, EnumVariantDef, ForeignAbiType, ForeignFunction, Program};
