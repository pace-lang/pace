pub mod arena;
pub mod expr;
pub mod lower;
pub mod stmt;

pub use arena::*;
pub use expr::*;
pub use lower::*;
pub use stmt::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hir_lowering() {
        let src = "func main() { let x = 5; }";
        let mut ast_arena = pace_ast::arena::AstArena::new();
        let (ast_stmts, _) = pace_parser::parse(&mut ast_arena, src, "test").unwrap();
        
        let builder = HirBuilder::new(&ast_arena);
        let (hir_arena, hir_stmts) = builder.build(&ast_stmts);
        
        assert_eq!(hir_stmts.len(), 1);
        let stmt = hir_arena.get_stmt(hir_stmts[0]);
        match stmt {
            Stmt::FuncDecl { name, body, .. } => {
                assert_eq!(name.as_str(), "main");
                assert_eq!(body.len(), 1);
            }
            _ => panic!("Expected FuncDecl"),
        }
    }
}
