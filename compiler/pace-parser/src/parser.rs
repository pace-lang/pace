use pace_ast::{Decl, Expr, Ident, Program};
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
        } else {
            Err("Unsupported declaration in early parser".to_string())
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        // Basic primary expression parsing
        let tok = self.current.clone().ok_or("Expected expression")?;
        self.advance();

        match tok.kind {
            TokenKind::Int(val) => Ok(Expr::IntLiteral(val.to_string(), tok.span)),
            TokenKind::Float(val) => Ok(Expr::FloatLiteral(val.to_string(), tok.span)),
            TokenKind::String(val) => Ok(Expr::StringLiteral(val.to_string(), tok.span)),
            TokenKind::Ident(name) => Ok(Expr::Ident(Ident { name: name.to_string(), span: tok.span })),
            _ => Err("Unexpected token in expression".to_string()),
        }
    }
}
