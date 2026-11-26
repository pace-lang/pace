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

    #[test]
    fn test_result_generic_and_try_operator_success() {
        let source = "fn helper(r: Result<int, string>) -> Result<int, string> { let x: int = r? return r }";
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
        tc.check_program(&hir).expect("Typecheck failed for Result and ?");
    }

    #[test]
    fn test_try_operator_in_non_result_function_diagnostic() {
        let source = "fn helper(r: Result<int, string>) -> void { let x = r? }";
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
        assert!(result.is_err(), "Expected error for ? in non-Result function");
        assert!(
            tc.reporter.diagnostics.iter().any(|d| d.message.contains("The '?' operator can only be used in a function that returns 'Result'")),
            "Expected non-Result function error: {:?}",
            tc.reporter.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_try_operator_error_type_mismatch_diagnostic() {
        let source = "fn helper(r: Result<int, string>, res: Result<int, int>) -> Result<int, int> { let x = r? return res }";
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
        assert!(result.is_err(), "Expected error for ? with mismatched error type");
        assert!(
            tc.reporter.diagnostics.iter().any(|d| d.message.contains("cannot propagate error type 'string' from function returning error type 'int'")),
            "Expected error type mismatch error: {:?}",
            tc.reporter.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_try_operator_optional_success() {
        let source = "fn helper(opt: int?) -> int? { let x: int = opt? return opt }";
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
        tc.check_program(&hir).expect("Typecheck failed for ? on optional");
    }

    fn check_two_modules(secret_src: &str, main_src: &str) -> Result<(), Vec<String>> {
        let mut modules = std::collections::HashMap::new();
        for (name, src) in [("secret", secret_src), ("main", main_src)] {
            let lexer = Lexer::new(src, pace_span::FileId::DUMMY);
            let mut parser = Parser::new(lexer);
            let (ast, diags, _) = parser.parse_program();
            assert!(diags.is_empty(), "Parse errors in {}: {:?}", name, diags);
            modules.insert(
                pace_span::intern(name),
                pace_ast::Module {
                    name: pace_span::intern(name),
                    file_id: pace_span::FileId::DUMMY,
                    declarations: ast,
                    comments: vec![],
                },
            );
        }
        let program = pace_ast::Program {
            modules,
            module_order: vec![pace_span::intern("secret"), pace_span::intern("main")],
            span: pace_span::Span::DUMMY,
        };
        let mut lowerer = LoweringContext::new();
        let hir = lowerer.lower_program(program).expect("HIR lowering failed");
        let mut tc = TypeChecker::new();
        match tc.check_program(&hir) {
            Ok(()) => Ok(()),
            Err(e) => {
                let mut msgs: Vec<String> =
                    tc.reporter.diagnostics.iter().map(|d| d.message.clone()).collect();
                msgs.push(e);
                Err(msgs)
            }
        }
    }

    #[test]
    fn test_top_level_private_wildcard_import_blocked() {
        let secret = "private fn hiddenFn() -> int { return 1 }\nfn publicFn() -> int { return hiddenFn() }";
        let main = "import secret\nfn main() { let x = hiddenFn() }";
        let res = check_two_modules(secret, main);
        assert!(res.is_err(), "Expected private symbol to be inaccessible via wildcard import");

        let main_ok = "import secret\nfn main() { let x = publicFn() }";
        assert!(check_two_modules(secret, main_ok).is_ok(), "Public symbol should be accessible");
    }

    #[test]
    fn test_top_level_private_alias_import_blocked() {
        let secret = "private fn hiddenFn() -> int { return 1 }\nprivate struct HiddenData { x: int }";
        let main = "import secret as s\nfn main() { let x = s.hiddenFn() }";
        let err = check_two_modules(secret, main).expect_err("Expected private symbol error");
        assert!(
            err.iter().any(|m| m.contains("is private in module 'secret'")),
            "Expected private diagnostic, got: {:?}",
            err
        );
    }

    #[test]
    fn test_enum_match_non_exhaustive_diagnostic() {
        let source = "
            enum Status { ok, pending, failed }
            fn check(s: Status) -> int {
                match s {
                    ok => 1
                    pending => 2
                }
            }
            fn main() {}
        ";
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
        let res = tc.check_program(&hir);
        assert!(res.is_err(), "Expected non-exhaustive match error");
        assert!(
            tc.reporter.diagnostics.iter().any(|d| d.message.contains("Non-exhaustive match: missing variant 'failed'")),
            "Expected missing variant 'failed', got: {:?}",
            tc.reporter.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_enum_match_exhaustive_and_catchall_success() {
        let source = "
            enum Status { ok, pending, failed }
            fn checkAll(s: Status) -> int {
                match s {
                    ok => 1
                    pending => 2
                    failed => 3
                }
            }
            fn checkCatchAll(s: Status) -> int {
                match s {
                    ok => 1
                    _ => 0
                }
            }
            fn main() {}
        ";
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
        tc.check_program(&hir).expect("Exhaustive match should typecheck");
    }

    #[test]
    fn test_unit_and_never_types() {
        let source = "
            fn logMsg(msg: string) -> Unit {
                println(msg)
            }
            fn failForever() -> Never {
                while true {}
            }
            fn main() {
                logMsg(\"hi\")
            }
        ";
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
        tc.check_program(&hir).expect("Unit and Never should typecheck");
    }
}

