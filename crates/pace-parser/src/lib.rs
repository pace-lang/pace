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

    #[test]
    fn test_parse_func_decl() {
        let src = "
            func add(a: Int, b: Int) -> Int {
                return a + b;
            }
        ";
        let mut arena = pace_ast::arena::AstArena::new();
        let (stmts, _) = crate::parse(&mut arena, src, "test").unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::FuncDecl {
            name,
            params,
            return_type,
            ..
        } = arena.get_stmt(stmts[0])
        {
            assert_eq!(name.as_str(), "add");
            assert_eq!(params.len(), 2);
            assert!(return_type.is_some());
        } else {
            panic!("Expected FuncDecl");
        }
    }

    #[test]
    fn test_parse_class_decl() {
        let src = "
            class User {
                let name: String;
                func init(name: String) {
                    self.name = name;
                }
            }
        ";
        let mut arena = pace_ast::arena::AstArena::new();
        let (stmts, _) = crate::parse(&mut arena, src, "test").unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::ClassDecl {
            name,
            fields,
            methods,
            ..
        } = arena.get_stmt(stmts[0])
        {
            assert_eq!(name.as_str(), "User");
            assert_eq!(fields.len(), 1);
            assert_eq!(methods.len(), 1);
        } else {
            panic!("Expected ClassDecl");
        }
    }

    #[test]
    fn test_parse_control_flow() {
        let src = "
            func process(x: Int) {
                if x > 10 {
                    return x;
                } else {
                    while x < 10 {
                    }
                }
            }
        ";
        let mut arena = pace_ast::arena::AstArena::new();
        let (stmts, _) = crate::parse(&mut arena, src, "test").unwrap();
        assert_eq!(stmts.len(), 1);
    }
}
