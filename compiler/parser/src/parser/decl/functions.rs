use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn init_declaration(&mut self, is_private: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;
        let name = self.session.interner.borrow_mut().intern("init");

        if !self.match_token(&[TokenKind::LeftParen]) {
            self.error_at_current("Expected '(' after init.");
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

        let return_type = Some(TypeExpr::Named(
            self.session.interner.borrow_mut().intern("Void"),
        ));

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before init body.");
            return None;
        }

        let body = self.block()?;
        let end_span = body.span;
        let span = Span::new(
            start_span.file_id,
            start_span.start,
            end_span.end,
            start_span.start_loc,
            end_span.end_loc,
        );
        Some(Stmt::new(
            StmtKind::Func {
                name,
                type_params: Vec::new(),
                params,
                return_type,
                body: self.session.ast_arena.alloc(body),
                is_private,
                is_async: false,
            },
            span,
        ))
    }

    pub(crate) fn function_declaration(&mut self, is_private: bool, is_async: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected function name.");
            return None;
        };

        let type_params = self.parse_type_params()?;

        if !self.match_token(&[TokenKind::LeftParen]) {
            self.error_at_current("Expected '(' after function name.");
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

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before function body.");
            return None;
        }

        let body = self.block()?;
        let end_span = body.span;
        let span = Span::new(
            start_span.file_id,
            start_span.start,
            end_span.end,
            start_span.start_loc,
            end_span.end_loc,
        );
        Some(Stmt::new(
            StmtKind::Func {
                name,
                type_params,
                params,
                return_type,
                body: self.session.ast_arena.alloc(body),
                is_private,
                is_async,
            },
            span,
        ))
    }
}
