use ast::{Expr, ExprKind, Stmt, StmtKind, Span, BinaryOp, UnaryOp, TypeExpr};
use lexer::{Token, TokenKind};
use diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticCode};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    pub errors: Vec<Diagnostic>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            current: 0,
            errors: Vec::new(),
        }
    }

    pub fn parse(&mut self) -> (Vec<Stmt>, Vec<Diagnostic>) {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            if let Some(stmt) = self.declaration() {
                statements.push(stmt);
            }
        }
        (statements, self.errors.clone())
    }

    fn parse_visibility(&mut self) -> bool {
        if self.match_token(&[TokenKind::Private]) {
            true
        } else if self.match_token(&[TokenKind::Public]) {
            false
        } else {
            false
        }
    }

    fn declaration(&mut self) -> Option<Stmt> {
        let is_private = self.parse_visibility();

        let res = if self.match_token(&[TokenKind::Interface]) {
            self.interface_declaration(is_private)
        } else if self.match_token(&[TokenKind::Class]) {
            self.class_declaration(is_private)
        } else if self.match_token(&[TokenKind::Enum]) {
            self.enum_declaration(is_private)
        } else if self.match_token(&[TokenKind::Func]) {
            self.function_declaration(is_private)
        } else if self.match_token(&[TokenKind::Let]) {
            self.variable_declaration(false, false, is_private)
        } else if self.match_token(&[TokenKind::Var]) {
            self.variable_declaration(true, false, is_private)
        } else if self.match_token(&[TokenKind::Weak]) {
            if self.match_token(&[TokenKind::Var]) {
                self.variable_declaration(true, true, is_private)
            } else {
                self.error_at_current("Expected 'var' after 'weak'.");
                None
            }
        } else if self.match_token(&[TokenKind::Foreign]) {
            self.foreign_declaration(is_private)
        } else if self.match_token(&[TokenKind::Import]) {
            if is_private {
                self.error_at_current("Visibility modifiers are not allowed on imports.");
            }
            self.import_declaration()
        } else if self.match_token(&[TokenKind::Export]) {
            if is_private {
                self.error_at_current("Visibility modifiers are not allowed on exports.");
            }
            self.export_declaration()
        } else {
            if is_private {
                self.error_at_current("Visibility modifiers are only allowed on declarations.");
            }
            self.statement()
        };

        if res.is_none() {
            self.synchronize();
        }
        res
    }

    fn parse_type_params(&mut self) -> Option<Vec<String>> {
        let mut type_params = Vec::new();
        if self.match_token(&[TokenKind::Less]) {
            if !self.check(&TokenKind::Greater) {
                loop {
                    if let Some(Token { kind: TokenKind::Identifier(t), .. }) = self.peek().cloned() {
                        self.advance();
                        type_params.push(t);
                    } else {
                        self.error_at_current("Expected generic parameter name.");
                        return None;
                    }
                    if !self.match_token(&[TokenKind::Comma]) {
                        break;
                    }
                }
            }
            if !self.match_token(&[TokenKind::Greater]) {
                self.error_at_current("Expected '>' after generic parameters.");
                return None;
            }
        }
        Some(type_params)
    }

    fn parse_type_expr(&mut self) -> Option<TypeExpr> {
        if self.match_token(&[TokenKind::LeftBracket]) {
            let inner = self.parse_type_expr()?;
            if !self.match_token(&[TokenKind::RightBracket]) {
                self.error_at_current("Expected ']' after array element type.");
                return None;
            }
            let mut ty = TypeExpr::Array(Box::new(inner));
            if self.match_token(&[TokenKind::Question]) {
                ty = TypeExpr::Optional(Box::new(ty));
            }
            return Some(ty);
        }

        if let Some(Token { kind: TokenKind::Identifier(t), .. }) = self.peek().cloned() {
            self.advance();
            let base_type = t;
            
            if self.match_token(&[TokenKind::Less]) {
                let mut type_args = Vec::new();
                if !self.check(&TokenKind::Greater) {
                    loop {
                        if let Some(ty) = self.parse_type_expr() {
                            type_args.push(ty);
                        } else {
                            return None;
                        }
                        if !self.match_token(&[TokenKind::Comma]) {
                            break;
                        }
                    }
                }
                
                if !self.match_token(&[TokenKind::Greater]) {
                    self.error_at_current("Expected '>' after generic type arguments.");
                    return None;
                }
                
                let mut ty = TypeExpr::GenericInstance(base_type, type_args);
                if self.match_token(&[TokenKind::Question]) {
                    ty = TypeExpr::Optional(Box::new(ty));
                }
                return Some(ty);
            }
            
            let mut ty = TypeExpr::Named(base_type);
            if self.match_token(&[TokenKind::Question]) {
                ty = TypeExpr::Optional(Box::new(ty));
            }
            return Some(ty);
        } else {
            self.error_at_current("Expected type name.");
            None
        }
    }

    fn enum_declaration(&mut self, is_private: bool) -> Option<Stmt> {
        let start_span = self.previous().span;

        let name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
            self.advance();
            n
        } else {
            self.error_at_current("Expected enum name.");
            return None;
        };

        let type_params = self.parse_type_params()?;

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before enum body.");
            return None;
        }

        let mut variants = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
                self.advance();
                let variant_name = n;
                
                let fields = if self.match_token(&[TokenKind::LeftParen]) {
                    let mut variant_fields = Vec::new();
                    if !self.check(&TokenKind::RightParen) {
                        loop {
                            let field_name = if let Some(Token { kind: TokenKind::Identifier(fname), .. }) = self.peek().cloned() {
                                // Could be a name `x: Int` or just a type `Int`
                                // Let's check if there's a colon next
                                let next_is_colon = self.tokens.get(self.current + 1).map(|t| t.kind == TokenKind::Colon).unwrap_or(false);
                                if next_is_colon {
                                    self.advance(); // consume name
                                    self.advance(); // consume colon
                                    Some(fname)
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            if let Some(ty) = self.parse_type_expr() {
                                variant_fields.push(ast::EnumField { name: field_name, ty });
                            } else {
                                return None;
                            }

                            if !self.match_token(&[TokenKind::Comma]) {
                                break;
                            }
                        }
                    }
                    if !self.match_token(&[TokenKind::RightParen]) {
                        self.error_at_current("Expected ')' after enum variant fields.");
                        return None;
                    }
                    Some(variant_fields)
                } else {
                    None
                };

                variants.push(ast::EnumVariant { name: variant_name, fields });

                if self.match_token(&[TokenKind::Comma]) {
                    continue;
                }
            } else {
                self.error_at_current("Expected enum variant name.");
                return None;
            }
        }

        if !self.match_token(&[TokenKind::RightBrace]) {
            self.error_at_current("Expected '}' after enum body.");
            return None;
        }

        let end_span = self.previous().span;
        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);

        Some(Stmt::new(StmtKind::Enum {
            name,
            type_params,
            variants,
            is_private,
        }, span))
    }

    fn class_declaration(&mut self, is_private: bool) -> Option<Stmt> {
        let start_span = self.previous().span;

        let name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
            self.advance();
            n
        } else {
            self.error_at_current("Expected class name.");
            return None;
        };

        let type_params = self.parse_type_params()?;

        let mut implements = Vec::new();
        if self.match_token(&[TokenKind::Implements]) {
            loop {
                if let Some(Token { kind: TokenKind::Identifier(i), .. }) = self.peek().cloned() {
                    self.advance();
                    implements.push(i);
                } else {
                    self.error_at_current("Expected interface name.");
                    return None;
                }
                
                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before class body.");
            return None;
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let item_is_private = self.parse_visibility();

            if self.match_token(&[TokenKind::Let]) {
                if let Some(field) = self.variable_declaration(false, false, item_is_private) {
                    fields.push(field);
                }
            } else if self.match_token(&[TokenKind::Var]) {
                if let Some(field) = self.variable_declaration(true, false, item_is_private) {
                    fields.push(field);
                }
            } else if self.match_token(&[TokenKind::Weak]) {
                if self.match_token(&[TokenKind::Var]) {
                    if let Some(field) = self.variable_declaration(true, true, item_is_private) {
                        fields.push(field);
                    }
                } else {
                    self.error_at_current("Expected 'var' after 'weak'.");
                }
            } else if self.match_token(&[TokenKind::Func]) {
                if self.match_token(&[TokenKind::Init]) {
                    if let Some(init_method) = self.init_declaration(item_is_private) {
                        methods.push(init_method);
                    }
                } else {
                    if let Some(method) = self.function_declaration(item_is_private) {
                        methods.push(method);
                    }
                }
            } else if self.match_token(&[TokenKind::Init]) {
                self.error_at_current("Constructors must be declared with 'func init'.");
                if let Some(init_method) = self.init_declaration(false) {
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
        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
        Some(Stmt::new(StmtKind::Class { name: name.clone(), type_params, implements, methods, fields, is_private }, span))
    }

    fn interface_declaration(&mut self, is_private: bool) -> Option<Stmt> {
        let start_span = self.previous().span;

        let name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
            self.advance();
            n
        } else {
            self.error_at_current("Expected interface name.");
            return None;
        };

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before interface body.");
            return None;
        }

        let mut methods = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let item_is_private = self.parse_visibility();

            if self.match_token(&[TokenKind::Func]) {
                if let Some(method) = self.interface_method_declaration(item_is_private) {
                    methods.push(method);
                }
            } else {
                self.error_at_current("Expected method signature inside interface.");
                self.advance();
            }
        }

        if !self.match_token(&[TokenKind::RightBrace]) {
            self.error_at_current("Expected '}' after interface body.");
            return None;
        }

        let end_span = self.previous().span;
        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
        Some(Stmt::new(StmtKind::Interface { name: name.clone(), methods, is_private }, span))
    }

    fn interface_method_declaration(&mut self, is_private: bool) -> Option<Stmt> {
        let start_span = self.previous().span;

        let name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
            self.advance();
            n
        } else {
            self.error_at_current("Expected method name.");
            return None;
        };

        if !self.match_token(&[TokenKind::LeftParen]) {
            self.error_at_current("Expected '(' after method name.");
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

                let param_type = self.parse_type_expr()?;

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
            return_type = Some(self.parse_type_expr()?);
        }

        let end_span = self.previous().span;
        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);

        // We use StmtKind::Func but with an empty block for the body.
        let empty_body = Box::new(Stmt::new(StmtKind::Block(Vec::new()), span.clone()));
        Some(Stmt::new(StmtKind::Func { name: name.clone(), type_params: Vec::new(), params, return_type, body: empty_body, is_private }, span))
    }

    fn init_declaration(&mut self, is_private: bool) -> Option<Stmt> {
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

                let param_type = self.parse_type_expr()?;

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

        let return_type = Some(TypeExpr::Named("Void".to_string()));

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before init body.");
            return None;
        }

        let body = self.block()?;
        let end_span = body.span;
        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
        Some(Stmt::new(StmtKind::Func { name: name.clone(), type_params: Vec::new(), params, return_type, body: Box::new(body), is_private }, span))
    }

    fn foreign_declaration(&mut self, is_private: bool) -> Option<Stmt> {
        let start_span = self.previous().span;

        if !self.match_token(&[TokenKind::Func]) {
            self.error_at_current("Expected 'func' after 'foreign'.");
            return None;
        }

        let name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
            self.advance();
            n
        } else {
            self.error_at_current("Expected foreign function name.");
            return None;
        };

        let type_params = self.parse_type_params()?;

        if !self.match_token(&[TokenKind::LeftParen]) {
            self.error_at_current("Expected '(' after foreign function name.");
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

                let param_type = self.parse_type_expr()?;

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
            return_type = Some(self.parse_type_expr()?);
        }

        if !self.match_token(&[TokenKind::Semicolon]) {
            self.error_at_current("Expected ';' after foreign function declaration.");
            return None;
        }

        let end_span = self.previous().span;
        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);

        Some(Stmt::new(StmtKind::ForeignFunc {
            name,
            type_params,
            params,
            return_type,
            is_private,
        }, span))
    }

    fn function_declaration(&mut self, is_private: bool) -> Option<Stmt> {
        let start_span = self.previous().span;

        let name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
            self.advance();
            n
        } else {
            self.error_at_current("Expected function name.");
            return None;
        };

        let type_params = self.parse_type_params()?;

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

                let param_type = self.parse_type_expr()?;

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
            return_type = Some(self.parse_type_expr()?);
        }

        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before function body.");
            return None;
        }

        let body = self.block()?;
        let end_span = body.span;
        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
        Some(Stmt::new(StmtKind::Func { name: name.clone(), type_params, params, return_type, body: Box::new(body), is_private }, span))
    }

    fn variable_declaration(&mut self, is_var: bool, is_weak: bool, is_private: bool) -> Option<Stmt> {
        let start_span = self.previous().span;

        let name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
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
        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
        
        let kind = if is_var {
            StmtKind::Var { name: name.clone(), type_annotation, initializer, is_weak, is_private }
        } else {
            StmtKind::Let { name: name.clone(), type_annotation, initializer, is_private }
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

        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
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

        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
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

        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
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
        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
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
        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
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
            let _equals = self.previous().clone();
            let value = self.assignment()?;

            match expr.kind {
                ExprKind::Variable(name) => {
                    let span = Span::new(expr.span.file_id, expr.span.start, value.span.end, expr.span.start_loc, value.span.end_loc);
                    return Some(Expr::new(ExprKind::Assign { name, value: Box::new(value) }, span));
                }
                ExprKind::Get { object, name } => {
                    let span = Span::new(expr.span.file_id, expr.span.start, value.span.end, expr.span.start_loc, value.span.end_loc);
                    return Some(Expr::new(ExprKind::Set { object, name, value: Box::new(value) }, span));
                }
                ExprKind::IndexGet { object, index } => {
                    let span = Span::new(expr.span.file_id, expr.span.start, value.span.end, expr.span.start_loc, value.span.end_loc);
                    return Some(Expr::new(ExprKind::IndexSet { object, index, value: Box::new(value) }, span));
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
            let span = Span::new(expr.span.file_id, expr.span.start, right.span.end, expr.span.start_loc, right.span.end_loc);
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
            let span = Span::new(expr.span.file_id, expr.span.start, right.span.end, expr.span.start_loc, right.span.end_loc);
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
            let span = Span::new(expr.span.file_id, expr.span.start, right.span.end, expr.span.start_loc, right.span.end_loc);
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
            let span = Span::new(expr.span.file_id, expr.span.start, right.span.end, expr.span.start_loc, right.span.end_loc);
            expr = Expr::new(ExprKind::Binary(Box::new(expr), operator, Box::new(right)), span);
        }

        Some(expr)
    }

    fn unary(&mut self) -> Option<Expr> {
        if self.match_token(&[TokenKind::Minus]) {
            let start_span = self.previous().span;
            let right = self.unary()?;
            let span = Span::new(start_span.file_id, start_span.start, right.span.end, start_span.start_loc, right.span.end_loc);
            return Some(Expr::new(ExprKind::Unary(UnaryOp::Negate, Box::new(right)), span));
        }

        self.call()
    }

    fn call(&mut self) -> Option<Expr> {
        let mut expr = self.primary()?;

        loop {
            if self.match_token(&[TokenKind::LeftParen]) {
                expr = self.finish_call(expr, Vec::new())?;
            } else if self.check_generic_call() {
                self.advance(); // consume '<'
                let mut type_args = Vec::new();
                if !self.check(&TokenKind::Greater) {
                    loop {
                        if let Some(ty) = self.parse_type_expr() {
                            type_args.push(ty);
                        } else {
                            return None;
                        }
                        if !self.match_token(&[TokenKind::Comma]) {
                            break;
                        }
                    }
                }
                if !self.match_token(&[TokenKind::Greater]) {
                    self.error_at_current("Expected '>' after generic type arguments.");
                    return None;
                }
                if !self.match_token(&[TokenKind::LeftParen]) {
                    self.error_at_current("Expected '(' after generic type arguments.");
                    return None;
                }
                expr = self.finish_call(expr, type_args)?;
            } else if self.match_token(&[TokenKind::Dot]) {
                let name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
                    self.advance();
                    n
                } else {
                    self.error_at_current("Expected property name after '.'.");
                    return None;
                };

                let span = Span::new(expr.span.file_id, expr.span.start, self.previous().span.end, expr.span.start_loc, self.previous().span.end_loc);
                expr = Expr::new(ExprKind::Get { object: Box::new(expr), name }, span);
            } else if self.match_token(&[TokenKind::QuestionDot]) {
                let name = if let Some(Token { kind: TokenKind::Identifier(n), .. }) = self.peek().cloned() {
                    self.advance();
                    n
                } else {
                    self.error_at_current("Expected property name after '?.'.");
                    return None;
                };

                let span = Span::new(expr.span.file_id, expr.span.start, self.previous().span.end, expr.span.start_loc, self.previous().span.end_loc);
                expr = Expr::new(ExprKind::OptionalGet { object: Box::new(expr), name }, span);
            } else if self.match_token(&[TokenKind::Bang]) {
                let span = Span::new(expr.span.file_id, expr.span.start, self.previous().span.end, expr.span.start_loc, self.previous().span.end_loc);
                expr = Expr::new(ExprKind::ForceUnwrap(Box::new(expr)), span);
            } else if self.match_token(&[TokenKind::LeftBracket]) {
                let index = self.expression()?;
                if !self.match_token(&[TokenKind::RightBracket]) {
                    self.error_at_current("Expected ']' after index.");
                    return None;
                }
                let span = Span::new(expr.span.file_id, expr.span.start, self.previous().span.end, expr.span.start_loc, self.previous().span.end_loc);
                expr = Expr::new(ExprKind::IndexGet { object: Box::new(expr), index: Box::new(index) }, span);
            } else {
                break;
            }
        }
        Some(expr)
    }

    fn finish_call(&mut self, callee: Expr, type_args: Vec<TypeExpr>) -> Option<Expr> {
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
        let span = Span::new(callee.span.file_id, callee.span.start, self.previous().span.end, callee.span.start_loc, self.previous().span.end_loc);
        Some(Expr::new(ExprKind::Call { callee: Box::new(callee), type_args, arguments }, span))
    }

    fn check_generic_call(&self) -> bool {
        if !self.check(&TokenKind::Less) {
            return false;
        }
        let mut i = self.current + 1;
        let mut depth = 1;
        while i < self.tokens.len() {
            match self.tokens[i].kind {
                TokenKind::Less => depth += 1,
                TokenKind::Greater => {
                    depth -= 1;
                    if depth == 0 {
                        // Check if the next token is '('
                        if i + 1 < self.tokens.len() && self.tokens[i + 1].kind == TokenKind::LeftParen {
                            return true;
                        } else {
                            return false;
                        }
                    }
                },
                TokenKind::LeftParen | TokenKind::LeftBrace | TokenKind::Semicolon | TokenKind::Equal => {
                    return false; // Definitely not generic type args
                },
                _ => {}
            }
            i += 1;
        }
        false
    }

    fn primary(&mut self) -> Option<Expr> {
        if self.match_token(&[TokenKind::False]) {
            return Some(Expr::new(ExprKind::Boolean(false), self.previous().span));
        }
        if self.match_token(&[TokenKind::True]) {
            return Some(Expr::new(ExprKind::Boolean(true), self.previous().span));
        }
        if self.match_token(&[TokenKind::Null]) {
            return Some(Expr::new(ExprKind::Null, self.previous().span));
        }
        if self.match_token(&[TokenKind::SelfKeyword]) {
            return Some(Expr::new(ExprKind::SelfRef, self.previous().span));
        }
        
        if let Some(token) = self.peek().cloned() {
            match token.kind {
                TokenKind::Match => {
                    self.advance();
                    return self.match_expression(token.span);
                }
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
                    let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
                    return Some(Expr::new(ExprKind::Grouping(Box::new(expr)), span));
                }
                TokenKind::LeftBracket => {
                    self.advance();
                    let start_span = token.span;
                    if self.match_token(&[TokenKind::RightBracket]) {
                        let end_span = self.previous().span;
                        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
                        return Some(Expr::new(ExprKind::Array(Vec::new()), span));
                    }
                    
                    let first_expr = self.expression()?;
                    if self.match_token(&[TokenKind::Semicolon]) {
                        let count = self.expression()?;
                        if !self.match_token(&[TokenKind::RightBracket]) {
                            self.error_at_current("Expected ']' after array repeat count.");
                            return None;
                        }
                        let end_span = self.previous().span;
                        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
                        return Some(Expr::new(ExprKind::ArrayRepeat { value: Box::new(first_expr), count: Box::new(count) }, span));
                    } else {
                        let mut elements = vec![first_expr];
                        while self.match_token(&[TokenKind::Comma]) {
                            if self.check(&TokenKind::RightBracket) {
                                break;
                            }
                            elements.push(self.expression()?);
                        }
                        if !self.match_token(&[TokenKind::RightBracket]) {
                            self.error_at_current("Expected ']' after array elements.");
                            return None;
                        }
                        let end_span = self.previous().span;
                        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
                        return Some(Expr::new(ExprKind::Array(elements), span));
                    }
                }
                _ => {}
            }
        }

        self.error_at_current("Expected expression.");
        None
    }

    fn match_expression(&mut self, start_span: Span) -> Option<Expr> {
        let value = self.expression()?;
        
        if !self.match_token(&[TokenKind::LeftBrace]) {
            self.error_at_current("Expected '{' before match arms.");
            return None;
        }

        let mut arms = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let pattern = if self.match_token(&[TokenKind::Underscore]) {
                ast::Pattern::Wildcard
            } else {
                let mut path = Vec::new();
                if let Some(Token { kind: TokenKind::Identifier(first), .. }) = self.peek().cloned() {
                    self.advance();
                    path.push(first);
                    while self.match_token(&[TokenKind::Dot]) {
                        if let Some(Token { kind: TokenKind::Identifier(next), .. }) = self.peek().cloned() {
                            self.advance();
                            path.push(next);
                        } else {
                            self.error_at_current("Expected identifier after '.'.");
                            return None;
                        }
                    }
                } else {
                    self.error_at_current("Expected pattern.");
                    return None;
                }

                let bindings = if self.match_token(&[TokenKind::LeftParen]) {
                    let mut b = Vec::new();
                    if !self.check(&TokenKind::RightParen) {
                        loop {
                            if self.match_token(&[TokenKind::Underscore]) {
                                b.push("_".to_string());
                            } else if let Some(Token { kind: TokenKind::Identifier(id), .. }) = self.peek().cloned() {
                                self.advance();
                                b.push(id);
                            } else {
                                self.error_at_current("Expected binding name or '_' in pattern.");
                                return None;
                            }
                            if !self.match_token(&[TokenKind::Comma]) {
                                break;
                            }
                        }
                    }
                    if !self.match_token(&[TokenKind::RightParen]) {
                        self.error_at_current("Expected ')' after pattern bindings.");
                        return None;
                    }
                    Some(b)
                } else {
                    None
                };

                ast::Pattern::Variant { path, bindings }
            };

            if !self.match_token(&[TokenKind::FatArrow]) {
                self.error_at_current("Expected '=>' after match pattern.");
                return None;
            }

            let body = self.expression()?;
            
            // Optional comma after arm
            self.match_token(&[TokenKind::Comma]);

            arms.push(ast::MatchArm { pattern, body: Box::new(body) });
        }

        if !self.match_token(&[TokenKind::RightBrace]) {
            self.error_at_current("Expected '}' after match arms.");
            return None;
        }

        let end_span = self.previous().span;
        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);
        
        Some(Expr::new(ExprKind::Match { value: Box::new(value), arms }, span))
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
            let diag = DiagnosticBuilder::error(
                DiagnosticCode::UnexpectedToken,
                message,
                token.span,
            ).build();
            self.errors.push(diag);
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

    fn import_declaration(&mut self) -> Option<Stmt> {
        let start_span = self.previous().span;
        
        let path = if let Some(Token { kind: TokenKind::String(p), .. }) = self.peek().cloned() {
            self.advance();
            p
        } else {
            self.error_at_current("Expected string after 'import'.");
            return None;
        };

        let mut alias = None;
        if self.match_token(&[TokenKind::As]) {
            if let Some(Token { kind: TokenKind::Identifier(a), .. }) = self.peek().cloned() {
                self.advance();
                alias = Some(a);
            } else {
                self.error_at_current("Expected identifier after 'as'.");
            }
        }
        
        let mut show = Vec::new();
        if self.match_token(&[TokenKind::Show]) {
            loop {
                if let Some(Token { kind: TokenKind::Identifier(i), .. }) = self.peek().cloned() {
                    self.advance();
                    show.push(i);
                } else {
                    self.error_at_current("Expected identifier after 'show'.");
                    break;
                }
                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        let mut hide = Vec::new();
        if self.match_token(&[TokenKind::Hide]) {
            loop {
                if let Some(Token { kind: TokenKind::Identifier(i), .. }) = self.peek().cloned() {
                    self.advance();
                    hide.push(i);
                } else {
                    self.error_at_current("Expected identifier after 'hide'.");
                    break;
                }
                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        if !self.match_token(&[TokenKind::Semicolon]) {
            self.error_at_current("Expected ';' after import declaration.");
            return None;
        }

        let end_span = self.previous().span;
        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);

        Some(Stmt::new(StmtKind::Import { path, alias, show, hide }, span))
    }

    fn export_declaration(&mut self) -> Option<Stmt> {
        let start_span = self.previous().span;
        
        let path = if let Some(Token { kind: TokenKind::String(p), .. }) = self.peek().cloned() {
            self.advance();
            p
        } else {
            self.error_at_current("Expected string after 'export'.");
            return None;
        };

        if !self.match_token(&[TokenKind::Semicolon]) {
            self.error_at_current("Expected ';' after export declaration.");
            return None;
        }

        let end_span = self.previous().span;
        let span = Span::new(start_span.file_id, start_span.start, end_span.end, start_span.start_loc, end_span.end_loc);

        Some(Stmt::new(StmtKind::Export { path }, span))
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use lexer::Scanner;

    #[test]
    fn test_func_declaration() {
        let source = "func add(a: Int, b: Int) -> Int { return a + b; }";
        let mut scanner = Scanner::new(0, source);
        let mut parser = Parser::new(scanner.scan_tokens());
        let (stmts, errors) = parser.parse();
        
        assert!(errors.is_empty(), "Parse errors: {:?}", errors);
        assert_eq!(stmts.len(), 1);
        match &stmts[0].kind {
            StmtKind::Func { name, params, return_type, .. } => {
                assert_eq!(name, "add");
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].0, "a");
                assert_eq!(return_type.as_ref().unwrap(), &ast::TypeExpr::Named("Int".to_string()));
            }
            _ => panic!("Expected Func statement"),
        }
    }

    #[test]
    fn test_visibility_modifiers() {
        let source = "private func hidden() {} public class Visible {} var unadorned = 1;";
        let mut scanner = Scanner::new(0, source);
        let mut parser = Parser::new(scanner.scan_tokens());
        let (stmts, errors) = parser.parse();
        
        assert!(errors.is_empty(), "Parse errors: {:?}", errors);
        assert_eq!(stmts.len(), 3);
        
        match &stmts[0].kind {
            StmtKind::Func { is_private, .. } => assert_eq!(*is_private, true),
            _ => panic!("Expected Func statement"),
        }
        
        match &stmts[1].kind {
            StmtKind::Class { is_private, .. } => assert_eq!(*is_private, false),
            _ => panic!("Expected Class statement"),
        }
        
        match &stmts[2].kind {
            StmtKind::Var { is_private, .. } => assert_eq!(*is_private, false),
            _ => panic!("Expected Var statement"),
        }
    }

    #[test]
    fn test_if_statement() {
        let source = "if count > 0 { let x = 1; } else { let x = 0; }";
        let mut scanner = Scanner::new(0, source);
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
