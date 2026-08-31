use crate::lexer::Token;
use pace_ast::{Param, Stmt, Visibility};
use super::Parser;

impl<'a, 'b> Parser<'a, 'b> {
    pub(crate) fn parse_func_decl(
        &mut self,
        is_async: bool,
        visibility: Visibility,
        is_static: bool,
        doc_comment: Option<ustr::Ustr>,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        let start_pos = self.current_span.0;
        self.advance(); // consume func

        let name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected function name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        if !self.match_token(Token::LParen) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '(' after function name".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut params = Vec::new();
        if self.current_token != Token::RParen {
            loop {
                let param_name = match &self.current_token {
                    Token::Ident(id) => *id,
                    _ => {
                        return Err(pace_errors::SyntaxError {
                            message: "Expected parameter name".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                    }
                };
                self.advance();

                if !self.match_token(Token::Colon) {
                    return Err(pace_errors::SyntaxError {
                        message: "Expected ':' after parameter name".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }

                let param_type = self.parse_type_annotation()?;

                params.push(Param {
                    name: param_name.into(),
                    type_annotation: param_type,
                });

                if !self.match_token(Token::Comma) {
                    break;
                }
            }
        }

        if !self.match_token(Token::RParen) {
            return Err(pace_errors::SyntaxError {
                message: "Expected ')' after parameters".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut return_type = None;
        if self.match_token(Token::Arrow) {
            return_type = Some(self.parse_type_annotation()?);
        }

        if !self.match_token(Token::LBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '{' before function body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut body = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            match self.parse_stmt() {
                Ok(stmt) => body.push(stmt),
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '}' after function body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::FuncDecl {
            name: name.into(),
            generic_params,
            params,
            return_type,
            body,
            is_async,
            is_static,
            visibility,
            doc_comment,
            span: pace_ast::Span::new(
                start_pos,
                (self.current_span.0 + self.current_span.1).saturating_sub(start_pos),
            ),
        }))
    }

    pub(crate) fn parse_class_decl(
        &mut self,
        doc_comment: Option<ustr::Ustr>,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        // Assume current_token is Class or Actor
        self.advance();
        let name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected class name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        let mut implements = None;
        if self.match_token(Token::Implement) {
            implements = Some(self.parse_type_annotation()?);
        }

        if !self.match_token(Token::LBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '{' before class body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            match self.parse_stmt() {
                Ok(stmt) => match self.arena.get_stmt(stmt) {
                    Stmt::VarDecl { .. } => fields.push(stmt),
                    Stmt::FuncDecl { .. } => methods.push(stmt),
                    Stmt::Expr(expr_id) => {
                        if let pace_ast::Expr::Call { callee, .. } = self.arena.get_expr(*expr_id) {
                            if let pace_ast::Expr::Identifier(name, _) = self.arena.get_expr(*callee) {
                                self.errors.push(pace_errors::SyntaxError { message: format!("Methods must be prefixed with 'func'. Did you forget 'func' before '{}'?", name), src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()), span: self.current_span });
                            } else {
                                self.errors.push(pace_errors::SyntaxError {
                                    message: "Classes can only contain fields and methods".to_string(),
                                    src: miette::NamedSource::new(
                                        self.file_name.clone(),
                                        self.src.to_string(),
                                ),
                                span: self.current_span,
                            });
                        }
                        }
                        self.synchronize();
                    }
                    _ => {
                        self.errors.push(pace_errors::SyntaxError {
                            message: "Classes can only contain fields and methods".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                        self.synchronize();
                    }
                },
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '}' after class body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::ClassDecl {
            name: name.into(),
            generic_params,
            fields,
            methods,
            implements,
            doc_comment,
        }))
    }

    pub(crate) fn parse_actor_decl(
        &mut self,
        doc_comment: Option<ustr::Ustr>,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance();
        let name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected actor name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        let mut implements = None;
        if self.match_token(Token::Implement) {
            implements = Some(self.parse_type_annotation()?);
        }

        if !self.match_token(Token::LBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '{' before actor body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            match self.parse_stmt() {
                Ok(stmt) => match self.arena.get_stmt(stmt) {
                    Stmt::VarDecl { .. } => fields.push(stmt),
                    Stmt::FuncDecl { .. } => methods.push(stmt),
                    Stmt::Expr(expr_id) => {
                        if let pace_ast::Expr::Call { callee, .. } = self.arena.get_expr(*expr_id) {
                            if let pace_ast::Expr::Identifier(name, _) = self.arena.get_expr(*callee) {
                                self.errors.push(pace_errors::SyntaxError { message: format!("Methods must be prefixed with 'func'. Did you forget 'func' before '{}'?", name), src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()), span: self.current_span });
                            } else {
                                self.errors.push(pace_errors::SyntaxError {
                                    message: "Actors can only contain fields and methods".to_string(),
                                    src: miette::NamedSource::new(
                                        self.file_name.clone(),
                                        self.src.to_string(),
                                ),
                                span: self.current_span,
                            });
                        }
                        }
                        self.synchronize();
                    }
                    _ => {
                        self.errors.push(pace_errors::SyntaxError {
                            message: "Actors can only contain fields and methods".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                        self.synchronize();
                    }
                },
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '}' after actor body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::ActorDecl {
            name: name.into(),
            generic_params,
            fields,
            methods,
            implements,
            doc_comment,
        }))
    }

    pub(crate) fn parse_interface_decl(
        &mut self,
        doc_comment: Option<ustr::Ustr>,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume interface

        let name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected interface name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        if !self.match_token(Token::LBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '{' before interface body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut methods = Vec::new();

        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let is_async = self.match_token(Token::Async);

            if !self.match_token(Token::Func) {
                return Err(pace_errors::SyntaxError {
                    message: "Expected 'func' in interface definition".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }

            let method_name = match &self.current_token {
                Token::Ident(id) => *id,
                _ => {
                    return Err(pace_errors::SyntaxError {
                        message: "Expected function name".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
            };
            self.advance();

            if !self.match_token(Token::LParen) {
                return Err(pace_errors::SyntaxError {
                    message: "Expected '(' after function name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }

            let mut params = Vec::new();
            if self.current_token != Token::RParen {
                loop {
                    let param_name = match &self.current_token {
                        Token::Ident(id) => *id,
                        _ => {
                            return Err(pace_errors::SyntaxError {
                                message: "Expected parameter name".to_string(),
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
                        return Err(pace_errors::SyntaxError {
                            message: "Expected ':' after parameter name".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                    }

                    let param_type = self.parse_type_annotation()?;

                    params.push(Param {
                        name: param_name.into(),
                        type_annotation: param_type,
                    });

                    if !self.match_token(Token::Comma) {
                        break;
                    }
                }
            }

            if !self.match_token(Token::RParen) {
                return Err(pace_errors::SyntaxError {
                    message: "Expected ')' after parameters".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }

            let mut return_type = None;
            if self.match_token(Token::Arrow) {
                return_type = Some(self.parse_type_annotation()?);
            }

            let mut body = vec![];
            if self.current_token == Token::LBrace {
                self.advance(); // consume '{'
                while self.current_token != Token::RBrace && self.current_token != Token::Eof {
                    match self.parse_stmt() {
                        Ok(stmt) => body.push(stmt),
                        Err(e) => {
                            self.errors.push(e);
                            self.synchronize();
                        }
                    }
                }
                self.match_token(Token::RBrace);
            } else {
                self.match_token(Token::Semi);
            }

            methods.push(self.alloc_stmt(Stmt::FuncDecl {
                name: method_name.into(),
                generic_params: None,
                params,
                return_type,
                body,
                is_async,
                is_static: false, // Interfaces methods are inherently non-static for now
                visibility: Visibility::Public,
                doc_comment: None,
                span: pace_ast::Span::default(),
            }));
        }

        if !self.match_token(Token::RBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '}' after interface body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::InterfaceDecl {
            name: name.into(),
            generic_params,
            methods,
            doc_comment,
        }))
    }

    pub(crate) fn parse_enum_decl(
        &mut self,
        doc_comment: Option<ustr::Ustr>,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume enum

        let name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected enum name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let generic_params = if self.match_token(Token::Less) {
            let mut params = Vec::new();
            while self.current_token != Token::Greater && self.current_token != Token::Eof {
                if let Token::Ident(id) = &self.current_token {
                    params.push((*id).into());
                    self.advance();
                } else {
                    return Err(pace_errors::SyntaxError {
                        message: "Expected generic parameter name".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }

                if self.match_token(Token::Comma) {
                    continue;
                }
            }
            if !self.match_token(Token::Greater) {
                return Err(pace_errors::SyntaxError {
                    message: "Expected '>' after generic parameters".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
            Some(params)
        } else {
            None
        };

        if !self.match_token(Token::LBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '{' after enum name".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut variants = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let variant_name = match &self.current_token {
                Token::Ident(id) => *id,
                _ => {
                    return Err(pace_errors::SyntaxError {
                        message: "Expected enum variant name".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
            };
            self.advance();

            let fields = if self.match_token(Token::LParen) {
                let mut variant_fields = Vec::new();
                while self.current_token != Token::RParen && self.current_token != Token::Eof {
                    variant_fields.push(self.parse_type_annotation()?);
                    if self.match_token(Token::Comma) {
                        continue;
                    }
                }
                if !self.match_token(Token::RParen) {
                    return Err(pace_errors::SyntaxError {
                        message: "Expected ')' after enum variant fields".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
                Some(variant_fields)
            } else {
                None
            };

            use pace_ast::EnumVariant;
            variants.push(EnumVariant {
                name: variant_name.into(),
                fields,
            });

            // Variants can optionally end with a comma
            self.match_token(Token::Comma);
        }

        if !self.match_token(Token::RBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '}' after enum body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::EnumDecl {
            name: name.into(),
            generic_params,
            variants,
            doc_comment,
        }))
    }

    pub(crate) fn parse_struct_decl(
        &mut self,
        doc_comment: Option<ustr::Ustr>,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume struct

        let name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected struct name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        if !self.match_token(Token::LBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '{' before struct body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut fields = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let stmt = self.parse_stmt()?;
            match self.arena.get_stmt(stmt) {
                Stmt::VarDecl { .. } => fields.push(stmt),
                _ => {
                    return Err(pace_errors::SyntaxError {
                        message: "Structs can only contain fields".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '}' after struct body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::StructDecl {
            name: name.into(),
            generic_params,
            fields,
            doc_comment,
        }))
    }

    pub(crate) fn parse_var_decl(
        &mut self,
        is_mutable: bool,
        is_static: bool,
        visibility: Visibility,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        let start_pos = self.current_span.0;
        self.advance(); // consume let/var
        let name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected identifier".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let mut type_annotation = None;
        if self.match_token(Token::Colon) {
            type_annotation = Some(self.parse_type_annotation()?);
        }

        let mut initializer = None;
        if self.match_token(Token::Eq) {
            initializer = Some(self.parse_expr()?);
        }

        if !self.match_token(Token::Semi) {
            return Err(pace_errors::SyntaxError {
                message: "Expected ';' after variable declaration".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::VarDecl {
            name: name.into(),
            is_mutable,
            type_annotation,
            is_static,
            visibility,
            initializer,
            span: pace_ast::Span::new(
                start_pos,
                (self.current_span.0 + self.current_span.1).saturating_sub(start_pos),
            ),
        }))
    }

}
