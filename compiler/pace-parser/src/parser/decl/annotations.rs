use pace_ast::{Annotation, CallArg, GenericParam, Ident, Type};
use pace_errors::Diagnostic;
use pace_lexer::{Token, TokenKind};

use super::super::Parser;

impl<'a> Parser<'a> {
    pub(crate) fn parse_annotations(&mut self) -> Result<Vec<Annotation>, Diagnostic> {
        let mut annotations = Vec::new();
        while self.check(&TokenKind::At) {
            let at_tok = self.expect(TokenKind::At)?;
            let name = match &self.current {
                Some(Token {
                    kind: TokenKind::Ident(n),
                    span,
                }) => {
                    let ident = Ident {
                        name: pace_span::intern(n),
                        span: *span,
                    };
                    self.advance();
                    ident
                }
                _ => {
                    return Err(Diagnostic::error("Expected annotation name after '@'")
                        .with_span(self.current_span()));
                }
            };
            let mut end_span = name.span;
            let args = if self.check(&TokenKind::LParen) {
                self.advance();
                let mut call_args = Vec::new();
                while !self.check(&TokenKind::RParen) && self.current.is_some() {
                    let arg_start = self.current_span();
                    let label = if let Some(Token {
                        kind: TokenKind::Ident(lbl),
                        span: lbl_span,
                    }) = self.current.clone()
                    {
                        let saved_lexer = self.lexer.clone();
                        let saved_current = self.current.clone();
                        self.advance();
                        if self.check(&TokenKind::Colon) {
                            self.advance();
                            Some(Ident {
                                name: pace_span::intern(&lbl),
                                span: lbl_span,
                            })
                        } else {
                            self.lexer = saved_lexer;
                            self.current = saved_current;
                            None
                        }
                    } else {
                        None
                    };
                    let expr = self.parse_expr()?;
                    let _ = arg_start.merge(expr.span());
                    call_args.push(CallArg { label, expr });
                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                let rparen = self.expect(TokenKind::RParen)?;
                end_span = rparen.span;
                Some(call_args)
            } else {
                None
            };
            annotations.push(Annotation {
                name,
                args,
                span: at_tok.span.merge(end_span),
            });
        }
        Ok(annotations)
    }

    pub(crate) fn parse_generic_params(&mut self) -> Result<Option<Vec<GenericParam>>, Diagnostic> {
        if self.check(&TokenKind::Lt) {
            self.advance();
            let mut params = Vec::new();
            while !self.check(&TokenKind::Gt) && self.current.is_some() {
                if let Some(Token {
                    kind: TokenKind::Ident(name),
                    span,
                }) = &self.current
                {
                    let param_name = Ident {
                        name: pace_span::intern(name),
                        span: *span,
                    };
                    self.advance();

                    let mut trait_bounds = Vec::new();
                    if self.check(&TokenKind::Colon) {
                        self.advance();
                        loop {
                            trait_bounds.push(self.parse_type()?);
                            if self.check(&TokenKind::Plus) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }

                    let mut default = None;
                    if self.check(&TokenKind::Eq) {
                        self.advance();
                        default = Some(self.parse_type()?);
                    }

                    params.push(GenericParam {
                        name: param_name,
                        trait_bounds,
                        default,
                    });

                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    }
                } else {
                    return Err(Diagnostic::error("Expected type parameter identifier")
                        .with_span(self.current_span()));
                }
            }
            self.expect(TokenKind::Gt)?;
            Ok(Some(params))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn parse_generic_args(&mut self) -> Result<Option<Vec<Type>>, Diagnostic> {
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

    pub(crate) fn parse_with_clause(&mut self) -> Result<Vec<Ident>, Diagnostic> {
        let mut traits = Vec::new();
        if self.check(&TokenKind::With) {
            self.advance();
            loop {
                if let Some(Token {
                    kind: TokenKind::Ident(name),
                    span,
                }) = &self.current
                {
                    traits.push(Ident {
                        name: pace_span::intern(name),
                        span: *span,
                    });
                    self.advance();
                } else {
                    return Err(Diagnostic::error("Expected trait name after 'with'")
                        .with_span(self.current_span()));
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

}
