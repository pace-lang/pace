pub mod lexer;
pub mod parser;

pub use lexer::{Lexer, Token};
use pace_ast::Stmt;
pub use parser::Parser;

pub fn parse(
    src: &str,
    file_name: &str,
) -> Result<(Vec<Stmt>, Vec<(usize, usize, String)>), Vec<pace_errors::SyntaxError>> {
    let mut parser = Parser::new(src, file_name);
    let stmts = parser.parse()?;
    Ok((stmts, parser.lexer.comments))
}

#[cfg(test)]
mod tests {

    use pace_ast::Stmt;

    #[test]
    fn test_parse_let_decl() {
        let src = "let x: Int = 5;";
        let (stmts, _) = crate::parse(src, "test").unwrap();
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn test_import_parsing() {
        let src = r#"
            import "std:string";
            import "./models/user";
            import "http";
        "#;
        let (stmts, _) = crate::parse(src, "test").unwrap();
        assert_eq!(stmts.len(), 3);

        match &stmts[0] {
            Stmt::Import { path, .. } => assert_eq!(path, "std:string"),
            _ => panic!("Expected Import"),
        }
        match &stmts[1] {
            Stmt::Import { path, .. } => assert_eq!(path, "./models/user"),
            _ => panic!("Expected Import"),
        }
        match &stmts[2] {
            Stmt::Import { path, .. } => assert_eq!(path, "http"),
            _ => panic!("Expected Import"),
        }
    }
}
