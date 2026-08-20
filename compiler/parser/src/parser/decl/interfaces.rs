use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn interface_declaration(&mut self, is_private: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected interface name.");
            return None;
        };

        let type_params = self.parse_type_params()?;

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before interface body.");
            return None;
        }

        let mut methods = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let item_is_private = self.parse_visibility();

            if self.match_token(&[TokenKind::Func]) {
                if let Some(method) = self.interface_method_declaration(item_is_private, false) {
                    methods.push(method);
                }
            } else if self.match_token(&[TokenKind::Async]) {
                if self.match_token(&[TokenKind::Func]) {
                    if let Some(method) = self.interface_method_declaration(item_is_private, true) {
                        methods.push(method);
                    }
                } else {
                    self.error_at_current("Expected 'func' after 'async'.");
                }
            } else {
                self.error_at_current("Expected method signature inside interface.");
                self.advance();
            }
        }

        if !self.match_token(&[TokenKind::RightBrace]) {
            self.error_at_current("Expected '}' after interface body.");
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
            StmtKind::Interface {
                name,
                type_params,
                methods,
                is_private,
            },
            span,
        ))
    }

    pub(crate) fn interface_method_declaration(&mut self, is_private: bool, is_async: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected method name.");
            return None;
        };

        if !self.match_token(&[TokenKind::LeftParen]) {
            self.error_at_current("Expected '(' after method name.");
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
            self.error_at_current("Expected ';' after interface method declaration.");
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

        // We use StmtKind::Func but with an empty block for the body.
        let empty_body = self
            .session
            .ast_arena
            .alloc(Stmt::new(StmtKind::Block(Vec::new()), span));
        Some(Stmt::new(
            StmtKind::Func {
                name,
                type_params: Vec::new(),
                params,
                return_type,
                body: empty_body,
                is_private,
                is_async,
            },
            span,
        ))
    }
}
