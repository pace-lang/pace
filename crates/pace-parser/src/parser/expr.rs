use super::Parser;
use crate::lexer::Token;
use pace_ast::{BinaryOp, Expr, Stmt, UnaryOp};

impl<'a, 'b> Parser<'a, 'b> {
    pub(crate) fn parse_expr(
        &mut self,
    ) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        self.parse_assignment()
    }

    pub(crate) fn parse_assignment(
        &mut self,
    ) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let expr = self.parse_null_coalesce()?;

        if self.match_token(Token::Eq) {
            let value = self.parse_assignment()?; // Right-associative

            match self.arena.get_expr(expr) {
                Expr::Identifier(_, _) | Expr::MemberAccess { .. } => {
                    return Ok(self.alloc_expr(Expr::Assign {
                        target: expr,
                        value,
                    }));
                }
                _ => {
                    return Err(pace_errors::SyntaxError::Generic {
                        message: "Invalid assignment target".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
            }
        }

        Ok(expr)
    }

    pub(crate) fn parse_null_coalesce(
        &mut self,
    ) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_logical_or()?;
        while self.match_token(Token::QuestionQuestion) {
            let right = self.parse_logical_or()?;
            expr = self.alloc_expr(Expr::NullCoalesce { left: expr, right });
        }
        Ok(expr)
    }

    pub(crate) fn parse_logical_or(
        &mut self,
    ) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_logical_and()?;
        while self.match_token(Token::PipePipe) {
            let right = self.parse_logical_and()?;
            expr = self.alloc_expr(Expr::Binary {
                left: expr,
                op: BinaryOp::Or,
                right,
            });
        }
        Ok(expr)
    }

    pub(crate) fn parse_logical_and(
        &mut self,
    ) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_equality()?;
        while self.match_token(Token::AndAnd) {
            let right = self.parse_equality()?;
            expr = self.alloc_expr(Expr::Binary {
                left: expr,
                op: BinaryOp::And,
                right,
            });
        }
        Ok(expr)
    }

    pub(crate) fn parse_equality(
        &mut self,
    ) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_relational()?;
        while self.current_token == Token::EqEq || self.current_token == Token::NotEq {
            let op = if self.current_token == Token::EqEq {
                BinaryOp::Eq
            } else {
                BinaryOp::NotEq
            };
            self.advance();
            let right = self.parse_relational()?;
            expr = self.alloc_expr(Expr::Binary {
                left: expr,
                op,
                right,
            });
        }
        Ok(expr)
    }

    pub(crate) fn parse_relational(
        &mut self,
    ) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_term()?;
        while matches!(
            self.current_token,
            Token::Less | Token::LessEq | Token::Greater | Token::GreaterEq
        ) {
            let op = match self.current_token {
                Token::Less => BinaryOp::Less,
                Token::LessEq => BinaryOp::LessEq,
                Token::Greater => BinaryOp::Greater,
                Token::GreaterEq => BinaryOp::GreaterEq,
                _ => unreachable!(),
            };
            self.advance();
            let right = self.parse_term()?;
            expr = self.alloc_expr(Expr::Binary {
                left: expr,
                op,
                right,
            });
        }
        Ok(expr)
    }

    pub(crate) fn parse_term(
        &mut self,
    ) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_factor()?;
        while self.current_token == Token::Plus || self.current_token == Token::Minus {
            let op = if self.current_token == Token::Plus {
                BinaryOp::Add
            } else {
                BinaryOp::Sub
            };
            self.advance();
            let right = self.parse_factor()?;
            expr = self.alloc_expr(Expr::Binary {
                left: expr,
                op,
                right,
            });
        }
        Ok(expr)
    }

    pub(crate) fn parse_factor(
        &mut self,
    ) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_unary()?;
        while self.current_token == Token::Star
            || self.current_token == Token::Slash
            || self.current_token == Token::Mod
        {
            let op = if self.current_token == Token::Star {
                BinaryOp::Mul
            } else if self.current_token == Token::Slash {
                BinaryOp::Div
            } else {
                BinaryOp::Mod
            };
            self.advance();
            let right = self.parse_unary()?;
            expr = self.alloc_expr(Expr::Binary {
                left: expr,
                op,
                right,
            });
        }
        Ok(expr)
    }

    pub(crate) fn parse_unary(
        &mut self,
    ) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        if self.match_token(Token::Await) {
            let expr = self.parse_unary()?;
            Ok(self.alloc_expr(Expr::Await(expr)))
        } else if self.current_token == Token::Bang
            || self.current_token == Token::Minus
            || self.current_token == Token::BitNot
        {
            let op = match self.current_token {
                Token::Bang => UnaryOp::Not,
                Token::Minus => UnaryOp::Neg,
                Token::BitNot => UnaryOp::BitNot,
                _ => unreachable!(),
            };
            self.advance();
            let expr = self.parse_unary()?;
            Ok(self.alloc_expr(Expr::Unary { op, expr }))
        } else {
            self.parse_postfix()
        }
    }

    pub(crate) fn parse_postfix(
        &mut self,
    ) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.match_token(Token::Bang) {
                expr = self.alloc_expr(Expr::Unwrap(expr));
            } else if self.match_token(Token::Question) {
                expr = self.alloc_expr(Expr::Try(expr));
            } else if self.match_token(Token::Dot) {
                let property = match &self.current_token {
                    Token::Ident(id) => *id,
                    _ => {
                        return Err(pace_errors::SyntaxError::Generic {
                            message: "Expected property name after '.'".to_string(),
                            src: miette::NamedSource::new(
                                self.file_name.clone(),
                                self.src.to_string(),
                            ),
                            span: self.current_span,
                        });
                    }
                };
                self.advance();
                expr = self.alloc_expr(Expr::MemberAccess {
                    object: expr,
                    property: property.into(),
                    computed_class: None,
                    is_static_operator: false,
                });
            } else if self.match_token(Token::ColonColon) {
                let property = match &self.current_token {
                    Token::Ident(id) => *id,
                    _ => {
                        return Err(pace_errors::SyntaxError::Generic {
                            message: "Expected variant name after '::'".to_string(),
                            src: miette::NamedSource::new(
                                self.file_name.clone(),
                                self.src.to_string(),
                            ),
                            span: self.current_span,
                        });
                    }
                };
                self.advance();
                expr = self.alloc_expr(Expr::MemberAccess {
                    object: expr,
                    property: property.into(),
                    computed_class: None,
                    is_static_operator: true,
                });
            } else if self.match_token(Token::QuestionDot) {
                let property = match &self.current_token {
                    Token::Ident(id) => *id,
                    _ => {
                        return Err(pace_errors::SyntaxError::Generic {
                            message: "Expected property name after '?.'".to_string(),
                            src: miette::NamedSource::new(
                                self.file_name.clone(),
                                self.src.to_string(),
                            ),
                            span: self.current_span,
                        });
                    }
                };
                self.advance();
                expr = self.alloc_expr(Expr::OptionalMemberAccess {
                    object: expr,
                    property: property.into(),
                });
            } else if self.current_token == Token::Less {
                let backup_lexer = self.lexer.clone();
                let backup_token = self.current_token.clone();
                let backup_span = self.current_span;
                let backup_errors = self.errors.clone();

                self.advance();
                let mut generic_args = Vec::new();
                let mut success = false;

                if self.current_token != Token::Greater {
                    loop {
                        match self.parse_type_annotation() {
                            Ok(ty) => generic_args.push(ty),
                            Err(_) => break,
                        }
                        if self.match_token(Token::Comma) {
                            continue;
                        }
                        break;
                    }
                }

                if self.match_token(Token::Greater) {
                    success = true;
                    expr = self.alloc_expr(Expr::GenericInstantiation {
                        callee: expr,
                        generic_args,
                    });
                }

                if !success {
                    // Backtrack, it's a Less-Than binary operator!
                    self.lexer = backup_lexer;
                    self.current_token = backup_token;
                    self.current_span = backup_span;
                    self.errors = backup_errors;
                    break;
                }
            } else if self.match_token(Token::LParen) {
                let mut args = Vec::new();
                if self.current_token != Token::RParen {
                    loop {
                        args.push(self.parse_expr()?);
                        if !self.match_token(Token::Comma) {
                            break;
                        }
                    }
                }
                if !self.match_token(Token::RParen) {
                    if self.current_token == Token::Colon {
                        return Err(pace_errors::SyntaxError::Generic { message: "Expected ')' after arguments. (Note: if you are declaring a function, ensure it is prefixed with the 'func' keyword)".to_string(), src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()), span: self.current_span });
                    }
                    return Err(pace_errors::SyntaxError::Generic {
                        message: "Expected ')' after arguments".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
                expr = self.alloc_expr(Expr::Call { callee: expr, args });
            } else {
                break;
            }
        }

        Ok(expr)
    }

    pub(crate) fn parse_primary(
        &mut self,
    ) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        match &self.current_token {
            Token::Int(n) => {
                let v = *n;
                self.advance();
                Ok(self.alloc_expr(Expr::IntLiteral(v)))
            }
            Token::Float(f) => {
                let v = *f;
                self.advance();
                Ok(self.alloc_expr(Expr::FloatLiteral(v)))
            }
            Token::String(s) => {
                let v = s.clone();
                let span = self.current_span;
                self.advance();
                self.parse_interpolated_string(v, span)
            }
            Token::Bool(b) => {
                let v = *b;
                self.advance();
                Ok(self.alloc_expr(Expr::BoolLiteral(v)))
            }
            Token::Null => {
                self.advance();
                Ok(self.alloc_expr(Expr::Null))
            }
            Token::Ident(id) => {
                let v = *id;
                let span = self.current_span;
                self.advance();
                Ok(self.alloc_expr(Expr::Identifier(v.into(), span)))
            }
            Token::LBracket => {
                self.advance();
                let mut elements = vec![];
                if self.current_token != Token::RBracket {
                    loop {
                        elements.push(self.parse_expr()?);
                        if self.match_token(Token::Comma) {
                            if self.current_token == Token::RBracket {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
                if !self.match_token(Token::RBracket) {
                    return Err(pace_errors::SyntaxError::Generic { message: "Expected ']'".to_string(), span: self.current_span, src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()) });
                }
                Ok(self.alloc_expr(Expr::ArrayLiteral(elements)))
            }
            Token::LBrace => {
                self.advance();
                let mut elements = vec![];
                if self.current_token != Token::RBrace {
                    loop {
                        let key = self.parse_expr()?;
                        if !self.match_token(Token::Colon) {
                            return Err(pace_errors::SyntaxError::Generic { message: "Expected ':' after map key".to_string(), span: self.current_span, src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()) });
                        }
                        let val = self.parse_expr()?;
                        elements.push((key, val));
                        if self.match_token(Token::Comma) {
                            if self.current_token == Token::RBrace {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
                if !self.match_token(Token::RBrace) {
                    return Err(pace_errors::SyntaxError::Generic { message: "Expected '}'".to_string(), span: self.current_span, src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()) });
                }
                Ok(self.alloc_expr(Expr::MapLiteral(elements)))
            }
            Token::LParen => {
                let _start_span = self.current_span;

                // Try parsing as a closure first by checking if it matches closure parameter syntax
                // To avoid messing up the main parser state if it fails, we can clone it
                let mut dummy_arena = pace_ast::arena::AstArena::new();
                let mut lookahead = Parser {
                    file_name: self.file_name.clone(),
                    src: self.src,
                    arena: &mut dummy_arena,
                    lexer: self.lexer.clone(),
                    current_token: self.current_token.clone(),
                    current_span: self.current_span,
                    errors: vec![],
                };

                lookahead.advance(); // consume '('
                let mut is_closure = false;
                if lookahead.current_token == Token::RParen {
                    lookahead.advance();
                    if lookahead.current_token == Token::FatArrow
                        || lookahead.current_token == Token::Arrow
                    {
                        is_closure = true;
                    }
                } else if let Token::Ident(_) = lookahead.current_token {
                    lookahead.advance();
                    if lookahead.current_token == Token::Colon {
                        is_closure = true; // (ident: type) -> definitely a closure
                    }
                }

                if is_closure {
                    self.advance(); // consume '('
                    let mut params = Vec::new();
                    while self.current_token != Token::RParen && self.current_token != Token::Eof {
                        let param_name = match &self.current_token {
                            Token::Ident(id) => *id,
                            _ => {
                                return Err(pace_errors::SyntaxError::Generic {
                                    message: "Expected parameter name in closure".to_string(),
                                    src: miette::NamedSource::new(
                                        self.file_name.clone(),
                                        self.src.to_string(),
                                    ),
                                    span: self.current_span,
                                });
                            }
                        };
                        self.advance();
                        if !self.match_token(Token::Colon) {
                            return Err(pace_errors::SyntaxError::Generic {
                                message: "Expected ':' after closure parameter name".to_string(),
                                src: miette::NamedSource::new(
                                    self.file_name.clone(),
                                    self.src.to_string(),
                                ),
                                span: self.current_span,
                            });
                        }
                        let param_ty = self.parse_type_annotation()?;
                        params.push((ustr::Ustr::from(param_name), param_ty));
                        if !self.match_token(Token::Comma) {
                            break;
                        }
                    }
                    if !self.match_token(Token::RParen) {
                        return Err(pace_errors::SyntaxError::Generic {
                            message: "Expected ')' after closure parameters".to_string(),
                            src: miette::NamedSource::new(
                                self.file_name.clone(),
                                self.src.to_string(),
                            ),
                            span: self.current_span,
                        });
                    }

                    let mut return_type = None;
                    if self.match_token(Token::Arrow) {
                        return_type = Some(self.parse_type_annotation()?);
                    }

                    if !self.match_token(Token::FatArrow) {
                        return Err(pace_errors::SyntaxError::Generic {
                            message: "Expected '=>' for closure body".to_string(),
                            src: miette::NamedSource::new(
                                self.file_name.clone(),
                                self.src.to_string(),
                            ),
                            span: self.current_span,
                        });
                    }

                    let body = if self.current_token == Token::LBrace {
                        let block = self.parse_block()?;
                        let stmts = match self.arena.get_stmt(block).clone() {
                            Stmt::Block(stmts) => stmts,
                            _ => vec![], // Shouldn't happen
                        };
                        self.alloc_expr(Expr::Block(stmts))
                    } else {
                        self.parse_expr()?
                    };

                    return Ok(self.alloc_expr(Expr::Closure {
                        params,
                        return_type,
                        body,
                    }));
                }

                self.advance(); // consume '(' as normal group
                let expr = self.parse_expr()?;
                if !self.match_token(Token::RParen) {
                    return Err(pace_errors::SyntaxError::Generic {
                        message: "Expected ')'".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
                Ok(expr)
            }
            _ => Err(pace_errors::SyntaxError::Generic {
                message: format!("Unexpected token: {:?}", self.current_token),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            }),
        }
    }

    pub(crate) fn parse_interpolated_string(
        &mut self,
        s: String,
        base_span: pace_span::Span,
    ) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut parts = Vec::new();
        let mut current_text = String::new();
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '$' && chars.peek() == Some(&'{') {
                chars.next(); // consume '{'
                if !current_text.is_empty() {
                    parts.push(self.alloc_expr(Expr::StringLiteral(current_text.clone().into())));
                    current_text.clear();
                }

                let mut expr_str = String::new();
                let mut depth = 1;
                for inner_c in chars.by_ref() {
                    if inner_c == '{' {
                        depth += 1;
                    }
                    if inner_c == '}' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    expr_str.push(inner_c);
                }

                if depth > 0 {
                    return Err(pace_errors::SyntaxError::Generic {
                        message: "Unclosed string interpolation block, expected '}'".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: base_span,
                    });
                }

                let mut nested_parser =
                    Parser::new_with_arena(&expr_str, &self.file_name, self.arena);
                let expr = nested_parser.parse_expr().map_err(|err| {
                    let msg = match err {
                        pace_errors::SyntaxError::Generic { message, .. } => message,
                    };
                    pace_errors::SyntaxError::Generic {
                        message: format!("In interpolated string: {}", msg),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: base_span,
                    }
                })?;

                if nested_parser.current_token != Token::Eof {
                    return Err(pace_errors::SyntaxError::Generic {
                        message: "Unexpected tokens in interpolated string".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: base_span,
                    });
                }

                self.errors.extend(nested_parser.errors);
                parts.push(expr);
            } else {
                current_text.push(c);
            }
        }

        if !current_text.is_empty() {
            parts.push(self.alloc_expr(Expr::StringLiteral(current_text.into())));
        }

        if parts.len() == 1 {
            if let Expr::StringLiteral(_) = self.arena.get_expr(parts[0]) {
                return Ok(parts.pop().unwrap());
            }
        } else if parts.is_empty() {
            return Ok(self.alloc_expr(Expr::StringLiteral(String::new().into())));
        }

        Ok(self.alloc_expr(Expr::InterpolatedString(parts)))
    }
}
