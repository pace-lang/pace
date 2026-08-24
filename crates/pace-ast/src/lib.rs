pub mod expr;
pub mod stmt;

pub use expr::*;
pub use stmt::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_snapshot() {
        // let x: Int = 42;
        let ast = Stmt::VarDecl {
            name: "x".to_string(),
            is_mutable: false,
            type_annotation: Some("Int".to_string()),
            initializer: Some(Expr::IntLiteral(42)),
            span: (0, 0),
        };

        // Snapshot test the debug output of the AST
        insta::assert_debug_snapshot!(ast);
    }
}
