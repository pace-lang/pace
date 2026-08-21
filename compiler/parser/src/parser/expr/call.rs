use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
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
                let matched_greater = if self.match_token(&[TokenKind::Greater]) {
                    true
                } else if self.check(&TokenKind::GreaterGreater) {
                    self.tokens[self.current].kind = TokenKind::Greater;
                    self.tokens[self.current].span.start += 1;
                    self.tokens[self.current].span.start_loc.column += 1;
                    true
                } else {
                    false
                };

                if !matched_greater {
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
            } else if self.check(&TokenKind::Question) {
                if self.is_ternary_question() {
                    break;
                }
                self.advance(); // consume '?'
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

    pub(crate) fn finish_call(
        &mut self,
        callee: Expr<'a>,
        type_args: Vec<TypeExpr<'a>>,
    ) -> Option<Expr<'a>> {
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
                        // Check if the next token is '(' or '.'
                        return i + 1 < self.tokens.len()
                            && (self.tokens[i + 1].kind == TokenKind::LeftParen
                                || self.tokens[i + 1].kind == TokenKind::Dot);
                    }
                }
                TokenKind::GreaterGreater => {
                    depth -= 2;
                    if depth < 0 {
                        return false;
                    }
                    if depth == 0 {
                        return i + 1 < self.tokens.len()
                            && (self.tokens[i + 1].kind == TokenKind::LeftParen
                                || self.tokens[i + 1].kind == TokenKind::Dot);
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

    pub(crate) fn is_ternary_question(&self) -> bool {
        let mut depth = 0;
        let mut i = self.current + 1; // start after '?'

        while i < self.tokens.len() {
            match self.tokens[i].kind {
                TokenKind::Question => depth += 1,
                TokenKind::Colon => {
                    if depth == 0 {
                        return true;
                    }
                    depth -= 1;
                }
                TokenKind::LeftParen | TokenKind::LeftBrace | TokenKind::LeftBracket => {
                    // skip over matched delimiters to avoid counting inner colons (e.g. in dicts/closures if any)
                    let mut delim_depth = 1;
                    let open_kind = self.tokens[i].kind.clone();
                    let close_kind = match open_kind {
                        TokenKind::LeftParen => TokenKind::RightParen,
                        TokenKind::LeftBrace => TokenKind::RightBrace,
                        TokenKind::LeftBracket => TokenKind::RightBracket,
                        _ => unreachable!(),
                    };
                    i += 1;
                    while i < self.tokens.len() && delim_depth > 0 {
                        if self.tokens[i].kind == open_kind {
                            delim_depth += 1;
                        } else if self.tokens[i].kind == close_kind {
                            delim_depth -= 1;
                        }
                        i += 1;
                    }
                    continue;
                }
                TokenKind::Semicolon | TokenKind::Comma | TokenKind::Eof => {
                    return false;
                }
                _ => {}
            }
            i += 1;
        }
        false
    }
}
