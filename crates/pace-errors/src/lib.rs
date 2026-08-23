use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
#[error("Syntax error: {message}")]
#[diagnostic(
    code(pace::syntax_error),
    help("Check the language syntax guidelines for proper formatting.")
)]
pub struct ParseError {
    pub message: String,
    
    // In the future, we will add source_code and spans here for rich errors
    // #[source_code]
    // pub src: String,
    
    // #[label("Here")]
    // pub span: (usize, usize),
}
