use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn match_expression(&mut self, start_span: Span) -> Option<Expr<'a>> {
        let value = self.expression()?;

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before match arms.");
            return None;
        }

        let mut arms = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let pattern = if self.match_token(&[TokenKind::Underscore]) {
                ast::Pattern::Wildcard
            } else {
                let mut path = Vec::new();
                if let Some(Token {
                    kind: TokenKind::Identifier(first),
                    ..
                }) = self.peek().cloned()
                {
                    self.advance();
                    path.push(first);
                    while self.match_token(&[TokenKind::Dot]) {
                        if let Some(Token {
                            kind: TokenKind::Identifier(next),
                            ..
                        }) = self.peek().cloned()
                        {
                            self.advance();
                            path.push(next);
                        } else {
                            self.error_at_current("Expected identifier after '.'.");
                            return None;
                        }
                    }
                } else {
                    self.error_at_current("Expected pattern.");
                    return None;
                }

                let bindings = if self.match_token(&[TokenKind::LeftParen]) {
                    let mut b = Vec::new();
                    if !self.check(&TokenKind::RightParen) {
                        loop {
                            if self.match_token(&[TokenKind::Underscore]) {
                                let b_symbol = self.session.interner.borrow_mut().intern("_");
                                b.push(b_symbol);
                            } else if let Some(Token {
                                kind: TokenKind::Identifier(id),
                                ..
                            }) = self.peek().cloned()
                            {
                                self.advance();
                                b.push(id);
                            } else {
                                self.error_at_current("Expected binding name or '_' in pattern.");
                                return None;
                            }
                            if !self.match_token(&[TokenKind::Comma]) {
                                break;
                            }
                        }
                    }
                    if !self.match_token(&[TokenKind::RightParen]) {
                        self.error_at_current("Expected ')' after pattern bindings.");
                        return None;
                    }
                    Some(b)
                } else {
                    None
                };

                ast::Pattern::Variant { path, bindings }
            };

            if !self.match_token(&[TokenKind::FatArrow]) {
                self.error_at_current("Expected '=>' after match pattern.");
                return None;
            }

            let body = self.expression()?;

            // Optional comma after arm
            self.match_token(&[TokenKind::Comma]);

            arms.push(ast::MatchArm {
                pattern,
                body: self.session.ast_arena.alloc(body),
            });
        }

        if !self.match_token(&[TokenKind::RightBrace]) {
            self.error_at_current("Expected '}' after match arms.");
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

        Some(Expr::new(
            ExprKind::Match {
                value: self.session.ast_arena.alloc(value),
                arms,
            },
            span,
        ))
    }
}
