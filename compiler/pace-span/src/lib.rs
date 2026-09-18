mod span;
pub mod symbol;

pub use span::{FileId, SourceMap, Span};
pub use symbol::{Symbol, intern};
