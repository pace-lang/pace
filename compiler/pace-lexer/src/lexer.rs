use crate::token::TokenKind;
use logos::Logos;
use pace_span::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub span: Span,
}

pub struct Lexer<'a> {
    inner: logos::Lexer<'a, TokenKind<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            inner: TokenKind::lexer(source),
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token<'a>, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        let kind = self.inner.next()?;
        let span = Span::from(self.inner.span());

        match kind {
            Ok(k) => Some(Ok(Token { kind: k, span })),
            Err(_) => Some(Err(())), // Logos returns Err for unknown tokens
        }
    }
}
