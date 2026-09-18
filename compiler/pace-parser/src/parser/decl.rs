use pace_ast::{Decl, EnumVariant, GenericParam, Ident, Type};
use pace_errors::Diagnostic;
use pace_lexer::{Token, TokenKind};

use super::Parser;

impl<'a> Parser<'a> {
    pub(crate) fn parse_decl(&mut self) -> Result<Decl, Diagnostic> {
        let mut is_private = false;
        let mut start_span = self.current_span();
        if self.check(&TokenKind::Private) {
            let tok = self.expect(TokenKind::Private)?;
            start_span = tok.span;
            is_private = true;
        }

        if self.check(&TokenKind::Import) {
            let start_tok = self.expect(TokenKind::Import)?;
            let mut path = Vec::new();

            loop {
                match &self.current {
                    Some(Token {
                        kind: TokenKind::Ident(name),
                        span,
                    }) => {
                        path.push(Ident {
                            name: pace_span::intern(name),
                            span: *span,
                        });
                        self.advance();
                    }
                    _ => {
                        return Err(Diagnostic::error("Expected identifier in import path")
                            .with_span(self.current_span()));
                    }
                }

                if self.check(&TokenKind::Dot) {
                    self.advance();
                } else {
                    break;
                }
            }

            let alias = if self.check(&TokenKind::As) {
                self.advance();
                match &self.current {
                    Some(Token {
                        kind: TokenKind::Ident(name),
                        span,
                    }) => {
                        let ident = Ident {
                            name: pace_span::intern(name),
                            span: *span,
                        };
                        self.advance();
                        Some(ident)
                    }
                    _ => {
                        return Err(Diagnostic::error("Expected identifier after 'as'")
                            .with_span(self.current_span()));
                    }
                }
            } else {
                None
            };

            let end_span = alias
                .as_ref()
                .map(|a| a.span)
                .unwrap_or_else(|| path.last().unwrap().span);

            Ok(Decl::Import {
                path,
                alias,
                span: start_tok.span.merge(end_span),
            })
        } else if self.check(&TokenKind::Let) {
            let start_tok = self.expect(TokenKind::Let)?;

            let name_tok = match &self.current {
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
                    return Err(Diagnostic::error("Expected identifier after 'let'")
                        .with_span(self.current_span()));
                }
            };

            let ty = if self.check(&TokenKind::Colon) {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };

            let mut value = None;
            let span_end = if self.check(&TokenKind::Eq) {
                self.advance();
                let expr = self.parse_expr()?;
                let end = expr.span();
                value = Some(expr);
                end
            } else {
                if ty.is_none() {
                    return Err(Diagnostic::error(
                        "Type annotation is required when an initializer is omitted",
                    )
                    .with_span(self.current_span()));
                }
                start_tok.span
            };

            Ok(Decl::Let {
                name: name_tok,
                ty,
                value,
                is_private,
                span: if is_private {
                    start_span.merge(span_end)
                } else {
                    start_tok.span.merge(span_end)
                },
            })
        } else if self.check(&TokenKind::Var) {
            let start_tok = self.expect(TokenKind::Var)?;

            let name_tok = match &self.current {
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
                    return Err(Diagnostic::error("Expected identifier after 'var'")
                        .with_span(self.current_span()));
                }
            };

            let ty = if self.check(&TokenKind::Colon) {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };

            let mut value = None;
            let span_end = if self.check(&TokenKind::Eq) {
                self.advance();
                let expr = self.parse_expr()?;
                let end = expr.span();
                value = Some(expr);
                end
            } else {
                if ty.is_none() {
                    return Err(Diagnostic::error(
                        "Type annotation is required when an initializer is omitted",
                    )
                    .with_span(self.current_span()));
                }
                start_tok.span
            };

