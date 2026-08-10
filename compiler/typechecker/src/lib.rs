pub mod types;
pub mod env;
pub mod checker;

pub use types::Type;
pub use env::{TypeEnvironment, Scope};
pub use checker::TypeChecker;
