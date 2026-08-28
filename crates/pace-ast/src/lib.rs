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
            is_static: false,
            visibility: crate::stmt::Visibility::Public,
            type_annotation: Some(crate::stmt::TypeAnnotation {
                module_prefix: None,
                name: "Int".to_string(),
                args: vec![],
                is_nullable: false,
                is_function: false,
                function_params: None,
                function_return: None
            }),
            initializer: Some(Expr::IntLiteral(42)),
            span: (0, 0),
        };

        // Snapshot test the debug output of the AST
        insta::assert_debug_snapshot!(ast);
    }
}
