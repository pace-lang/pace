pub mod arena;
pub mod clone;
pub mod expr;
pub mod stmt;

pub use arena::*;
pub use expr::*;
pub use pace_common::{BinaryOp, TypeAnnotation, UnaryOp, Visibility};
pub use pace_span::Span;
pub use stmt::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_snapshot() {
        let mut arena = AstArena::new();
        let init_expr = arena.alloc_expr(Expr::IntLiteral(42));
        // let x: Int = 42;
        let ast = Stmt::VarDecl {
            name: ustr::Ustr::from("x"),
            is_mutable: false,
            is_static: false,
            visibility: Visibility::Public,
            type_annotation: Some(pace_common::TypeAnnotation {
                module_prefix: None,
                name: ustr::Ustr::from("Int"),
                args: vec![],
                is_nullable: false,
                is_function: false,
                function_params: None,
                function_return: None,
            }),
            initializer: Some(init_expr),
            span: Span::default(),
        };

        // Snapshot test the debug output of the AST
        insta::assert_debug_snapshot!(ast);
    }
}
