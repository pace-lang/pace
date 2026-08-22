use super::super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn variable_declaration(
        &mut self,
        mutability: Mutability,
        is_weak: bool,
        is_private: bool,
        is_static: bool,
    ) -> Option<Stmt<'a>> {
        let mut start_span = self.previous().span;
        // If there are no modifiers for this mutable binding, the previous token is from the last statement!
        if mutability == Mutability::Mutable && !is_weak && !is_private && !is_static {
            if let Some(tok) = self.peek() {
                start_span = tok.span;
            }
        }

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

        let mut type_annotation = None;
        let mut initializer = None;

        if self.match_token(&[TokenKind::ColonEqual]) {
            initializer = Some(self.expression()?);
        } else if self.match_token(&[TokenKind::Colon]) {
            type_annotation = Some(self.parse_type_expr()?);
            if self.match_token(&[TokenKind::Equal]) {
                initializer = Some(self.expression()?);
            }
        } else if self.match_token(&[TokenKind::Equal]) {
            initializer = Some(self.expression()?);
        }

        if mutability == Mutability::Const && initializer.is_none() {
            self.error_at_current("Constants must be initialized at declaration.");
        }

        self.match_token(&[TokenKind::Semicolon]);

        let end_span = self.previous().span;
        let span = Span::new(
            start_span.file_id,
            start_span.start,
            end_span.end,
            start_span.start_loc,
            end_span.end_loc,
        );

        let kind = StmtKind::Binding {
            name,
            type_annotation,
            initializer: initializer.map(|e| &*self.session.ast_arena.alloc(e)),
            mutability,
            is_weak,
            is_private,
            is_static,
        };

        Some(Stmt::new(kind, span))
    }
}
