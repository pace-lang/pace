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
}
