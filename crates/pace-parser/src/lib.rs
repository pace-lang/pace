pub mod lexer;
pub mod parser;

use pace_ast::Stmt;
pub use lexer::{Lexer, Token};
pub use parser::Parser;

pub fn parse(src: &str) -> Result<Vec<Stmt>, Vec<(String, (usize, usize))>> {
    let mut parser = Parser::new(src);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_let_decl() {
        let src = "let x: Int = 42;";
        let ast = parse(src).expect("Failed to parse");
        insta::assert_debug_snapshot!(ast);
    }
}
