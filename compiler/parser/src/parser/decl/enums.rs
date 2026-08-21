use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
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
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let is_static = self.match_token(&[TokenKind::Static]);
            let is_method = self.check(&TokenKind::Func) || self.check(&TokenKind::Private) || self.check(&TokenKind::Async) || is_static;
            if is_method {
                // If we consumed `static`, we need to handle the method parsing since `declaration()` doesn't expect `static` at the top level
                // Actually, `declaration()` doesn't handle `static` at all right now, but for enums, methods are parsed just like top-level declarations
                // Wait, if we use `self.declaration()`, it will fail because `static` isn't handled in `decl.rs`.
                // Let's parse it inline like in classes/structs instead of using `self.declaration()`.
                // However, `self.declaration()` also handles variables and things. 
                // Wait, enums only have methods, not fields (besides variants).
                if self.match_token(&[TokenKind::Func]) {
                    if let Some(method) = self.function_declaration(false, false, is_static) {
                        methods.push(method);
                    }
                } else if self.match_token(&[TokenKind::Async]) {
                    if self.match_token(&[TokenKind::Func]) {
                        if let Some(method) = self.function_declaration(false, true, is_static) {
                            methods.push(method);
                        }
                    } else {
                        self.error_at_current("Expected 'func' after 'async'.");
                    }
                } else if is_static {
                    self.error_at_current("Expected 'func' or 'async func' after 'static'.");
                } else {
                     if let Some(method) = self.declaration() {
                         methods.push(method);
                     }
                }
                continue;
            }

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
                self.error_at_current("Expected enum variant name or method declaration.");
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
                methods,
                is_private,
            },
            span,
        ))
    }
}
