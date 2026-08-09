use ast::{Expr, ExprKind, Stmt, StmtKind, Span, BinaryOp, UnaryOp};
use lexer::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    errors: Vec<String>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            current: 0,
            errors: Vec::new(),
        }
    }

    pub fn parse(&mut self) -> (Vec<Stmt>, Vec<String>) {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            if let Some(stmt) = self.declaration() {
                statements.push(stmt);
            }
        }
        (statements, self.errors.clone())
    }

    fn declaration(&mut self) -> Option<Stmt> {
        let res = if self.match_token(&[TokenKind::Let]) {
            self.let_declaration(false)
        } else if self.match_token(&[TokenKind::Var]) {
            self.let_declaration(true)
        } else {
            self.statement()
        };

        if res.is_none() {
            self.synchronize();
        }
        res
    }

    fn let_declaration(&mut self, is_var: bool) -> Option<Stmt> {
        let start_span = self.previous().span;

        let name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
            self.advance();
            n
        } else {
            self.error_at_current("Expected variable name.");
            return None;
        };

        if !self.match_token(&[TokenKind::Equal]) {
            self.error_at_current("Expected '=' after variable name.");
            return None;
        }

        let initializer = self.expression()?;

        if !self.match_token(&[TokenKind::Semicolon]) {
            self.error_at_current("Expected ';' after variable declaration.");
            return None;
        }

        let end_span = self.previous().span;
        let span = Span::new(start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);

        let kind = if is_var {
            StmtKind::Var { name, initializer }
        } else {
            StmtKind::Let { name, initializer }
        };

        Some(Stmt::new(kind, span))
    }

    fn statement(&mut self) -> Option<Stmt> {
        self.expression_statement()
    }

    fn expression_statement(&mut self) -> Option<Stmt> {
        let expr = self.expression()?;

        if !self.match_token(&[TokenKind::Semicolon]) {
            self.error_at_current("Expected ';' after expression.");
            return None;
        }

        let span = expr.span;
        Some(Stmt::new(StmtKind::Expression(expr), span))
    }

    fn expression(&mut self) -> Option<Expr> {
        self.equality()
    }

    fn equality(&mut self) -> Option<Expr> {
        let mut expr = self.comparison()?;

        while self.match_token(&[TokenKind::EqualEqual, TokenKind::BangEqual]) {
            let operator = match self.previous().kind {
                TokenKind::EqualEqual => BinaryOp::Equal,
                TokenKind::BangEqual => BinaryOp::NotEqual,
                _ => unreachable!(),
            };
            let right = self.comparison()?;
            let span = Span::new(expr.span.start, right.span.end, expr.span.start_loc, right.span.end_loc);
            expr = Expr::new(ExprKind::Binary(Box::new(expr), operator, Box::new(right)), span);
        }

        Some(expr)
    }

    fn comparison(&mut self) -> Option<Expr> {
        let mut expr = self.term()?;

        while self.match_token(&[TokenKind::Greater, TokenKind::GreaterEqual, TokenKind::Less, TokenKind::LessEqual]) {
            let operator = match self.previous().kind {
                TokenKind::Greater => BinaryOp::Greater,
                TokenKind::GreaterEqual => BinaryOp::GreaterEqual,
                TokenKind::Less => BinaryOp::Less,
                TokenKind::LessEqual => BinaryOp::LessEqual,
                _ => unreachable!(),
            };
            let right = self.term()?;
            let span = Span::new(expr.span.start, right.span.end, expr.span.start_loc, right.span.end_loc);
            expr = Expr::new(ExprKind::Binary(Box::new(expr), operator, Box::new(right)), span);
        }

        Some(expr)
    }

    fn term(&mut self) -> Option<Expr> {
        let mut expr = self.factor()?;

        while self.match_token(&[TokenKind::Plus, TokenKind::Minus]) {
            let operator = match self.previous().kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Subtract,
                _ => unreachable!(),
            };
            let right = self.factor()?;
            let span = Span::new(expr.span.start, right.span.end, expr.span.start_loc, right.span.end_loc);
            expr = Expr::new(ExprKind::Binary(Box::new(expr), operator, Box::new(right)), span);
        }

        Some(expr)
    }

    fn factor(&mut self) -> Option<Expr> {
        let mut expr = self.unary()?;

        while self.match_token(&[TokenKind::Star, TokenKind::Slash]) {
            let operator = match self.previous().kind {
                TokenKind::Star => BinaryOp::Multiply,
                TokenKind::Slash => BinaryOp::Divide,
                _ => unreachable!(),
            };
            let right = self.unary()?;
            let span = Span::new(expr.span.start, right.span.end, expr.span.start_loc, right.span.end_loc);
            expr = Expr::new(ExprKind::Binary(Box::new(expr), operator, Box::new(right)), span);
        }

        Some(expr)
    }

    fn unary(&mut self) -> Option<Expr> {
        if self.match_token(&[TokenKind::Minus]) {
            let start_span = self.previous().span;
            let right = self.unary()?;
            let span = Span::new(start_span.start, right.span.end, start_span.start_loc, right.span.end_loc);
            return Some(Expr::new(ExprKind::Unary(UnaryOp::Negate, Box::new(right)), span));
        }

        self.primary()
    }

    fn primary(&mut self) -> Option<Expr> {
        if self.match_token(&[TokenKind::False]) {
            return Some(Expr::new(ExprKind::Boolean(false), self.previous().span));
        }
        if self.match_token(&[TokenKind::True]) {
            return Some(Expr::new(ExprKind::Boolean(true), self.previous().span));
        }
        
        if let Some(token) = self.peek().cloned() {
            match token.kind {
                TokenKind::Integer(i) => {
                    self.advance();
                    return Some(Expr::new(ExprKind::Integer(i), token.span));
                }
                TokenKind::Float(f) => {
                    self.advance();
                    return Some(Expr::new(ExprKind::Float(f), token.span));
                }
                TokenKind::String(ref s) => {
                    self.advance();
                    return Some(Expr::new(ExprKind::String(s.clone()), token.span));
                }
                TokenKind::Identifier(ref i) => {
                    self.advance();
                    return Some(Expr::new(ExprKind::Variable(i.clone()), token.span));
                }
                TokenKind::LeftParen => {
                    self.advance();
                    let start_span = token.span;
                    let expr = self.expression()?;
                    if !self.match_token(&[TokenKind::RightParen]) {
                        self.error_at_current("Expected ')' after expression.");
                        return None;
                    }
                    let end_span = self.previous().span;
                    let span = Span::new(start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
                    return Some(Expr::new(ExprKind::Grouping(Box::new(expr)), span));
                }
                _ => {}
            }
        }

        self.error_at_current("Expected expression.");
        None
    }

    fn match_token(&mut self, types: &[TokenKind]) -> bool {
        for t in types {
            if self.check(t) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() {
            return false;
        }
        self.peek().map(|t| &t.kind == kind).unwrap_or(false)
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    fn is_at_end(&self) -> bool {
        self.peek().map(|t| t.kind == TokenKind::Eof).unwrap_or(true)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.current)
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    fn error_at_current(&mut self, message: &str) {
        if let Some(token) = self.peek() {
            let err = format!("Error at line {}: {}", token.span.start_loc.line, message);
            self.errors.push(err);
        } else {
            self.errors.push(message.into());
        }
    }

    fn synchronize(&mut self) {
        self.advance();
        while !self.is_at_end() {
            if self.previous().kind == TokenKind::Semicolon {
                return;
            }
            if let Some(token) = self.peek() {
                match token.kind {
                    TokenKind::Class | TokenKind::Func | TokenKind::Var | TokenKind::For | TokenKind::If | TokenKind::While | TokenKind::Return | TokenKind::Let => {
                        return;
                    }
                    _ => {}
                }
            }
            self.advance();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lexer::Scanner;

    #[test]
    fn test_let_statement() {
        let source = "let count = 10 + 5;";
        let mut scanner = Scanner::new(source);
        let tokens = scanner.scan_tokens();
        
        let mut parser = Parser::new(tokens);
        let (stmts, errors) = parser.parse();
        
        assert!(errors.is_empty());
        assert_eq!(stmts.len(), 1);
        
        match &stmts[0].kind {
            StmtKind::Let { name, initializer } => {
                assert_eq!(name, "count");
                match &initializer.kind {
                    ExprKind::Binary(left, op, right) => {
                        assert_eq!(*op, BinaryOp::Add);
                        assert!(matches!(left.kind, ExprKind::Integer(10)));
                        assert!(matches!(right.kind, ExprKind::Integer(5)));
                    }
                    _ => panic!("Expected Binary expression"),
                }
            }
            _ => panic!("Expected Let statement"),
        }
    }
    
    #[test]
    fn test_error_sync() {
        let source = "let x = @; let y = 20;";
        let mut scanner = Scanner::new(source);
        let tokens = scanner.scan_tokens();
        
        let mut parser = Parser::new(tokens);
        let (stmts, errors) = parser.parse();
        
        assert!(!errors.is_empty());
        assert_eq!(stmts.len(), 1); // Only the second statement is successfully parsed
        
        match &stmts[0].kind {
            StmtKind::Let { name, .. } => {
                assert_eq!(name, "y");
            }
            _ => panic!("Expected Let statement"),
        }
    }
}
