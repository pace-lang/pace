use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn assignment(&mut self) -> Option<Expr<'a>> {
        let expr = self.ternary()?;

        if self.match_token(&[
            TokenKind::Equal,
            TokenKind::QuestionQuestionEqual,
            TokenKind::PlusEqual,
            TokenKind::MinusEqual,
            TokenKind::StarEqual,
            TokenKind::SlashEqual,
            TokenKind::PercentEqual,
            TokenKind::AmpersandEqual,
            TokenKind::PipeEqual,
            TokenKind::CaretEqual,
            TokenKind::LessLessEqual,
            TokenKind::GreaterGreaterEqual,
        ]) {
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
            } else if op.kind != TokenKind::Equal {
                let operator = match op.kind {
                    TokenKind::PlusEqual => BinaryOp::Add,
                    TokenKind::MinusEqual => BinaryOp::Subtract,
                    TokenKind::StarEqual => BinaryOp::Multiply,
                    TokenKind::SlashEqual => BinaryOp::Divide,
                    TokenKind::PercentEqual => BinaryOp::Modulo,
                    TokenKind::AmpersandEqual => BinaryOp::BitwiseAnd,
                    TokenKind::PipeEqual => BinaryOp::BitwiseOr,
                    TokenKind::CaretEqual => BinaryOp::BitwiseXor,
                    TokenKind::LessLessEqual => BinaryOp::ShiftLeft,
                    TokenKind::GreaterGreaterEqual => BinaryOp::ShiftRight,
                    _ => unreachable!(),
                };
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
                            ExprKind::CompoundAssign {
                                target: self.session.ast_arena.alloc(expr),
                                operator,
                                value: self.session.ast_arena.alloc(value),
                            },
                            span,
                        ));
                    }
                    _ => {
                        self.error_at_current("Invalid assignment target for compound assignment.");
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

    pub(crate) fn ternary(&mut self) -> Option<Expr<'a>> {
        let mut expr = self.null_coalesce()?;

        if self.match_token(&[TokenKind::Question]) {
            let true_expr = self.expression()?;
            if !self.match_token(&[TokenKind::Colon]) {
                self.error_at_current("Expected ':' after true branch of ternary operator.");
            }
            let false_expr = self.ternary()?;

            let span = Span::new(
                expr.span.file_id,
                expr.span.start,
                false_expr.span.end,
                expr.span.start_loc,
                false_expr.span.end_loc,
            );

            expr = Expr::new(
                ExprKind::Ternary {
                    condition: self.session.ast_arena.alloc(expr),
                    true_expr: self.session.ast_arena.alloc(true_expr),
                    false_expr: self.session.ast_arena.alloc(false_expr),
                },
                span,
            );
        }

        Some(expr)
    }
}