            Ok(Decl::Var {
                name: name_tok,
                ty,
                value,
                is_private,
                span: if is_private {
                    start_span.merge(span_end)
                } else {
                    start_tok.span.merge(span_end)
                },
            })
        } else if self.check(&TokenKind::Const) {
            let start_tok = self.expect(TokenKind::Const)?;

            let name_tok = match &self.current {
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
                    return Err(Diagnostic::error("Expected identifier after 'const'")
                        .with_span(self.current_span()));
                }
            };

            let ty = if self.check(&TokenKind::Colon) {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };

            self.expect(TokenKind::Eq)?;
            let value = self.parse_expr()?;
            let end_span = value.span();

            Ok(Decl::Const {
                name: name_tok,
                ty,
                value,
                is_private,
                span: if is_private {
                    start_span.merge(end_span)
                } else {
                    start_tok.span.merge(end_span)
                },
            })
        } else if self.check(&TokenKind::Fn) {
            self.parse_fn_decl(is_private, start_span)
        } else if self.check(&TokenKind::Struct) {
            self.parse_struct_decl(is_private, start_span)
        } else if self.check(&TokenKind::Class) {
            self.parse_class_decl(is_private, start_span)
        } else if self.check(&TokenKind::Trait) {
            self.parse_trait_decl(is_private, start_span)
        } else if self.check(&TokenKind::Enum) {
            self.parse_enum_decl(is_private, start_span)
        } else {
            let expr = self.parse_expr()?;
            let span = expr.span();
            Ok(Decl::Expr(expr, span))
        }
    }
    pub(crate) fn parse_generic_params(&mut self) -> Result<Option<Vec<GenericParam>>, Diagnostic> {
        if self.check(&TokenKind::Lt) {
            self.advance();
            let mut params = Vec::new();
            while !self.check(&TokenKind::Gt) && self.current.is_some() {
                if let Some(Token {
                    kind: TokenKind::Ident(name),
                    span,
                }) = &self.current
                {
                    let param_name = Ident {
                        name: pace_span::intern(name),
                        span: *span,
                    };
                    self.advance();

                    let mut default = None;
                    if self.check(&TokenKind::Eq) {
                        self.advance();
                        default = Some(self.parse_type()?);
                    }

                    params.push(GenericParam {
                        name: param_name,
                        default,
                    });

                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    }
                } else {
                    return Err(Diagnostic::error("Expected type parameter identifier")
                        .with_span(self.current_span()));
                }
            }
            self.expect(TokenKind::Gt)?;
            Ok(Some(params))
        } else {
            Ok(None)
        }
    }

    #[allow(dead_code)]
    pub(crate) fn parse_generic_args(&mut self) -> Result<Option<Vec<Type>>, Diagnostic> {
        if self.check(&TokenKind::Lt) {
            self.advance();
            let mut args = Vec::new();
            while !self.check(&TokenKind::Gt) && self.current.is_some() {
                args.push(self.parse_type()?);
                if self.check(&TokenKind::Comma) {
                    self.advance();
                }
            }
            self.expect(TokenKind::Gt)?;
            Ok(Some(args))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn parse_with_clause(&mut self) -> Result<Vec<Ident>, Diagnostic> {
        let mut traits = Vec::new();
        if self.check(&TokenKind::With) {
            self.advance();
            loop {
                if let Some(Token {
                    kind: TokenKind::Ident(name),
                    span,
                }) = &self.current
                {
                    traits.push(Ident {
                        name: pace_span::intern(name),
                        span: *span,
                    });
                    self.advance();
                } else {
                    return Err(Diagnostic::error("Expected trait name after 'with'")
                        .with_span(self.current_span()));
                }
                if self.check(&TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        Ok(traits)
    }

    pub(crate) fn parse_struct_decl(
        &mut self,
        is_private: bool,
        start_span: pace_span::Span,
    ) -> Result<Decl, Diagnostic> {
        let start_tok = self.expect(TokenKind::Struct)?;

        let name_tok = match &self.current {
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
                return Err(Diagnostic::error("Expected identifier after 'struct'")
                    .with_span(self.current_span()));
            }
        };

        let generic_params = self.parse_generic_params()?;

        let with = self.parse_with_clause()?;

        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        let mut static_fields = Vec::new();
        let mut const_fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            let mut is_field_private = false;
            if self.check(&TokenKind::Private) {
                self.advance();
                is_field_private = true;
            }

            if self.check(&TokenKind::Const) {
                self.advance();
                if let Some(Token {
                    kind: TokenKind::Ident(name),
                    span,
                }) = &self.current
                {
                    let field_name = Ident {
                        name: pace_span::intern(name),
                        span: *span,
                    };
                    self.advance();

                    self.expect(TokenKind::Colon)?;
                    let field_type = self.parse_type()?;

                    self.expect(TokenKind::Eq)?;
                    let field_value = self.parse_expr()?;

                    const_fields.push(pace_ast::ConstFieldDef {
                        name: field_name,
                        ty: field_type,
                        value: field_value,
                        is_private: is_field_private,
                    });

                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    }
                } else {
                    return Err(Diagnostic::error("Expected identifier after 'const'")
                        .with_span(self.current_span()));
                }
            } else if self.check(&TokenKind::Static) {
                self.advance();

                if self.check(&TokenKind::Fn) {
                    let mut func = self.parse_fn_decl(is_field_private, self.current_span())?;
                    if let Decl::Function {
                        ref mut is_static, ..
                    } = func
                    {
                        *is_static = true;
                    }
                    methods.push(func);
                } else if let Some(Token {
                    kind: TokenKind::Ident(name),
                    span,
                }) = &self.current
                {
                    let field_name = Ident {
                        name: pace_span::intern(name),
                        span: *span,
                    };
                    self.advance();

                    self.expect(TokenKind::Colon)?;
                    let field_type = self.parse_type()?;

                    self.expect(TokenKind::Eq)?;
                    let field_value = self.parse_expr()?;

                    static_fields.push(pace_ast::StaticFieldDef {
                        name: field_name,
                        ty: field_type,
                        value: field_value,
                        is_private: is_field_private,
                    });

                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    }
                } else {
                    return Err(Diagnostic::error(
                        "Expected function or identifier after 'static'",
                    )
                    .with_span(self.current_span()));
                }
            } else if self.check(&TokenKind::Fn) {
                methods.push(self.parse_fn_decl(is_field_private, self.current_span())?);
            } else if self.check(&TokenKind::Let)
                || self.check(&TokenKind::Var)
                || matches!(
                    &self.current,
                    Some(Token {
                        kind: TokenKind::Ident(_),
                        ..
                    })
                )
            {
                let mut is_mut = true;
                if self.check(&TokenKind::Let) {
                    is_mut = false;
                    self.advance();
                } else if self.check(&TokenKind::Var) {
                    self.advance();
                }

                if let Some(Token {
                    kind: TokenKind::Ident(name),
                    span,
                }) = self.current.clone()
                {
                    if name == "init" && is_mut {
                        let init_span_start = span;
                        self.advance(); // consume 'init'
                        let ident = Ident {
                            name: pace_span::intern("init"),
                            span: init_span_start,
                        };

                        self.expect(TokenKind::LParen)?;
                        let mut params = Vec::new();
                        while !self.check(&TokenKind::RParen) && self.current.is_some() {
                            let param_name = match &self.current {
                                Some(Token {
                                    kind: TokenKind::Ident(n),
                                    span,
                                }) => Ident {
                                    name: pace_span::intern(n),
                                    span: *span,
                                },
                                _ => {
                                    return Err(Diagnostic::error("Expected parameter name")
                                        .with_span(self.current_span()));
                                }
                            };
                            self.advance();
                            self.expect(TokenKind::Colon)?;
                            let param_ty = self.parse_type()?;
                            params.push((param_name, param_ty));
                            if self.check(&TokenKind::Comma) {
                                self.advance();
                            }
                        }
                        self.expect(TokenKind::RParen)?;

                        let body = self.parse_block()?;
                        methods.push(Decl::Function {
                            name: ident,
                            generic_params: None,
                            params,
                            return_type: None,
                            body: body.clone(),
                            is_static: false,
                            is_override: false,
                            is_private: is_field_private,
                            span: init_span_start.merge(body.span),
                        });
                        continue;
                    }

                    let field_name = Ident {
                        name: pace_span::intern(name),
                        span,
                    };
                    self.advance();

                    self.expect(TokenKind::Colon)?;
                    let field_type = self.parse_type()?;

                    let mut field_value = None;
                    if self.check(&TokenKind::Eq) {
                        self.advance();
                        field_value = Some(self.parse_expr()?);
                    }

                    fields.push(pace_ast::FieldDef {
                        name: field_name,
                        ty: field_type,
                        default_value: field_value,
                        is_mut,
                        is_private: is_field_private,
                    });
                } else {
                    return Err(Diagnostic::error("Expected field name after let/var")
                        .with_span(self.current_span()));
                }

                if self.check(&TokenKind::Comma) {
                    self.advance();
                }
            } else {
                return Err(Diagnostic::error("Expected field or method in struct")
                    .with_span(self.current_span()));
            }
        }

        let end_tok = self.expect(TokenKind::RBrace)?;

        Ok(Decl::Struct {
            name: name_tok,
            generic_params,
            with,
            fields,
            static_fields,
            const_fields,
            methods,
            is_private,
            span: if is_private {
                start_span.merge(end_tok.span)
            } else {
                start_tok.span.merge(end_tok.span)
            },
        })
    }

    pub(crate) fn parse_trait_decl(
        &mut self,
        is_private: bool,
        start_span: pace_span::Span,
    ) -> Result<Decl, Diagnostic> {
        let start_tok = self.expect(TokenKind::Trait)?;

        let name_tok = match &self.current {
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
                return Err(Diagnostic::error("Expected identifier after 'trait'")
                    .with_span(self.current_span()));
            }
        };

        let generic_params = self.parse_generic_params()?;

        self.expect(TokenKind::LBrace)?;
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            let mut is_field_private = false;
            if self.check(&TokenKind::Private) {
                self.advance();
                is_field_private = true;
            }

            if self.check(&TokenKind::Fn) {
                methods.push(self.parse_fn_decl(is_field_private, self.current_span())?);
            } else {
                return Err(
                    Diagnostic::error("Expected method in trait").with_span(self.current_span())
                );
            }
        }
        let end_tok = self.expect(TokenKind::RBrace)?;

        Ok(Decl::Trait {
            name: name_tok,
            generic_params,
            methods,
            is_private,
            span: if is_private {
                start_span.merge(end_tok.span)
            } else {
                start_tok.span.merge(end_tok.span)
            },
        })
    }

    pub(crate) fn parse_class_decl(
        &mut self,
        is_private: bool,
        start_span: pace_span::Span,
    ) -> Result<Decl, Diagnostic> {
        let start_tok = self.expect(TokenKind::Class)?;

        let name_tok = match &self.current {
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
                return Err(Diagnostic::error("Expected identifier after 'class'")
                    .with_span(self.current_span()));
            }
        };

        let generic_params = self.parse_generic_params()?;

        let mut extends = None;
        if self.check(&TokenKind::Extends) {
            self.advance();
            if let Some(Token {
                kind: TokenKind::Ident(name),
                span,
            }) = &self.current
            {
                extends = Some(Ident {
                    name: pace_span::intern(name),
                    span: *span,
                });
                self.advance();
            } else {
                return Err(Diagnostic::error("Expected class name after 'extends'")
                    .with_span(self.current_span()));
            }
        }

        let with = self.parse_with_clause()?;

        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        let mut static_fields = Vec::new();
        let mut const_fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            let mut is_field_private = false;
            if self.check(&TokenKind::Private) {
                self.advance();
                is_field_private = true;
            }

            if self.check(&TokenKind::Const) {
                self.advance();
                if let Some(Token {
                    kind: TokenKind::Ident(name),
                    span,
                }) = &self.current
                {
                    let field_name = Ident {
                        name: pace_span::intern(name),
                        span: *span,
                    };
                    self.advance();

                    self.expect(TokenKind::Colon)?;
                    let field_type = self.parse_type()?;

                    self.expect(TokenKind::Eq)?;
                    let field_value = self.parse_expr()?;

                    const_fields.push(pace_ast::ConstFieldDef {
                        name: field_name,
                        ty: field_type,
                        value: field_value,
                        is_private: is_field_private,
                    });

                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    }
                } else {
                    return Err(Diagnostic::error("Expected identifier after 'const'")
                        .with_span(self.current_span()));
                }
            } else if self.check(&TokenKind::Static) {
                self.advance();

                if self.check(&TokenKind::Fn) {
                    let mut func = self.parse_fn_decl(is_field_private, self.current_span())?;
                    if let Decl::Function {
                        ref mut is_static, ..
                    } = func
                    {
                        *is_static = true;
                    }
                    methods.push(func);
                } else if let Some(Token {
                    kind: TokenKind::Ident(name),
                    span,
                }) = &self.current
                {
                    let field_name = Ident {
                        name: pace_span::intern(name),
                        span: *span,
                    };
                    self.advance();

                    self.expect(TokenKind::Colon)?;
                    let field_type = self.parse_type()?;

                    self.expect(TokenKind::Eq)?;
                    let field_value = self.parse_expr()?;

                    static_fields.push(pace_ast::StaticFieldDef {
                        name: field_name,
                        ty: field_type,
                        value: field_value,
                        is_private: is_field_private,
                    });

                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    }
                } else {
                    return Err(Diagnostic::error(
                        "Expected function or identifier after 'static'",
                    )
                    .with_span(self.current_span()));
                }
            } else if self.check(&TokenKind::Fn) || self.check(&TokenKind::Override) {
                let is_override = self.check(&TokenKind::Override);
                if is_override {
                    self.advance();
                }
                if self.check(&TokenKind::Fn) {
                    let mut func = self.parse_fn_decl(is_field_private, self.current_span())?;
                    if let Decl::Function {
                        is_override: ref mut override_flag,
                        ..
                    } = func
                    {
                        *override_flag = is_override;
                    }
                    methods.push(func);
                } else {
                    return Err(Diagnostic::error("Expected 'fn' after 'override'")
                        .with_span(self.current_span()));
                }
            } else if self.check(&TokenKind::Let)
                || self.check(&TokenKind::Var)
                || matches!(
                    &self.current,
                    Some(Token {
                        kind: TokenKind::Ident(_),
                        ..
                    })
                )
            {
                let mut is_mut = true;
                if self.check(&TokenKind::Let) {
                    is_mut = false;
                    self.advance();
                } else if self.check(&TokenKind::Var) {
                    self.advance();
                }

                if let Some(Token {
                    kind: TokenKind::Ident(name),
                    span,
                }) = self.current.clone()
                {
                    if name == "init" && is_mut {
                        let init_span_start = span;
                        self.advance(); // consume 'init'
                        let ident = Ident {
                            name: pace_span::intern("init"),
                            span: init_span_start,
                        };

                        self.expect(TokenKind::LParen)?;
                        let mut params = Vec::new();
                        while !self.check(&TokenKind::RParen) && self.current.is_some() {
                            let param_name = match &self.current {
                                Some(Token {
                                    kind: TokenKind::Ident(n),
                                    span,
                                }) => Ident {
                                    name: pace_span::intern(n),
                                    span: *span,
                                },
                                _ => {
                                    return Err(Diagnostic::error("Expected parameter name")
                                        .with_span(self.current_span()));
                                }
                            };
                            self.advance();
                            self.expect(TokenKind::Colon)?;
                            let param_ty = self.parse_type()?;
                            params.push((param_name, param_ty));
                            if self.check(&TokenKind::Comma) {
                                self.advance();
                            }
                        }
                        self.expect(TokenKind::RParen)?;

                        let body = self.parse_block()?;
                        methods.push(Decl::Function {
                            name: ident,
                            generic_params: None,
                            params,
                            return_type: None,
                            body: body.clone(),
                            is_static: false,
                            is_override: false,
                            is_private: is_field_private,
                            span: init_span_start.merge(body.span),
                        });
                        continue;
                    }

                    let field_name = Ident {
                        name: pace_span::intern(name),
                        span,
                    };
                    self.advance();

                    self.expect(TokenKind::Colon)?;
                    let field_type = self.parse_type()?;

                    let mut field_value = None;
                    if self.check(&TokenKind::Eq) {
                        self.advance();
                        field_value = Some(self.parse_expr()?);
                    }

                    fields.push(pace_ast::FieldDef {
                        name: field_name,
                        ty: field_type,
                        default_value: field_value,
                        is_mut,
                        is_private: is_field_private,
                    });
                } else {
                    return Err(Diagnostic::error("Expected field name after let/var")
                        .with_span(self.current_span()));
                }

                if self.check(&TokenKind::Comma) {
                    self.advance();
                }
            } else {
                return Err(Diagnostic::error("Expected field or method in class")
                    .with_span(self.current_span()));
            }
        }

        let end_tok = self.expect(TokenKind::RBrace)?;

        Ok(Decl::Class {
            name: name_tok,
            generic_params,
            extends,
            with,
            fields,
            static_fields,
            const_fields,
            methods,
            is_private,
            span: if is_private {
                start_span.merge(end_tok.span)
            } else {
                start_tok.span.merge(end_tok.span)
            },
        })
    }

    pub(crate) fn parse_enum_decl(
        &mut self,
        is_private: bool,
        start_span: pace_span::Span,
    ) -> Result<Decl, Diagnostic> {
        let start_tok = self.expect(TokenKind::Enum)?;

        let name_tok = match &self.current {
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
                return Err(Diagnostic::error("Expected identifier after 'enum'")
                    .with_span(self.current_span()));
            }
        };

        let generic_params = self.parse_generic_params()?;

        self.expect(TokenKind::LBrace)?;
        let mut variants = Vec::new();

        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            let variant_name = match &self.current {
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
                    return Err(
                        Diagnostic::error("Expected variant name").with_span(self.current_span())
                    );
                }
            };

            let mut fields = None;
            let mut v_end_span = variant_name.span;

            if self.check(&TokenKind::LParen) {
                self.advance();
                let mut vfields = Vec::new();
                while !self.check(&TokenKind::RParen) && self.current.is_some() {
                    let field_name = match &self.current {
                        Some(Token {
                            kind: TokenKind::Ident(n),
                            span,
                        }) => Ident {
                            name: pace_span::intern(n),
                            span: *span,
                        },
                        _ => {
                            return Err(Diagnostic::error("Expected field name")
                                .with_span(self.current_span()));
                        }
                    };
                    self.advance();
                    self.expect(TokenKind::Colon)?;
                    let field_ty = self.parse_type()?;
                    vfields.push((field_name, field_ty));
                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    }
                }
                let rparen = self.expect(TokenKind::RParen)?;
                v_end_span = rparen.span;
                fields = Some(vfields);
            }

            if self.check(&TokenKind::Comma) {
                self.advance();
            }

            variants.push(EnumVariant {
                name: variant_name.clone(),
                fields,
                span: variant_name.span.merge(v_end_span),
            });
        }

        let end_tok = self.expect(TokenKind::RBrace)?;

        Ok(Decl::Enum {
            name: name_tok,
            generic_params,
            variants,
            is_private,
            span: if is_private {
                start_span.merge(end_tok.span)
            } else {
                start_tok.span.merge(end_tok.span)
            },
        })
    }

    pub(crate) fn parse_fn_decl(
        &mut self,
        is_private: bool,
        start_span: pace_span::Span,
    ) -> Result<Decl, Diagnostic> {
        let start_tok = self.expect(TokenKind::Fn)?;

        let name_tok = match &self.current {
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
                return Err(Diagnostic::error("Expected identifier after 'fn'")
                    .with_span(self.current_span()));
            }
        };

        let generic_params = self.parse_generic_params()?;

        self.expect(TokenKind::LParen)?;

        let mut params = Vec::new();
        while !self.check(&TokenKind::RParen) {
            let param_name = match &self.current {
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
                    return Err(
                        Diagnostic::error("Expected parameter name").with_span(self.current_span())
                    );
                }
            };

            self.expect(TokenKind::Colon)?;
            let param_type = self.parse_type()?;

            params.push((param_name, param_type));

            if self.check(&TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;

        let mut return_type = None;
        if self.check(&TokenKind::Arrow) {
            self.advance();
            return_type = Some(self.parse_type()?);
        }

        let body = self.parse_block()?;
        let end_span = body.span;

        Ok(Decl::Function {
            name: name_tok,
            generic_params,
            params,
            return_type,
            body,
            is_static: false,
            is_override: false,
            is_private,
            span: if is_private {
                start_span.merge(end_span)
            } else {
                start_tok.span.merge(end_span)
            },
        })
    }
}
