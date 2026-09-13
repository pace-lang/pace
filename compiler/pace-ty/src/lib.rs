pub mod ty;
pub mod typechecker;

pub use ty::Ty;
pub use typechecker::TypeChecker;

#[cfg(test)]
mod tests {
    use super::*;
    use pace_hir::LoweringContext;
    use pace_lexer::Lexer;
    use pace_parser::Parser;

    #[test]
    fn test_typechecker_success() {
        let source = "let x = 42 let y = x + 1";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);
        let (ast, diags, _comments) = parser.parse_program();
        assert!(diags.is_empty(), "Parse errors");

        let mut lowerer = LoweringContext::new();
        let hir = lowerer.lower_program(ast).unwrap();

        let mut tc = TypeChecker::new();
        tc.check_program(&hir).expect("Typecheck failed");
    }

    #[test]
    fn test_typechecker_failure() {
        let source = "let x = \"hello\" let y = x + 1";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);
        let (ast, diags, _comments) = parser.parse_program();
        assert!(diags.is_empty(), "Parse errors");

        let mut lowerer = LoweringContext::new();
        let hir = lowerer.lower_program(ast).unwrap();

        let mut tc = TypeChecker::new();
        let result = tc.check_program(&hir);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Type mismatch in binary operation: String and Int")
        );
    }
}
