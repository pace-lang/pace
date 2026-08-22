use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
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
                if let Some(ty) = self.parse_type_expr() {
                    implements.push(ty);
                } else {
                    self.error_at_current("Expected interface name or type.");
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

            let is_static = self.match_token(&[TokenKind::Static]);

            if self.match_token(&[TokenKind::Final]) {
                if let Some(field) = self.variable_declaration(Mutability::Final, false, item_is_private, is_static) {
                    fields.push(field);
                }
            } else if let Some(Token { kind: TokenKind::Identifier(_), .. }) = self.peek()
                && let Some(Token { kind: next_kind, .. }) = self.peek_next()
                && (matches!(next_kind, TokenKind::Colon) || matches!(next_kind, TokenKind::ColonEqual))
            {
                if let Some(field) = self.variable_declaration(Mutability::Mutable, false, item_is_private, is_static) {
                    fields.push(field);
                }

            } else if self.match_token(&[TokenKind::Weak]) {
                if is_static {
                    self.error_at_current("Static properties cannot be weak.");
                }
                if self.match_token(&[TokenKind::Final]) {
                    if let Some(field) = self.variable_declaration(Mutability::Final, true, item_is_private, is_static) {
                        fields.push(field);
                    }
                } else if let Some(Token { kind: TokenKind::Identifier(_), .. }) = self.peek()
                    && let Some(Token { kind: next_kind, .. }) = self.peek_next()
                    && (matches!(next_kind, TokenKind::Colon) || matches!(next_kind, TokenKind::ColonEqual))
                {
                    if let Some(field) = self.variable_declaration(Mutability::Mutable, true, item_is_private, is_static) {
                        fields.push(field);
                    }
                } else {
                    self.error_at_current("Expected variable declaration after 'weak'.");
                }
            } else if self.match_token(&[TokenKind::Func]) {
                if self.match_token(&[TokenKind::Init]) {
                    if is_static {
                        self.error_at_current("Constructors cannot be static.");
                    }
                    if let Some(init_method) = self.init_declaration(item_is_private) {
                        methods.push(init_method);
                    }
                } else {
                    if let Some(method) = self.function_declaration(item_is_private, false, is_static) {
                        methods.push(method);
                    }
                }
            } else if self.match_token(&[TokenKind::Async]) {
                if self.match_token(&[TokenKind::Func]) {
                    if let Some(method) = self.function_declaration(item_is_private, true, is_static) {
                        methods.push(method);
                    }
                } else {
                    self.error_at_current("Expected 'func' after 'async'.");
                }
            } else if self.match_token(&[TokenKind::Init]) {
                if is_static {
                    self.error_at_current("Constructors cannot be static.");
                }
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

    pub(crate) fn actor_declaration(&mut self, is_private: bool) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected actor name.");
            return None;
        };

        let type_params = self.parse_type_params()?;

        let mut implements = Vec::new();
        if self.match_token(&[TokenKind::Implements]) {
            loop {
                if let Some(ty) = self.parse_type_expr() {
                    implements.push(ty);
                } else {
                    self.error_at_current("Expected interface name or type.");
                    return None;
                }

                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before actor body.");
            return None;
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let item_is_private = self.parse_visibility();

            let is_static = self.match_token(&[TokenKind::Static]);

            if self.match_token(&[TokenKind::Final]) {
                if let Some(field) = self.variable_declaration(Mutability::Final, false, item_is_private, is_static) {
                    fields.push(field);
                }
            } else if let Some(Token { kind: TokenKind::Identifier(_), .. }) = self.peek()
                && let Some(Token { kind: next_kind, .. }) = self.peek_next()
                && (matches!(next_kind, TokenKind::Colon) || matches!(next_kind, TokenKind::ColonEqual))
            {
                if let Some(field) = self.variable_declaration(Mutability::Mutable, false, item_is_private, is_static) {
                    fields.push(field);
                }

            } else if self.match_token(&[TokenKind::Weak]) {
                if is_static {
                    self.error_at_current("Static properties cannot be weak.");
                }
                if self.match_token(&[TokenKind::Final]) {
                    if let Some(field) = self.variable_declaration(Mutability::Final, true, item_is_private, is_static) {
                        fields.push(field);
                    }
                } else if let Some(Token { kind: TokenKind::Identifier(_), .. }) = self.peek()
                    && let Some(Token { kind: next_kind, .. }) = self.peek_next()
                    && (matches!(next_kind, TokenKind::Colon) || matches!(next_kind, TokenKind::ColonEqual))
                {
                    if let Some(field) = self.variable_declaration(Mutability::Mutable, true, item_is_private, is_static) {
                        fields.push(field);
                    }
                } else {
                    self.error_at_current("Expected variable declaration after 'weak'.");
                }
            } else if self.match_token(&[TokenKind::Func]) {
                if self.match_token(&[TokenKind::Init]) {
                    if is_static {
                        self.error_at_current("Constructors cannot be static.");
                    }
                    if let Some(init_method) = self.init_declaration(item_is_private) {
                        methods.push(init_method);
                    }
                } else {
                    if let Some(method) = self.function_declaration(item_is_private, false, is_static) {
                        methods.push(method);
                    }
                }
            } else if self.match_token(&[TokenKind::Async]) {
                if self.match_token(&[TokenKind::Func]) {
                    if let Some(method) = self.function_declaration(item_is_private, true, is_static) {
                        methods.push(method);
                    }
                } else {
                    self.error_at_current("Expected 'func' after 'async'.");
                }
            } else if self.match_token(&[TokenKind::Init]) {
                if is_static {
                    self.error_at_current("Constructors cannot be static.");
                }
                self.error_at_current("Constructors must be declared with 'func init'.");
                if let Some(init_method) = self.init_declaration(false) {
                    methods.push(init_method);
                }
            } else {
                self.error_at_current("Expected property or method inside actor.");
                self.advance();
            }
        }

        if !self.match_token(&[TokenKind::RightBrace]) {
            self.error_at_current("Expected '}' after actor body.");
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
            StmtKind::Actor {
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
}
