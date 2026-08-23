pub mod checker;
pub mod env;

pub use checker::{TypeChecker, TypeError};
pub use env::{Environment, Type};

pub fn check(stmts: &[pace_ast::Stmt]) -> Result<(), TypeError> {
    let mut checker = TypeChecker::new();
    checker.check(stmts)
}
