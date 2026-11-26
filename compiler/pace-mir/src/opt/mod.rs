pub mod constant_fold;
pub mod escape_analysis;

pub use constant_fold::ConstantFolder;
pub use escape_analysis::EscapeAnalyzer;
