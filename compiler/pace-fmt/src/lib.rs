pub mod formatter;

pub use formatter::Formatter;

/// Formats a complete AST module and returns the formatted source code as a string.
pub fn format_module(module: &pace_ast::Module) -> String {
    let mut formatter = Formatter::new();
    formatter.format_module(module)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pace_lexer::Lexer;
    use pace_parser::Parser;
    use pace_span::SourceMap;

    fn format_source(source: &str) -> String {
        let mut sm = SourceMap::new();
        let file_id = sm.add_file("test.pace".to_string(), source.to_string());
        
        let lexer = Lexer::new(source, file_id);
        let mut parser = Parser::new(lexer);
        let (declarations, diags, comments) = parser.parse_program();
        if !diags.is_empty() {
            panic!("Parse errors: {:?}", diags);
        }
        let module = pace_ast::Module {
            name: pace_span::intern("test"),
            file_id,
            declarations,
            comments,
        };
        format_module(&module)
    }

    #[test]
    fn test_format_function() {
        let source = "fn foo(a: int) -> int { let x = 5
return a + x }";
        let expected = "fn foo(a: int) -> int {\n    let x = 5\n    return a + x\n}\n";
        assert_eq!(format_source(source), expected);
    }

    #[test]
    fn test_format_struct() {
        let source = "struct Point { x: int, y: int }";
        let expected = "struct Point {\n    var x: int\n    var y: int\n}\n";
        assert_eq!(format_source(source), expected);
    }
}
