pub mod parser;

pub use parser::Parser;

#[cfg(test)]
mod tests {
    use super::*;
    use pace_lexer::Lexer;
    use pace_ast::{Decl, Expr};

    #[test]
    fn test_parse_let_binding() {
        let source = "let answer = 42";
        let lexer = Lexer::new(source);
        let mut parser = Parser::new(lexer);

        let program = parser.parse_program().unwrap();
        assert_eq!(program.declarations.len(), 1);

        match &program.declarations[0] {
            Decl::Let { name, value, .. } => {
                assert_eq!(name.name, "answer");
                match value {
                    Expr::IntLiteral(val, _) => assert_eq!(val, "42"),
                    _ => panic!("Expected IntLiteral"),
                }
            }
            _ => panic!("Expected Let declaration"),
        }
    }
}
