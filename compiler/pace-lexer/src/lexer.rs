use crate::token::TokenKind;
use logos::Logos;
use pace_span::{FileId, Span};

#[derive(Debug, Clone, PartialEq)]
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub span: Span,
}

#[derive(Clone)]
pub struct Lexer<'a> {
    inner: logos::Lexer<'a, TokenKind<'a>>,
    file_id: FileId,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str, file_id: FileId) -> Self {
        Self {
            inner: TokenKind::lexer(source),
            file_id,
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token<'a>, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        let kind = self.inner.next()?;
        let span = Span::from((self.file_id, self.inner.span()));

        match kind {
            Ok(k) => Some(Ok(Token { kind: k, span })),
            Err(_) => Some(Err(())), // Logos returns Err for unknown tokens
        }
    }
}
