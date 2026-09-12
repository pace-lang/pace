use pace_ast::{Decl, Ident, Type};
use pace_errors::Diagnostic;
use pace_lexer::{Lexer, Token, TokenKind};
use pace_span::Span;

mod decl;
mod expr;
mod stmt;

pub struct Parser<'a> {
    pub(crate) lexer: Lexer<'a>,
    pub(crate) current: Option<Token<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current = lexer.next().and_then(|r| r.ok());
        Self { lexer, current }
    }

    pub(crate) fn current_span(&self) -> Span {
        self.current.as_ref().map(|t| t.span).unwrap_or(Span::DUMMY)
    }

    pub(crate) fn advance(&mut self) {
        self.current = self.lexer.next().and_then(|r| r.ok());
    }

    pub(crate) fn check(&self, kind: &TokenKind) -> bool {
        self.current.as_ref().map_or(false, |t| &t.kind == kind)
    }

    pub(crate) fn expect(&mut self, kind: TokenKind<'a>) -> Result<Token<'a>, Diagnostic> {
        if self.check(&kind) {
            let tok = self.current.clone().unwrap();
            self.advance();
            Ok(tok)
        } else {
            Err(Diagnostic::error(format!("Expected {:?}", kind)).with_span(self.current_span()))
        }
    }

    pub fn parse_program(&mut self) -> Result<Vec<Decl>, Diagnostic> {
        let mut declarations = Vec::new();
        while self.current.is_some() {
            declarations.push(self.parse_decl()?);
        }
        Ok(declarations)
    }

    pub(crate) fn parse_type(&mut self) -> Result<Type, Diagnostic> {
        let (ident_name, ident_span) = match &self.current {
            Some(Token {
                kind: TokenKind::Ident(name),
                span,
            }) => (name.to_string(), *span),
            _ => return Err(Diagnostic::error("Expected type name").with_span(self.current_span())),
        };

        self.advance();
        let ident = Ident {
            name: ident_name,
            span: ident_span,
        };

        let mut base_type = if self.check(&TokenKind::Lt) {
            let mut args = Vec::new();
            self.advance();
            let mut end_span = ident_span;
            while !self.check(&TokenKind::Gt) && self.current.is_some() {
                args.push(self.parse_type()?);
                if self.check(&TokenKind::Comma) {
                    self.advance();
                }
            }
            if let Some(tok) = &self.current {
                end_span = tok.span;
            }
            self.expect(TokenKind::Gt)?;
            Type::Generic(
                Box::new(Type::Named(ident)),
                args,
                ident_span.merge(end_span),
            )
        } else {
            Type::Named(ident)
        };

        if self.check(&TokenKind::Question) {
            let span = base_type.span().merge(self.current.as_ref().unwrap().span);
            self.advance();
            base_type = Type::Optional(Box::new(base_type), span);
        }

        Ok(base_type)
    }
}
