use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
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
}
