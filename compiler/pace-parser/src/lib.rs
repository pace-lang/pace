pub mod parser;

pub use parser::Parser;

#[cfg(test)]
mod tests {
    use super::*;
    use pace_ast::{Decl, Expr, Stmt, Type};
    use pace_lexer::Lexer;

    #[test]
    fn test_parse_let_binding() {
        let source = "let answer = 42";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);

        let (declarations, diags, _comments) = parser.parse_program();
        assert!(diags.is_empty(), "Expected no errors");
        assert_eq!(declarations.len(), 1);

        match &declarations[0] {
            Decl::Let { name, value, .. } => {
                assert_eq!(name.name, "answer");
                match value {
                    Some(Expr::IntLiteral(val, _)) => assert_eq!(val, "42"),
                    _ => panic!("Expected IntLiteral"),
                }
            }
            _ => panic!("Expected Let declaration"),
        }
    }

    #[test]
    fn test_parse_function() {
        let source = "fn identity(a: int) -> int { return a }";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);

        let (declarations, diags, _comments) = parser.parse_program();
        assert!(diags.is_empty(), "Expected no errors");
        assert_eq!(declarations.len(), 1);

        match &declarations[0] {
            Decl::Function {
                name,
                params,
                return_type,
                body,
                ..
            } => {
                assert_eq!(name.name, "identity");
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].0.name, "a");
                match &params[0].1 {
                    Type::Named(id) => assert_eq!(id.name, "int"),
                    _ => panic!("Expected Named type"),
                }
                match return_type.as_ref().unwrap() {
                    Type::Named(id) => assert_eq!(id.name, "int"),
                    _ => panic!("Expected Named return type"),
                }
                assert_eq!(body.statements.len(), 1);
                match &body.statements[0] {
                    Stmt::Return(Some(Expr::Ident(id, _)), _) => assert_eq!(id.name, "a"),
                    _ => panic!("Expected Return statement with Ident"),
                }
            }
            _ => panic!("Expected Function declaration"),
        }
    }

    #[test]
    fn test_parse_class() {
        let source = "class User { name: string }";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);

        let (declarations, diags, _comments) = parser.parse_program();
        assert!(diags.is_empty(), "Expected no errors");
        assert_eq!(declarations.len(), 1);

        match &declarations[0] {
            Decl::Class {
                name,
                fields,
                methods,
                ..
            } => {
                assert_eq!(name.name, "User");
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name.name, "name");
                assert_eq!(methods.len(), 0);
            }
            _ => panic!("Expected Class declaration"),
        }
    }

    #[test]
    fn test_parser_synchronization() {
        let source = "let = 42\nactor Worker {}\nimport math";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);

        let (declarations, diags, _) = parser.parse_program();
        assert!(!diags.is_empty(), "Expected syntax error on broken let");
        assert_eq!(declarations.len(), 2, "Expected actor and import to recover and parse");
    }

    #[test]
    fn test_string_interpolation_diagnostics() {
        let source = r#"let s = "hello ${let = 5} world""#;
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);

        let (_declarations, diags, _) = parser.parse_program();
        assert!(!diags.is_empty(), "Expected error from invalid syntax inside string interpolation");
    }

    #[test]
    fn test_parse_mod_operator() {
        let source = "let rem = 10 % 3";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);

        let (declarations, diags, _) = parser.parse_program();
        assert!(diags.is_empty());
        assert_eq!(declarations.len(), 1);
        if let Decl::Let {
            value: Some(Expr::Binary { op, .. }),
            ..
        } = &declarations[0]
        {
            assert_eq!(*op, pace_ast::BinaryOp::Mod);
        } else {
            panic!("Expected binary mod expression");
        }
    }

    #[test]
    fn test_unrecognized_token_diagnostic() {
        let source = "let x = @123";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);

        let (_declarations, diags, _) = parser.parse_program();
        assert!(
            diags.iter().any(|d| d.message.contains("Unrecognized token")),
            "Expected Unrecognized token diagnostic"
        );
    }

    #[test]
    fn test_parse_break_and_continue() {
        let source = "fn loop_test() -> void { while true { break continue } }";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);

        let (declarations, diags, _) = parser.parse_program();
        assert!(diags.is_empty(), "Parse errors: {:?}", diags);
        assert_eq!(declarations.len(), 1);
        if let Decl::Function { body: block, .. } = &declarations[0] {
            assert_eq!(block.statements.len(), 1);
            if let Stmt::ExprStmt(Expr::While { body, .. }, _) = &block.statements[0] {
                assert_eq!(body.statements.len(), 2);
                assert!(matches!(body.statements[0], Stmt::Break(_)));
                assert!(matches!(body.statements[1], Stmt::Continue(_)));
            } else {
                panic!("Expected while loop statement");
            }
        } else {
            panic!("Expected function declaration");
        }
    }
}

