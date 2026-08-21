use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn unary(&mut self) -> Option<Expr<'a>> {
        if self.match_token(&[TokenKind::Minus, TokenKind::Bang, TokenKind::Tilde]) {
            let operator = match self.previous().kind {
                TokenKind::Minus => UnaryOp::Negate,
                TokenKind::Bang => UnaryOp::Not,
                TokenKind::Tilde => UnaryOp::BitwiseNot,
                _ => unreachable!(),
            };
            let start_span = self.previous().span;
            let right = self.unary()?;
            let span = Span::new(
                start_span.file_id,
                start_span.start,
                right.span.end,
                start_span.start_loc,
                right.span.end_loc,
            );
            return Some(Expr::new(
                ExprKind::Unary(operator, self.session.ast_arena.alloc(right)),
                span,
            ));
        }
        
        if self.match_token(&[TokenKind::Await]) {
            let start_span = self.previous().span;
            let right = self.unary()?;
            let span = Span::new(
                start_span.file_id,
                start_span.start,
                right.span.end,
                start_span.start_loc,
                right.span.end_loc,
            );
            return Some(Expr::new(
                ExprKind::Await(self.session.ast_arena.alloc(right)),
                span,
            ));
        }
        
        if self.match_token(&[TokenKind::Spawn]) {
            let start_span = self.previous().span;
            let right = self.unary()?;
            let span = Span::new(
                start_span.file_id,
                start_span.start,
                right.span.end,
                start_span.start_loc,
                right.span.end_loc,
            );
            return Some(Expr::new(
                ExprKind::Spawn(self.session.ast_arena.alloc(right)),
                span,
            ));
        }

        self.call()
    }
}
