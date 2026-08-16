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
}
