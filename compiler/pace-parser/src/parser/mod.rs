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
    pub diagnostics: Vec<Diagnostic>,
    pub comments: Vec<(Span, String)>,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let mut parser = Self { lexer, current: None, diagnostics: Vec::new(), comments: Vec::new() };
        parser.advance();
        parser
    }

    pub(crate) fn current_span(&self) -> Span {
        self.current.as_ref().map(|t| t.span).unwrap_or(Span::DUMMY)
    }

    pub(crate) fn advance(&mut self) {
        loop {
            let next = self.lexer.next().and_then(|r| r.ok());
            if let Some(tok) = &next {
                if let TokenKind::Comment(text) = tok.kind {
                    self.comments.push((tok.span, text.to_string()));
                    continue;
                }
            }
            self.current = next;
            break;
        }
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

    pub fn parse_program(&mut self) -> (Vec<Decl>, Vec<Diagnostic>, Vec<(Span, String)>) {
        let mut declarations = Vec::new();
        while self.current.is_some() {
            match self.parse_decl() {
                Ok(decl) => declarations.push(decl),
                Err(diag) => {
                    self.diagnostics.push(diag);
                    self.synchronize();
                }
            }
        }
        (declarations, self.diagnostics.clone(), self.comments.clone())
    }

    fn synchronize(&mut self) {
        self.advance();
        while self.current.is_some() {
            if let Some(tok) = &self.current {
                match tok.kind {
                    TokenKind::Class
                    | TokenKind::Struct
                    | TokenKind::Enum
                    | TokenKind::Trait
                    | TokenKind::Fn
                    | TokenKind::Let
                    | TokenKind::Const
                    | TokenKind::Var => return,
                    _ => {}
                }
            }
            self.advance();
        }
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
