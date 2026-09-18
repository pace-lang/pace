pub mod hir;
pub mod lowering;

pub use hir::*;
pub use lowering::LoweringContext;

#[cfg(test)]
mod tests {
    use super::*;
    use pace_lexer::Lexer;
    use pace_parser::Parser;
    use pace_span::SourceMap;
    use std::collections::HashMap;

    fn parse_and_lower(source: &str) -> Result<Program, String> {
        let mut sm = SourceMap::new();
        let file_id = sm.add_file("test.pace".to_string(), source.to_string());

        let lexer = Lexer::new(source, file_id);
        let mut parser = Parser::new(lexer);
        let (declarations, diags, comments) = parser.parse_program();
        if !diags.is_empty() {
            panic!("Parse errors: {:?}", diags);
        }

        let module_name = pace_span::intern("test");
        let mut modules = HashMap::new();
        modules.insert(
            module_name,
            pace_ast::Module {
                name: module_name,
                file_id,
                declarations,
                comments,
            },
        );

        let ast_program = pace_ast::Program {
            modules,
            module_order: vec![module_name],
            span: pace_span::Span::DUMMY,
        };

        let mut lowerer = lowering::LoweringContext::new();
        lowerer.lower_program(ast_program)
    }

    #[test]
    fn test_lower_function() {
        let source = "fn identity(a: int) -> int { return a }";
        let hir_program = parse_and_lower(source).expect("Lowering failed");

        let module = hir_program.modules.get(&pace_span::intern("test")).unwrap();
        assert_eq!(module.declarations.len(), 1);

        match &module.declarations[0] {
            Decl::Function {
                name, params, body, ..
            } => {
                assert_eq!(name.as_str(), "identity");
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].1.as_str(), "a");

                assert_eq!(body.statements.len(), 1);
                match &body.statements[0] {
                    Stmt::Return(Some(Expr::Ident(_, id, _, _)), _) => {
                        assert_eq!(id.as_str(), "a");
                    }
                    _ => panic!("Expected return statement with identifier"),
                }
            }
            _ => panic!("Expected a function"),
        }
    }

    #[test]
    fn test_lower_struct() {
        let source = "struct Point { var x: int, var y: int }";
        let hir_program = parse_and_lower(source).expect("Lowering failed");

        let module = hir_program.modules.get(&pace_span::intern("test")).unwrap();
        assert_eq!(module.declarations.len(), 1);

        match &module.declarations[0] {
            Decl::Struct { name, fields, .. } => {
                assert_eq!(name.as_str(), "Point");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name.as_str(), "x");
                assert_eq!(fields[1].name.as_str(), "y");
            }
            _ => panic!("Expected a struct"),
        }
    }
}
