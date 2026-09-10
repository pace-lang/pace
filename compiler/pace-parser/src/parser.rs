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
            Decl::Const { span, .. } => *span,
            Decl::Function { span, .. } => *span,
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
        } else if self.check(&TokenKind::Fn) {
            self.parse_fn_decl()
        } else {
            Err("Unsupported declaration in early parser".to_string())
        }
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
        let start_tok = self.expect(TokenKind::LBrace)?;
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
        if self.check(&TokenKind::Return) {
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

    fn parse_expr(&mut self) -> Result<Expr, String> {
        let left = self.parse_primary()?;
        
        if self.check(&TokenKind::Plus) {
            self.advance();
            let right = self.parse_primary()?;
            
            return Ok(Expr::Binary {
                span: left.span().merge(right.span()),
                left: Box::new(left),
                op: pace_ast::BinaryOp::Add,
                right: Box::new(right),
            });
        }
        
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let tok = self.current.clone().ok_or("Expected expression")?;
        self.advance();

        match tok.kind {
            TokenKind::Int(val) => Ok(Expr::IntLiteral(val.to_string(), tok.span)),
            TokenKind::Float(val) => Ok(Expr::FloatLiteral(val.to_string(), tok.span)),
            TokenKind::String(val) => Ok(Expr::StringLiteral(val.to_string(), tok.span)),
            TokenKind::Ident(name) => Ok(Expr::Ident(Ident { name: name.to_string(), span: tok.span })),
            _ => Err(format!("Unexpected token in expression: {:?}", tok.kind)),
        }
    }
}
