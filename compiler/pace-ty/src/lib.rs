pub mod reassign;
pub mod ty;
pub mod typechecker;

pub use ty::Ty;
pub use typechecker::TypeChecker;
pub use typechecker::{ResolvedField, ResolvedMethod, VTableEntry};

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

        let mut modules = std::collections::HashMap::new();
        modules.insert(
            pace_span::intern("main"),
            pace_ast::Module {
                name: pace_span::intern("main"),
                file_id: pace_span::FileId::DUMMY,
                declarations: ast,
                comments: vec![],
            },
        );
        let program = pace_ast::Program {
            modules,
            module_order: vec![pace_span::intern("main")],
            span: pace_span::Span::DUMMY,
        };

        let mut lowerer = LoweringContext::new();
        let hir = lowerer.lower_program(program).expect("HIR lowering failed");

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

        let mut modules = std::collections::HashMap::new();
        modules.insert(
            pace_span::intern("main"),
            pace_ast::Module {
                name: pace_span::intern("main"),
                file_id: pace_span::FileId::DUMMY,
                declarations: ast,
                comments: vec![],
            },
        );
        let program = pace_ast::Program {
            modules,
            module_order: vec![pace_span::intern("main")],
            span: pace_span::Span::DUMMY,
        };

        let mut lowerer = LoweringContext::new();
        let hir = lowerer.lower_program(program).expect("HIR lowering failed");

        let mut tc = TypeChecker::new();
        let result = tc.check_program(&hir);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Type mismatch in binary operation: string and int")
        );
    }

    #[test]
    fn test_user_facing_type_diagnostics() {
        let source = "class User {} let u: User = 123";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);
        let (ast, diags, _) = parser.parse_program();
        assert!(diags.is_empty(), "Parse errors");

        let mut modules = std::collections::HashMap::new();
        modules.insert(
            pace_span::intern("main"),
            pace_ast::Module {
                name: pace_span::intern("main"),
                file_id: pace_span::FileId::DUMMY,
                declarations: ast,
                comments: vec![],
            },
        );
        let program = pace_ast::Program {
            modules,
            module_order: vec![pace_span::intern("main")],
            span: pace_span::Span::DUMMY,
        };

        let mut lowerer = LoweringContext::new();
        let hir = lowerer.lower_program(program).expect("HIR lowering failed");

        let mut tc = TypeChecker::new();
        let result = tc.check_program(&hir);
        assert!(result.is_err());
        let errs = &tc.reporter.diagnostics;
        assert!(!errs.is_empty());
        assert!(
            errs.iter().any(|d| d.message.contains("Type mismatch: expected User, got int")),
            "Expected 'Type mismatch: expected User, got int' in diagnostics: {:?}",
            errs.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_unknown_type_error() {
        let source = "class Service { fn process(user: NonExistentType) -> void {} }";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);
        let (ast, diags, _) = parser.parse_program();
        assert!(diags.is_empty(), "Parse errors");

        let mut modules = std::collections::HashMap::new();
        modules.insert(
            pace_span::intern("main"),
            pace_ast::Module {
                name: pace_span::intern("main"),
                file_id: pace_span::FileId::DUMMY,
                declarations: ast,
                comments: vec![],
            },
        );
        let program = pace_ast::Program {
            modules,
            module_order: vec![pace_span::intern("main")],
            span: pace_span::Span::DUMMY,
        };

        let mut lowerer = LoweringContext::new();
        let hir = lowerer.lower_program(program).expect("HIR lowering failed");

        let mut tc = TypeChecker::new();
        let result = tc.check_program(&hir);

        assert!(result.is_err(), "Expected type checker error on unknown type");
        assert!(
            result.unwrap_err().contains("Type resolution failed"),
            "Expected type resolution failure"
        );
    }

    #[test]
    fn test_loop_break_continue_success() {
        let source = "fn run() -> void { while true { if true { break } else { continue } } }";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);
        let (ast, diags, _) = parser.parse_program();
        assert!(diags.is_empty(), "Parse errors: {:?}", diags);

        let mut modules = std::collections::HashMap::new();
        modules.insert(
            pace_span::intern("main"),
            pace_ast::Module {
                name: pace_span::intern("main"),
                file_id: pace_span::FileId::DUMMY,
                declarations: ast,
                comments: vec![],
            },
        );
        let program = pace_ast::Program {
            modules,
            module_order: vec![pace_span::intern("main")],
            span: pace_span::Span::DUMMY,
        };

        let mut lowerer = LoweringContext::new();
        let hir = lowerer.lower_program(program).expect("HIR lowering failed");

        let mut tc = TypeChecker::new();
        tc.check_program(&hir).expect("Typecheck failed for valid break/continue");
    }

    #[test]
    fn test_break_outside_loop_diagnostic() {
        let source = "fn run() -> void { break }";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);
        let (ast, diags, _) = parser.parse_program();
        assert!(diags.is_empty(), "Parse errors: {:?}", diags);

        let mut modules = std::collections::HashMap::new();
        modules.insert(
            pace_span::intern("main"),
            pace_ast::Module {
                name: pace_span::intern("main"),
                file_id: pace_span::FileId::DUMMY,
                declarations: ast,
                comments: vec![],
            },
        );
        let program = pace_ast::Program {
            modules,
            module_order: vec![pace_span::intern("main")],
            span: pace_span::Span::DUMMY,
        };

        let mut lowerer = LoweringContext::new();
        let hir = lowerer.lower_program(program).expect("HIR lowering failed");

        let mut tc = TypeChecker::new();
        let result = tc.check_program(&hir);
        assert!(result.is_err(), "Expected error for break outside loop");
        assert!(
            tc.reporter.diagnostics.iter().any(|d| d.message.contains("Cannot 'break' outside of a loop")),
            "Expected \"Cannot 'break' outside of a loop\" error: {:?}",
            tc.reporter.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_continue_outside_loop_diagnostic() {
        let source = "fn run() -> void { continue }";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);
        let (ast, diags, _) = parser.parse_program();
        assert!(diags.is_empty(), "Parse errors: {:?}", diags);

        let mut modules = std::collections::HashMap::new();
        modules.insert(
            pace_span::intern("main"),
            pace_ast::Module {
                name: pace_span::intern("main"),
                file_id: pace_span::FileId::DUMMY,
                declarations: ast,
                comments: vec![],
            },
        );
        let program = pace_ast::Program {
            modules,
            module_order: vec![pace_span::intern("main")],
            span: pace_span::Span::DUMMY,
        };

        let mut lowerer = LoweringContext::new();
        let hir = lowerer.lower_program(program).expect("HIR lowering failed");

        let mut tc = TypeChecker::new();
        let result = tc.check_program(&hir);
        assert!(result.is_err(), "Expected error for continue outside loop");
        assert!(
            tc.reporter.diagnostics.iter().any(|d| d.message.contains("Cannot 'continue' outside of a loop")),
            "Expected \"Cannot 'continue' outside of a loop\" error: {:?}",
            tc.reporter.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_trait_method_resolution() {
        let source = "trait Printable { fn toString() -> string } struct Item with Printable { fn toString() -> string { return \"item\" } } fn main() -> void {}";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);
        let (ast, diags, _) = parser.parse_program();
        assert!(diags.is_empty(), "Parse errors: {:?}", diags);

        let mut modules = std::collections::HashMap::new();
        modules.insert(
            pace_span::intern("main"),
            pace_ast::Module {
                name: pace_span::intern("main"),
                file_id: pace_span::FileId::DUMMY,
                declarations: ast,
                comments: vec![],
            },
        );
        let program = pace_ast::Program {
            modules,
            module_order: vec![pace_span::intern("main")],
            span: pace_span::Span::DUMMY,
        };

        let mut lowerer = LoweringContext::new();
        let mut hir = lowerer.lower_program(program).expect("HIR lowering failed");

        let mut tc = TypeChecker::new();
        hir.resolve_traits(&mut tc.reporter);
        assert!(
            tc.reporter.diagnostics.is_empty(),
            "Expected no trait resolution errors, got: {:?}",
            tc.reporter.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        tc.check_program(&hir).expect("Typecheck failed for trait method");
    }

    #[test]
    fn test_trait_missing_method_diagnostic() {
        let source = "trait Printable { fn toString() -> string } struct Item with Printable {} fn main() -> void {}";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);
        let (ast, diags, _) = parser.parse_program();
        assert!(diags.is_empty(), "Parse errors: {:?}", diags);

        let mut modules = std::collections::HashMap::new();
        modules.insert(
            pace_span::intern("main"),
            pace_ast::Module {
                name: pace_span::intern("main"),
                file_id: pace_span::FileId::DUMMY,
                declarations: ast,
                comments: vec![],
            },
        );
        let program = pace_ast::Program {
            modules,
            module_order: vec![pace_span::intern("main")],
            span: pace_span::Span::DUMMY,
        };

        let mut lowerer = LoweringContext::new();
        let mut hir = lowerer.lower_program(program).expect("HIR lowering failed");

        let mut tc = TypeChecker::new();
        hir.resolve_traits(&mut tc.reporter);
        assert!(
            tc.reporter.diagnostics.iter().any(|d| d.message.contains("required method 'toString'")),
            "Expected diagnostic mentioning 'toString', got: {:?}",
            tc.reporter.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_option_generic_type() {
        let source = "let a: Option<int> = null let b: int? = a";
        let lexer = Lexer::new(source, pace_span::FileId::DUMMY);
        let mut parser = Parser::new(lexer);
        let (ast, diags, _) = parser.parse_program();
        assert!(diags.is_empty(), "Parse errors: {:?}", diags);

        let mut modules = std::collections::HashMap::new();
        modules.insert(
            pace_span::intern("main"),
            pace_ast::Module {
                name: pace_span::intern("main"),
                file_id: pace_span::FileId::DUMMY,
                declarations: ast,
                comments: vec![],
            },
        );
        let program = pace_ast::Program {
            modules,
            module_order: vec![pace_span::intern("main")],
            span: pace_span::Span::DUMMY,
        };

        let mut lowerer = LoweringContext::new();
        let hir = lowerer.lower_program(program).expect("HIR lowering failed");

        let mut tc = TypeChecker::new();
        tc.check_program(&hir).expect("Typecheck failed for Option<int>");
    }
}

