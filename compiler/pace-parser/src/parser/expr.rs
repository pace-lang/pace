use pace_ast::{Expr, Ident, MatchArm, Pattern, Type};
use pace_errors::Diagnostic;
use pace_lexer::{Token, TokenKind};

use super::Parser;

impl<'a> Parser<'a> {
    pub(crate) fn parse_postfix_expr(&mut self) -> Result<Expr, Diagnostic> {
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
            } else {
                break;
            }
        }
        Ok(left)
    }

    pub(crate) fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_postfix_expr()?;

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

        loop {
            let op = if self.check(&TokenKind::Plus) {
                Some(pace_ast::BinaryOp::Add)
            } else if self.check(&TokenKind::Minus) {
                Some(pace_ast::BinaryOp::Sub)
            } else if self.check(&TokenKind::Star) {
                Some(pace_ast::BinaryOp::Mul)
            } else if self.check(&TokenKind::Slash) {
                Some(pace_ast::BinaryOp::Div)
            } else if self.check(&TokenKind::EqEq) {
                Some(pace_ast::BinaryOp::EqEq)
            } else if self.check(&TokenKind::NotEq) {
                Some(pace_ast::BinaryOp::NotEq)
            } else if self.check(&TokenKind::Gt) {
                Some(pace_ast::BinaryOp::Gt)
            } else if self.check(&TokenKind::Lt) {
                Some(pace_ast::BinaryOp::Lt)
            } else if self.check(&TokenKind::GtEq) {
                Some(pace_ast::BinaryOp::GtEq)
            } else if self.check(&TokenKind::LtEq) {
                Some(pace_ast::BinaryOp::LtEq)
            } else if self.check(&TokenKind::AndAnd) {
                Some(pace_ast::BinaryOp::And)
            } else if self.check(&TokenKind::OrOr) {
                Some(pace_ast::BinaryOp::Or)
            } else if self.check(&TokenKind::NullCoalesce) {
                Some(pace_ast::BinaryOp::NullCoalesce)
            } else {
                None
            };

            if let Some(op) = op {
                self.advance();
                let right = self.parse_postfix_expr()?;
                left = Expr::Binary {
                    span: left.span().merge(right.span()),
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                };
            } else {
                break;
            }
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
            let _block = self.parse_block()?;
            self.parse_expr()?
        } else {
            self.parse_expr()?
        };

        let span = start_span.merge(body.span());
        Ok(Some(Expr::Closure {
            params,
            return_type,
            body: Box::new(body),
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
                            for (_, inner_c) in chars.by_ref() {
                                if inner_c == '{' {
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
                            let (decls, _, _) = sub_parser.parse_program();
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
