use miette::{Diagnostic, NamedSource};
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
#[error("Syntax error: {message}")]
#[diagnostic(
    code(pace::syntax_error),
    help("Check the language syntax guidelines for proper formatting.")
)]
pub struct ParseError {
    pub message: String,
    
    #[source_code]
    pub src: NamedSource<String>,
    
    #[label("Here")]
    pub span: (usize, usize),
}

#[derive(Error, Diagnostic, Debug)]
pub enum SemanticWarning {
    #[error("Variable or function '{name}' should use camelCase")]
    #[diagnostic(code(pace::naming_convention), severity(warning))]
    NamingConvention {
        name: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("Consider renaming to camelCase")]
        span: (usize, usize),
    },
    
    #[error("Unused {kind} '{name}'")]
    #[diagnostic(code(pace::unused_item), severity(warning))]
    UnusedItem {
        kind: String, // "variable" or "function"
        name: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("This is never used")]
        span: (usize, usize),
    },
}
