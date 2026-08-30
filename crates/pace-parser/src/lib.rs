pub mod lexer;
pub mod parser;

pub use lexer::{Lexer, Token};
use pace_ast::arena::StmtId;
pub use parser::Parser;

pub fn parse(
    arena: &mut pace_ast::arena::AstArena,
    src: &str,
    file_name: &str,
) -> Result<(Vec<StmtId>, Vec<(usize, usize, String)>), Vec<pace_errors::SyntaxError>> {
    let mut parser = Parser::new_with_arena(src, file_name, arena);
    let stmts = parser.parse()?;
    Ok((stmts, parser.lexer.comments))
}

#[cfg(test)]
mod tests {

    use pace_ast::Stmt;

    #[test]
    fn test_parse_let_decl() {
        let src = "let x: Int = 5;";
        let mut arena = pace_ast::arena::AstArena::new();
        let (stmts, _) = crate::parse(&mut arena, src, "test").unwrap();
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn test_import_parsing() {
        let src = r#"
            import "std:string";
            import "./models/user";
            import "http";
        "#;
        let mut arena = pace_ast::arena::AstArena::new();
        let (stmts, _) = crate::parse(&mut arena, src, "test").unwrap();
        assert_eq!(stmts.len(), 3);

        match arena.get_stmt(stmts[0]) {
            Stmt::Import { path, .. } => assert_eq!(path.as_str(), "std:string"),
            _ => panic!("Expected Import"),
        }
        match arena.get_stmt(stmts[1]) {
            Stmt::Import { path, .. } => assert_eq!(path.as_str(), "./models/user"),
            _ => panic!("Expected Import"),
        }
        match arena.get_stmt(stmts[2]) {
            Stmt::Import { path, .. } => assert_eq!(path.as_str(), "http"),
            _ => panic!("Expected Import"),
        }
    }
}
