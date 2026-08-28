pub mod checker;
pub mod env;

pub use checker::TypeChecker;
pub use pace_errors::TypeError;
pub use env::{Environment, Type};

pub use checker::check;
