use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn extension_declaration(&mut self) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let mut type_params = Vec::new();
        if self.match_token(&[TokenKind::Less]) {
            if !self.check(&TokenKind::Greater) {
                loop {
                    if let Some(Token {
                        kind: TokenKind::Identifier(n),
                        ..
                    }) = self.peek().cloned()
                    {
                        self.advance();
                        type_params.push(n);
                    } else {
                        self.error_at_current("Expected type parameter name.");
                        return None;
                    }

                    if !self.match_token(&[TokenKind::Comma]) {
                        break;
                    }
                }
            }
            if !self.match_token(&[TokenKind::Greater]) {
                self.error_at_current("Expected '>' after type parameters.");
                return None;
            }
        }

        let target_type = self.parse_type_expr()?;

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before extension body.");
            return None;
        }

        let mut methods = Vec::new();
        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let is_private = self.parse_visibility();
            if self.match_token(&[TokenKind::Func]) {
                if let Some(method) = self.function_declaration(is_private, false, false) {
                    methods.push(method);
                }
            } else if self.match_token(&[TokenKind::Async]) {
                if self.match_token(&[TokenKind::Func]) {
                    if let Some(method) = self.function_declaration(is_private, true, false) {
                        methods.push(method);
                    }
                } else {
                    self.error_at_current("Expected 'func' after 'async'.");
                }
            } else {
                self.error_at_current("Expected method declaration in extension body.");
                self.advance();
            }
        }

        if !self.match_token(&[TokenKind::RightBrace]) {
            self.error_at_current("Expected '}' after extension body.");
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
            StmtKind::Extension {
                target_type,
                type_params,
                methods,
            },
            span,
        ))
    }
}
