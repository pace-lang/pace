use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn foreign_declaration(&mut self, is_private: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        if !self.match_token(&[TokenKind::Func]) {
            self.error_at_current("Expected 'func' after 'foreign'.");
            return None;
        }

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected foreign function name.");
            return None;
        };

        let type_params = self.parse_type_params()?;

        if !self.match_token(&[TokenKind::LeftParen]) {
            self.error_at_current("Expected '(' after foreign function name.");
            return None;
        }

        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let param_name = if let Some(Token {
                    kind: TokenKind::Identifier(n),
                    ..
                }) = self.peek().cloned()
                {
                    self.advance();
                    n
                } else {
                    self.error_at_current("Expected parameter name.");
                    return None;
                };

                if !self.match_token(&[TokenKind::Colon]) {
                    self.error_at_current("Expected ':' after parameter name.");
                    return None;
                }

                let param_type = self.parse_type_expr()?;

                params.push((param_name, param_type));

                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        if !self.match_token(&[TokenKind::RightParen]) {
            self.error_at_current("Expected ')' after parameters.");
            return None;
        }

        let mut return_type = None;
        if self.match_token(&[TokenKind::Colon]) {
            return_type = Some(self.parse_type_expr()?);
        }

        if !self.match_token(&[TokenKind::Semicolon]) {
            self.error_at_current("Expected ';' after foreign function declaration.");
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
            StmtKind::ForeignFunc {
                name,
                base_name: name,
                type_params,
                params,
                return_type,
                is_private,
                is_static: false,
            },
            span,
        ))
    }
}
