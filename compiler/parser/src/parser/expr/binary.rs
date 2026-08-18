use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn null_coalesce(&mut self) -> Option<Expr<'a>> {
        let mut expr = self.range()?;

        while self.match_token(&[TokenKind::QuestionQuestion]) {
            let right = self.range()?;
            let span = Span::new(
                expr.span.file_id,
                expr.span.start,
                right.span.end,
                expr.span.start_loc,
                right.span.end_loc,
            );
            expr = Expr::new(
                ExprKind::NullCoalesce {
                    left: self.session.ast_arena.alloc(expr),
                    right: self.session.ast_arena.alloc(right),
                },
                span,
            );
        }

        Some(expr)
    }

    pub(crate) fn range(&mut self) -> Option<Expr<'a>> {
        let mut expr = self.equality()?;

        while self.match_token(&[TokenKind::DotDot]) {
            let right = self.equality()?;
            let span = Span::new(
                expr.span.file_id,
                expr.span.start,
                right.span.end,
                expr.span.start_loc,
                right.span.end_loc,
            );
            expr = Expr::new(
                ExprKind::Range {
                    start: self.session.ast_arena.alloc(expr),
                    end: self.session.ast_arena.alloc(right),
                },
                span,
            );
        }

        Some(expr)
    }

    pub(crate) fn equality(&mut self) -> Option<Expr<'a>> {
        let mut expr = self.comparison()?;

        while self.match_token(&[TokenKind::EqualEqual, TokenKind::BangEqual]) {
            let operator = match self.previous().kind {
                TokenKind::EqualEqual => BinaryOp::Equal,
                TokenKind::BangEqual => BinaryOp::NotEqual,
                _ => unreachable!(),
            };
            let right = self.comparison()?;
            let span = Span::new(
                expr.span.file_id,
                expr.span.start,
                right.span.end,
                expr.span.start_loc,
                right.span.end_loc,
            );
            expr = Expr::new(
                ExprKind::Binary(
                    self.session.ast_arena.alloc(expr),
                    operator,
                    self.session.ast_arena.alloc(right),
                ),
                span,
            );
        }

        Some(expr)
    }

    pub(crate) fn comparison(&mut self) -> Option<Expr<'a>> {
        let mut expr = self.term()?;

        while self.match_token(&[
            TokenKind::Greater,
            TokenKind::GreaterEqual,
            TokenKind::Less,
            TokenKind::LessEqual,
        ]) {
            let operator = match self.previous().kind {
                TokenKind::Greater => BinaryOp::Greater,
                TokenKind::GreaterEqual => BinaryOp::GreaterEqual,
                TokenKind::Less => BinaryOp::Less,
                TokenKind::LessEqual => BinaryOp::LessEqual,
                _ => unreachable!(),
            };
            let right = self.term()?;
            let span = Span::new(
                expr.span.file_id,
                expr.span.start,
                right.span.end,
                expr.span.start_loc,
                right.span.end_loc,
            );
            expr = Expr::new(
                ExprKind::Binary(
                    self.session.ast_arena.alloc(expr),
                    operator,
                    self.session.ast_arena.alloc(right),
                ),
                span,
            );
        }

        Some(expr)
    }

    pub(crate) fn term(&mut self) -> Option<Expr<'a>> {
        let mut expr = self.factor()?;

        while self.match_token(&[TokenKind::Plus, TokenKind::Minus]) {
            let operator = match self.previous().kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Subtract,
                _ => unreachable!(),
            };
            let right = self.factor()?;
            let span = Span::new(
                expr.span.file_id,
                expr.span.start,
                right.span.end,
                expr.span.start_loc,
                right.span.end_loc,
            );
            expr = Expr::new(
                ExprKind::Binary(
                    self.session.ast_arena.alloc(expr),
                    operator,
                    self.session.ast_arena.alloc(right),
                ),
                span,
            );
        }

        Some(expr)
    }

    pub(crate) fn factor(&mut self) -> Option<Expr<'a>> {
        let mut expr = self.unary()?;

        while self.match_token(&[TokenKind::Star, TokenKind::Slash, TokenKind::Percent]) {
            let operator = match self.previous().kind {
                TokenKind::Star => BinaryOp::Multiply,
                TokenKind::Slash => BinaryOp::Divide,
                TokenKind::Percent => BinaryOp::Modulo,
                _ => unreachable!(),
            };
            let right = self.unary()?;
            let span = Span::new(
                expr.span.file_id,
                expr.span.start,
                right.span.end,
                expr.span.start_loc,
                right.span.end_loc,
            );
            expr = Expr::new(
                ExprKind::Binary(
                    self.session.ast_arena.alloc(expr),
                    operator,
                    self.session.ast_arena.alloc(right),
                ),
                span,
            );
        }

        Some(expr)
    }
}
