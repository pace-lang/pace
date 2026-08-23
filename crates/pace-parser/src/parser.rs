use pace_ast::{Expr, Stmt, BinaryOp, Param, Visibility};
use crate::lexer::{Lexer, Token};

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
}

impl<'a> Parser<'a> {
    pub fn new(src: &'a str) -> Self {
        let mut lexer = Lexer::new(src);
        let current_token = lexer.next_token();
        Self {
            lexer,
            current_token,
        }
    }

    fn advance(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    fn match_token(&mut self, expected: Token) -> bool {
        if self.current_token == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt>, String> {
        let mut stmts = Vec::new();
        while self.current_token != Token::Eof {
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        let visibility = if self.match_token(Token::Public) {
            Visibility::Public
        } else if self.match_token(Token::Private) {
            Visibility::Private
        } else {
            Visibility::Private // Default could be internal/private
        };

        let is_async = self.match_token(Token::Async);

        match self.current_token {
            Token::Let => self.parse_var_decl(false),
            Token::Var => self.parse_var_decl(true),
            Token::Func => self.parse_func_decl(is_async, visibility),
            Token::Class => self.parse_class_decl(),
            Token::Interface => self.parse_interface_decl(),
            Token::Struct => self.parse_struct_decl(),
            Token::If => self.parse_if_stmt(),
            Token::While => self.parse_while_stmt(),
            Token::Loop => self.parse_loop_stmt(),
            Token::For => self.parse_for_stmt(),
            Token::Match => self.parse_match_stmt(),
            Token::Import => self.parse_import_stmt(),
            Token::LBrace => self.parse_block(),
            Token::Return => {
                self.advance();
                let expr = if self.current_token != Token::Semi {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                if !self.match_token(Token::Semi) {
                    return Err("Expected ';' after return".to_string());
                }
                Ok(Stmt::Return(expr))
            }
            _ => {
                let expr = self.parse_expr()?;
                if !self.match_token(Token::Semi) {
                    return Err("Expected ';' after expression".to_string());
                }
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_block(&mut self) -> Result<Stmt, String> {
        self.advance(); // consume '{'
        let mut stmts = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            stmts.push(self.parse_stmt()?);
        }
        if !self.match_token(Token::RBrace) {
            return Err("Expected '}' after block".to_string());
        }
        Ok(Stmt::Block(stmts))
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt, String> {
        self.advance(); // consume if
        let condition = self.parse_expr()?;
        
        let then_branch = Box::new(self.parse_stmt()?);
        
        let mut else_branch = None;
        if self.match_token(Token::Else) {
            else_branch = Some(Box::new(self.parse_stmt()?));
        }

        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn parse_while_stmt(&mut self) -> Result<Stmt, String> {
        self.advance(); // consume while
        let condition = self.parse_expr()?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::While {
            condition,
            body,
        })
    }

    fn parse_loop_stmt(&mut self) -> Result<Stmt, String> {
        self.advance(); // consume loop
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::Loop {
            body,
        })
    }

    fn parse_for_stmt(&mut self) -> Result<Stmt, String> {
        self.advance(); // consume for
        let item = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err("Expected identifier in for loop".to_string()),
        };
        self.advance();

        if !self.match_token(Token::In) {
            return Err("Expected 'in' in for loop".to_string());
        }

        let iterable = self.parse_expr()?;
        let body = Box::new(self.parse_stmt()?);

        Ok(Stmt::ForIn {
            item,
            iterable,
            body,
        })
    }

    fn parse_match_stmt(&mut self) -> Result<Stmt, String> {
        self.advance(); // consume match
        let expr = self.parse_expr()?;
        
        if !self.match_token(Token::LBrace) {
            return Err("Expected '{' before match arms".to_string());
        }

        let mut arms = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let pattern = self.parse_expr()?;
            if !self.match_token(Token::Arrow) {
                return Err("Expected '=>' after match pattern".to_string());
            }
            let body = Box::new(self.parse_stmt()?);
            arms.push((pattern, body));
        }

        if !self.match_token(Token::RBrace) {
            return Err("Expected '}' after match arms".to_string());
        }

        Ok(Stmt::Match {
            expr,
            arms,
        })
    }

    fn parse_import_stmt(&mut self) -> Result<Stmt, String> {
        self.advance(); // consume 'import'
        
        let mut items = None;
        let path;

        // Check if it's a specific import: `import { ... } from "path";`
        if self.match_token(Token::LBrace) {
            let mut import_items = Vec::new();
            if self.current_token != Token::RBrace {
                loop {
                    match &self.current_token {
                        Token::Ident(id) => import_items.push(id.clone()),
                        _ => return Err("Expected identifier in import list".to_string()),
                    }
                    self.advance();

                    if !self.match_token(Token::Comma) {
                        break;
                    }
                }
            }
            if !self.match_token(Token::RBrace) {
                return Err("Expected '}' after import list".to_string());
            }

            if !self.match_token(Token::From) {
                return Err("Expected 'from' after import list".to_string());
            }

            path = match &self.current_token {
                Token::String(s) => s.clone(),
                _ => return Err("Expected string literal for import path".to_string()),
            };
            self.advance();
            items = Some(import_items);
        } else {
            // Namespace or side-effect import: `import "path";` or `import ident;`
            match &self.current_token {
                Token::String(s) => {
                    path = s.clone();
                    self.advance();
                }
                Token::Ident(id) => {
                    path = id.clone();
                    self.advance();
                }
                _ => return Err("Expected string literal or identifier for import path".to_string()),
            }
        }

        if !self.match_token(Token::Semi) {
            return Err("Expected ';' after import statement".to_string());
        }

        Ok(Stmt::Import { path, items })
    }

    fn parse_func_decl(&mut self, is_async: bool, visibility: Visibility) -> Result<Stmt, String> {
        self.advance(); // consume func

        let name = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err("Expected function name".to_string()),
        };
        self.advance();

        if !self.match_token(Token::LParen) {
            return Err("Expected '(' after function name".to_string());
        }

        let mut params = Vec::new();
        if self.current_token != Token::RParen {
            loop {
                let param_name = match &self.current_token {
                    Token::Ident(id) => id.clone(),
                    _ => return Err("Expected parameter name".to_string()),
                };
                self.advance();

                if !self.match_token(Token::Colon) {
                    return Err("Expected ':' after parameter name".to_string());
                }

                let param_type = match &self.current_token {
                    Token::Ident(id) => id.clone(),
                    _ => return Err("Expected parameter type".to_string()),
                };
                self.advance();

                params.push(Param {
                    name: param_name,
                    type_annotation: param_type,
                });

                if !self.match_token(Token::Comma) {
                    break;
                }
            }
        }

        if !self.match_token(Token::RParen) {
            return Err("Expected ')' after parameters".to_string());
        }

        let mut return_type = None;
        if self.match_token(Token::Arrow) {
            return_type = match &self.current_token {
                Token::Ident(id) => Some(id.clone()),
                _ => return Err("Expected return type after '->'".to_string()),
            };
            self.advance();
        }

        if !self.match_token(Token::LBrace) {
            return Err("Expected '{' before function body".to_string());
        }

        let mut body = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            body.push(self.parse_stmt()?);
        }

        if !self.match_token(Token::RBrace) {
            return Err("Expected '}' after function body".to_string());
        }

        Ok(Stmt::FuncDecl {
            name,
            params,
            return_type,
            body,
            is_async,
            visibility,
        })
    }

    fn parse_class_decl(&mut self) -> Result<Stmt, String> {
        self.advance(); // consume class

        let name = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err("Expected class name".to_string()),
        };
        self.advance();

        let mut implements = None;
        if self.match_token(Token::Implement) {
            implements = match &self.current_token {
                Token::Ident(id) => Some(id.clone()),
                _ => return Err("Expected interface name after 'implement'".to_string()),
            };
            self.advance();
        }

        if !self.match_token(Token::LBrace) {
            return Err("Expected '{' before class body".to_string());
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let stmt = self.parse_stmt()?;
            match stmt {
                Stmt::VarDecl { .. } => fields.push(stmt),
                Stmt::FuncDecl { .. } => methods.push(stmt),
                _ => return Err("Classes can only contain fields and methods".to_string()),
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err("Expected '}' after class body".to_string());
        }

        Ok(Stmt::ClassDecl {
            name,
            fields,
            methods,
            implements,
        })
    }

    fn parse_interface_decl(&mut self) -> Result<Stmt, String> {
        self.advance(); // consume interface

        let name = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err("Expected interface name".to_string()),
        };
        self.advance();

        if !self.match_token(Token::LBrace) {
            return Err("Expected '{' before interface body".to_string());
        }

        let mut methods = Vec::new();

        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let is_async = self.match_token(Token::Async);
            
            if !self.match_token(Token::Func) {
                return Err("Expected 'func' in interface definition".to_string());
            }

            let method_name = match &self.current_token {
                Token::Ident(id) => id.clone(),
                _ => return Err("Expected function name".to_string()),
            };
            self.advance();

            if !self.match_token(Token::LParen) {
                return Err("Expected '(' after function name".to_string());
            }

            let mut params = Vec::new();
            if self.current_token != Token::RParen {
                loop {
                    let param_name = match &self.current_token {
                        Token::Ident(id) => id.clone(),
                        _ => return Err("Expected parameter name".to_string()),
                    };
                    self.advance();

                    if !self.match_token(Token::Colon) {
                        return Err("Expected ':' after parameter name".to_string());
                    }

                    let param_type = match &self.current_token {
                        Token::Ident(id) => id.clone(),
                        _ => return Err("Expected parameter type".to_string()),
                    };
                    self.advance();

                    params.push(Param {
                        name: param_name,
                        type_annotation: param_type,
                    });

                    if !self.match_token(Token::Comma) {
                        break;
                    }
                }
            }

            if !self.match_token(Token::RParen) {
                return Err("Expected ')' after parameters".to_string());
            }

            let mut return_type = None;
            if self.match_token(Token::Arrow) {
                return_type = match &self.current_token {
                    Token::Ident(id) => Some(id.clone()),
                    _ => return Err("Expected return type after '->'".to_string()),
                };
                self.advance();
            }

            methods.push(Stmt::FuncDecl {
                name: method_name,
                params,
                return_type,
                body: vec![],
                is_async,
                visibility: Visibility::Public,
            });
        }

        if !self.match_token(Token::RBrace) {
            return Err("Expected '}' after interface body".to_string());
        }

        Ok(Stmt::InterfaceDecl {
            name,
            methods,
        })
    }

