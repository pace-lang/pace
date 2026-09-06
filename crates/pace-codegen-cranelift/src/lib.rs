use miette::Diagnostic;
use thiserror::Error;

pub mod aot;
pub mod context;
pub mod translator;

pub use aot::AotCompiler;
pub mod runtime;

#[derive(Error, Diagnostic, Debug)]
#[error("Codegen error: {message}")]
#[diagnostic(code(pace::codegen_error))]
pub struct CodegenError {
    pub message: String,
}
