pub mod constant_fold;
pub mod dce;
pub mod escape_analysis;

pub use constant_fold::ConstantFolder;
pub use dce::{DceOptimizer, Devirtualizer};
pub use escape_analysis::EscapeAnalyzer;
