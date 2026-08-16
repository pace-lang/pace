use super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {

    pub(crate) fn parse_visibility(&mut self) -> bool {
        if self.match_token(&[TokenKind::Private]) {
            true
        } else {
            let _ = self.match_token(&[TokenKind::Public]);
            false
        }
    }

    pub(crate) fn declaration(&mut self) -> Option<Stmt<'a>> {
        let is_private = self.parse_visibility();

        let res = if self.match_token(&[TokenKind::Interface]) {
            self.interface_declaration(is_private)
        } else if self.match_token(&[TokenKind::Type]) {
            self.type_alias_declaration(is_private)
        } else if self.match_token(&[TokenKind::Struct]) {
            self.struct_declaration(is_private)
        } else if self.match_token(&[TokenKind::Class]) {
            self.class_declaration(is_private)
        } else if self.match_token(&[TokenKind::Enum]) {
            self.enum_declaration(is_private)
        } else if self.match_token(&[TokenKind::Func]) {
            self.function_declaration(is_private)
        } else if self.match_token(&[TokenKind::Let]) {
            self.variable_declaration(false, false, is_private)
        } else if self.match_token(&[TokenKind::Var]) {
            self.variable_declaration(true, false, is_private)
        } else if self.match_token(&[TokenKind::Weak]) {
            if self.match_token(&[TokenKind::Var]) {
                self.variable_declaration(true, true, is_private)
            } else {
                self.error_at_current("Expected 'var' after 'weak'.");
                None
            }
        } else if self.match_token(&[TokenKind::Foreign]) {
            self.foreign_declaration(is_private)
        } else if self.match_token(&[TokenKind::Import]) {
            if is_private {
                self.error_at_current("Visibility modifiers are not allowed on imports.");
            }
            self.import_declaration()
        } else if self.match_token(&[TokenKind::Export]) {
            if is_private {
                self.error_at_current("Visibility modifiers are not allowed on exports.");
            }
            self.export_declaration()
        } else {
            if is_private {
                self.error_at_current("Visibility modifiers are only allowed on declarations.");
            }
            self.statement()
        };

        if res.is_none() {
            self.synchronize();
        }
        res
    }

    pub(crate) fn parse_type_params(&mut self) -> Option<Vec<session::Symbol>> {
        let mut type_params = Vec::new();
        if self.match_token(&[TokenKind::Less]) {
            if !self.check(&TokenKind::Greater) {
                loop {
                    if let Some(Token {
                        kind: TokenKind::Identifier(t),
                        ..
                    }) = self.peek().cloned()
                    {
                        self.advance();
                        type_params.push(t);
                    } else {
                        self.error_at_current("Expected generic parameter name.");
                        return None;
                    }
                    if !self.match_token(&[TokenKind::Comma]) {
                        break;
                    }
                }
            }
            if !self.match_token(&[TokenKind::Greater]) {
                self.error_at_current("Expected '>' after generic parameters.");
                return None;
            }
        }
        Some(type_params)
    }

    pub(crate) fn parse_type_expr(&mut self) -> Option<TypeExpr<'a>> {
        if self.match_token(&[TokenKind::LeftBracket]) {
            let inner = self.parse_type_expr()?;
            if !self.match_token(&[TokenKind::RightBracket]) {
                self.error_at_current("Expected ']' after array element type.");
                return None;
            }
            let mut ty = TypeExpr::Array(self.session.ast_arena.alloc(inner));
            if self.match_token(&[TokenKind::Question]) {
                ty = TypeExpr::Optional(self.session.ast_arena.alloc(ty));
            }
            return Some(ty);
        }

        if let Some(Token {
            kind: TokenKind::Identifier(t),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            let base_type = t;

            if self.match_token(&[TokenKind::Less]) {
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

                let mut ty = TypeExpr::GenericInstance(base_type, type_args);
                if self.match_token(&[TokenKind::Question]) {
                    ty = TypeExpr::Optional(self.session.ast_arena.alloc(ty));
                }
                return Some(ty);
            }

            let mut ty = TypeExpr::Named(base_type);
            if self.match_token(&[TokenKind::Question]) {
                ty = TypeExpr::Optional(self.session.ast_arena.alloc(ty));
            }
            Some(ty)
        } else {
            self.error_at_current("Expected type name.");
            None
        }
    }

    pub(crate) fn enum_declaration(&mut self, is_private: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected enum name.");
            return None;
        };

        let type_params = self.parse_type_params()?;

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before enum body.");
            return None;
        }

        let mut variants = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            if let Some(Token {
                kind: TokenKind::Identifier(n),
                ..
            }) = self.peek().cloned()
            {
                self.advance();
                let variant_name = n;

                let fields = if self.match_token(&[TokenKind::LeftParen]) {
                    let mut variant_fields = Vec::new();
                    if !self.check(&TokenKind::RightParen) {
                        loop {
                            let field_name = if let Some(Token {
                                kind: TokenKind::Identifier(fname),
                                ..
                            }) = self.peek().cloned()
                            {
                                // Could be a name `x: Int` or just a type `Int`
                                // Let's check if there's a colon next
                                let next_is_colon = self
                                    .tokens
                                    .get(self.current + 1)
                                    .map(|t| t.kind == TokenKind::Colon)
                                    .unwrap_or(false);
                                if next_is_colon {
                                    self.advance(); // consume name
                                    self.advance(); // consume colon
                                    Some(fname)
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            {
                                let ty = self.parse_type_expr()?;
                                variant_fields.push(ast::EnumField {
                                    name: field_name,
                                    ty,
                                });
                            }

                            if !self.match_token(&[TokenKind::Comma]) {
                                break;
                            }
                        }
                    }
                    if !self.match_token(&[TokenKind::RightParen]) {
                        self.error_at_current("Expected ')' after enum variant fields.");
                        return None;
                    }
                    Some(variant_fields)
                } else {
                    None
                };

                variants.push(ast::EnumVariant {
                    name: variant_name,
                    fields,
                });

                if self.match_token(&[TokenKind::Comma]) {
                    continue;
                }
            } else {
                self.error_at_current("Expected enum variant name.");
                return None;
            }
        }

        if !self.match_token(&[TokenKind::RightBrace]) {
            self.error_at_current("Expected '}' after enum body.");
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

        Some(Stmt::new(
            StmtKind::Enum {
                name,
                type_params,
                variants,
                is_private,
            },
            span,
        ))
    }

    pub(crate) fn class_declaration(&mut self, is_private: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected class name.");
            return None;
        };

        let type_params = self.parse_type_params()?;

        let mut implements = Vec::new();
        if self.match_token(&[TokenKind::Implements]) {
            loop {
                if let Some(Token {
                    kind: TokenKind::Identifier(i),
                    ..
                }) = self.peek().cloned()
                {
                    self.advance();
                    implements.push(i);
                } else {
                    self.error_at_current("Expected interface name.");
                    return None;
                }

                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before class body.");
            return None;
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let item_is_private = self.parse_visibility();

            if self.match_token(&[TokenKind::Let]) {
                if let Some(field) = self.variable_declaration(false, false, item_is_private) {
                    fields.push(field);
                }
            } else if self.match_token(&[TokenKind::Var]) {
                if let Some(field) = self.variable_declaration(true, false, item_is_private) {
                    fields.push(field);
                }
            } else if self.match_token(&[TokenKind::Weak]) {
                if self.match_token(&[TokenKind::Var]) {
                    if let Some(field) = self.variable_declaration(true, true, item_is_private) {
                        fields.push(field);
                    }
                } else {
                    self.error_at_current("Expected 'var' after 'weak'.");
                }
            } else if self.match_token(&[TokenKind::Func]) {
                if self.match_token(&[TokenKind::Init]) {
                    if let Some(init_method) = self.init_declaration(item_is_private) {
                        methods.push(init_method);
                    }
                } else {
                    if let Some(method) = self.function_declaration(item_is_private) {
                        methods.push(method);
                    }
                }
            } else if self.match_token(&[TokenKind::Init]) {
                self.error_at_current("Constructors must be declared with 'func init'.");
                if let Some(init_method) = self.init_declaration(false) {
                    methods.push(init_method);
                }
            } else {
                self.error_at_current("Expected property or method inside class.");
                self.advance();
            }
        }

        if !self.match_token(&[TokenKind::RightBrace]) {
            self.error_at_current("Expected '}' after class body.");
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
        Some(Stmt::new(
            StmtKind::Class {
                name,
                type_params,
                implements,
                methods,
                fields,
                is_private,
            },
            span,
        ))
    }

    pub(crate) fn type_alias_declaration(&mut self, is_private: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected type alias name.");
            return None;
        };

        let type_params = self.parse_type_params()?;

        if !self.match_token(&[TokenKind::Equal]) {
            self.error_at_current("Expected '=' after type alias name.");
            return None;
        }

        let target_type = self.parse_type_expr()?;

        if !self.match_token(&[TokenKind::Semicolon]) {
            self.error_at_current("Expected ';' after type alias declaration.");
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

        Some(Stmt::new(
            StmtKind::TypeAlias {
                name,
                type_params,
                target_type,
                is_private,
            },
            span,
        ))
    }

    pub(crate) fn struct_declaration(&mut self, is_private: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected struct name.");
            return None;
        };

        let type_params = self.parse_type_params()?;

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before struct body.");
            return None;
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let item_is_private = self.parse_visibility();

            if self.match_token(&[TokenKind::Let]) {
                if let Some(field) = self.variable_declaration(false, false, item_is_private) {
                    fields.push(field);
                }
            } else if self.match_token(&[TokenKind::Var]) {
                if let Some(field) = self.variable_declaration(true, false, item_is_private) {
                    fields.push(field);
                }
            } else if self.match_token(&[TokenKind::Weak]) {
                self.error_at_current("Structs cannot contain weak references.");
                if self.match_token(&[TokenKind::Var]) {
                    self.variable_declaration(true, true, item_is_private); // Parse it anyway to recover
                }
            } else if self.match_token(&[TokenKind::Func]) {
                if self.match_token(&[TokenKind::Init]) {
                    if let Some(init_method) = self.init_declaration(item_is_private) {
                        methods.push(init_method);
                    }
                } else {
                    if let Some(method) = self.function_declaration(item_is_private) {
                        methods.push(method);
                    }
                }
            } else if self.match_token(&[TokenKind::Init]) {
                self.error_at_current("Constructors must be declared with 'func init'.");
                if let Some(init_method) = self.init_declaration(false) {
                    methods.push(init_method);
                }
            } else {
                self.error_at_current("Expected property or method inside struct.");
                self.advance();
            }
        }

        if !self.match_token(&[TokenKind::RightBrace]) {
            self.error_at_current("Expected '}' after struct body.");
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
        Some(Stmt::new(
            StmtKind::Struct {
                name,
                type_params,
                methods,
                fields,
                is_private,
            },
            span,
        ))
    }

    pub(crate) fn interface_declaration(&mut self, is_private: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected interface name.");
            return None;
        };

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before interface body.");
            return None;
        }

        let mut methods = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let item_is_private = self.parse_visibility();

            if self.match_token(&[TokenKind::Func]) {
                if let Some(method) = self.interface_method_declaration(item_is_private) {
                    methods.push(method);
                }
            } else {
                self.error_at_current("Expected method signature inside interface.");
                self.advance();
            }
        }

        if !self.match_token(&[TokenKind::RightBrace]) {
            self.error_at_current("Expected '}' after interface body.");
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
        Some(Stmt::new(
            StmtKind::Interface {
                name,
                methods,
                is_private,
            },
            span,
        ))
    }

    pub(crate) fn interface_method_declaration(&mut self, is_private: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected method name.");
            return None;
        };

        if !self.match_token(&[TokenKind::LeftParen]) {
            self.error_at_current("Expected '(' after method name.");
            return None;
        }

        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let param_name = if let Some(Token {
                    kind: TokenKind::Identifier(n),
                    ..
                }) = self.peek().cloned()
                {
                    self.advance();
                    n
                } else {
                    self.error_at_current("Expected parameter name.");
                    return None;
                };

                if !self.match_token(&[TokenKind::Colon]) {
                    self.error_at_current("Expected ':' after parameter name.");
                    return None;
                }

                let param_type = self.parse_type_expr()?;

                params.push((param_name, param_type));

                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        if !self.match_token(&[TokenKind::RightParen]) {
            self.error_at_current("Expected ')' after parameters.");
            return None;
        }

        let mut return_type = None;
        if self.match_token(&[TokenKind::Arrow]) {
            return_type = Some(self.parse_type_expr()?);
        }

        if !self.match_token(&[TokenKind::Semicolon]) {
            self.error_at_current("Expected ';' after interface method declaration.");
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

        // We use StmtKind::Func but with an empty block for the body.
        let empty_body = self
            .session
            .ast_arena
            .alloc(Stmt::new(StmtKind::Block(Vec::new()), span));
        Some(Stmt::new(
            StmtKind::Func {
                name,
                type_params: Vec::new(),
                params,
                return_type,
                body: empty_body,
                is_private,
            },
            span,
        ))
    }

    pub(crate) fn init_declaration(&mut self, is_private: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;
        let name = self.session.interner.borrow_mut().intern("init");

        if !self.match_token(&[TokenKind::LeftParen]) {
            self.error_at_current("Expected '(' after init.");
            return None;
        }

        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let param_name = if let Some(Token {
                    kind: TokenKind::Identifier(n),
                    ..
                }) = self.peek().cloned()
                {
                    self.advance();
                    n
                } else {
                    self.error_at_current("Expected parameter name.");
                    return None;
                };

                if !self.match_token(&[TokenKind::Colon]) {
                    self.error_at_current("Expected ':' after parameter name.");
                    return None;
                }

                let param_type = self.parse_type_expr()?;

                params.push((param_name, param_type));

                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        if !self.match_token(&[TokenKind::RightParen]) {
            self.error_at_current("Expected ')' after parameters.");
            return None;
        }

        let return_type = Some(TypeExpr::Named(
            self.session.interner.borrow_mut().intern("Void"),
        ));

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before init body.");
            return None;
        }

        let body = self.block()?;
        let end_span = body.span;
        let span = Span::new(
            start_span.file_id,
            start_span.start,
            end_span.end,
            start_span.start_loc,
            end_span.end_loc,
        );
        Some(Stmt::new(
            StmtKind::Func {
                name,
                type_params: Vec::new(),
                params,
                return_type,
                body: self.session.ast_arena.alloc(body),
                is_private,
            },
            span,
        ))
    }

    pub(crate) fn foreign_declaration(&mut self, is_private: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        if !self.match_token(&[TokenKind::Func]) {
            self.error_at_current("Expected 'func' after 'foreign'.");
            return None;
        }

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected foreign function name.");
            return None;
        };

        let type_params = self.parse_type_params()?;

        if !self.match_token(&[TokenKind::LeftParen]) {
            self.error_at_current("Expected '(' after foreign function name.");
            return None;
        }

        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let param_name = if let Some(Token {
                    kind: TokenKind::Identifier(n),
                    ..
                }) = self.peek().cloned()
                {
                    self.advance();
                    n
                } else {
                    self.error_at_current("Expected parameter name.");
                    return None;
                };

                if !self.match_token(&[TokenKind::Colon]) {
                    self.error_at_current("Expected ':' after parameter name.");
                    return None;
                }

                let param_type = self.parse_type_expr()?;

                params.push((param_name, param_type));

                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        if !self.match_token(&[TokenKind::RightParen]) {
            self.error_at_current("Expected ')' after parameters.");
            return None;
        }

        let mut return_type = None;
        if self.match_token(&[TokenKind::Arrow]) {
            return_type = Some(self.parse_type_expr()?);
        }

        if !self.match_token(&[TokenKind::Semicolon]) {
            self.error_at_current("Expected ';' after foreign function declaration.");
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

        Some(Stmt::new(
            StmtKind::ForeignFunc {
                name,
                type_params,
                params,
                return_type,
                is_private,
            },
            span,
        ))
    }

    pub(crate) fn function_declaration(&mut self, is_private: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected function name.");
            return None;
        };

        let type_params = self.parse_type_params()?;

        if !self.match_token(&[TokenKind::LeftParen]) {
            self.error_at_current("Expected '(' after function name.");
            return None;
        }

        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let param_name = if let Some(Token {
                    kind: TokenKind::Identifier(n),
                    ..
                }) = self.peek().cloned()
                {
                    self.advance();
                    n
                } else {
                    self.error_at_current("Expected parameter name.");
                    return None;
                };

                if !self.match_token(&[TokenKind::Colon]) {
                    self.error_at_current("Expected ':' after parameter name.");
                    return None;
                }

                let param_type = self.parse_type_expr()?;

                params.push((param_name, param_type));

                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        if !self.match_token(&[TokenKind::RightParen]) {
            self.error_at_current("Expected ')' after parameters.");
            return None;
        }

        let mut return_type = None;
        if self.match_token(&[TokenKind::Arrow]) {
            return_type = Some(self.parse_type_expr()?);
        }

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before function body.");
            return None;
        }

        let body = self.block()?;
        let end_span = body.span;
        let span = Span::new(
            start_span.file_id,
            start_span.start,
            end_span.end,
            start_span.start_loc,
            end_span.end_loc,
        );
        Some(Stmt::new(
            StmtKind::Func {
                name,
                type_params,
                params,
                return_type,
                body: self.session.ast_arena.alloc(body),
                is_private,
            },
            span,
        ))
    }

    pub(crate) fn variable_declaration(
        &mut self,
        is_var: bool,
        is_weak: bool,
        is_private: bool,
    ) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected variable name.");
            return None;
        };

        let type_annotation = if self.match_token(&[TokenKind::Colon]) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let initializer = if self.match_token(&[TokenKind::Equal]) {
            Some(self.expression()?)
        } else {
            None
        };

        self.match_token(&[TokenKind::Semicolon]);

        let end_span = self.previous().span;
        let span = Span::new(
            start_span.file_id,
            start_span.start,
            end_span.end,
            start_span.start_loc,
            end_span.end_loc,
        );

        let kind = if is_var {
            StmtKind::Var {
                name,
                type_annotation,
                initializer: initializer.map(|e| &*self.session.ast_arena.alloc(e)),
                is_weak,
                is_private,
            }
        } else {
            StmtKind::Let {
                name,
                type_annotation,
                initializer: initializer.map(|e| &*self.session.ast_arena.alloc(e)),
                is_private,
            }
        };

        Some(Stmt::new(kind, span))
    }

    pub(crate) fn import_declaration(&mut self) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let mut path = session::Symbol(0);
        if self.match_token(&[TokenKind::StringStart]) {
            if let Some(Token {
                kind: TokenKind::StringPart(p),
                ..
            }) = self.peek().cloned()
            {
                self.advance();
                path = p;
            }
            if !self.match_token(&[TokenKind::StringEnd]) {
                self.error_at_current("Expected '\"' after string.");
                return None;
            }
        } else {
            self.error_at_current("Expected string after 'import'.");
            return None;
        }

        let mut alias = None;
        if self.match_token(&[TokenKind::As]) {
            if let Some(Token {
                kind: TokenKind::Identifier(a),
                ..
            }) = self.peek().cloned()
            {
                self.advance();
                alias = Some(a);
            } else {
                self.error_at_current("Expected identifier after 'as'.");
            }
        }

        let mut show = Vec::new();
        if self.match_token(&[TokenKind::Show]) {
            loop {
                if let Some(Token {
                    kind: TokenKind::Identifier(i),
                    ..
                }) = self.peek().cloned()
                {
                    self.advance();
                    show.push(i);
                } else {
                    self.error_at_current("Expected identifier after 'show'.");
                    break;
                }
                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        let mut hide = Vec::new();
        if self.match_token(&[TokenKind::Hide]) {
            loop {
                if let Some(Token {
                    kind: TokenKind::Identifier(i),
                    ..
                }) = self.peek().cloned()
                {
                    self.advance();
                    hide.push(i);
                } else {
                    self.error_at_current("Expected identifier after 'hide'.");
                    break;
                }
                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        if !self.match_token(&[TokenKind::Semicolon]) {
            self.error_at_current("Expected ';' after import declaration.");
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

        Some(Stmt::new(
            StmtKind::Import {
                path,
                alias,
                show,
                hide,
            },
            span,
        ))
    }

    pub(crate) fn export_declaration(&mut self) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let mut path = session::Symbol(0);
        if self.match_token(&[TokenKind::StringStart]) {
            if let Some(Token {
                kind: TokenKind::StringPart(p),
                ..
            }) = self.peek().cloned()
            {
                self.advance();
                path = p;
            }
            if !self.match_token(&[TokenKind::StringEnd]) {
                self.error_at_current("Expected '\"' after string.");
                return None;
            }
        } else {
            self.error_at_current("Expected string after 'export'.");
            return None;
        }

        if !self.match_token(&[TokenKind::Semicolon]) {
            self.error_at_current("Expected ';' after export declaration.");
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

        Some(Stmt::new(StmtKind::Export { path }, span))
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use lexer::Scanner;


}
