use pace_ast::{Block, Decl, Expr, Ident, Program, Stmt, Type, EnumVariant, MatchArm, Pattern, GenericParam};
use pace_lexer::{Lexer, Token, TokenKind};
use pace_errors::Diagnostic;
use pace_span::Span;

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Option<Token<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current = lexer.next().and_then(|r| r.ok());
        Self { lexer, current }
    }

    fn current_span(&self) -> Span {
        self.current.as_ref().map(|t| t.span).unwrap_or(Span::DUMMY)
    }

    fn advance(&mut self) {
        self.current = self.lexer.next().and_then(|r| r.ok());
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.current.as_ref().map_or(false, |t| &t.kind == kind)
    }

    fn expect(&mut self, kind: TokenKind<'a>) -> Result<Token<'a>, Diagnostic> {
        if self.check(&kind) {
            let tok = self.current.clone().unwrap();
            self.advance();
            Ok(tok)
        } else {
            Err(Diagnostic::error(format!("Expected {:?}", kind)).with_span(self.current_span()))
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, Diagnostic> {
        let mut declarations = Vec::new();
        let start_span = self.current.as_ref().map(|t| t.span).unwrap_or(Span::DUMMY);

        while self.current.is_some() {
            declarations.push(self.parse_decl()?);
        }

        let end_span = declarations.last().map(|d| match d {
            Decl::Let { span, .. } => *span,
            Decl::Var { span, .. } => *span,
            Decl::Const { span, .. } => *span,
            Decl::Function { span, .. } => *span,
            Decl::Struct { span, .. } => *span,
            Decl::Class { span, .. } => *span,
            Decl::Trait { span, .. } => *span,
            Decl::Enum { span, .. } => *span,
            Decl::Expr(_, span) => *span,
        }).unwrap_or(start_span);

        Ok(Program {
            declarations,
            span: start_span.merge(end_span),
        })
    }

    fn parse_decl(&mut self) -> Result<Decl, Diagnostic> {
        if self.check(&TokenKind::Let) {
            let start_tok = self.expect(TokenKind::Let)?;
            
            let name_tok = match &self.current {
                Some(Token { kind: TokenKind::Ident(name), span }) => {
                    let ident = Ident { name: name.to_string(), span: *span };
                    self.advance();
                    ident
                }
                _ => return Err(Diagnostic::error("Expected identifier after 'let'").with_span(self.current_span())),
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
                    return Err(Diagnostic::error("Type annotation is required when an initializer is omitted").with_span(self.current_span()));
                }
                start_tok.span
            };

            Ok(Decl::Let {
                name: name_tok,
                ty,
                value,
                span: start_tok.span.merge(span_end),
            })
        } else if self.check(&TokenKind::Var) {
            let start_tok = self.expect(TokenKind::Var)?;
            
            let name_tok = match &self.current {
                Some(Token { kind: TokenKind::Ident(name), span }) => {
                    let ident = Ident { name: name.to_string(), span: *span };
                    self.advance();
                    ident
                }
                _ => return Err(Diagnostic::error("Expected identifier after 'var'").with_span(self.current_span())),
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
                    return Err(Diagnostic::error("Type annotation is required when an initializer is omitted").with_span(self.current_span()));
                }
                start_tok.span
            };

            Ok(Decl::Var {
                name: name_tok,
                ty,
                value,
                span: start_tok.span.merge(span_end),
            })
        } else if self.check(&TokenKind::Const) {
            let start_tok = self.expect(TokenKind::Const)?;
            
            let name_tok = match &self.current {
                Some(Token { kind: TokenKind::Ident(name), span }) => {
                    let ident = Ident { name: name.to_string(), span: *span };
                    self.advance();
                    ident
                }
                _ => return Err(Diagnostic::error("Expected identifier after 'const'").with_span(self.current_span())),
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
                span: start_tok.span.merge(end_span),
            })
        } else if self.check(&TokenKind::Fn) {
            self.parse_fn_decl()
        } else if self.check(&TokenKind::Struct) {
            self.parse_struct_decl()
        } else if self.check(&TokenKind::Class) {
            self.parse_class_decl()
        } else if self.check(&TokenKind::Trait) {
            self.parse_trait_decl()
        } else if self.check(&TokenKind::Enum) {
            self.parse_enum_decl()
        } else {
            let expr = self.parse_expr()?;
            let span = expr.span();
            Ok(Decl::Expr(expr, span))
        }
    }
    fn parse_generic_params(&mut self) -> Result<Option<Vec<GenericParam>>, Diagnostic> {
        if self.check(&TokenKind::Lt) {
            self.advance();
            let mut params = Vec::new();
            while !self.check(&TokenKind::Gt) && self.current.is_some() {
                if let Some(Token { kind: TokenKind::Ident(name), span }) = &self.current {
                    let param_name = Ident { name: name.to_string(), span: *span };
                    self.advance();
                    
                    let mut default = None;
                    if self.check(&TokenKind::Eq) {
                        self.advance();
                        default = Some(self.parse_type()?);
                    }
                    
                    params.push(GenericParam { name: param_name, default });
                    
                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    }
                } else {
                    return Err(Diagnostic::error("Expected type parameter identifier").with_span(self.current_span()));
                }
            }
            self.expect(TokenKind::Gt)?;
            Ok(Some(params))
        } else {
            Ok(None)
        }
    }

    fn parse_generic_args(&mut self) -> Result<Option<Vec<Type>>, Diagnostic> {
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

    fn parse_with_clause(&mut self) -> Result<Vec<Ident>, Diagnostic> {
        let mut traits = Vec::new();
        if self.check(&TokenKind::With) {
            self.advance();
            loop {
                if let Some(Token { kind: TokenKind::Ident(name), span }) = &self.current {
                    traits.push(Ident { name: name.to_string(), span: *span });
                    self.advance();
                } else {
                    return Err(Diagnostic::error("Expected trait name after 'with'").with_span(self.current_span()));
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

    fn parse_struct_decl(&mut self) -> Result<Decl, Diagnostic> {
        let start_tok = self.expect(TokenKind::Struct)?;
        
        let name_tok = match &self.current {
            Some(Token { kind: TokenKind::Ident(name), span }) => {
                let ident = Ident { name: name.to_string(), span: *span };
                self.advance();
                ident
            }
            _ => return Err(Diagnostic::error("Expected identifier after 'struct'").with_span(self.current_span())),
        };

        let generic_params = self.parse_generic_params()?;
        
        let with = self.parse_with_clause()?;

        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        let mut static_fields = Vec::new();
        let mut const_fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            if self.check(&TokenKind::Const) {
                self.advance();
                if let Some(Token { kind: TokenKind::Ident(name), span }) = &self.current {
                    let field_name = Ident { name: name.to_string(), span: *span };
                    self.advance();
                    
                    self.expect(TokenKind::Colon)?;
                    let field_type = self.parse_type()?;
                    
                    self.expect(TokenKind::Eq)?;
                    let field_value = self.parse_expr()?;
                    
                    const_fields.push((field_name, field_type, field_value));
                    
                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    }
                } else {
                    return Err(Diagnostic::error("Expected identifier after 'const'").with_span(self.current_span()));
                }
            } else if self.check(&TokenKind::Static) {
                self.advance();
                
                if self.check(&TokenKind::Fn) {
                    let mut func = self.parse_fn_decl()?;
                    if let Decl::Function { ref mut is_static, .. } = func {
                        *is_static = true;
                    }
                    methods.push(func);
                } else if let Some(Token { kind: TokenKind::Ident(name), span }) = &self.current {
                    let field_name = Ident { name: name.to_string(), span: *span };
                    self.advance();
                    
                    self.expect(TokenKind::Colon)?;
                    let field_type = self.parse_type()?;
                    
                    self.expect(TokenKind::Eq)?;
                    let field_value = self.parse_expr()?;
                    
                    static_fields.push((field_name, field_type, field_value));
                    
                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    }
                } else {
                    return Err(Diagnostic::error("Expected function or identifier after 'static'").with_span(self.current_span()));
                }
            } else if self.check(&TokenKind::Fn) {
                methods.push(self.parse_fn_decl()?);
            } else if let Some(Token { kind: TokenKind::Ident(name), span }) = &self.current {
                if *name == "init" {
                    let init_span_start = *span;
                    self.advance(); // consume 'init'
                    let ident = Ident { name: "init".to_string(), span: init_span_start };
                    
                    self.expect(TokenKind::LParen)?;
                    let mut params = Vec::new();
                    while !self.check(&TokenKind::RParen) && self.current.is_some() {
                        let param_name = match &self.current {
                            Some(Token { kind: TokenKind::Ident(n), span }) => Ident { name: n.to_string(), span: *span },
                            _ => return Err(Diagnostic::error("Expected parameter name").with_span(self.current_span())),
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
                        span: init_span_start.merge(body.span),
                    });
                    continue;
                }
                
                let field_name = Ident { name: name.to_string(), span: self.current.as_ref().unwrap().span };
                self.advance();
                
                self.expect(TokenKind::Colon)?;
                let field_type = self.parse_type()?;
                
                let mut field_value = None;
                if self.check(&TokenKind::Eq) {
                    self.advance();
                    field_value = Some(self.parse_expr()?);
                }
                
                fields.push((field_name, field_type, field_value));
                
                if self.check(&TokenKind::Comma) {
                    self.advance();
                }
            } else {
                return Err(Diagnostic::error("Expected field or method in struct").with_span(self.current_span()));
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
            span: start_tok.span.merge(end_tok.span),
        })
    }

    fn parse_trait_decl(&mut self) -> Result<Decl, Diagnostic> {
        let start_tok = self.expect(TokenKind::Trait)?;
        
        let name_tok = match &self.current {
            Some(Token { kind: TokenKind::Ident(name), span }) => {
                let ident = Ident { name: name.to_string(), span: *span };
                self.advance();
                ident
            }
            _ => return Err(Diagnostic::error("Expected identifier after 'trait'").with_span(self.current_span())),
        };

        let generic_params = self.parse_generic_params()?;

        self.expect(TokenKind::LBrace)?;
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            if self.check(&TokenKind::Fn) {
                methods.push(self.parse_fn_decl()?);
            } else {
                return Err(Diagnostic::error("Expected method in trait").with_span(self.current_span()));
            }
        }
        let end_tok = self.expect(TokenKind::RBrace)?;

        Ok(Decl::Trait {
            name: name_tok,
            generic_params,
            methods,
            span: start_tok.span.merge(end_tok.span),
        })
    }

    fn parse_class_decl(&mut self) -> Result<Decl, Diagnostic> {
        let start_tok = self.expect(TokenKind::Class)?;
        
        let name_tok = match &self.current {
            Some(Token { kind: TokenKind::Ident(name), span }) => {
                let ident = Ident { name: name.to_string(), span: *span };
                self.advance();
                ident
            }
            _ => return Err(Diagnostic::error("Expected identifier after 'class'").with_span(self.current_span())),
        };

        let generic_params = self.parse_generic_params()?;
        
        let with = self.parse_with_clause()?;

        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        let mut static_fields = Vec::new();
        let mut const_fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            if self.check(&TokenKind::Const) {
                self.advance();
                if let Some(Token { kind: TokenKind::Ident(name), span }) = &self.current {
                    let field_name = Ident { name: name.to_string(), span: *span };
                    self.advance();
                    
                    self.expect(TokenKind::Colon)?;
                    let field_type = self.parse_type()?;
                    
                    self.expect(TokenKind::Eq)?;
                    let field_value = self.parse_expr()?;
                    
                    const_fields.push((field_name, field_type, field_value));
                    
                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    }
                } else {
                    return Err(Diagnostic::error("Expected identifier after 'const'").with_span(self.current_span()));
                }
            } else if self.check(&TokenKind::Static) {
                self.advance();
                
                if self.check(&TokenKind::Fn) {
                    let mut func = self.parse_fn_decl()?;
                    if let Decl::Function { ref mut is_static, .. } = func {
                        *is_static = true;
                    }
                    methods.push(func);
                } else if let Some(Token { kind: TokenKind::Ident(name), span }) = &self.current {
                    let field_name = Ident { name: name.to_string(), span: *span };
                    self.advance();
                    
                    self.expect(TokenKind::Colon)?;
                    let field_type = self.parse_type()?;
                    
                    self.expect(TokenKind::Eq)?;
                    let field_value = self.parse_expr()?;
                    
                    static_fields.push((field_name, field_type, field_value));
                    
                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    }
                } else {
                    return Err(Diagnostic::error("Expected function or identifier after 'static'").with_span(self.current_span()));
                }
            } else if self.check(&TokenKind::Fn) {
                methods.push(self.parse_fn_decl()?);
            } else if let Some(Token { kind: TokenKind::Ident(name), span }) = &self.current {
                if *name == "init" {
                    let init_span_start = *span;
                    self.advance(); // consume 'init'
                    let ident = Ident { name: "init".to_string(), span: init_span_start };
                    
                    self.expect(TokenKind::LParen)?;
                    let mut params = Vec::new();
                    while !self.check(&TokenKind::RParen) && self.current.is_some() {
                        let param_name = match &self.current {
                            Some(Token { kind: TokenKind::Ident(n), span }) => Ident { name: n.to_string(), span: *span },
                            _ => return Err(Diagnostic::error("Expected parameter name").with_span(self.current_span())),
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
                        span: init_span_start.merge(body.span),
                    });
                    continue;
                }
                
                let field_name = Ident { name: name.to_string(), span: self.current.as_ref().unwrap().span };
                self.advance();
                
                self.expect(TokenKind::Colon)?;
                let field_type = self.parse_type()?;
                
                let mut field_value = None;
                if self.check(&TokenKind::Eq) {
                    self.advance();
                    field_value = Some(self.parse_expr()?);
                }
                
                fields.push((field_name, field_type, field_value));
                
                if self.check(&TokenKind::Comma) {
                    self.advance();
                }
            } else {
                return Err(Diagnostic::error("Expected field or method in class").with_span(self.current_span()));
            }
        }
        
        let end_tok = self.expect(TokenKind::RBrace)?;

        Ok(Decl::Class {
            name: name_tok,
            generic_params,
            with,
            fields,
            static_fields,
            const_fields,
            methods,
            span: start_tok.span.merge(end_tok.span),
        })
    }

    fn parse_enum_decl(&mut self) -> Result<Decl, Diagnostic> {
        let start_tok = self.expect(TokenKind::Enum)?;
        
        let name_tok = match &self.current {
            Some(Token { kind: TokenKind::Ident(name), span }) => {
                let ident = Ident { name: name.to_string(), span: *span };
                self.advance();
                ident
            }
            _ => return Err(Diagnostic::error("Expected identifier after 'enum'").with_span(self.current_span())),
        };

        let generic_params = self.parse_generic_params()?;

        self.expect(TokenKind::LBrace)?;
        let mut variants = Vec::new();

        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            let variant_name = match &self.current {
                Some(Token { kind: TokenKind::Ident(name), span }) => {
                    let ident = Ident { name: name.to_string(), span: *span };
                    self.advance();
                    ident
                }
                _ => return Err(Diagnostic::error("Expected variant name").with_span(self.current_span())),
            };

            let mut fields = None;
            let mut v_end_span = variant_name.span;
            
            if self.check(&TokenKind::LParen) {
                self.advance();
                let mut vfields = Vec::new();
                while !self.check(&TokenKind::RParen) && self.current.is_some() {
                    let field_name = match &self.current {
                        Some(Token { kind: TokenKind::Ident(n), span }) => Ident { name: n.to_string(), span: *span },
                        _ => return Err(Diagnostic::error("Expected field name").with_span(self.current_span())),
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

            variants.push(EnumVariant { name: variant_name.clone(), fields, span: variant_name.span.merge(v_end_span) });
        }
        
        let end_tok = self.expect(TokenKind::RBrace)?;

        Ok(Decl::Enum {
            name: name_tok,
            generic_params,
            variants,
            span: start_tok.span.merge(end_tok.span),
        })
    }

    fn parse_fn_decl(&mut self) -> Result<Decl, Diagnostic> {
        let start_tok = self.expect(TokenKind::Fn)?;
        
        let name_tok = match &self.current {
            Some(Token { kind: TokenKind::Ident(name), span }) => {
                let ident = Ident { name: name.to_string(), span: *span };
                self.advance();
                ident
            }
            _ => return Err(Diagnostic::error("Expected identifier after 'fn'").with_span(self.current_span())),
        };

        let generic_params = self.parse_generic_params()?;

        self.expect(TokenKind::LParen)?;
        
        let mut params = Vec::new();
        while !self.check(&TokenKind::RParen) {
            let param_name = match &self.current {
                Some(Token { kind: TokenKind::Ident(name), span }) => {
                    let ident = Ident { name: name.to_string(), span: *span };
                    self.advance();
                    ident
                }
                _ => return Err(Diagnostic::error("Expected parameter name").with_span(self.current_span())),
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
            span: start_tok.span.merge(end_span),
        })
    }

    fn parse_type(&mut self) -> Result<Type, Diagnostic> {
        let (ident_name, ident_span) = match &self.current {
            Some(Token { kind: TokenKind::Ident(name), span }) => (name.to_string(), *span),
            _ => return Err(Diagnostic::error("Expected type name").with_span(self.current_span())),
        };
        
        self.advance();
        let ident = Ident { name: ident_name, span: ident_span };
        
        let mut base_type = if self.check(&TokenKind::Lt) {
            let mut args = Vec::new();
            self.advance();
            let mut end_span = ident_span;
            while !self.check(&TokenKind::Gt) && self.current.is_some() {
                args.push(self.parse_type()?);
                if self.check(&TokenKind::Comma) {
                    self.advance();
                }
            }
            if let Some(tok) = &self.current {
                end_span = tok.span;
            }
            self.expect(TokenKind::Gt)?;
            Type::Generic(Box::new(Type::Named(ident)), args, ident_span.merge(end_span))
        } else {
            Type::Named(ident)
        };

        if self.check(&TokenKind::Question) {
            let span = base_type.span().merge(self.current.as_ref().unwrap().span);
            self.advance();
            base_type = Type::Optional(Box::new(base_type), span);
        }

        Ok(base_type)
    }

    fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let start_tok = self.expect(TokenKind::LBrace).map_err(|_| Diagnostic::error(format!("Expected LBrace (found {:?})", self.current)).with_span(self.current_span()))?;
        let mut statements = Vec::new();

        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            statements.push(self.parse_stmt()?);
        }

        let end_tok = self.expect(TokenKind::RBrace)?;

        Ok(Block {
            statements,
            span: start_tok.span.merge(end_tok.span),
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        if self.check(&TokenKind::Let) {
            let start_tok = self.expect(TokenKind::Let)?;
            let name_tok = self.current.take().ok_or_else(|| Diagnostic::error("Expected identifier after 'let'").with_span(self.current_span()))?;
            let name = match name_tok.kind {
                TokenKind::Ident(n) => Ident { name: n.to_string(), span: name_tok.span },
                _ => return Err(Diagnostic::error("Expected identifier after 'let'").with_span(self.current_span())),
            };
            self.advance();
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
                    return Err(Diagnostic::error("Type annotation is required when an initializer is omitted").with_span(self.current_span()));
                }
                start_tok.span
            };
            
            let span = start_tok.span.merge(span_end);
            Ok(Stmt::Let { name, ty, value, span })
        } else if self.check(&TokenKind::Var) {
            let start_tok = self.expect(TokenKind::Var)?;
            let name_tok = self.current.take().ok_or_else(|| Diagnostic::error("Expected identifier after 'var'").with_span(self.current_span()))?;
            let name = match name_tok.kind {
                TokenKind::Ident(n) => Ident { name: n.to_string(), span: name_tok.span },
                _ => return Err(Diagnostic::error("Expected identifier after 'var'").with_span(self.current_span())),
            };
            self.advance();
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
                    return Err(Diagnostic::error("Type annotation is required when an initializer is omitted").with_span(self.current_span()));
                }
                start_tok.span
            };
            
            let span = start_tok.span.merge(span_end);
            Ok(Stmt::Var { name, ty, value, span })
        } else if self.check(&TokenKind::Return) {
            let start_tok = self.expect(TokenKind::Return)?;
            
            let mut expr = None;
            let mut end_span = start_tok.span;
            
            if !self.check(&TokenKind::RBrace) {
                let e = self.parse_expr()?;
                end_span = end_span.merge(e.span());
                expr = Some(e);
            }

            Ok(Stmt::Return(expr, start_tok.span.merge(end_span)))
        } else {
            let expr = self.parse_expr()?;
            let span = expr.span();
            Ok(Stmt::ExprStmt(expr, span))
        }
    }

    fn parse_postfix_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_primary()?;
        
        loop {
            if self.check(&TokenKind::Dot) {
                self.advance();
                let member_name = match &self.current {
                    Some(Token { kind: TokenKind::Ident(name), span }) => {
                        let ident = Ident { name: name.to_string(), span: *span };
                        self.advance();
                        ident
                    }
                    _ => return Err(Diagnostic::error("Expected member name after '.'").with_span(self.current_span())),
                };
                
                let span = left.span().merge(member_name.span);
                left = Expr::MemberAccess {
                    span,
                    object: Box::new(left),
                    member: member_name,
                };
            } else if self.check(&TokenKind::LParen) {
                self.advance();
                let mut args = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        // Check for named argument: `ident:`
                        let mut label = None;
                        
                        let mut parsed_expr = self.parse_expr()?;
                        
                        if let Expr::Ident(ref ident) = parsed_expr {
                            if self.check(&TokenKind::Colon) {
                                self.advance(); // consume `:`
                                label = Some(ident.clone());
                                parsed_expr = self.parse_expr()?;
                            }
                        }
                        args.push((label, parsed_expr));
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

    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_postfix_expr()?;

        if self.check(&TokenKind::Eq) {
            self.advance();
            let right = self.parse_expr()?;
            let span = left.span().merge(right.span());
            return Ok(Expr::Assign { target: Box::new(left), value: Box::new(right), span });
        }

        loop {
            let op = if self.check(&TokenKind::Plus) { Some(pace_ast::BinaryOp::Add) }
            else if self.check(&TokenKind::Minus) { Some(pace_ast::BinaryOp::Sub) }
            else if self.check(&TokenKind::Star) { Some(pace_ast::BinaryOp::Mul) }
            else if self.check(&TokenKind::Slash) { Some(pace_ast::BinaryOp::Div) }
            else if self.check(&TokenKind::EqEq) { Some(pace_ast::BinaryOp::EqEq) }
            else if self.check(&TokenKind::NotEq) { Some(pace_ast::BinaryOp::NotEq) }
            else if self.check(&TokenKind::Gt) { Some(pace_ast::BinaryOp::Gt) }
            else if self.check(&TokenKind::Lt) { Some(pace_ast::BinaryOp::Lt) }
            else if self.check(&TokenKind::GtEq) { Some(pace_ast::BinaryOp::GtEq) }
            else if self.check(&TokenKind::LtEq) { Some(pace_ast::BinaryOp::LtEq) }
            else { None };

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

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
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

        let tok = self.current.take().ok_or_else(|| Diagnostic::error("Expected expression, found EOF").with_span(self.current_span()))?;
        self.advance();

        match tok.kind {
            TokenKind::Int(val) => Ok(Expr::IntLiteral(val.to_string(), tok.span)),
            TokenKind::Float(val) => Ok(Expr::FloatLiteral(val.to_string(), tok.span)),
            TokenKind::String(val) => Ok(Expr::StringLiteral(val.to_string(), tok.span)),
            TokenKind::Ident(name) => {
                let ident = Ident { name: name.to_string(), span: tok.span };
                Ok(Expr::Ident(ident))
            }
            _ => Err(Diagnostic::error(format!("Unexpected token in expression: {:?}", tok.kind)).with_span(self.current_span())),
        }
    }

    fn parse_match_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_tok = self.expect(TokenKind::Match)?;
        let subject = self.parse_expr()?;
        self.expect(TokenKind::LBrace)?;

        let mut arms = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            let pattern = if let Some(Token { kind: TokenKind::Ident(name), span }) = &self.current {
                if *name == "_" {
                    let s = *span;
                    self.advance();
                    Pattern::CatchAll(s)
                } else {
                    let ident = Ident { name: name.to_string(), span: *span };
                    self.advance();
                    
                    if self.check(&TokenKind::LParen) {
                        self.advance();
                        let mut fields = Vec::new();
                        while !self.check(&TokenKind::RParen) && self.current.is_some() {
                            if let Some(Token { kind: TokenKind::Ident(n), span: fspan }) = &self.current {
                                fields.push(Ident { name: n.to_string(), span: *fspan });
                                self.advance();
                                if self.check(&TokenKind::Comma) {
                                    self.advance();
                                }
                            } else {
                                return Err(Diagnostic::error("Expected identifier in match pattern").with_span(self.current_span()));
                            }
                        }
                        self.expect(TokenKind::RParen)?;
                        Pattern::Variant { name: ident.clone(), fields: Some(fields), span: ident.span }
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
