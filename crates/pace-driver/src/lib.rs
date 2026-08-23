use miette::{Result, IntoDiagnostic, Report};
use pace_ast::Stmt;
use pace_errors::ParseError;

pub struct CompilerSession;

impl CompilerSession {
    pub fn new() -> Self {
        Self
    }

    pub fn check_file(&self, path: &str) -> Result<Vec<Stmt>> {
        let src = std::fs::read_to_string(path)
            .into_diagnostic()?;
        
        self.check_source(&src)
    }

    pub fn check_source(&self, src: &str) -> Result<Vec<Stmt>> {
        match pace_parser::parse(src) {
            Ok(ast) => Ok(ast),
            Err(err_msg) => {
                let err = ParseError {
                    message: err_msg,
                };
                // In the future, we will attach source spans to this error
                Err(Report::new(err))
            }
        }
    }
}