    fn parse_struct_decl(&mut self) -> Result<Stmt, String> {
        self.advance(); // consume struct

        let name = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err("Expected struct name".to_string()),
        };
        self.advance();

        if !self.match_token(Token::LBrace) {
            return Err("Expected '{' before struct body".to_string());
        }

        let mut fields = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let stmt = self.parse_stmt()?;
            match stmt {
                Stmt::VarDecl { .. } => fields.push(stmt),
                _ => return Err("Structs can only contain fields".to_string()),
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err("Expected '}' after struct body".to_string());
        }

        Ok(Stmt::StructDecl {
            name,
            fields,
        })
    }

    fn parse_var_decl(&mut self, is_mutable: bool) -> Result<Stmt, String> {
        self.advance(); // consume let/var
        let name = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err("Expected identifier".to_string()),
        };
        self.advance();

        let mut type_annotation = None;
        if self.match_token(Token::Colon) {
            type_annotation = match &self.current_token {
                Token::Ident(id) => Some(id.clone()),
                _ => return Err("Expected type identifier after ':'".to_string()),
            };
            self.advance();
        }

        let mut initializer = None;
        if self.match_token(Token::Eq) {
            initializer = Some(self.parse_expr()?);
        }

        if !self.match_token(Token::Semi) {
            return Err("Expected ';' after variable declaration".to_string());
        }

        Ok(Stmt::VarDecl {
            name,
            is_mutable,
            type_annotation,
            initializer,
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_equality()
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_term()?;
        while self.current_token == Token::EqEq || self.current_token == Token::NotEq {
            let op = if self.current_token == Token::EqEq { BinaryOp::Eq } else { BinaryOp::NotEq };
            self.advance();
            let right = self.parse_term()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_factor()?;
        while self.current_token == Token::Plus || self.current_token == Token::Minus {
            let op = if self.current_token == Token::Plus { BinaryOp::Add } else { BinaryOp::Sub };
            self.advance();
            let right = self.parse_factor()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_postfix()?;
        while self.current_token == Token::Star || self.current_token == Token::Slash {
            let op = if self.current_token == Token::Star { BinaryOp::Mul } else { BinaryOp::Div };
            self.advance();
            let right = self.parse_postfix()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;
        
        loop {
            if self.match_token(Token::Dot) {
                let property = match &self.current_token {
                    Token::Ident(id) => id.clone(),
                    _ => return Err("Expected property name after '.'".to_string()),
                };
                self.advance();
                expr = Expr::MemberAccess {
                    object: Box::new(expr),
                    property: property,
                    computed_class: None,
                };
            } else if self.match_token(Token::LParen) {
                let mut args = Vec::new();
                if self.current_token != Token::RParen {
                    loop {
                        args.push(self.parse_expr()?);
                        if !self.match_token(Token::Comma) {
                            break;
                        }
                    }
                }
                if !self.match_token(Token::RParen) {
                    return Err("Expected ')' after arguments".to_string());
                }
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match &self.current_token {
            Token::Int(n) => {
                let v = *n;
                self.advance();
                Ok(Expr::IntLiteral(v))
            }
            Token::Float(f) => {
                let v = *f;
                self.advance();
                Ok(Expr::FloatLiteral(v))
            }
            Token::String(s) => {
                let v = s.clone();
                self.advance();
                Ok(Expr::StringLiteral(v))
            }
            Token::Bool(b) => {
                let v = *b;
                self.advance();
                Ok(Expr::BoolLiteral(v))
            }
            Token::Null => {
                self.advance();
                Ok(Expr::Null)
            }
            Token::Ident(id) => {
                let v = id.clone();
                self.advance();
                Ok(Expr::Identifier(v))
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                if !self.match_token(Token::RParen) {
                    return Err("Expected ')'".to_string());
                }
                Ok(expr)
            }
            _ => Err(format!("Unexpected token: {:?}", self.current_token)),
        }
    }
}
