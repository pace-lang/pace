use ast::Stmt;
use diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticCode};
use lexer::{Token, TokenKind};

pub struct Parser<'a> {
    tokens: Vec<Token>,
    current: usize,
    pub errors: Vec<Diagnostic>,
    pub session: &'a session::CompilerSession,
}

pub mod decl;
pub mod expr;
pub mod stmt;
#[cfg(test)]
mod tests;

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token>, session: &'a session::CompilerSession) -> Self {
        Self {
            tokens,
            current: 0,
            errors: Vec::new(),
            session,
        }
    }

    pub fn parse(&mut self) -> (Vec<Stmt<'a>>, Vec<Diagnostic>) {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            if let Some(stmt) = self.declaration() {
                statements.push(stmt);
            }
        }
        (statements, self.errors.clone())
    }

    pub(crate) fn match_token(&mut self, types: &[TokenKind]) -> bool {
        for t in types {
            if self.check(t) {
                self.advance();
                return true;
            }
        }
        false
    }

    pub(crate) fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() {
            return false;
        }
        self.peek().map(|t| &t.kind == kind).unwrap_or(false)
    }

    pub(crate) fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    pub(crate) fn is_at_end(&self) -> bool {
        self.peek()
            .map(|t| t.kind == TokenKind::Eof)
            .unwrap_or(true)
    }

    pub(crate) fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.current)
    }

    pub(crate) fn peek_next(&self) -> Option<&Token> {
        self.tokens.get(self.current + 1)
    }

    pub(crate) fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    pub(crate) fn error_at_current(&mut self, message: &str) {
        if let Some(token) = self.peek() {
            let diag =
                DiagnosticBuilder::error(DiagnosticCode::UnexpectedToken, message, token.span)
                    .build();
            self.errors.push(diag);
        }
    }

    pub(crate) fn synchronize(&mut self) {
        self.advance();
        while !self.is_at_end() {
            if self.previous().kind == TokenKind::Semicolon {
                return;
            }
            if let Some(token) = self.peek() {
                match token.kind {
                    TokenKind::Class
                    | TokenKind::Func
                    | TokenKind::Final
                    | TokenKind::For
                    | TokenKind::If
                    | TokenKind::While
                    | TokenKind::Return => {
                        return;
                    }
                    _ => {}
                }
            }
            self.advance();
        }
    }
}
