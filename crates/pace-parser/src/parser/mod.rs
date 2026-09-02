use crate::lexer::{Lexer, Token};
use pace_ast::arena::AstArena;

pub mod decl;
pub mod expr;
pub mod stmt;

pub struct Parser<'a, 'b> {
    pub lexer: Lexer<'a>,
    pub current_token: Token<'a>,
    pub current_span: pace_span::Span,
    pub errors: Vec<pace_errors::SyntaxError>,
    pub file_name: String,
    pub src: &'a str,
    pub arena: &'b mut AstArena,
}

impl<'a, 'b> Parser<'a, 'b> {
    pub fn new_with_arena(src: &'a str, file_name: &str, arena: &'b mut AstArena) -> Self {
        let mut lexer = Lexer::new(src);
        let (current_token, current_span) = lexer.next_token();
        Self {
            file_name: file_name.to_string(),
            src,
            lexer,
            current_token,
            current_span: current_span.into(),
            errors: Vec::new(),
            arena,
        }
    }

    pub fn alloc_expr(&mut self, expr: pace_ast::Expr) -> pace_ast::arena::ExprId {
        self.arena.alloc_expr(expr)
    }

    pub fn alloc_stmt(&mut self, stmt: pace_ast::Stmt) -> pace_ast::arena::StmtId {
        self.arena.alloc_stmt(stmt)
    }

    fn advance(&mut self) {
        let (tok, span) = self.lexer.next_token();
        self.current_token = tok;
        self.current_span = span.into();
    }

    fn match_token(&mut self, expected: Token<'a>) -> bool {
        if self.current_token == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn parse(&mut self) -> Result<Vec<pace_ast::arena::StmtId>, Vec<pace_errors::SyntaxError>> {
        let mut stmts = Vec::new();
        while self.current_token != Token::Eof {
            match self.parse_stmt() {
                Ok(stmt) => stmts.push(stmt),
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }

        if self.errors.is_empty() {
            Ok(stmts)
        } else {
            Err(self.errors.clone())
        }
    }

    fn synchronize(&mut self) {
        self.advance();
        while self.current_token != Token::Eof {
            if self.current_token == Token::Semi {
                self.advance();
                return;
            }

            match self.current_token {
                Token::Class
                | Token::Actor
                | Token::Func
                | Token::Var
                | Token::Let
                | Token::For
                | Token::If
                | Token::While
                | Token::Return
                | Token::RBrace => return,
                _ => self.advance(),
            }
        }
    }
}
