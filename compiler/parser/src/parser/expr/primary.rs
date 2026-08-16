use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn primary(&mut self) -> Option<Expr<'a>> {
        if self.match_token(&[TokenKind::False]) {
            return Some(Expr::new(ExprKind::Boolean(false), self.previous().span));
        }
        if self.match_token(&[TokenKind::True]) {
            return Some(Expr::new(ExprKind::Boolean(true), self.previous().span));
        }
        if self.match_token(&[TokenKind::Null]) {
            return Some(Expr::new(ExprKind::Null, self.previous().span));
        }
        if self.match_token(&[TokenKind::SelfKeyword]) {
            return Some(Expr::new(ExprKind::SelfRef, self.previous().span));
        }

        if let Some(token) = self.peek().cloned() {
            match token.kind {
                TokenKind::Match => {
                    self.advance();
                    return self.match_expression(token.span);
                }
                TokenKind::Integer(i) => {
                    self.advance();
                    return Some(Expr::new(ExprKind::Integer(i), token.span));
                }
                TokenKind::Float(f) => {
                    self.advance();
                    return Some(Expr::new(ExprKind::Float(f), token.span));
                }
                TokenKind::StringStart => {
                    let start_span = token.span;
                    self.advance();
                    let mut pieces = Vec::new();
                    while let Some(tok) = self.peek().cloned() {
                        match &tok.kind {
                            TokenKind::StringPart(s) => {
                                pieces.push(Expr::new(ExprKind::String(*s), tok.span));
                                self.advance();
                            }
                            TokenKind::InterpolationStart => {
                                self.advance();
                                if let Some(expr) = self.expression() {
                                    pieces.push(expr);
                                }
                                if !self.match_token(&[TokenKind::InterpolationEnd]) {
                                    self.error_at_current(
                                        "Expected '}' after string interpolation expression.",
                                    );
                                }
                            }
                            TokenKind::StringEnd => {
                                self.advance();
                                break;
                            }
                            TokenKind::Error(e) => {
                                self.error_at_current(e);
                                self.advance();
                                break;
                            }
                            TokenKind::Eof => {
                                self.error_at_current("Unterminated string.");
                                break;
                            }
                            _ => {
                                self.error_at_current("Unexpected token in string.");
                                self.advance();
                                break;
                            }
                        }
                    }
                    let end_span = self.previous().span;
                    let span = Span::new(
                        start_span.file_id,
                        start_span.start,
                        end_span.end,
                        start_span.start_loc,
                        end_span.end_loc,
                    );

                    if pieces.len() == 1 {
                        if let ExprKind::String(s) = &pieces[0].kind {
                            return Some(Expr::new(ExprKind::String(*s), span));
                        }
                    } else if pieces.is_empty() {
                        return Some(Expr::new(
                            ExprKind::String(self.session.interner.borrow_mut().intern("")),
                            span,
                        ));
                    }

                    return Some(Expr::new(ExprKind::InterpolatedString(pieces), span));
                }
                TokenKind::Identifier(ref i) => {
                    self.advance();
                    return Some(Expr::new(ExprKind::Variable(*i), token.span));
                }
                TokenKind::LeftParen => {
                    self.advance();
                    let start_span = token.span;
                    let expr = self.expression()?;
                    if !self.match_token(&[TokenKind::RightParen]) {
                        self.error_at_current("Expected ')' after expression.");
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
                    return Some(Expr::new(
                        ExprKind::Grouping(self.session.ast_arena.alloc(expr)),
                        span,
                    ));
                }
                TokenKind::LeftBracket => {
                    self.advance();
                    let start_span = token.span;
                    if self.match_token(&[TokenKind::RightBracket]) {
                        let end_span = self.previous().span;
                        let span = Span::new(
                            start_span.file_id,
                            start_span.start,
                            end_span.end,
                            start_span.start_loc,
                            end_span.end_loc,
                        );
                        return Some(Expr::new(ExprKind::Array(Vec::new()), span));
                    }

                    let first_expr = self.expression()?;
                    if self.match_token(&[TokenKind::Semicolon]) {
                        let count = self.expression()?;
                        if !self.match_token(&[TokenKind::RightBracket]) {
                            self.error_at_current("Expected ']' after array repeat count.");
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
                        return Some(Expr::new(
                            ExprKind::ArrayRepeat {
                                value: self.session.ast_arena.alloc(first_expr),
                                count: self.session.ast_arena.alloc(count),
                            },
                            span,
                        ));
                    } else if self.match_token(&[TokenKind::For]) {
                        let item_name = if let Some(Token {
                            kind: TokenKind::Identifier(n),
                            ..
                        }) = self.peek().cloned()
                        {
                            self.advance();
                            n
                        } else {
                            self.error_at_current(
                                "Expected item name after 'for' in list comprehension.",
                            );
                            return None;
                        };

                        let is_in = if let Some(Token {
                            kind: TokenKind::Identifier(s),
                            ..
                        }) = self.peek()
                        {
                            self.session.interner.borrow().lookup(*s) == "in"
                        } else {
                            false
                        };

                        if is_in {
                            self.advance();
                        } else {
                            self.error_at_current(
                                "Expected 'in' after item name in list comprehension.",
                            );
                            return None;
                        }

                        let iterator = self.expression()?;
                        if !self.match_token(&[TokenKind::RightBracket]) {
                            self.error_at_current("Expected ']' after list comprehension.");
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
                        return Some(Expr::new(
                            ExprKind::ListComprehension {
                                expr: self.session.ast_arena.alloc(first_expr),
                                item_name,
                                iterator: self.session.ast_arena.alloc(iterator),
                            },
                            span,
                        ));
                    } else {
                        let mut elements = vec![first_expr];
                        while self.match_token(&[TokenKind::Comma]) {
                            if self.check(&TokenKind::RightBracket) {
                                break;
                            }
                            elements.push(self.expression()?);
                        }
                        if !self.match_token(&[TokenKind::RightBracket]) {
                            self.error_at_current("Expected ']' after array elements.");
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
                        return Some(Expr::new(ExprKind::Array(elements), span));
                    }
                }
                _ => {}
            }
        }

        self.error_at_current("Expected expression.");
        None
    }
}
