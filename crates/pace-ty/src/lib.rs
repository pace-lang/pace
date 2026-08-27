pub mod checker;
pub mod env;

pub use checker::TypeChecker;
pub use pace_errors::TypeError;
pub use env::{Environment, Type};

pub fn check(stmts: &[pace_ast::Stmt], src: &str, file_name: &str) -> (Vec<pace_errors::SemanticWarning>, Vec<TypeError>, Environment) {
    let mut checker = TypeChecker::new(src, file_name);
    checker.check(stmts);
    (checker.warnings, checker.errors, checker.env)
}
