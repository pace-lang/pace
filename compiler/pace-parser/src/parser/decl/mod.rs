mod annotations;
mod types;

use pace_ast::{Annotation, Decl, Ident};
use pace_errors::Diagnostic;
use pace_lexer::{Token, TokenKind};

use super::Parser;

impl<'a> Parser<'a> {
    pub(super) fn attach_annotations(decl: &mut Decl, annots: Vec<Annotation>) {
        match decl {
            Decl::Function { annotations, .. }
            | Decl::Struct { annotations, .. }
            | Decl::Class { annotations, .. }
            | Decl::Actor { annotations, .. }
            | Decl::Trait { annotations, .. }
            | Decl::Enum { annotations, .. } => {
                *annotations = annots;
            }
            _ => {}
        }
    }

    pub(crate) fn parse_decl(&mut self) -> Result<Decl, Diagnostic> {
        let annotations = self.parse_annotations()?;
        let mut is_private = false;
        let mut start_span = self.current_span();
        if self.check(&TokenKind::Private) {
            let tok = self.expect(TokenKind::Private)?;
            start_span = tok.span;
            is_private = true;
        }

        if self.check(&TokenKind::Import) {
            let start_tok = self.expect(TokenKind::Import)?;
            let mut path = Vec::new();

            loop {
                match &self.current {
                    Some(Token {
                        kind: TokenKind::Ident(name),
                        span,
                    }) => {
                        path.push(Ident {
                            name: pace_span::intern(name),
                            span: *span,
                        });
                        self.advance();
                    }
                    _ => {
                        return Err(Diagnostic::error("Expected identifier in import path")
                            .with_span(self.current_span()));
                    }
                }

                if self.check(&TokenKind::Dot) {
                    self.advance();
                } else {
                    break;
                }
            }

            let alias = if self.check(&TokenKind::As) {
                self.advance();
                match &self.current {
                    Some(Token {
                        kind: TokenKind::Ident(name),
                        span,
                    }) => {
                        let ident = Ident {
                            name: pace_span::intern(name),
                            span: *span,
                        };
                        self.advance();
                        Some(ident)
                    }
                    _ => {
                        return Err(Diagnostic::error("Expected identifier after 'as'")
                            .with_span(self.current_span()));
                    }
                }
            } else {
                None
            };

            let end_span = alias
                .as_ref()
                .map(|a| a.span)
                .unwrap_or_else(|| path.last().unwrap().span);

            Ok(Decl::Import {
                path,
                alias,
                span: start_tok.span.merge(end_span),
            })
        } else if self.check(&TokenKind::Let) {
            let start_tok = self.expect(TokenKind::Let)?;

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
                    return Err(Diagnostic::error("Expected identifier after 'let'")
                        .with_span(self.current_span()));
                }
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
                    return Err(Diagnostic::error(
                        "Type annotation is required when an initializer is omitted",
                    )
                    .with_span(self.current_span()));
                }
                start_tok.span
            };

            Ok(Decl::Let {
                name: name_tok,
                ty,
                value,
                is_private,
                span: if is_private {
                    start_span.merge(span_end)
                } else {
                    start_tok.span.merge(span_end)
                },
            })
        } else if self.check(&TokenKind::Var) {
            let start_tok = self.expect(TokenKind::Var)?;

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
                    return Err(Diagnostic::error("Expected identifier after 'var'")
                        .with_span(self.current_span()));
                }
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
                    return Err(Diagnostic::error(
                        "Type annotation is required when an initializer is omitted",
                    )
                    .with_span(self.current_span()));
                }
                start_tok.span
            };

            Ok(Decl::Var {
                name: name_tok,
                ty,
                value,
                is_private,
                span: if is_private {
                    start_span.merge(span_end)
                } else {
                    start_tok.span.merge(span_end)
                },
            })
        } else if self.check(&TokenKind::Const) {
            let start_tok = self.expect(TokenKind::Const)?;

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
                    return Err(Diagnostic::error("Expected identifier after 'const'")
                        .with_span(self.current_span()));
                }
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
                is_private,
                span: if is_private {
                    start_span.merge(end_span)
                } else {
                    start_tok.span.merge(end_span)
                },
            })
        } else if self.check(&TokenKind::Async) {
            self.advance();
            let mut decl = self.parse_fn_decl(is_private, true, start_span)?;
            Self::attach_annotations(&mut decl, annotations);
            Ok(decl)
        } else if self.check(&TokenKind::Fn) {
            let mut decl = self.parse_fn_decl(is_private, false, start_span)?;
            Self::attach_annotations(&mut decl, annotations);
            Ok(decl)
        } else if self.check(&TokenKind::Struct) {
            let mut decl = self.parse_struct_decl(is_private, start_span)?;
            Self::attach_annotations(&mut decl, annotations);
            Ok(decl)
        } else if self.check(&TokenKind::Class) {
            let mut decl = self.parse_class_decl(is_private, start_span)?;
            Self::attach_annotations(&mut decl, annotations);
            Ok(decl)
        } else if self.check(&TokenKind::Actor) {
            let mut decl = self.parse_actor_decl(is_private, start_span)?;
            Self::attach_annotations(&mut decl, annotations);
            Ok(decl)
        } else if self.check(&TokenKind::Trait) {
            let mut decl = self.parse_trait_decl(is_private, start_span)?;
            Self::attach_annotations(&mut decl, annotations);
            Ok(decl)
        } else if self.check(&TokenKind::Enum) {
            let mut decl = self.parse_enum_decl(is_private, start_span)?;
            Self::attach_annotations(&mut decl, annotations);
            Ok(decl)
        } else if self.check(&TokenKind::Try) || self.check(&TokenKind::Catch) {
            let span = self.current_span();
            self.advance();
            Err(Diagnostic::error(
                "Traditional 'try/catch' exceptions are not supported in Pace. Pace uses 'Result<T, E>' and '?' for explicit error handling.",
            ).with_span(span))
        } else if self.check(&TokenKind::Throw) {
            let span = self.current_span();
            self.advance();
            Err(Diagnostic::error(
                "'throw' exceptions are not supported in Pace. Return an error value or use 'Result<T, E>' for explicit error handling.",
            ).with_span(span))
        } else {
            let expr = self.parse_expr()?;
            let span = expr.span();
            Ok(Decl::Expr(expr, span))
        }
    }
    pub(crate) fn parse_fn_decl(
        &mut self,
        is_private: bool,
        is_async: bool,
        start_span: pace_span::Span,
    ) -> Result<Decl, Diagnostic> {
        let start_tok = self.expect(TokenKind::Fn)?;

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
                return Err(Diagnostic::error("Expected identifier after 'fn'")
                    .with_span(self.current_span()));
            }
        };

        let generic_params = self.parse_generic_params()?;

        self.expect(TokenKind::LParen)?;

        let mut params = Vec::new();
        while !self.check(&TokenKind::RParen) {
            let param_name = match &self.current {
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
                        Diagnostic::error("Expected parameter name").with_span(self.current_span())
                    );
                }
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

        let body = if self.check(&TokenKind::LBrace) {
            self.parse_block()?
        } else {
            pace_ast::Block {
                statements: vec![],
                span: self.current_span(),
            }
        };
        let end_span = body.span;

        Ok(Decl::Function {
            name: name_tok,
            generic_params,
            params,
            return_type,
            body,
            is_static: false,
            is_private,
            is_async,
            annotations: Vec::new(),
            span: if is_private {
                start_span.merge(end_span)
            } else {
                start_tok.span.merge(end_span)
            },
        })
    }
}
