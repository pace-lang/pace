use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn import_declaration(&mut self) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let mut path = session::Symbol(0);
        if self.match_token(&[TokenKind::StringStart]) {
            if let Some(Token {
                kind: TokenKind::StringPart(p),
                ..
            }) = self.peek().cloned()
            {
                self.advance();
                path = p;
            }
            if !self.match_token(&[TokenKind::StringEnd]) {
                self.error_at_current("Expected '\"' after string.");
                return None;
            }
        } else {
            self.error_at_current("Expected string after 'import'.");
            return None;
        }

        let mut alias = None;
        if self.match_token(&[TokenKind::As]) {
            if let Some(Token {
                kind: TokenKind::Identifier(a),
                ..
            }) = self.peek().cloned()
            {
                self.advance();
                alias = Some(a);
            } else {
                self.error_at_current("Expected identifier after 'as'.");
            }
        }


        if !self.match_token(&[TokenKind::Semicolon]) {
            self.error_at_current("Expected ';' after import declaration.");
            return None;
        }

        let end_span = self.previous().span;
        let span = Span::new(
            start_span.file_id,
            start_span.start,
            end_span.end,
            start_span.start_loc,
            end_span.end_loc,
        );

        Some(Stmt::new(
            StmtKind::Import {
                path,
                alias,
            },
            span,
        ))
    }

    pub(crate) fn export_declaration(&mut self) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let mut path = session::Symbol(0);
        if self.match_token(&[TokenKind::StringStart]) {
            if let Some(Token {
                kind: TokenKind::StringPart(p),
                ..
            }) = self.peek().cloned()
            {
                self.advance();
                path = p;
            }
            if !self.match_token(&[TokenKind::StringEnd]) {
                self.error_at_current("Expected '\"' after string.");
                return None;
            }
        } else {
            self.error_at_current("Expected string after 'export'.");
            return None;
        }

        if !self.match_token(&[TokenKind::Semicolon]) {
            self.error_at_current("Expected ';' after export declaration.");
            return None;
        }

        let end_span = self.previous().span;
        let span = Span::new(
            start_span.file_id,
            start_span.start,
            end_span.end,
            start_span.start_loc,
            end_span.end_loc,
        );

        Some(Stmt::new(StmtKind::Export { path }, span))
    }
}
