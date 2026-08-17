use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
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

        if self.match_token(&[TokenKind::LeftParen]) {
            let mut param_types = Vec::new();
            if !self.check(&TokenKind::RightParen) {
                loop {
                    let ty = self.parse_type_expr()?;
                    param_types.push(ty);
                    if !self.match_token(&[TokenKind::Comma]) {
                        break;
                    }
                }
            }
            if !self.match_token(&[TokenKind::RightParen]) {
                self.error_at_current("Expected ')' after function parameter types.");
                return None;
            }
            if !self.match_token(&[TokenKind::Arrow]) {
                self.error_at_current("Expected '->' for function type.");
                return None;
            }
            let return_type = self.parse_type_expr()?;
            
            let mut ty = TypeExpr::Function(
                param_types,
                Some(self.session.ast_arena.alloc(return_type)),
            );
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
}
