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
    pub file_id: FileId,
    pub base_offset: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str, file_id: FileId) -> Self {
        Self {
            inner: TokenKind::lexer(source),
            file_id,
            base_offset: 0,
        }
    }
    
    pub fn with_offset(source: &'a str, file_id: FileId, base_offset: usize) -> Self {
        Self {
            inner: TokenKind::lexer(source),
            file_id,
            base_offset,
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token<'a>, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        let kind = self.inner.next()?;
        let mut span_range = self.inner.span();
        span_range.start += self.base_offset;
        span_range.end += self.base_offset;
        let span = Span::from((self.file_id, span_range));

        match kind {
            Ok(k) => Some(Ok(Token { kind: k, span })),
            Err(_) => Some(Err(())), // Logos returns Err for unknown tokens
        }
    }
}
