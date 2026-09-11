use pace_ast::{Block, Decl, Expr, Ident, Program, Stmt, Type};
use pace_lexer::{Lexer, Token, TokenKind};
use pace_span::Span;

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Option<Token<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current = lexer.next().and_then(|r| r.ok());
        Self { lexer, current }
    }

    fn advance(&mut self) {
        self.current = self.lexer.next().and_then(|r| r.ok());
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.current.as_ref().map_or(false, |t| &t.kind == kind)
    }

    fn expect(&mut self, kind: TokenKind<'a>) -> Result<Token<'a>, String> {
        if self.check(&kind) {
            let tok = self.current.clone().unwrap();
            self.advance();
            Ok(tok)
        } else {
            Err(format!("Expected {:?}", kind))
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut declarations = Vec::new();
        let start_span = self.current.as_ref().map(|t| t.span).unwrap_or(Span::DUMMY);

        while self.current.is_some() {
            declarations.push(self.parse_decl()?);
        }

        let end_span = declarations.last().map(|d| match d {
            Decl::Let { span, .. } => *span,
            Decl::Var { span, .. } => *span,
            Decl::Const { span, .. } => *span,
            Decl::Function { span, .. } => *span,
            Decl::Struct { span, .. } => *span,
            Decl::Class { span, .. } => *span,
            Decl::Expr(_, span) => *span,
        }).unwrap_or(start_span);

        Ok(Program {
            declarations,
            span: start_span.merge(end_span),
        })
    }

    fn parse_decl(&mut self) -> Result<Decl, String> {
        if self.check(&TokenKind::Let) {
            let start_tok = self.expect(TokenKind::Let)?;
            
            let name_tok = match &self.current {
                Some(Token { kind: TokenKind::Ident(name), span }) => {
                    let ident = Ident { name: name.to_string(), span: *span };
                    self.advance();
                    ident
                }
                _ => return Err("Expected identifier after 'let'".to_string()),
            };

            self.expect(TokenKind::Eq)?;
            let value = self.parse_expr()?;
            let end_span = value.span();

            Ok(Decl::Let {
                name: name_tok,
                value,
                span: start_tok.span.merge(end_span),
            })
        } else if self.check(&TokenKind::Var) {
            let start_tok = self.expect(TokenKind::Var)?;
            
            let name_tok = match &self.current {
                Some(Token { kind: TokenKind::Ident(name), span }) => {
                    let ident = Ident { name: name.to_string(), span: *span };
                    self.advance();
                    ident
                }
                _ => return Err("Expected identifier after 'var'".to_string()),
            };

            self.expect(TokenKind::Eq)?;
            let value = self.parse_expr()?;
            let end_span = value.span();

            Ok(Decl::Var {
                name: name_tok,
                value,
                span: start_tok.span.merge(end_span),
            })
        } else if self.check(&TokenKind::Fn) {
            self.parse_fn_decl()
        } else if self.check(&TokenKind::Struct) {
            self.parse_struct_decl()
        } else if self.check(&TokenKind::Class) {
            self.parse_class_decl()
        } else {
            let expr = self.parse_expr()?;
            let span = expr.span();
            Ok(Decl::Expr(expr, span))
        }
    }

    fn parse_struct_decl(&mut self) -> Result<Decl, String> {
        let start_tok = self.expect(TokenKind::Struct)?;
        
        let name_tok = match &self.current {
            Some(Token { kind: TokenKind::Ident(name), span }) => {
                let ident = Ident { name: name.to_string(), span: *span };
                self.advance();
                ident
            }
            _ => return Err("Expected identifier after 'struct'".to_string()),
        };

        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            if self.check(&TokenKind::Fn) {
                methods.push(self.parse_fn_decl()?);
            } else if let Some(Token { kind: TokenKind::Ident(name), span }) = &self.current {
                if *name == "init" {
                    let init_span_start = *span;
                    self.advance(); // consume 'init'
                    let ident = Ident { name: "init".to_string(), span: init_span_start };
                    
                    self.expect(TokenKind::LParen)?;
                    let mut params = Vec::new();
                    while !self.check(&TokenKind::RParen) && self.current.is_some() {
                        let param_name = match &self.current {
                            Some(Token { kind: TokenKind::Ident(n), span }) => Ident { name: n.to_string(), span: *span },
                            _ => return Err("Expected parameter name".to_string()),
                        };
                        self.advance();
                        self.expect(TokenKind::Colon)?;
                        let param_ty = self.parse_type()?;
                        params.push((param_name, param_ty));
                        if self.check(&TokenKind::Comma) {
                            self.advance();
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    
                    let body = self.parse_block()?;
                    methods.push(Decl::Function {
                        name: ident,
                        params,
                        return_type: None,
                        body: body.clone(),
                        span: init_span_start.merge(body.span),
                    });
                    continue;
                }
                
                let field_name = Ident { name: name.to_string(), span: self.current.as_ref().unwrap().span };
                self.advance();
                
                self.expect(TokenKind::Colon)?;
                let field_type = self.parse_type()?;
                fields.push((field_name, field_type));
                
                if self.check(&TokenKind::Comma) {
                    self.advance();
                }
            } else {
                return Err("Expected field or method in struct".to_string());
            }
        }
        
        let end_tok = self.expect(TokenKind::RBrace)?;

        Ok(Decl::Struct {
            name: name_tok,
            fields,
            methods,
            span: start_tok.span.merge(end_tok.span),
        })
    }

    fn parse_class_decl(&mut self) -> Result<Decl, String> {
        let start_tok = self.expect(TokenKind::Class)?;
        
        let name_tok = match &self.current {
            Some(Token { kind: TokenKind::Ident(name), span }) => {
                let ident = Ident { name: name.to_string(), span: *span };
                self.advance();
                ident
            }
            _ => return Err("Expected identifier after 'class'".to_string()),
        };

        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            if self.check(&TokenKind::Fn) {
                methods.push(self.parse_fn_decl()?);
            } else if let Some(Token { kind: TokenKind::Ident(name), span }) = &self.current {
                if *name == "init" {
                    let init_span_start = *span;
                    self.advance(); // consume 'init'
                    let ident = Ident { name: "init".to_string(), span: init_span_start };
                    
                    self.expect(TokenKind::LParen)?;
                    let mut params = Vec::new();
                    while !self.check(&TokenKind::RParen) && self.current.is_some() {
                        let param_name = match &self.current {
                            Some(Token { kind: TokenKind::Ident(n), span }) => Ident { name: n.to_string(), span: *span },
                            _ => return Err("Expected parameter name".to_string()),
                        };
                        self.advance();
                        self.expect(TokenKind::Colon)?;
                        let param_ty = self.parse_type()?;
                        params.push((param_name, param_ty));
                        if self.check(&TokenKind::Comma) {
                            self.advance();
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    
                    let body = self.parse_block()?;
                    methods.push(Decl::Function {
                        name: ident,
                        params,
                        return_type: None,
                        body: body.clone(),
                        span: init_span_start.merge(body.span),
                    });
                    continue;
                }
                
                let field_name = Ident { name: name.to_string(), span: self.current.as_ref().unwrap().span };
                self.advance();
                
                self.expect(TokenKind::Colon)?;
                let field_type = self.parse_type()?;
                fields.push((field_name, field_type));
                
                if self.check(&TokenKind::Comma) {
                    self.advance();
                }
            } else {
                return Err("Expected field or method in class".to_string());
            }
        }
        
        let end_tok = self.expect(TokenKind::RBrace)?;

        Ok(Decl::Class {
            name: name_tok,
            fields,
            methods,
            span: start_tok.span.merge(end_tok.span),
        })
    }

    fn parse_fn_decl(&mut self) -> Result<Decl, String> {
        let start_tok = self.expect(TokenKind::Fn)?;
        
        let name_tok = match &self.current {
            Some(Token { kind: TokenKind::Ident(name), span }) => {
                let ident = Ident { name: name.to_string(), span: *span };
                self.advance();
                ident
            }
            _ => return Err("Expected identifier after 'fn'".to_string()),
        };

        self.expect(TokenKind::LParen)?;
        
        let mut params = Vec::new();
        while !self.check(&TokenKind::RParen) {
            let param_name = match &self.current {
                Some(Token { kind: TokenKind::Ident(name), span }) => {
                    let ident = Ident { name: name.to_string(), span: *span };
                    self.advance();
                    ident
                }
                _ => return Err("Expected parameter name".to_string()),
            };

            self.expect(TokenKind::Colon)?;
            let param_type = self.parse_type()?;
            
            params.push((param_name, param_type));

            if self.check(&TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;

        let mut return_type = None;
        if self.check(&TokenKind::Arrow) {
            self.advance();
            return_type = Some(self.parse_type()?);
        }

        let body = self.parse_block()?;
        let end_span = body.span;

        Ok(Decl::Function {
            name: name_tok,
            params,
            return_type,
            body,
            span: start_tok.span.merge(end_span),
        })
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        let mut base_type = match &self.current {
            Some(Token { kind: TokenKind::Ident(name), span }) => {
                let ident = Ident { name: name.to_string(), span: *span };
                self.advance();
                Type::Named(ident)
            }
            _ => return Err("Expected type name".to_string()),
        };

        if self.check(&TokenKind::Question) {
            let span = base_type.span().merge(self.current.as_ref().unwrap().span);
            self.advance();
            base_type = Type::Optional(Box::new(base_type), span);
        }

        Ok(base_type)
    }

    fn parse_block(&mut self) -> Result<Block, String> {
        let start_tok = self.expect(TokenKind::LBrace).map_err(|e| format!("{} (found {:?})", e, self.current))?;
        let mut statements = Vec::new();

        while !self.check(&TokenKind::RBrace) && self.current.is_some() {
            statements.push(self.parse_stmt()?);
        }

        let end_tok = self.expect(TokenKind::RBrace)?;

        Ok(Block {
            statements,
            span: start_tok.span.merge(end_tok.span),
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        if self.check(&TokenKind::Let) {
            let start_tok = self.expect(TokenKind::Let)?;
            let name_tok = self.current.take().ok_or("Expected identifier after 'let'".to_string())?;
            let name = match name_tok.kind {
                TokenKind::Ident(n) => Ident { name: n.to_string(), span: name_tok.span },
                _ => return Err("Expected identifier after 'let'".to_string()),
            };
            self.advance();
            self.expect(TokenKind::Eq)?;
            let value = self.parse_expr()?;
            let span = start_tok.span.merge(value.span());
            Ok(Stmt::Let { name, value, span })
        } else if self.check(&TokenKind::Var) {
            let start_tok = self.expect(TokenKind::Var)?;
            let name_tok = self.current.take().ok_or("Expected identifier after 'var'".to_string())?;
            let name = match name_tok.kind {
                TokenKind::Ident(n) => Ident { name: n.to_string(), span: name_tok.span },
                _ => return Err("Expected identifier after 'var'".to_string()),
            };
            self.advance();
            self.expect(TokenKind::Eq)?;
            let value = self.parse_expr()?;
            let span = start_tok.span.merge(value.span());
            Ok(Stmt::Var { name, value, span })
        } else if self.check(&TokenKind::Return) {
            let start_tok = self.expect(TokenKind::Return)?;
            
            let mut expr = None;
            let mut end_span = start_tok.span;
            
            if !self.check(&TokenKind::RBrace) {
                let e = self.parse_expr()?;
                end_span = end_span.merge(e.span());
                expr = Some(e);
            }

            Ok(Stmt::Return(expr, start_tok.span.merge(end_span)))
        } else {
            let expr = self.parse_expr()?;
            let span = expr.span();
            Ok(Stmt::ExprStmt(expr, span))
        }
    }

    fn parse_postfix_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_primary()?;
        
        loop {
            if self.check(&TokenKind::Dot) {
                self.advance();
                let member_name = match &self.current {
                    Some(Token { kind: TokenKind::Ident(name), span }) => {
                        let ident = Ident { name: name.to_string(), span: *span };
                        self.advance();
                        ident
                    }
                    _ => return Err("Expected member name after '.'".to_string()),
                };
                
                let span = left.span().merge(member_name.span);
                left = Expr::MemberAccess {
                    span,
                    object: Box::new(left),
                    member: member_name,
                };
            } else if self.check(&TokenKind::LParen) {
                self.advance();
                let mut args = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        // Check for named argument: `ident:`
                        let mut label = None;
                        
                        let mut parsed_expr = self.parse_expr()?;
                        
                        if let Expr::Ident(ref ident) = parsed_expr {
                            if self.check(&TokenKind::Colon) {
                                self.advance(); // consume `:`
                                label = Some(ident.clone());
                                parsed_expr = self.parse_expr()?;
                            }
                        }
                        args.push((label, parsed_expr));
                        if self.check(&TokenKind::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                let rparen = self.expect(TokenKind::RParen)?;
                left = Expr::Call {
                    span: left.span().merge(rparen.span),
                    callee: Box::new(left),
                    args,
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_postfix_expr()?;

        if self.check(&TokenKind::Eq) {
            self.advance();
            let right = self.parse_expr()?;
            let span = left.span().merge(right.span());
            return Ok(Expr::Assign { target: Box::new(left), value: Box::new(right), span });
        }

        loop {
            let op = if self.check(&TokenKind::Plus) { Some(pace_ast::BinaryOp::Add) }
            else if self.check(&TokenKind::Minus) { Some(pace_ast::BinaryOp::Sub) }
            else if self.check(&TokenKind::Star) { Some(pace_ast::BinaryOp::Mul) }
            else if self.check(&TokenKind::Slash) { Some(pace_ast::BinaryOp::Div) }
            else if self.check(&TokenKind::EqEq) { Some(pace_ast::BinaryOp::EqEq) }
            else if self.check(&TokenKind::NotEq) { Some(pace_ast::BinaryOp::NotEq) }
            else if self.check(&TokenKind::Gt) { Some(pace_ast::BinaryOp::Gt) }
            else if self.check(&TokenKind::Lt) { Some(pace_ast::BinaryOp::Lt) }
            else if self.check(&TokenKind::GtEq) { Some(pace_ast::BinaryOp::GtEq) }
            else if self.check(&TokenKind::LtEq) { Some(pace_ast::BinaryOp::LtEq) }
            else { None };

            if let Some(op) = op {
                self.advance();
                let right = self.parse_postfix_expr()?;
                left = Expr::Binary {
                    span: left.span().merge(right.span()),
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        if self.check(&TokenKind::If) {
            let start_tok = self.expect(TokenKind::If)?;
            let cond = self.parse_expr()?;
            let then_block = self.parse_block()?;
            let mut else_block = None;
            let mut end_span = then_block.span;
            if self.check(&TokenKind::Else) {
                self.advance();
                let b = self.parse_block()?;
                end_span = b.span;
                else_block = Some(b);
            }
            return Ok(Expr::If {
                cond: Box::new(cond),
                then_block,
                else_block,
                span: start_tok.span.merge(end_span),
            });
        }
        if self.check(&TokenKind::While) {
            let start_tok = self.expect(TokenKind::While)?;
            let cond = self.parse_expr()?;
            let body = self.parse_block()?;
            return Ok(Expr::While {
                cond: Box::new(cond),
                span: start_tok.span.merge(body.span),
                body,
            });
        }

        let tok = self.current.take().ok_or("Expected expression, found EOF".to_string())?;
        self.advance();

        match tok.kind {
            TokenKind::Int(val) => Ok(Expr::IntLiteral(val.to_string(), tok.span)),
            TokenKind::Float(val) => Ok(Expr::FloatLiteral(val.to_string(), tok.span)),
            TokenKind::String(val) => Ok(Expr::StringLiteral(val.to_string(), tok.span)),
            TokenKind::Ident(name) => {
                let ident = Ident { name: name.to_string(), span: tok.span };
                Ok(Expr::Ident(ident))
            }
            _ => Err(format!("Unexpected token in expression: {:?}", tok.kind)),
        }
    }
}
