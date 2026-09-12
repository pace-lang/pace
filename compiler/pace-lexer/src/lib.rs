pub mod lexer;
pub mod token;

pub use lexer::{Lexer, Token};
pub use token::TokenKind;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keywords_and_identifiers() {
        let source = "let user_name = class";
        let mut lexer = Lexer::new(source, pace_span::FileId::DUMMY);

        assert_eq!(lexer.next().unwrap().unwrap().kind, TokenKind::Let);
        assert_eq!(
            lexer.next().unwrap().unwrap().kind,
            TokenKind::Ident("user_name")
        );
        assert_eq!(lexer.next().unwrap().unwrap().kind, TokenKind::Eq);
        assert_eq!(lexer.next().unwrap().unwrap().kind, TokenKind::Class);
        assert!(lexer.next().is_none());
    }

    #[test]
    fn test_literals() {
        let source = r#"42 3.14 "hello pace""#;
        let mut lexer = Lexer::new(source, pace_span::FileId::DUMMY);

        assert_eq!(lexer.next().unwrap().unwrap().kind, TokenKind::Int("42"));
        assert_eq!(
            lexer.next().unwrap().unwrap().kind,
            TokenKind::Float("3.14")
        );
        assert_eq!(
            lexer.next().unwrap().unwrap().kind,
            TokenKind::String(r#""hello pace""#)
        );
        assert!(lexer.next().is_none());
    }

    #[test]
    fn test_operators() {
        let source = "?? ?. == => ->";
        let mut lexer = Lexer::new(source, pace_span::FileId::DUMMY);

        assert_eq!(lexer.next().unwrap().unwrap().kind, TokenKind::NullCoalesce);
        assert_eq!(lexer.next().unwrap().unwrap().kind, TokenKind::OptChain);
        assert_eq!(lexer.next().unwrap().unwrap().kind, TokenKind::EqEq);
        assert_eq!(lexer.next().unwrap().unwrap().kind, TokenKind::FatArrow);
        assert_eq!(lexer.next().unwrap().unwrap().kind, TokenKind::Arrow);
        assert!(lexer.next().is_none());
    }
}
