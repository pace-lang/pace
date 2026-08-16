use super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {

    pub(crate) fn expression(&mut self) -> Option<Expr<'a>> {
        self.assignment()
    }

    pub(crate) fn assignment(&mut self) -> Option<Expr<'a>> {
        let expr = self.null_coalesce()?;

        if self.match_token(&[TokenKind::Equal, TokenKind::QuestionQuestionEqual]) {
            let op = self.previous().clone();
            let value = self.assignment()?;

            if op.kind == TokenKind::QuestionQuestionEqual {
                match expr.kind {
                    ExprKind::Variable(_) | ExprKind::Get { .. } | ExprKind::IndexGet { .. } => {
                        let span = Span::new(
                            expr.span.file_id,
                            expr.span.start,
                            value.span.end,
                            expr.span.start_loc,
                            value.span.end_loc,
                        );
                        return Some(Expr::new(
                            ExprKind::NullCoalesceAssign {
                                left: self.session.ast_arena.alloc(expr),
                                right: self.session.ast_arena.alloc(value),
                            },
                            span,
                        ));
                    }
                    _ => {
                        self.error_at_current("Invalid assignment target for ??=.");
                    }
                }
            } else {
                match expr.kind {
                    ExprKind::Variable(name) => {
                        let span = Span::new(
                            expr.span.file_id,
                            expr.span.start,
                            value.span.end,
                            expr.span.start_loc,
                            value.span.end_loc,
                        );
                        return Some(Expr::new(
                            ExprKind::Assign {
                                name,
                                value: self.session.ast_arena.alloc(value),
                            },
                            span,
                        ));
                    }
                    ExprKind::Get { object, name } => {
                        let span = Span::new(
                            expr.span.file_id,
                            expr.span.start,
                            value.span.end,
                            expr.span.start_loc,
                            value.span.end_loc,
                        );
                        return Some(Expr::new(
                            ExprKind::Set {
                                object,
                                name,
                                value: self.session.ast_arena.alloc(value),
                            },
                            span,
                        ));
                    }
                    ExprKind::IndexGet { object, index } => {
                        let span = Span::new(
                            expr.span.file_id,
                            expr.span.start,
                            value.span.end,
                            expr.span.start_loc,
                            value.span.end_loc,
                        );
                        return Some(Expr::new(
                            ExprKind::IndexSet {
                                object,
                                index,
                                value: self.session.ast_arena.alloc(value),
                            },
                            span,
                        ));
                    }
                    _ => {
                        self.error_at_current("Invalid assignment target.");
                    }
                }
            }
        }

        Some(expr)
    }

    pub(crate) fn null_coalesce(&mut self) -> Option<Expr<'a>> {
        let mut expr = self.range()?;

        while self.match_token(&[TokenKind::QuestionQuestion]) {
            let right = self.range()?;
            let span = Span::new(
                expr.span.file_id,
                expr.span.start,
                right.span.end,
                expr.span.start_loc,
                right.span.end_loc,
            );
            expr = Expr::new(
                ExprKind::NullCoalesce {
                    left: self.session.ast_arena.alloc(expr),
                    right: self.session.ast_arena.alloc(right),
                },
                span,
            );
        }

        Some(expr)
    }

    pub(crate) fn range(&mut self) -> Option<Expr<'a>> {
        let mut expr = self.equality()?;

        while self.match_token(&[TokenKind::DotDot]) {
            let right = self.equality()?;
            let span = Span::new(
                expr.span.file_id,
                expr.span.start,
                right.span.end,
                expr.span.start_loc,
                right.span.end_loc,
            );
            expr = Expr::new(
                ExprKind::Range {
                    start: self.session.ast_arena.alloc(expr),
                    end: self.session.ast_arena.alloc(right),
                },
                span,
            );
        }

        Some(expr)
    }

    pub(crate) fn equality(&mut self) -> Option<Expr<'a>> {
        let mut expr = self.comparison()?;

        while self.match_token(&[TokenKind::EqualEqual, TokenKind::BangEqual]) {
            let operator = match self.previous().kind {
                TokenKind::EqualEqual => BinaryOp::Equal,
                TokenKind::BangEqual => BinaryOp::NotEqual,
                _ => unreachable!(),
            };
            let right = self.comparison()?;
            let span = Span::new(
                expr.span.file_id,
                expr.span.start,
                right.span.end,
                expr.span.start_loc,
                right.span.end_loc,
            );
            expr = Expr::new(
                ExprKind::Binary(
                    self.session.ast_arena.alloc(expr),
                    operator,
                    self.session.ast_arena.alloc(right),
                ),
                span,
            );
        }

        Some(expr)
    }

    pub(crate) fn comparison(&mut self) -> Option<Expr<'a>> {
        let mut expr = self.term()?;

        while self.match_token(&[
            TokenKind::Greater,
            TokenKind::GreaterEqual,
            TokenKind::Less,
            TokenKind::LessEqual,
        ]) {
            let operator = match self.previous().kind {
                TokenKind::Greater => BinaryOp::Greater,
                TokenKind::GreaterEqual => BinaryOp::GreaterEqual,
                TokenKind::Less => BinaryOp::Less,
                TokenKind::LessEqual => BinaryOp::LessEqual,
                _ => unreachable!(),
            };
            let right = self.term()?;
            let span = Span::new(
                expr.span.file_id,
                expr.span.start,
                right.span.end,
                expr.span.start_loc,
                right.span.end_loc,
            );
            expr = Expr::new(
                ExprKind::Binary(
                    self.session.ast_arena.alloc(expr),
                    operator,
                    self.session.ast_arena.alloc(right),
                ),
                span,
            );
        }

        Some(expr)
    }

    pub(crate) fn term(&mut self) -> Option<Expr<'a>> {
        let mut expr = self.factor()?;

        while self.match_token(&[TokenKind::Plus, TokenKind::Minus]) {
            let operator = match self.previous().kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Subtract,
                _ => unreachable!(),
            };
            let right = self.factor()?;
            let span = Span::new(
                expr.span.file_id,
                expr.span.start,
                right.span.end,
                expr.span.start_loc,
                right.span.end_loc,
            );
            expr = Expr::new(
                ExprKind::Binary(
                    self.session.ast_arena.alloc(expr),
                    operator,
                    self.session.ast_arena.alloc(right),
                ),
                span,
            );
        }

        Some(expr)
    }

    pub(crate) fn factor(&mut self) -> Option<Expr<'a>> {
        let mut expr = self.unary()?;

        while self.match_token(&[TokenKind::Star, TokenKind::Slash]) {
            let operator = match self.previous().kind {
                TokenKind::Star => BinaryOp::Multiply,
                TokenKind::Slash => BinaryOp::Divide,
                _ => unreachable!(),
            };
            let right = self.unary()?;
            let span = Span::new(
                expr.span.file_id,
                expr.span.start,
                right.span.end,
                expr.span.start_loc,
                right.span.end_loc,
            );
            expr = Expr::new(
                ExprKind::Binary(
                    self.session.ast_arena.alloc(expr),
                    operator,
                    self.session.ast_arena.alloc(right),
                ),
                span,
            );
        }

        Some(expr)
    }

    pub(crate) fn unary(&mut self) -> Option<Expr<'a>> {
        if self.match_token(&[TokenKind::Minus]) {
            let start_span = self.previous().span;
            let right = self.unary()?;
            let span = Span::new(
                start_span.file_id,
                start_span.start,
                right.span.end,
                start_span.start_loc,
                right.span.end_loc,
            );
            return Some(Expr::new(
                ExprKind::Unary(UnaryOp::Negate, self.session.ast_arena.alloc(right)),
                span,
            ));
        }

        self.call()
    }

    pub(crate) fn call(&mut self) -> Option<Expr<'a>> {
        let mut expr = self.primary()?;

        loop {
            if self.match_token(&[TokenKind::LeftParen]) {
                expr = self.finish_call(expr, Vec::new())?;
            } else if self.check_generic_call() {
                self.advance(); // consume '<'
                let mut type_args = Vec::new();
                if !self.check(&TokenKind::Greater) {
                    loop {
                        {
                            let ty = self.parse_type_expr()?;
                            type_args.push(ty);
                        }
                        if !self.match_token(&[TokenKind::Comma]) {
                            break;
                        }
                    }
                }
                if !self.match_token(&[TokenKind::Greater]) {
                    self.error_at_current("Expected '>' after generic type arguments.");
                    return None;
                }
                if !self.match_token(&[TokenKind::LeftParen]) {
                    self.error_at_current("Expected '(' after generic type arguments.");
                    return None;
                }
                expr = self.finish_call(expr, type_args)?;
            } else if self.match_token(&[TokenKind::Dot]) {
                let name = if let Some(Token {
                    kind: TokenKind::Identifier(n),
                    ..
                }) = self.peek().cloned()
                {
                    self.advance();
                    n
                } else {
                    self.error_at_current("Expected property name after '.'.");
                    return None;
                };

                let span = Span::new(
                    expr.span.file_id,
                    expr.span.start,
                    self.previous().span.end,
                    expr.span.start_loc,
                    self.previous().span.end_loc,
                );
                expr = Expr::new(
                    ExprKind::Get {
                        object: self.session.ast_arena.alloc(expr),
                        name,
                    },
                    span,
                );
            } else if self.match_token(&[TokenKind::QuestionDot]) {
                let name = if let Some(Token {
                    kind: TokenKind::Identifier(n),
                    ..
                }) = self.peek().cloned()
                {
                    self.advance();
                    n
                } else {
                    self.error_at_current("Expected property name after '?.'.");
                    return None;
                };

                let span = Span::new(
                    expr.span.file_id,
                    expr.span.start,
                    self.previous().span.end,
                    expr.span.start_loc,
                    self.previous().span.end_loc,
                );
                expr = Expr::new(
                    ExprKind::OptionalGet {
                        object: self.session.ast_arena.alloc(expr),
                        name,
                    },
                    span,
                );
            } else if self.match_token(&[TokenKind::Bang]) {
                let span = Span::new(
                    expr.span.file_id,
                    expr.span.start,
                    self.previous().span.end,
                    expr.span.start_loc,
                    self.previous().span.end_loc,
                );
                expr = Expr::new(
                    ExprKind::ForceUnwrap(self.session.ast_arena.alloc(expr)),
                    span,
                );
            } else if self.match_token(&[TokenKind::Question]) {
                let span = Span::new(
                    expr.span.file_id,
                    expr.span.start,
                    self.previous().span.end,
                    expr.span.start_loc,
                    self.previous().span.end_loc,
                );
                expr = Expr::new(
                    ExprKind::PostfixTry(self.session.ast_arena.alloc(expr)),
                    span,
                );
            } else if self.match_token(&[TokenKind::LeftBracket]) {
                let index = self.expression()?;
                if !self.match_token(&[TokenKind::RightBracket]) {
                    self.error_at_current("Expected ']' after index.");
                    return None;
                }
                let span = Span::new(
                    expr.span.file_id,
                    expr.span.start,
                    self.previous().span.end,
                    expr.span.start_loc,
                    self.previous().span.end_loc,
                );
                expr = Expr::new(
                    ExprKind::IndexGet {
                        object: self.session.ast_arena.alloc(expr),
                        index: self.session.ast_arena.alloc(index),
                    },
                    span,
                );
            } else {
                break;
            }
        }
        Some(expr)
    }

    pub(crate) fn finish_call(&mut self, callee: Expr<'a>, type_args: Vec<TypeExpr<'a>>) -> Option<Expr<'a>> {
        let mut arguments = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                arguments.push(self.expression()?);
                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }
        if !self.match_token(&[TokenKind::RightParen]) {
            self.error_at_current("Expected ')' after arguments.");
            return None;
        }
        let span = Span::new(
            callee.span.file_id,
            callee.span.start,
            self.previous().span.end,
            callee.span.start_loc,
            self.previous().span.end_loc,
        );
        Some(Expr::new(
            ExprKind::Call {
                callee: self.session.ast_arena.alloc(callee),
                type_args,
                arguments,
            },
            span,
        ))
    }

    pub(crate) fn check_generic_call(&self) -> bool {
        if !self.check(&TokenKind::Less) {
            return false;
        }
        let mut i = self.current + 1;
        let mut depth = 1;
        while i < self.tokens.len() {
            match self.tokens[i].kind {
                TokenKind::Less => depth += 1,
                TokenKind::Greater => {
                    depth -= 1;
                    if depth == 0 {
                        // Check if the next token is '('
                        return i + 1 < self.tokens.len()
                            && self.tokens[i + 1].kind == TokenKind::LeftParen;
                    }
                }
                TokenKind::LeftParen
                | TokenKind::LeftBrace
                | TokenKind::Semicolon
                | TokenKind::Equal => {
                    return false; // Definitely not generic type args
                }
                _ => {}
            }
            i += 1;
        }
        false
    }

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
