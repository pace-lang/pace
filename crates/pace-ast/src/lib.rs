pub mod expr;
pub mod stmt;
pub mod span;
pub mod arena;
pub mod clone;

pub use expr::*;
pub use stmt::*;
pub use span::*;
pub use arena::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_snapshot() {
        // let x: Int = 42;
        let ast = Stmt::VarDecl {
            name: ustr::Ustr::from("x"),
            is_mutable: false,
            is_static: false,
            visibility: crate::stmt::Visibility::Public,
            type_annotation: Some(crate::stmt::TypeAnnotation {
                module_prefix: None,
                name: ustr::Ustr::from("Int"),
                args: vec![],
                is_nullable: false,
                is_function: false,
                function_params: None,
                function_return: None,
            }),
            initializer: Some(Expr::IntLiteral(42)),
            span: span::Span::default(),
        };

        // Snapshot test the debug output of the AST
        insta::assert_debug_snapshot!(ast);
    }
}
