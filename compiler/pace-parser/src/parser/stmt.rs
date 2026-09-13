use pace_ast::{Block, Ident, Stmt};
use pace_errors::Diagnostic;
use pace_lexer::TokenKind;

use super::Parser;

impl<'a> Parser<'a> {
    pub(crate) fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let start_tok = self.expect(TokenKind::LBrace).map_err(|_| {
            Diagnostic::error(format!("Expected LBrace (found {:?})", self.current))
                .with_span(self.current_span())
        })?;
        let mut statements = Vec::new();

        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            statements.push(self.parse_stmt()?);
        }

        let end_tok = self.expect(TokenKind::RBrace)?;

        Ok(Block {
            statements,
            span: start_tok.span.merge(end_tok.span),
        })
    }

    pub(crate) fn parse_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        if self.check(&TokenKind::Let) {
            let start_tok = self.expect(TokenKind::Let)?;
            let name_tok = self.current.take().ok_or_else(|| {
                Diagnostic::error("Expected identifier after 'let'").with_span(self.current_span())
            })?;
            let name = match name_tok.kind {
                TokenKind::Ident(n) => Ident {
                    name: n.to_string(),
                    span: name_tok.span,
                },
                _ => {
                    return Err(Diagnostic::error("Expected identifier after 'let'")
                        .with_span(self.current_span()));
                }
            };
            self.advance();
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

            let span = start_tok.span.merge(span_end);
            Ok(Stmt::Let {
                name,
                ty,
                value,
                span,
            })
        } else if self.check(&TokenKind::Var) {
            let start_tok = self.expect(TokenKind::Var)?;
            let name_tok = self.current.take().ok_or_else(|| {
                Diagnostic::error("Expected identifier after 'var'").with_span(self.current_span())
            })?;
            let name = match name_tok.kind {
                TokenKind::Ident(n) => Ident {
                    name: n.to_string(),
                    span: name_tok.span,
                },
                _ => {
                    return Err(Diagnostic::error("Expected identifier after 'var'")
                        .with_span(self.current_span()));
                }
            };
            self.advance();
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

            let span = start_tok.span.merge(span_end);
            Ok(Stmt::Var {
                name,
                ty,
                value,
                span,
            })
        } else if self.check(&TokenKind::Return) {
            let start_tok = self.expect(TokenKind::Return)?;

            let mut expr = None;
            let mut end_span = start_tok.span;

            if !self.check(&TokenKind::RBrace) {
                let e = self.parse_expr()?;
                end_span = end_span.merge(e.span());
                expr = Some(e);
            }

            Ok(Stmt::Return(expr, start_tok.span.merge(end_span)))
        } else {
            let expr = self.parse_expr()?;
            let span = expr.span();
            Ok(Stmt::ExprStmt(expr, span))
        }
    }
}
