use pace_ast::{Expr, Ident, MatchArm, Pattern, Type};
use pace_errors::Diagnostic;
use pace_lexer::{Token, TokenKind};

use super::Parser;

impl<'a> Parser<'a> {
    pub(crate) fn parse_postfix_expr(&mut self) -> Result<Expr, Diagnostic> {
        if self.check(&TokenKind::Await) {
            let start_tok = self.expect(TokenKind::Await)?;
            let inner = self.parse_postfix_expr()?;
            let span = start_tok.span.merge(inner.span());
            return Ok(Expr::Await(Box::new(inner), span));
        }

        let mut left = self.parse_primary()?;

        loop {
            if self.check(&TokenKind::Dot) || self.check(&TokenKind::OptChain) {
                let is_opt = self.check(&TokenKind::OptChain);
                self.advance();
                let member_name = match &self.current {
                    Some(Token {
                        kind: TokenKind::Ident(name),
                        span,
                    }) => {
                        let ident = Ident {
                            name: pace_span::intern(name),
                            span: *span,
                        };
                        self.advance();
                        ident
                    }
                    _ => {
                        return Err(Diagnostic::error("Expected member name after '.' or '?.'")
                            .with_span(self.current_span()));
                    }
                };

                let span = left.span().merge(member_name.span);
                if is_opt {
                    left = Expr::OptionalMemberAccess {
                        span,
                        object: Box::new(left),
                        member: member_name,
                    };
                } else {
                    left = Expr::MemberAccess {
                        span,
                        object: Box::new(left),
                        member: member_name,
                    };
                }
            } else if self.check(&TokenKind::LParen) {
                self.advance();
                let mut args = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        // Check for named argument: `ident:`
                        let mut label = None;

                        let mut parsed_expr = self.parse_expr()?;

                        if let Expr::Ident(ref ident, _) = parsed_expr
                            && self.check(&TokenKind::Colon)
                        {
                            self.advance(); // consume `:`
                            label = Some(ident.clone());
                            parsed_expr = self.parse_expr()?;
                        }
                        args.push(pace_ast::CallArg {
                            label,
                            expr: parsed_expr,
                        });
                        if self.check(&TokenKind::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                let rparen = self.expect(TokenKind::RParen)?;
                left = Expr::Call {
                    span: left.span().merge(rparen.span),
                    callee: Box::new(left),
                    args,
                };
            } else if self.check(&TokenKind::Question) {
                let q_tok = self.expect(TokenKind::Question)?;
                let span = left.span().merge(q_tok.span);
                left = Expr::Try(Box::new(left), span);
            } else if self.check(&TokenKind::LBracket) {
                self.advance();
                let index_expr = self.parse_expr()?;
                let rbracket = self.expect(TokenKind::RBracket)?;
                let span = left.span().merge(rbracket.span);
                left = Expr::Index {
                    target: Box::new(left),
                    index: Box::new(index_expr),
                    span,
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_unary_expr(&mut self) -> Result<Expr, Diagnostic> {
        if self.check(&TokenKind::Minus) || self.check(&TokenKind::Bang) {
            let tok = self.current.take().unwrap();
            self.advance();
            let op = match tok.kind {
                TokenKind::Minus => pace_ast::UnaryOp::Neg,
                TokenKind::Bang => pace_ast::UnaryOp::Not,
                _ => unreachable!(),
            };
            let expr = self.parse_unary_expr()?;
            let span = tok.span.merge(expr.span());
            return Ok(Expr::Unary {
                op,
                expr: Box::new(expr),
                span,
            });
        }
        self.parse_postfix_expr()
    }

    fn peek_binary_op(&self) -> Option<(pace_ast::BinaryOp, u8)> {
        let op = if self.check(&TokenKind::OrOr) {
            (pace_ast::BinaryOp::Or, 1)
        } else if self.check(&TokenKind::AndAnd) {
            (pace_ast::BinaryOp::And, 2)
        } else if self.check(&TokenKind::NullCoalesce) {
            (pace_ast::BinaryOp::NullCoalesce, 3)
        } else if self.check(&TokenKind::EqEq) {
            (pace_ast::BinaryOp::EqEq, 4)
        } else if self.check(&TokenKind::NotEq) {
            (pace_ast::BinaryOp::NotEq, 4)
        } else if self.check(&TokenKind::Gt) {
            (pace_ast::BinaryOp::Gt, 5)
        } else if self.check(&TokenKind::Lt) {
            (pace_ast::BinaryOp::Lt, 5)
        } else if self.check(&TokenKind::GtEq) {
            (pace_ast::BinaryOp::GtEq, 5)
        } else if self.check(&TokenKind::LtEq) {
            (pace_ast::BinaryOp::LtEq, 5)
        } else if self.check(&TokenKind::Plus) {
            (pace_ast::BinaryOp::Add, 6)
        } else if self.check(&TokenKind::Minus) {
            (pace_ast::BinaryOp::Sub, 6)
        } else if self.check(&TokenKind::Star) {
            (pace_ast::BinaryOp::Mul, 7)
        } else if self.check(&TokenKind::Slash) {
            (pace_ast::BinaryOp::Div, 7)
        } else if self.check(&TokenKind::Percent) {
            (pace_ast::BinaryOp::Mod, 7)
        } else {
            return None;
        };
        Some(op)
    }

    pub(crate) fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        let left = self.parse_unary_expr()?;

        if self.check(&TokenKind::Eq) {
            self.advance();
            let right = self.parse_expr()?;
            let span = left.span().merge(right.span());
            return Ok(Expr::Assign {
                target: Box::new(left),
                value: Box::new(right),
                span,
            });
        }

        self.parse_binary_rhs(left, 1)
    }

    fn parse_binary_rhs(&mut self, mut left: Expr, min_prec: u8) -> Result<Expr, Diagnostic> {
        while let Some((op, prec)) = self.peek_binary_op() {
            if prec < min_prec {
                break;
            }
            self.advance();
            let mut right = self.parse_unary_expr()?;
            while let Some((_, next_prec)) = self.peek_binary_op() {
                if next_prec > prec {
                    right = self.parse_binary_rhs(right, prec + 1)?;
                } else {
                    break;
                }
            }
            left = Expr::Binary {
                span: left.span().merge(right.span()),
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn try_parse_generic_args_expr(&mut self) -> Option<Vec<Type>> {
        let saved_lexer = self.lexer.clone();
        let saved_current = self.current.clone();
        match self.parse_generic_args() {
            Ok(Some(args)) => Some(args),
            _ => {
                self.lexer = saved_lexer;
                self.current = saved_current;
                None
            }
        }
    }

    fn try_parse_closure_expr(&mut self) -> Result<Option<Expr>, Diagnostic> {
        let saved_lexer = self.lexer.clone();
        let saved_current = self.current.clone();

        // Must start with '('
        if !self.check(&TokenKind::LParen) {
            return Ok(None);
        }
        let start_span = self.current_span();
        self.advance(); // consume '('

        let mut params = Vec::new();
        let mut valid_params = true;

        if !self.check(&TokenKind::RParen) {
            loop {
                if let Some(Token { kind: TokenKind::Ident(name), span }) = &self.current {
                    let ident = Ident {
                        name: pace_span::intern(name),
                        span: *span,
                    };
                    self.advance();
                    
                    let ty = if self.check(&TokenKind::Colon) {
                        self.advance();
                        if let Ok(t) = self.parse_type() {
                            Some(t)
                        } else {
                            valid_params = false;
                            break;
                        }
                    } else {
                        None
                    };

                    params.push(pace_ast::ClosureParam { name: ident, ty });

                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                } else {
                    valid_params = false;
                    break;
                }
            }
        }

        if !valid_params || !self.check(&TokenKind::RParen) {
            self.lexer = saved_lexer;
            self.current = saved_current;
            return Ok(None);
        }
        self.advance(); // consume ')'

        let return_type = if self.check(&TokenKind::Arrow) {
            self.advance();
            if let Ok(t) = self.parse_type() {
                Some(t)
            } else {
                self.lexer = saved_lexer;
                self.current = saved_current;
                return Ok(None);
            }
        } else {
            None
        };

        if !self.check(&TokenKind::FatArrow) {
            self.lexer = saved_lexer;
            self.current = saved_current;
            return Ok(None);
        }
        self.advance(); // consume '=>'

        let body = if self.check(&TokenKind::LBrace) {
            pace_ast::ClosureBody::Block(self.parse_block()?)
        } else {
            pace_ast::ClosureBody::Expr(Box::new(self.parse_expr()?))
        };

        let body_span = match &body {
            pace_ast::ClosureBody::Expr(e) => e.span(),
            pace_ast::ClosureBody::Block(b) => b.span,
        };
        let span = start_span.merge(body_span);
        Ok(Some(Expr::Closure {
            params,
            return_type,
            body,
            span,
        }))
    }

    pub(crate) fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        if let Some(closure) = self.try_parse_closure_expr()? {
            return Ok(closure);
        }

        if self.check(&TokenKind::If) {
            let start_tok = self.expect(TokenKind::If)?;
            let cond = self.parse_expr()?;
            let then_block = self.parse_block()?;
            let mut else_block = None;
            let mut end_span = then_block.span;
            if self.check(&TokenKind::Else) {
                self.advance();
                let b = self.parse_block()?;
                end_span = b.span;
                else_block = Some(b);
            }
            return Ok(Expr::If {
                cond: Box::new(cond),
                then_block,
                else_block,
                span: start_tok.span.merge(end_span),
            });
        }
        if self.check(&TokenKind::While) {
            let start_tok = self.expect(TokenKind::While)?;
            let cond = self.parse_expr()?;
            let body = self.parse_block()?;
            return Ok(Expr::While {
                cond: Box::new(cond),
                span: start_tok.span.merge(body.span),
                body,
            });
        }
        if self.check(&TokenKind::Match) {
            return self.parse_match_expr();
        }

        if self.check(&TokenKind::LBracket) {
            let start_tok = self.expect(TokenKind::LBracket)?;
            let mut elements = Vec::new();
            if !self.check(&TokenKind::RBracket) {
                loop {
                    elements.push(self.parse_expr()?);
                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
            let rbracket = self.expect(TokenKind::RBracket)?;
            return Ok(Expr::ListLiteral(elements, start_tok.span.merge(rbracket.span)));
        }

        if self.check(&TokenKind::LBrace) {
            let saved_lexer = self.lexer.clone();
            let saved_current = self.current.clone();

            let start_tok = self.expect(TokenKind::LBrace)?;
            if self.check(&TokenKind::RBrace) {
                let rbrace = self.expect(TokenKind::RBrace)?;
                return Ok(Expr::MapLiteral(Vec::new(), start_tok.span.merge(rbrace.span)));
            }

            let mut entries = Vec::new();
            let mut is_map = false;
            if let Ok(key) = self.parse_expr() {
                if self.check(&TokenKind::Colon) {
                    self.advance();
                    if let Ok(val) = self.parse_expr() {
                        entries.push((key, val));
                        is_map = true;
                        while self.check(&TokenKind::Comma) {
                            self.advance();
                            if self.check(&TokenKind::RBrace) {
                                break;
                            }
                            if let Ok(k) = self.parse_expr() {
                                if self.check(&TokenKind::Colon) {
                                    self.advance();
                                    if let Ok(v) = self.parse_expr() {
                                        entries.push((k, v));
                                        continue;
                                    }
                                }
                            }
                            is_map = false;
                            break;
                        }
                    }
                }
            }
            if is_map && self.check(&TokenKind::RBrace) {
                let rbrace = self.expect(TokenKind::RBrace)?;
                return Ok(Expr::MapLiteral(entries, start_tok.span.merge(rbrace.span)));
            } else {
                self.lexer = saved_lexer;
                self.current = saved_current;
            }
        }

        let tok = self.current.take().ok_or_else(|| {
            Diagnostic::error("Expected expression, found EOF").with_span(self.current_span())
        })?;
        self.advance();

        match tok.kind {
            TokenKind::Int(val) => Ok(Expr::IntLiteral(val.to_string(), tok.span)),
            TokenKind::Float(val) => Ok(Expr::FloatLiteral(val.to_string(), tok.span)),
            TokenKind::String(val) => {
                let inner = &val[1..val.len() - 1]; // Remove outer quotes
                if !inner.contains('$') {
                    return Ok(Expr::StringLiteral(val.to_string(), tok.span));
                }

                let mut exprs = Vec::new();
                let mut chars = inner.char_indices().peekable();
                let mut current_str = String::new();

                while let Some((idx, c)) = chars.next() {
                    if c == '$' {
                        if !current_str.is_empty() {
                            let end_idx = tok.span.start + 1 + idx as u32;
                            let start_idx = end_idx - current_str.len() as u32;
                            exprs.push(Expr::StringLiteral(
                                format!("\"{}\"", current_str),
                                pace_span::Span::new(tok.span.file_id, start_idx, end_idx),
                            ));
                            current_str.clear();
                        }

                        if let Some(&(_, '{')) = chars.peek() {
                            chars.next(); // consume '{'
                            let mut expr_str = String::new();
                            let mut brace_count = 1;
                            let mut in_nested_str = false;
                            let mut escaped = false;
                            for (_, inner_c) in chars.by_ref() {
                                if in_nested_str {
                                    if escaped {
                                        escaped = false;
                                    } else if inner_c == '\\' {
                                        escaped = true;
                                    } else if inner_c == '"' {
                                        in_nested_str = false;
                                    }
                                    expr_str.push(inner_c);
                                    continue;
                                }
                                if inner_c == '"' {
                                    in_nested_str = true;
                                } else if inner_c == '{' {
                                    brace_count += 1;
                                } else if inner_c == '}' {
                                    brace_count -= 1;
                                    if brace_count == 0 {
                                        break;
                                    }
                                }
                                expr_str.push(inner_c);
                            }

                            let brace_pos = tok.span.start as usize + 1 + idx + 1;
                            let sub_lexer = pace_lexer::Lexer::with_offset(
                                &expr_str,
                                self.lexer.file_id,
                                brace_pos + 1, // Skip '{'
                            );
                            let mut sub_parser = crate::Parser::new(sub_lexer);
                            let (decls, diags, _) = sub_parser.parse_program();
                            self.diagnostics.extend(diags);
                            if !decls.is_empty()
                                && let pace_ast::Decl::Expr(e, _) = &decls[0]
                            {
                                exprs.push(e.clone());
                            }
                        } else {
                            // Parse $var
                            let mut var_str = String::new();
                            while let Some(&(_, inner_c)) = chars.peek() {
                                if inner_c.is_alphanumeric() || inner_c == '_' {
                                    var_str.push(inner_c);
                                    chars.next();
                                } else {
                                    break;
                                }
                            }
                            if !var_str.is_empty() {
                                let start_pos = tok.span.start + 1 + idx as u32 + 1;
                                let end_pos = start_pos + var_str.len() as u32;
                                exprs.push(Expr::Ident(
                                    Ident {
                                        name: pace_span::intern(&var_str),
                                        span: pace_span::Span::new(
                                            tok.span.file_id,
                                            start_pos,
                                            end_pos,
                                        ),
                                    },
                                    None,
                                ));
                            } else {
                                current_str.push('$');
                            }
                        }
                    } else {
                        current_str.push(c);
                    }
                }

                if !current_str.is_empty() {
                    let end_idx = tok.span.end - 1;
                    let start_idx = end_idx - current_str.len() as u32;
                    exprs.push(Expr::StringLiteral(
                        format!("\"{}\"", current_str),
                        pace_span::Span::new(tok.span.file_id, start_idx, end_idx),
                    ));
                }

                Ok(Expr::InterpolatedString(exprs, tok.span))
            }
            TokenKind::Ident(name) => {
                let ident = Ident {
                    name: pace_span::intern(name),
                    span: tok.span,
                };
                let generic_args = self.try_parse_generic_args_expr();
                Ok(Expr::Ident(ident, generic_args))
            }
            TokenKind::Null => Ok(Expr::Null(tok.span)),
            TokenKind::LParen => {
                let expr = self.parse_expr()?;
                let _rparen = self.expect(TokenKind::RParen)?;
                Ok(expr) // For now, we drop the span of the parens to keep AST simple, or we could add Expr::Paren. But returning the inner expr is fine.
            }
            TokenKind::Super => Ok(Expr::Super(tok.span)),
            _ => Err(
                Diagnostic::error(format!("Unexpected token in expression: {:?}", tok.kind))
                    .with_span(self.current_span()),
            ),
        }
    }

    pub(crate) fn parse_match_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_tok = self.expect(TokenKind::Match)?;
        let subject = self.parse_expr()?;
        self.expect(TokenKind::LBrace)?;

        let mut arms = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            let pattern = if let Some(Token {
                kind: TokenKind::Ident(name),
                span,
            }) = &self.current
            {
                if *name == "_" {
                    let s = *span;
                    self.advance();
                    Pattern::CatchAll(s)
                } else {
                    let ident = Ident {
                        name: pace_span::intern(name),
                        span: *span,
                    };
                    self.advance();

                    if self.check(&TokenKind::LParen) {
                        self.advance();
                        let mut fields = Vec::new();
                        while !self.check(&TokenKind::RParen) && self.current.is_some() {
                            if let Some(Token {
                                kind: TokenKind::Ident(n),
                                span: fspan,
                            }) = &self.current
                            {
                                fields.push(Ident {
                                    name: pace_span::intern(n),
                                    span: *fspan,
                                });
                                self.advance();
                                if self.check(&TokenKind::Comma) {
                                    self.advance();
                                }
                            } else {
                                return Err(Diagnostic::error(
                                    "Expected identifier in match pattern",
                                )
                                .with_span(self.current_span()));
                            }
                        }
                        self.expect(TokenKind::RParen)?;
                        Pattern::Variant {
                            name: ident.clone(),
                            fields: Some(fields),
                            span: ident.span,
                        }
                    } else {
                        Pattern::Ident(ident)
                    }
                }
            } else {
                return Err(Diagnostic::error("Expected pattern").with_span(self.current_span()));
            };

            self.expect(TokenKind::FatArrow)?;
            let body = self.parse_expr()?;

            if self.check(&TokenKind::Comma) {
                self.advance();
            }

            let p_span = match &pattern {
                Pattern::Ident(id) => id.span,
                Pattern::Variant { span, .. } => *span,
                Pattern::CatchAll(s) => *s,
            };

            arms.push(MatchArm {
                pattern,
                span: p_span.merge(body.span()),
                body,
            });
        }

        let end_tok = self.expect(TokenKind::RBrace)?;
        Ok(Expr::Match {
            subject: Box::new(subject),
            arms,
            span: start_tok.span.merge(end_tok.span),
        })
    }
}
