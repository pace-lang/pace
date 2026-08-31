use super::Parser;
use crate::lexer::Token;
use pace_ast::{Expr, Stmt, TypeAnnotation, Visibility};

impl<'a, 'b> Parser<'a, 'b> {
    pub(crate) fn parse_stmt(
        &mut self,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {        let visibility = if self.match_token(Token::Private) {
            Visibility::Private
        } else {
            Visibility::Public // Default is public
        };

        let is_async = self.match_token(Token::Async);
        let is_static = self.match_token(Token::Static);
        let is_extern = self.match_token(Token::Extern);

        match self.current_token {
            Token::Let => self.parse_var_decl(false, is_static, visibility), // doc_comments on vars omitted for now
            Token::Var => self.parse_var_decl(true, is_static, visibility),
            Token::Func => self.parse_func_decl(
                is_async,
                visibility,
                is_static,
                is_extern,
            ),
            Token::Class => self.parse_class_decl(),
            Token::Actor => self.parse_actor_decl(),
            Token::Interface => {
                self.parse_interface_decl()
            }
            Token::Struct => self.parse_struct_decl(),
            Token::Enum => self.parse_enum_decl(),
            Token::If => self.parse_if_stmt(),
            Token::While => self.parse_while_stmt(),
            Token::Loop => self.parse_loop_stmt(),
            Token::For => self.parse_for_stmt(),
            Token::Match => self.parse_match_stmt(),
            Token::Import => self.parse_import_stmt(),
            Token::Export => self.parse_export_stmt(),
            Token::LBrace => self.parse_block(),
            Token::Return => {
                self.advance();
                let expr = if self.current_token != Token::Semi {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                if !self.match_token(Token::Semi) {
                    return Err(pace_errors::SyntaxError::Generic {
                        message: "Expected ';' after return".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
                Ok(self.alloc_stmt(Stmt::Return(expr)))
            }
            _ => {
                let expr = self.parse_expr()?;
                if !self.match_token(Token::Semi) {
                    if let pace_ast::Expr::Call { callee, .. } = self.arena.get_expr(expr)
                        && (self.current_token == Token::LBrace
                            || self.current_token == Token::Arrow)
                        && let Expr::Identifier(name, _) = self.arena.get_expr(*callee)
                    {
                        return Err(pace_errors::SyntaxError::Generic {
                            message: format!(
                                "Functions and methods must be prefixed with 'func'. Did you forget 'func' before '{}'?",
                                name
                            ),
                            src: miette::NamedSource::new(
                                self.file_name.clone(),
                                self.src.to_string(),
                            ),
                            span: self.current_span,
                        });
                    }
                    return Err(pace_errors::SyntaxError::Generic {
                        message: "Expected ';' after expression".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
                Ok(self.alloc_stmt(Stmt::Expr(expr)))
            }
        }
    }

    pub(crate) fn parse_generic_params(
        &mut self,
    ) -> Result<Option<Vec<ustr::Ustr>>, pace_errors::SyntaxError> {
        if self.match_token(Token::Less) {
            let mut params = Vec::new();
            while self.current_token != Token::Greater && self.current_token != Token::Eof {
                if let Token::Ident(id) = &self.current_token {
                    params.push((*id).into());
                    self.advance();
                } else {
                    return Err(pace_errors::SyntaxError::Generic {
                        message: "Expected generic parameter name".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
                if !self.match_token(Token::Comma) {
                    break;
                }
            }
            if !self.match_token(Token::Greater) {
                return Err(pace_errors::SyntaxError::Generic {
                    message: "Expected '>' after generic parameters".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
            Ok(Some(params))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn parse_type_annotation(
        &mut self,
    ) -> Result<TypeAnnotation, pace_errors::SyntaxError> {
        // Parse function types like (Int, String) -> Bool
        if self.match_token(Token::LParen) {
            let mut params = Vec::new();
            while self.current_token != Token::RParen && self.current_token != Token::Eof {
                params.push(self.parse_type_annotation()?);
                if !self.match_token(Token::Comma) {
                    break;
                }
            }
            if !self.match_token(Token::RParen) {
                return Err(pace_errors::SyntaxError::Generic {
                    message: "Expected ')' after function type parameters".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
            if !self.match_token(Token::Arrow) {
                return Err(pace_errors::SyntaxError::Generic {
                    message: "Expected '->' after function parameters".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
            let return_type = self.parse_type_annotation()?;

            return Ok(TypeAnnotation {
                module_prefix: None,
                name: "Function".to_string().into(),
                args: vec![],
                is_nullable: false,
                is_function: true,
                function_params: Some(params),
                function_return: Some(Box::new(return_type)),
            });
        }

        let mut name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError::Generic {
                    message: "Expected type name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let mut module_prefix = None;
        if self.match_token(Token::Dot) {
            module_prefix = Some(name);
            name = match &self.current_token {
                Token::Ident(id) => *id,
                _ => {
                    return Err(pace_errors::SyntaxError::Generic {
                        message: "Expected type name after '.'".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
            };
            self.advance();
        }

        let mut args = Vec::new();
        if self.match_token(Token::Less) {
            while self.current_token != Token::Greater && self.current_token != Token::Eof {
                args.push(self.parse_type_annotation()?);
                if !self.match_token(Token::Comma) {
                    break;
                }
            }
            if !self.match_token(Token::Greater) {
                return Err(pace_errors::SyntaxError::Generic {
                    message: "Expected '>' after generic arguments".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        }

        let is_nullable = self.match_token(Token::Question);

        Ok(TypeAnnotation {
            module_prefix: module_prefix.map(ustr::Ustr::from),
            name: name.into(),
            args,
            is_nullable,
            is_function: false,
            function_params: None,
            function_return: None,
        })
    }

    pub(crate) fn parse_block(
        &mut self,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume '{'
        let mut stmts = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            match self.parse_stmt() {
                Ok(stmt) => stmts.push(stmt),
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }
        if !self.match_token(Token::RBrace) {
            return Err(pace_errors::SyntaxError::Generic {
                message: "Expected '}' after block".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }
        Ok(self.alloc_stmt(Stmt::Block(stmts)))
    }

    pub(crate) fn parse_if_stmt(
        &mut self,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume if
        let condition = self.parse_expr()?;

        let then_branch = self.parse_stmt()?;

        let mut else_branch = None;
        if self.match_token(Token::Else) {
            else_branch = Some(self.parse_stmt()?);
        }

        Ok(self.alloc_stmt(Stmt::If {
            condition,
            then_branch,
            else_branch,
        }))
    }

    pub(crate) fn parse_while_stmt(
        &mut self,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume while
        let condition = self.parse_expr()?;
        let body = self.parse_stmt()?;
        Ok(self.alloc_stmt(Stmt::While { condition, body }))
    }

    pub(crate) fn parse_loop_stmt(
        &mut self,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume loop
        let body = self.parse_stmt()?;
        Ok(self.alloc_stmt(Stmt::Loop { body }))
    }

    pub(crate) fn parse_for_stmt(
        &mut self,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume for
        let item = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError::Generic {
                    message: "Expected identifier in for loop".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        if !self.match_token(Token::In) {
            return Err(pace_errors::SyntaxError::Generic {
                message: "Expected 'in' in for loop".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let iterable = self.parse_expr()?;
        let body = self.parse_stmt()?;

        Ok(self.alloc_stmt(Stmt::ForIn {
            item: item.into(),
            iterable,
            body,
        }))
    }

    pub(crate) fn parse_pattern(&mut self) -> Result<pace_ast::Pattern, pace_errors::SyntaxError> {
        match self.current_token.clone() {
            Token::Ident(name) => {
                self.advance();

                if name == "_" {
                    return Ok(pace_ast::Pattern::Wildcard);
                }

                let mut generic_args = None;
                if self.current_token == Token::Less {
                    self.advance();
                    let mut args = Vec::new();
                    while self.current_token != Token::Greater && self.current_token != Token::Eof {
                        args.push(self.parse_type_annotation()?);
                        if self.match_token(Token::Comma) {
                            continue;
                        }
                        break;
                    }
                    if !self.match_token(Token::Greater) {
                        return Err(pace_errors::SyntaxError::Generic {
                            message: "Expected '>' after generic arguments in pattern".to_string(),
                            src: miette::NamedSource::new(
                                self.file_name.clone(),
                                self.src.to_string(),
                            ),
                            span: self.current_span,
                        });
                    }
                    generic_args = Some(args);
                }

                // Could be a variable, or an Enum variant like `Some(x)` or `Option::Some(x)`
                // Let's check for `::`
                let mut enum_name = None;
                let mut variant_name = name;

                if self.current_token == Token::ColonColon {
                    self.advance();
                    if let Token::Ident(v_name) = self.current_token.clone() {
                        self.advance();
                        enum_name = Some(name);
                        variant_name = v_name;
                    } else {
                        return Err(pace_errors::SyntaxError::Generic {
                            message: "Expected variant name after :: in pattern".to_string(),
                            src: miette::NamedSource::new(
                                self.file_name.clone(),
                                self.src.to_string(),
                            ),
                            span: self.current_span,
                        });
                    }
                }

                // If there's an open parenthesis, it's a variant with fields
                if self.current_token == Token::LParen {
                    self.advance();
                    let mut fields = Vec::new();
                    if self.current_token != Token::RParen {
                        loop {
                            fields.push(self.parse_pattern()?);
                            if self.match_token(Token::Comma) {
                                continue;
                            }
                            break;
                        }
                    }
                    if !self.match_token(Token::RParen) {
                        return Err(pace_errors::SyntaxError::Generic {
                            message: "Expected ')' after pattern fields".to_string(),
                            src: miette::NamedSource::new(
                                self.file_name.clone(),
                                self.src.to_string(),
                            ),
                            span: self.current_span,
                        });
                    }
                    Ok(pace_ast::Pattern::Variant {
                        enum_name: enum_name.map(ustr::Ustr::from),
                        variant_name: variant_name.into(),
                        fields: Some(fields),
                        generic_args,
                    })
                } else if enum_name.is_some() || variant_name.chars().next().unwrap().is_uppercase()
                {
                    // It's a variant without fields (like `None`)
                    Ok(pace_ast::Pattern::Variant {
                        enum_name: enum_name.map(ustr::Ustr::from),
                        variant_name: variant_name.into(),
                        fields: None,
                        generic_args,
                    })
                } else {
                    // Lowercase without parenthesis -> Variable binding
                    Ok(pace_ast::Pattern::Variable(
                        variant_name.into(),
                        self.current_span,
                    ))
                }
            }
            Token::Int(_) | Token::Float(_) | Token::String(_) | Token::Bool(_) => {
                let expr = self.parse_primary()?;
                Ok(pace_ast::Pattern::Literal(expr))
            }
            _ => Err(pace_errors::SyntaxError::Generic {
                message: "Expected pattern".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            }),
        }
    }

    pub(crate) fn parse_match_stmt(
        &mut self,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume match
        let expr = self.parse_expr()?;

        if !self.match_token(Token::LBrace) {
            return Err(pace_errors::SyntaxError::Generic {
                message: "Expected '{' before match arms".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut arms = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let pattern = self.parse_pattern()?;
            if !self.match_token(Token::FatArrow) && !self.match_token(Token::Arrow) {
                return Err(pace_errors::SyntaxError::Generic {
                    message: "Expected '=>' after match pattern".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
            let body = self.parse_stmt()?;
            arms.push((pattern, body));

            // Optional comma after arm
            if self.current_token == Token::Comma {
                self.advance();
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err(pace_errors::SyntaxError::Generic {
                message: "Expected '}' after match arms".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::Match { expr, arms }))
    }

    pub(crate) fn parse_import_stmt(
        &mut self,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume 'import'

        let path = match &self.current_token {
            Token::String(s) => {
                let p = s.clone();
                self.advance();
                p
            }
            _ => {
                return Err(pace_errors::SyntaxError::Generic {
                    message: "Import path must be a quoted string".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };

        let mut alias = None;
        let mut show = None;
        let mut hide = None;

        if self.match_token(Token::As) {
            alias = Some(match &self.current_token {
                Token::Ident(id) => *id,
                _ => {
                    return Err(pace_errors::SyntaxError::Generic {
                        message: "Expected alias identifier after 'as'".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
            });
            self.advance();
        }

        if self.match_token(Token::Show) {
            let mut items = Vec::new();
            loop {
                match &self.current_token {
                    Token::Ident(id) => {
                        items.push(*id);
                        self.advance();
                    }
                    _ => {
                        return Err(pace_errors::SyntaxError::Generic {
                            message: "Expected identifier to show".to_string(),
                            src: miette::NamedSource::new(
                                self.file_name.clone(),
                                self.src.to_string(),
                            ),
                            span: self.current_span,
                        });
                    }
                }
                if self.match_token(Token::Comma) {
                    continue;
                }
                break;
            }
            show = Some(items);
        } else if self.match_token(Token::Hide) {
            let mut items = Vec::new();
            loop {
                match &self.current_token {
                    Token::Ident(id) => {
                        items.push(*id);
                        self.advance();
                    }
                    _ => {
                        return Err(pace_errors::SyntaxError::Generic {
                            message: "Expected identifier to hide".to_string(),
                            src: miette::NamedSource::new(
                                self.file_name.clone(),
                                self.src.to_string(),
                            ),
                            span: self.current_span,
                        });
                    }
                }
                if self.match_token(Token::Comma) {
                    continue;
                }
                break;
            }
            hide = Some(items);
        }

        if !self.match_token(Token::Semi) {
            return Err(pace_errors::SyntaxError::Generic {
                message: "Expected ';' after import statement".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::Import {
            path: path.into(),
            alias: alias.map(ustr::Ustr::from),
            show: show.map(|v| v.iter().map(|s| ustr::Ustr::from(s)).collect()),
            hide: hide.map(|v| v.iter().map(|s| ustr::Ustr::from(s)).collect()),
        }))
    }

    pub(crate) fn parse_export_stmt(
        &mut self,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume 'export'

        let path = match &self.current_token {
            Token::String(s) => {
                let p = s.clone();
                self.advance();
                p
            }
            _ => {
                return Err(pace_errors::SyntaxError::Generic {
                    message: "Export path must be a quoted string".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };

        if !self.match_token(Token::Semi) {
            return Err(pace_errors::SyntaxError::Generic {
                message: "Expected ';' after export statement".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::Export { path: path.into() }))
    }
}
