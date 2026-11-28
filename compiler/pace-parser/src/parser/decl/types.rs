use pace_ast::{Decl, EnumVariant, Ident};
use pace_errors::Diagnostic;
use pace_lexer::{Token, TokenKind};

use super::super::Parser;

impl<'a> Parser<'a> {
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
            let member_annotations = self.parse_annotations()?;
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

                let mut is_async = false;
                if self.check(&TokenKind::Async) {
                    self.advance();
                    is_async = true;
                }

                if self.check(&TokenKind::Fn) {
                    let mut func = self.parse_fn_decl(is_field_private, is_async, self.current_span())?;
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
            } else if self.check(&TokenKind::Async) {
                self.advance();
                let mut func = self.parse_fn_decl(is_field_private, true, self.current_span())?;
                Self::attach_annotations(&mut func, member_annotations);
                methods.push(func);
            } else if self.check(&TokenKind::Fn) {
                let mut func = self.parse_fn_decl(is_field_private, false, self.current_span())?;
                Self::attach_annotations(&mut func, member_annotations);
                methods.push(func);
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
                            is_private: is_field_private,
                            is_async: false,
                            annotations: member_annotations,
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
                        annotations: member_annotations,
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
            annotations: Vec::new(),
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
            let member_annotations = self.parse_annotations()?;
            let mut is_field_private = false;
            if self.check(&TokenKind::Private) {
                self.advance();
                is_field_private = true;
            }

            if self.check(&TokenKind::Async) {
                self.advance();
                let mut func = self.parse_fn_decl(is_field_private, true, self.current_span())?;
                Self::attach_annotations(&mut func, member_annotations);
                methods.push(func);
            } else if self.check(&TokenKind::Fn) {
                let mut func = self.parse_fn_decl(is_field_private, false, self.current_span())?;
                Self::attach_annotations(&mut func, member_annotations);
                methods.push(func);
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
            annotations: Vec::new(),
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
            let member_annotations = self.parse_annotations()?;
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

                let mut is_async = false;
                if self.check(&TokenKind::Async) {
                    self.advance();
                    is_async = true;
                }

                if self.check(&TokenKind::Fn) {
                    let mut func = self.parse_fn_decl(is_field_private, is_async, self.current_span())?;
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
            } else if matches!(&self.current, Some(Token { kind: TokenKind::Ident("override"), .. })) {
                let span = self.current_span();
                return Err(Diagnostic::error(
                    "The 'override' keyword was replaced by '@override' in v0.5.0",
                )
                .with_span(span)
                .with_hint("Use '@override fn ...' instead of 'override fn ...'"));
            } else if self.check(&TokenKind::Async) || self.check(&TokenKind::Fn) {
                let mut is_async = false;
                if self.check(&TokenKind::Async) {
                    self.advance();
                    is_async = true;
                }
                
                if self.check(&TokenKind::Fn) {
                    let mut func = self.parse_fn_decl(is_field_private, is_async, self.current_span())?;
                    Self::attach_annotations(&mut func, member_annotations);
                    methods.push(func);
                } else {
                    return Err(Diagnostic::error("Expected 'fn' after 'async'")
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
                            is_private: is_field_private,
                            is_async: false,
                            annotations: member_annotations,
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
                        annotations: member_annotations,
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
            annotations: Vec::new(),
            span: if is_private {
                start_span.merge(end_tok.span)
            } else {
                start_tok.span.merge(end_tok.span)
            },
        })
    }


    pub(crate) fn parse_actor_decl(
        &mut self,
        is_private: bool,
        start_span: pace_span::Span,
    ) -> Result<Decl, Diagnostic> {
        let start_tok = self.expect(TokenKind::Actor)?;

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
                return Err(Diagnostic::error("Expected identifier after 'actor'")
                    .with_span(self.current_span()));
            }
        };

        let generic_params = self.parse_generic_params()?;
        let with = self.parse_with_clause()?;

        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            let member_annotations = self.parse_annotations()?;
            let mut is_field_private = false;
            if self.check(&TokenKind::Private) {
                self.advance();
                is_field_private = true;
            }

            if self.check(&TokenKind::Async) || self.check(&TokenKind::Fn) {
                let mut is_async = false;
                if self.check(&TokenKind::Async) {
                    self.advance();
                    is_async = true;
                }
                
                if self.check(&TokenKind::Fn) {
                    let mut func = self.parse_fn_decl(is_field_private, is_async, self.current_span())?;
                    Self::attach_annotations(&mut func, member_annotations);
                    methods.push(func);
                } else {
                    return Err(Diagnostic::error("Expected 'fn' after 'async'")
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
                            is_private: is_field_private,
                            is_async: false,
                            annotations: member_annotations,
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
                        annotations: member_annotations,
                    });
                } else {
                    return Err(Diagnostic::error("Expected field name after let/var")
                        .with_span(self.current_span()));
                }

                if self.check(&TokenKind::Semi) {
                    self.advance();
                }
            } else {
                return Err(Diagnostic::error("Expected field or method in actor")
                    .with_span(self.current_span()));
            }
        }

        let end_tok = self.expect(TokenKind::RBrace)?;

        Ok(Decl::Actor {
            name: name_tok,
            generic_params,
            with,
            fields,
            methods,
            is_private,
            annotations: Vec::new(),
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
            let variant_annotations = self.parse_annotations()?;
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
                annotations: variant_annotations,
                span: variant_name.span.merge(v_end_span),
            });
        }

        let end_tok = self.expect(TokenKind::RBrace)?;

        Ok(Decl::Enum {
            name: name_tok,
            generic_params,
            variants,
            is_private,
            annotations: Vec::new(),
            span: if is_private {
                start_span.merge(end_tok.span)
            } else {
                start_tok.span.merge(end_tok.span)
            },
        })
    }

}
