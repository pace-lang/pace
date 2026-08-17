use super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn statement(&mut self) -> Option<Stmt<'a>> {
        if self.match_token(&[TokenKind::If]) {
            self.if_statement()
        } else if self.match_token(&[TokenKind::While]) {
            self.while_statement()
        } else if self.match_token(&[TokenKind::For]) {
            self.for_statement()
        } else if self.match_token(&[TokenKind::Return]) {
            self.return_statement()
        } else if self.match_token(&[TokenKind::LeftBrace]) {
            self.block()
        } else {
            self.expression_statement()
        }
    }

    pub(crate) fn if_statement(&mut self) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let condition = self.expression()?;

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' after if condition.");
            return None;
        }

        let then_branch = self.block()?;

        let mut else_branch = None;
        let mut end_span = then_branch.span;

        if self.match_token(&[TokenKind::Else]) {
            if self.match_token(&[TokenKind::If]) {
                let e_branch = self.if_statement()?;
                end_span = e_branch.span;
                else_branch = Some(self.session.ast_arena.alloc(e_branch));
            } else if self.match_token(&[TokenKind::LeftBrace]) {
                let e_branch = self.block()?;
                end_span = e_branch.span;
                else_branch = Some(self.session.ast_arena.alloc(e_branch));
            } else {
                self.error_at_current("Expected '{' or 'if' after else.");
                return None;
            }
        }

        let span = Span::new(
            start_span.file_id,
            start_span.start,
            end_span.end,
            start_span.start_loc,
            end_span.end_loc,
        );
        Some(Stmt::new(
            StmtKind::If {
                condition: self.session.ast_arena.alloc(condition),
                then_branch: self.session.ast_arena.alloc(then_branch),
                else_branch: else_branch.map(|e| &*e),
            },
            span,
        ))
    }

    pub(crate) fn while_statement(&mut self) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let condition = self.expression()?;

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' after while condition.");
            return None;
        }

        let body = self.block()?;
        let end_span = body.span;

        let span = Span::new(
            start_span.file_id,
            start_span.start,
            end_span.end,
            start_span.start_loc,
            end_span.end_loc,
        );
        Some(Stmt::new(
            StmtKind::While {
                condition: self.session.ast_arena.alloc(condition),
                body: self.session.ast_arena.alloc(body),
            },
            span,
        ))
    }

    pub(crate) fn for_statement(&mut self) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let item_name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected item name after 'for'.");
            return None;
        };

        let is_in = if let Some(Token {
            kind: TokenKind::Identifier(s),
            ..
        }) = self.peek()
        {
            self.session.interner.borrow().lookup(*s) == "in"
        } else {
            false
        };

        if is_in {
            self.advance();
        } else {
            self.error_at_current("Expected 'in' after for item name.");
            return None;
        }

        let iterator = self.expression()?;

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' after for iterator.");
            return None;
        }

        let body = self.block()?;
        let end_span = body.span;

        let span = Span::new(
            start_span.file_id,
            start_span.start,
            end_span.end,
            start_span.start_loc,
            end_span.end_loc,
        );
        Some(Stmt::new(
            StmtKind::For {
                item_name,
                iterator: self.session.ast_arena.alloc(iterator),
                body: self.session.ast_arena.alloc(body),
            },
            span,
        ))
    }

    pub(crate) fn return_statement(&mut self) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let value = if !self.check(&TokenKind::Semicolon) {
            Some(self.expression()?)
        } else {
            None
        };

        if !self.match_token(&[TokenKind::Semicolon]) {
            self.error_at_current("Expected ';' after return value.");
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
            StmtKind::Return {
                value: value.map(|e| &*self.session.ast_arena.alloc(e)),
            },
            span,
        ))
    }

    pub(crate) fn block(&mut self) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;
        let mut statements = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            if let Some(stmt) = self.declaration() {
                statements.push(stmt);
            }
        }

        if !self.match_token(&[TokenKind::RightBrace]) {
            self.error_at_current("Expected '}' after block.");
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
        Some(Stmt::new(StmtKind::Block(statements), span))
    }

    pub(crate) fn expression_statement(&mut self) -> Option<Stmt<'a>> {
        let expr = self.expression()?;

        self.match_token(&[TokenKind::Semicolon]);

        let span = expr.span;
        Some(Stmt::new(
            StmtKind::Expression(self.session.ast_arena.alloc(expr)),
            span,
        ))
    }
}
