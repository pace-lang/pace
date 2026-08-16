use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn variable_declaration(
        &mut self,
        is_var: bool,
        is_weak: bool,
        is_private: bool,
    ) -> Option<Stmt<'a>> {
        let start_span = self.previous().span;

        let name = if let Some(Token {
            kind: TokenKind::Identifier(n),
            ..
        }) = self.peek().cloned()
        {
            self.advance();
            n
        } else {
            self.error_at_current("Expected variable name.");
            return None;
        };

        let type_annotation = if self.match_token(&[TokenKind::Colon]) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let initializer = if self.match_token(&[TokenKind::Equal]) {
            Some(self.expression()?)
        } else {
            None
        };

        self.match_token(&[TokenKind::Semicolon]);

        let end_span = self.previous().span;
        let span = Span::new(
            start_span.file_id,
            start_span.start,
            end_span.end,
            start_span.start_loc,
            end_span.end_loc,
        );

        let kind = if is_var {
            StmtKind::Var {
                name,
                type_annotation,
                initializer: initializer.map(|e| &*self.session.ast_arena.alloc(e)),
                is_weak,
                is_private,
            }
        } else {
            StmtKind::Let {
                name,
                type_annotation,
                initializer: initializer.map(|e| &*self.session.ast_arena.alloc(e)),
                is_private,
            }
        };

        Some(Stmt::new(kind, span))
    }
}
