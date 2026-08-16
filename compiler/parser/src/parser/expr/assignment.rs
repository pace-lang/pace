use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn assignment(&mut self) -> Option<Expr<'a>> {
        let expr = self.null_coalesce()?;

        if self.match_token(&[TokenKind::Equal, TokenKind::QuestionQuestionEqual]) {
            let op = self.previous().clone();
            let value = self.assignment()?;

            if op.kind == TokenKind::QuestionQuestionEqual {
                match expr.kind {
                    ExprKind::Variable(_) | ExprKind::Get { .. } | ExprKind::IndexGet { .. } => {
                        let span = Span::new(
                            expr.span.file_id,
                            expr.span.start,
                            value.span.end,
                            expr.span.start_loc,
                            value.span.end_loc,
                        );
                        return Some(Expr::new(
                            ExprKind::NullCoalesceAssign {
                                left: self.session.ast_arena.alloc(expr),
                                right: self.session.ast_arena.alloc(value),
                            },
                            span,
                        ));
                    }
                    _ => {
                        self.error_at_current("Invalid assignment target for ??=.");
                    }
                }
            } else {
                match expr.kind {
                    ExprKind::Variable(name) => {
                        let span = Span::new(
                            expr.span.file_id,
                            expr.span.start,
                            value.span.end,
                            expr.span.start_loc,
                            value.span.end_loc,
                        );
                        return Some(Expr::new(
                            ExprKind::Assign {
                                name,
                                value: self.session.ast_arena.alloc(value),
                            },
                            span,
                        ));
                    }
                    ExprKind::Get { object, name } => {
                        let span = Span::new(
                            expr.span.file_id,
                            expr.span.start,
                            value.span.end,
                            expr.span.start_loc,
                            value.span.end_loc,
                        );
                        return Some(Expr::new(
                            ExprKind::Set {
                                object,
                                name,
                                value: self.session.ast_arena.alloc(value),
                            },
                            span,
                        ));
                    }
                    ExprKind::IndexGet { object, index } => {
                        let span = Span::new(
                            expr.span.file_id,
                            expr.span.start,
                            value.span.end,
                            expr.span.start_loc,
                            value.span.end_loc,
                        );
                        return Some(Expr::new(
                            ExprKind::IndexSet {
                                object,
                                index,
                                value: self.session.ast_arena.alloc(value),
                            },
                            span,
                        ));
                    }
                    _ => {
                        self.error_at_current("Invalid assignment target.");
                    }
                }
            }
        }

        Some(expr)
    }
}
