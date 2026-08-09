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
        let res = if self.match_token(&[TokenKind::Class]) {
            self.class_declaration()
        } else if self.match_token(&[TokenKind::Func]) {
            self.function_declaration()
        } else if self.match_token(&[TokenKind::Let]) {
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

    fn class_declaration(&mut self) -> Option<Stmt> {
        let start_span = self.previous().span;

        let name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
            self.advance();
            n
        } else {
            self.error_at_current("Expected class name.");
            return None;
        };

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before class body.");
            return None;
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            if self.match_token(&[TokenKind::Let]) {
                if let Some(field) = self.let_declaration(false) {
                    fields.push(field);
                }
            } else if self.match_token(&[TokenKind::Var]) {
                if let Some(field) = self.let_declaration(true) {
                    fields.push(field);
                }
            } else if self.match_token(&[TokenKind::Func]) {
                if let Some(method) = self.function_declaration() {
                    methods.push(method);
                }
            } else if self.match_token(&[TokenKind::Init]) {
                if let Some(init_method) = self.init_declaration() {
                    methods.push(init_method);
                }
            } else {
                self.error_at_current("Expected property or method inside class.");
                self.advance();
            }
        }

        if !self.match_token(&[TokenKind::RightBrace]) {
            self.error_at_current("Expected '}' after class body.");
            return None;
        }

        let end_span = self.previous().span;
        let span = Span::new(start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
        Some(Stmt::new(StmtKind::Class { name, methods, fields }, span))
    }

    fn init_declaration(&mut self) -> Option<Stmt> {
        let start_span = self.previous().span;
        let name = "init".to_string();

        if !self.match_token(&[TokenKind::LeftParen]) {
            self.error_at_current("Expected '(' after init.");
            return None;
        }

        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let param_name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
                    self.advance();
                    n
                } else {
                    self.error_at_current("Expected parameter name.");
                    return None;
                };

                if !self.match_token(&[TokenKind::Colon]) {
                    self.error_at_current("Expected ':' after parameter name.");
                    return None;
                }

                let param_type = if let Some(Token { kind: TokenKind::Identifier(t), .. }) = self.peek().cloned() {
                    self.advance();
                    t
                } else {
                    self.error_at_current("Expected parameter type.");
                    return None;
                };

                params.push((param_name, param_type));

                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        if !self.match_token(&[TokenKind::RightParen]) {
            self.error_at_current("Expected ')' after parameters.");
            return None;
        }

        let return_type = Some("Void".to_string());

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before init body.");
            return None;
        }

        let body = self.block()?;
        let end_span = body.span;
        let span = Span::new(start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);

        Some(Stmt::new(StmtKind::Func { name, params, return_type, body: Box::new(body) }, span))
    }

    fn function_declaration(&mut self) -> Option<Stmt> {
        let start_span = self.previous().span;

        let name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
            self.advance();
            n
        } else {
            self.error_at_current("Expected function name.");
            return None;
        };

        if !self.match_token(&[TokenKind::LeftParen]) {
            self.error_at_current("Expected '(' after function name.");
            return None;
        }

        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let param_name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
                    self.advance();
                    n
                } else {
                    self.error_at_current("Expected parameter name.");
                    return None;
                };

                if !self.match_token(&[TokenKind::Colon]) {
                    self.error_at_current("Expected ':' after parameter name.");
                    return None;
                }

                let param_type = if let Some(Token { kind: TokenKind::Identifier(t), .. }) = self.peek().cloned() {
                    self.advance();
                    t
                } else {
                    self.error_at_current("Expected parameter type.");
                    return None;
                };

                params.push((param_name, param_type));

                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        if !self.match_token(&[TokenKind::RightParen]) {
            self.error_at_current("Expected ')' after parameters.");
            return None;
        }

        let mut return_type = None;
        if self.match_token(&[TokenKind::Arrow]) {
            if let Some(Token { kind: TokenKind::Identifier(t), .. }) = self.peek().cloned() {
                self.advance();
                return_type = Some(t);
            } else {
                self.error_at_current("Expected return type after '->'.");
                return None;
            }
        }

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before function body.");
            return None;
        }

        let body = self.block()?;
        let end_span = body.span;
        let span = Span::new(start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);

        Some(Stmt::new(StmtKind::Func { name, params, return_type, body: Box::new(body) }, span))
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

        let type_annotation = if self.match_token(&[TokenKind::Colon]) {
            if let Some(Token { kind: TokenKind::Identifier(t), .. }) = self.peek().cloned() {
                self.advance();
                Some(t)
            } else {
                self.error_at_current("Expected type after ':'.");
                return None;
            }
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
        let span = Span::new(start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);

        let kind = if is_var {
            StmtKind::Var { name, type_annotation, initializer }
        } else {
            StmtKind::Let { name, type_annotation, initializer }
        };

        Some(Stmt::new(kind, span))
    }

    fn statement(&mut self) -> Option<Stmt> {
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

    fn if_statement(&mut self) -> Option<Stmt> {
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
                else_branch = Some(Box::new(e_branch));
            } else if self.match_token(&[TokenKind::LeftBrace]) {
                let e_branch = self.block()?;
                end_span = e_branch.span;
                else_branch = Some(Box::new(e_branch));
            } else {
                self.error_at_current("Expected '{' or 'if' after else.");
                return None;
            }
        }

        let span = Span::new(start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
        Some(Stmt::new(StmtKind::If { condition, then_branch: Box::new(then_branch), else_branch }, span))
    }

    fn while_statement(&mut self) -> Option<Stmt> {
        let start_span = self.previous().span;

        let condition = self.expression()?;

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' after while condition.");
            return None;
        }

        let body = self.block()?;
        let end_span = body.span;

        let span = Span::new(start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
        Some(Stmt::new(StmtKind::While { condition, body: Box::new(body) }, span))
    }

    fn for_statement(&mut self) -> Option<Stmt> {
        let start_span = self.previous().span;

        let item_name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
            self.advance();
            n
        } else {
            self.error_at_current("Expected item name after 'for'.");
            return None;
        };

        let is_in = if let Some(Token { kind: TokenKind::Identifier(s), .. }) = self.peek() {
            s == "in"
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

        let span = Span::new(start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
        Some(Stmt::new(StmtKind::For { item_name, iterator, body: Box::new(body) }, span))
    }

    fn return_statement(&mut self) -> Option<Stmt> {
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
        let span = Span::new(start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
        Some(Stmt::new(StmtKind::Return { value }, span))
    }

    fn block(&mut self) -> Option<Stmt> {
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
        let span = Span::new(start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
        Some(Stmt::new(StmtKind::Block(statements), span))
    }

    fn expression_statement(&mut self) -> Option<Stmt> {
        let expr = self.expression()?;

        self.match_token(&[TokenKind::Semicolon]);

        let span = expr.span;
        Some(Stmt::new(StmtKind::Expression(expr), span))
    }

    fn expression(&mut self) -> Option<Expr> {
        self.assignment()
    }

    fn assignment(&mut self) -> Option<Expr> {
        let expr = self.equality()?;

        if self.match_token(&[TokenKind::Equal]) {
            let equals = self.previous().clone();
            let value = self.assignment()?;

            match expr.kind {
                ExprKind::Variable(name) => {
                    let span = Span::new(expr.span.start, value.span.end, expr.span.start_loc, value.span.end_loc);
                    return Some(Expr::new(ExprKind::Assign { name, value: Box::new(value) }, span));
                }
                ExprKind::Get { object, name } => {
                    let span = Span::new(expr.span.start, value.span.end, expr.span.start_loc, value.span.end_loc);
                    return Some(Expr::new(ExprKind::Set { object, name, value: Box::new(value) }, span));
                }
                _ => {
                    self.error_at_current("Invalid assignment target.");
                }
            }
        }

        Some(expr)
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

        self.call()
    }

    fn call(&mut self) -> Option<Expr> {
        let mut expr = self.primary()?;

        loop {
            if self.match_token(&[TokenKind::LeftParen]) {
                expr = self.finish_call(expr)?;
            } else if self.match_token(&[TokenKind::Dot]) {
                let name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
                    self.advance();
                    n
                } else {
                    self.error_at_current("Expected property name after '.'.");
                    return None;
                };

                let span = Span::new(expr.span.start, self.previous().span.end, expr.span.start_loc, self.previous().span.end_loc);
                expr = Expr::new(ExprKind::Get { object: Box::new(expr), name }, span);
            } else {
                break;
            }
        }
        Some(expr)
    }

    fn finish_call(&mut self, callee: Expr) -> Option<Expr> {
        let mut arguments = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                arguments.push(self.expression()?);
                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }
        if !self.match_token(&[TokenKind::RightParen]) {
            self.error_at_current("Expected ')' after arguments.");
            return None;
        }
        let span = Span::new(callee.span.start, self.previous().span.end, callee.span.start_loc, self.previous().span.end_loc);
        Some(Expr::new(ExprKind::Call { callee: Box::new(callee), arguments }, span))
    }

    fn primary(&mut self) -> Option<Expr> {
        if self.match_token(&[TokenKind::False]) {
            return Some(Expr::new(ExprKind::Boolean(false), self.previous().span));
        }
        if self.match_token(&[TokenKind::True]) {
            return Some(Expr::new(ExprKind::Boolean(true), self.previous().span));
        }
        if self.match_token(&[TokenKind::SelfKeyword]) {
            return Some(Expr::new(ExprKind::SelfRef, self.previous().span));
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
    fn test_func_declaration() {
        let source = "func add(a: Int, b: Int) -> Int { return a + b; }";
        let mut scanner = Scanner::new(source);
        let mut parser = Parser::new(scanner.scan_tokens());
        let (stmts, errors) = parser.parse();
        
        assert!(errors.is_empty(), "Parse errors: {:?}", errors);
        assert_eq!(stmts.len(), 1);
        match &stmts[0].kind {
            StmtKind::Func { name, params, return_type, .. } => {
                assert_eq!(name, "add");
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].0, "a");
                assert_eq!(return_type.as_ref().unwrap(), "Int");
            }
            _ => panic!("Expected Func statement"),
        }
    }

    #[test]
    fn test_if_statement() {
        let source = "if count > 0 { let x = 1; } else { let x = 0; }";
        let mut scanner = Scanner::new(source);
        let mut parser = Parser::new(scanner.scan_tokens());
        let (stmts, errors) = parser.parse();
        
        assert!(errors.is_empty(), "Parse errors: {:?}", errors);
        assert_eq!(stmts.len(), 1);
        match &stmts[0].kind {
            StmtKind::If { then_branch, else_branch, .. } => {
                assert!(matches!(then_branch.kind, StmtKind::Block(_)));
                assert!(else_branch.is_some());
            }
            _ => panic!("Expected If statement"),
        }
    }
}
