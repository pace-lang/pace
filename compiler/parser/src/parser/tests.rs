use super::Parser;
use ast::*;
use lexer::*;
use diagnostics::*;

#[cfg(test)]
    #[test]
    fn test_func_declaration() {
        let source = "func add(a: Int, b: Int) -> Int { return a + b; }";
        let mut session = session::CompilerSession::new();
        let mut scanner = Scanner::new(0, source);
        let mut parser = Parser::new(scanner.scan_tokens(&mut session), &session);
        let (stmts, errors) = parser.parse();

        assert!(errors.is_empty(), "Parse errors: {:?}", errors);
        assert_eq!(stmts.len(), 1);
        match &stmts[0].kind {
            StmtKind::Func {
                name,
                params,
                return_type,
                ..
            } => {
                assert_eq!(session.interner.borrow().lookup(*name), "add");
                assert_eq!(params.len(), 2);
                assert_eq!(session.interner.borrow().lookup(params[0].0), "a");
                assert_eq!(
                    return_type.as_ref().unwrap(),
                    &ast::TypeExpr::Named(session.interner.borrow_mut().intern("Int"))
                );
            }
            _ => panic!("Expected Func statement"),
        }
    }

    #[test]
    fn test_visibility_modifiers() {
        let source = "private func hidden() {} public class Visible {} var unadorned = 1;";
        let mut session = session::CompilerSession::new();
        let mut scanner = Scanner::new(0, source);
        let mut parser = Parser::new(scanner.scan_tokens(&mut session), &session);
        let (stmts, errors) = parser.parse();

        assert!(errors.is_empty(), "Parse errors: {:?}", errors);
        assert_eq!(stmts.len(), 3);

        match &stmts[0].kind {
            StmtKind::Func { is_private, .. } => assert!(*is_private),
            _ => panic!("Expected Func statement"),
        }

        match &stmts[1].kind {
            StmtKind::Class { is_private, .. } => assert!(!(*is_private)),
            _ => panic!("Expected Class statement"),
        }

        match &stmts[2].kind {
            StmtKind::Var { is_private, .. } => assert!(!(*is_private)),
            _ => panic!("Expected Var statement"),
        }
    }

    #[test]
    fn test_if_statement() {
        let source = "if count > 0 { let x = 1; } else { let x = 0; }";
        let mut session = session::CompilerSession::new();
        let mut scanner = Scanner::new(0, source);
        let mut parser = Parser::new(scanner.scan_tokens(&mut session), &session);
        let (stmts, errors) = parser.parse();

        assert!(errors.is_empty(), "Parse errors: {:?}", errors);
        assert_eq!(stmts.len(), 1);
        match &stmts[0].kind {
            StmtKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                assert!(matches!(then_branch.kind, StmtKind::Block(_)));
                assert!(else_branch.is_some());
            }
            _ => panic!("Expected If statement"),
        }
    }

