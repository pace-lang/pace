use pace_ast::{Expr, Stmt, BinaryOp, Param, Visibility, TypeAnnotation};
use crate::lexer::{Lexer, Token};

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
    current_span: (usize, usize),
    pub errors: Vec<(String, (usize, usize))>,
}

impl<'a> Parser<'a> {
    pub fn new(src: &'a str) -> Self {
        let mut lexer = Lexer::new(src);
        let (current_token, current_span) = lexer.next_token();
        Self {
            lexer,
            current_token,
            current_span,
            errors: Vec::new(),
        }
    }

    fn advance(&mut self) {
        let (tok, span) = self.lexer.next_token();
        self.current_token = tok;
        self.current_span = span;
    }

    fn match_token(&mut self, expected: Token) -> bool {
        if self.current_token == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt>, Vec<(String, (usize, usize))>> {
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
                Token::Class | Token::Actor | Token::Func | Token::Var | Token::Let |
                Token::For | Token::If | Token::While | Token::Return | Token::RBrace => return,
                _ => self.advance(),
            }
        }
    }

    fn parse_stmt(&mut self) -> Result<Stmt, (String, (usize, usize))> {
        let visibility = if self.match_token(Token::Public) {
            Visibility::Public
        } else if self.match_token(Token::Private) {
            Visibility::Private
        } else {
            Visibility::Public // Default is public
        };

        let is_async = self.match_token(Token::Async);
        let is_static = self.match_token(Token::Static);

        match self.current_token {
            Token::Let => self.parse_var_decl(false, is_static),
            Token::Var => self.parse_var_decl(true, is_static),
            Token::Func => self.parse_func_decl(is_async, visibility, is_static),
            Token::Class => self.parse_class_decl(),
            Token::Actor => self.parse_actor_decl(),
            Token::Interface => self.parse_interface_decl(),
            Token::Struct => self.parse_struct_decl(),
            Token::Enum => self.parse_enum_decl(),
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
                    return Err(("Expected ';' after return".to_string(), self.current_span));
                }
                Ok(Stmt::Return(expr))
            }
            _ => {
                let expr = self.parse_expr()?;
                if !self.match_token(Token::Semi) {
                    return Err(("Expected ';' after expression".to_string(), self.current_span));
                }
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_generic_params(&mut self) -> Result<Option<Vec<String>>, (String, (usize, usize))> {
        if self.match_token(Token::Less) {
            let mut params = Vec::new();
            while self.current_token != Token::Greater && self.current_token != Token::Eof {
                if let Token::Ident(id) = &self.current_token {
                    params.push(id.clone());
                    self.advance();
                } else {
                    return Err(("Expected generic parameter name".to_string(), self.current_span));
                }
                if !self.match_token(Token::Comma) {
                    break;
                }
            }
            if !self.match_token(Token::Greater) {
                return Err(("Expected '>' after generic parameters".to_string(), self.current_span));
            }
            Ok(Some(params))
        } else {
            Ok(None)
        }
    }

    fn parse_type_annotation(&mut self) -> Result<TypeAnnotation, (String, (usize, usize))> {
        let mut name = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err(("Expected type name".to_string(), self.current_span)),
        };
        self.advance();

        let mut module_prefix = None;
        if self.match_token(Token::Dot) {
            module_prefix = Some(name);
            name = match &self.current_token {
                Token::Ident(id) => id.clone(),
                _ => return Err(("Expected type name after '.'".to_string(), self.current_span)),
            };
            self.advance();
        }

        let mut args = Vec::new();
        if self.match_token(Token::Less) {
            while self.current_token != Token::Greater && self.current_token != Token::Eof {
                args.push(self.parse_type_annotation()?);
                if !self.match_token(Token::Comma) {
                    break;
                }
            }
            if !self.match_token(Token::Greater) {
                return Err(("Expected '>' after generic arguments".to_string(), self.current_span));
            }
        }

        let is_nullable = self.match_token(Token::Question);

        Ok(TypeAnnotation {
            module_prefix,
            name,
            args,
            is_nullable,
        })
    }

    fn parse_block(&mut self) -> Result<Stmt, (String, (usize, usize))> {
        self.advance(); // consume '{'
        let mut stmts = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            match self.parse_stmt() {
                Ok(stmt) => stmts.push(stmt),
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }
        if !self.match_token(Token::RBrace) {
            return Err(("Expected '}' after block".to_string(), self.current_span));
        }
        Ok(Stmt::Block(stmts))
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt, (String, (usize, usize))> {
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

    fn parse_while_stmt(&mut self) -> Result<Stmt, (String, (usize, usize))> {
        self.advance(); // consume while
        let condition = self.parse_expr()?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::While {
            condition,
            body,
        })
    }

    fn parse_loop_stmt(&mut self) -> Result<Stmt, (String, (usize, usize))> {
        self.advance(); // consume loop
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::Loop {
            body,
        })
    }

    fn parse_for_stmt(&mut self) -> Result<Stmt, (String, (usize, usize))> {
        self.advance(); // consume for
        let item = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err(("Expected identifier in for loop".to_string(), self.current_span)),
        };
        self.advance();

        if !self.match_token(Token::In) {
            return Err(("Expected 'in' in for loop".to_string(), self.current_span));
        }

        let iterable = self.parse_expr()?;
        let body = Box::new(self.parse_stmt()?);

        Ok(Stmt::ForIn {
            item,
            iterable,
            body,
        })
    }

    fn parse_pattern(&mut self) -> Result<pace_ast::Pattern, (String, (usize, usize))> {
        match self.current_token.clone() {
            Token::Ident(name) => {
                self.advance();
                
                if name == "_" {
                    return Ok(pace_ast::Pattern::Wildcard);
                }
                
                let mut generic_args = None;
                if self.current_token == Token::Less {
                    self.advance();
                    let mut args = Vec::new();
                    while self.current_token != Token::Greater && self.current_token != Token::Eof {
                        args.push(self.parse_type_annotation()?);
                        if self.match_token(Token::Comma) {
                            continue;
                        }
                        break;
                    }
                    if !self.match_token(Token::Greater) {
                        return Err(("Expected '>' after generic arguments in pattern".to_string(), self.current_span));
                    }
                    generic_args = Some(args);
                }
                
                // Could be a variable, or an Enum variant like `Some(x)` or `Option::Some(x)`
                // Let's check for `::`
                let mut enum_name = None;
                let mut variant_name = name.clone();
                
                if self.current_token == Token::ColonColon {
                    self.advance();
                    if let Token::Ident(v_name) = self.current_token.clone() {
                        self.advance();
                        enum_name = Some(name);
                        variant_name = v_name;
                    } else {
                        return Err(("Expected variant name after :: in pattern".to_string(), self.current_span));
                    }
                }
                
                // If there's an open parenthesis, it's a variant with fields
                if self.current_token == Token::LParen {
                    self.advance();
                    let mut fields = Vec::new();
                    if self.current_token != Token::RParen {
                        loop {
                            fields.push(self.parse_pattern()?);
                            if self.match_token(Token::Comma) {
                                continue;
                            }
                            break;
                        }
                    }
                    if !self.match_token(Token::RParen) {
                        return Err(("Expected ')' after pattern fields".to_string(), self.current_span));
                    }
                    Ok(pace_ast::Pattern::Variant {
                        enum_name,
                        variant_name,
                        fields: Some(fields),
                        generic_args,
                    })
                } else if enum_name.is_some() || variant_name.chars().next().unwrap().is_uppercase() {
                    // It's a variant without fields (like `None`)
                    Ok(pace_ast::Pattern::Variant {
                        enum_name,
                        variant_name,
                        fields: None,
                        generic_args,
                    })
                } else {
                    // Lowercase without parenthesis -> Variable binding
                    Ok(pace_ast::Pattern::Variable(variant_name, self.current_span))
                }
            },
            Token::Int(_) | Token::Float(_) | Token::String(_) | Token::Bool(_) => {
                let expr = self.parse_primary()?;
                Ok(pace_ast::Pattern::Literal(expr))
            },
            _ => Err(("Expected pattern".to_string(), self.current_span))
        }
    }

    fn parse_match_stmt(&mut self) -> Result<Stmt, (String, (usize, usize))> {
        self.advance(); // consume match
        let expr = self.parse_expr()?;
        
        if !self.match_token(Token::LBrace) {
            return Err(("Expected '{' before match arms".to_string(), self.current_span));
        }

        let mut arms = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let pattern = self.parse_pattern()?;
            if !self.match_token(Token::FatArrow) && !self.match_token(Token::Arrow) {
                return Err(("Expected '=>' after match pattern".to_string(), self.current_span));
            }
            let body = Box::new(self.parse_stmt()?);
            arms.push((pattern, body));
            
            // Optional comma after arm
            if self.current_token == Token::Comma {
                self.advance();
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err(("Expected '}' after match arms".to_string(), self.current_span));
        }

        Ok(Stmt::Match {
            expr,
            arms,
        })
    }

    fn parse_unquoted_path(&mut self) -> Result<String, (String, (usize, usize))> {
        let mut path = String::new();
        loop {
            match &self.current_token {
                Token::Dot => path.push('.'),
                Token::Slash => path.push('/'),
                Token::Minus => path.push('-'),
                Token::Colon => path.push(':'),
                Token::Ident(id) => path.push_str(id),
                Token::Int(n) => path.push_str(&n.to_string()),
                _ => break,
            }
            self.advance();
        }
        if path.is_empty() {
            Err(("Expected path".to_string(), self.current_span))
        } else {
            Ok(path)
        }
    }

    fn parse_import_stmt(&mut self) -> Result<Stmt, (String, (usize, usize))> {
        self.advance(); // consume 'import'
        
        let path = match &self.current_token {
            Token::String(s) => {
                let p = s.clone();
                self.advance();
                p
            }
            _ => self.parse_unquoted_path()?,
        };

        let mut alias = None;
        let mut show = None;
        let mut hide = None;

        if self.match_token(Token::As) {
            alias = Some(match &self.current_token {
                Token::Ident(id) => id.clone(),
                _ => return Err(("Expected alias identifier after 'as'".to_string(), self.current_span)),
            });
            self.advance();
        }

        if self.match_token(Token::Show) {
            let mut items = Vec::new();
            loop {
                match &self.current_token {
                    Token::Ident(id) => {
                        items.push(id.clone());
                        self.advance();
                    }
                    _ => return Err(("Expected identifier to show".to_string(), self.current_span)),
                }
                if self.match_token(Token::Comma) {
                    continue;
                }
                break;
            }
            show = Some(items);
        } else if self.match_token(Token::Hide) {
            let mut items = Vec::new();
            loop {
                match &self.current_token {
                    Token::Ident(id) => {
                        items.push(id.clone());
                        self.advance();
                    }
                    _ => return Err(("Expected identifier to hide".to_string(), self.current_span)),
                }
                if self.match_token(Token::Comma) {
                    continue;
                }
                break;
            }
            hide = Some(items);
        }

        if !self.match_token(Token::Semi) {
            return Err(("Expected ';' after import statement".to_string(), self.current_span));
        }

        Ok(Stmt::Import { path, alias, show, hide })
    }

    fn parse_func_decl(&mut self, is_async: bool, visibility: Visibility, is_static: bool) -> Result<Stmt, (String, (usize, usize))> {
        let start_pos = self.current_span.0;
        self.advance(); // consume func

        let name = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err(("Expected function name".to_string(), self.current_span)),
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        if !self.match_token(Token::LParen) {
            return Err(("Expected '(' after function name".to_string(), self.current_span));
        }

        let mut params = Vec::new();
        if self.current_token != Token::RParen {
            loop {
                let param_name = match &self.current_token {
                    Token::Ident(id) => id.clone(),
                    _ => return Err(("Expected parameter name".to_string(), self.current_span)),
                };
                self.advance();

                if !self.match_token(Token::Colon) {
                    return Err(("Expected ':' after parameter name".to_string(), self.current_span));
                }

                let param_type = self.parse_type_annotation()?;

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
            return Err(("Expected ')' after parameters".to_string(), self.current_span));
        }

        let mut return_type = None;
        if self.match_token(Token::Arrow) {
            return_type = Some(self.parse_type_annotation()?);
        }

        if !self.match_token(Token::LBrace) {
            return Err(("Expected '{' before function body".to_string(), self.current_span));
        }

        let mut body = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            match self.parse_stmt() {
                Ok(stmt) => body.push(stmt),
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err(("Expected '}' after function body".to_string(), self.current_span));
        }

        Ok(Stmt::FuncDecl {
            name,
            generic_params,
            params,
            return_type,
            body,
            is_async,
            is_static,
            visibility,
            span: (start_pos, (self.current_span.0 + self.current_span.1).saturating_sub(start_pos)),
        })
    }

    fn parse_class_decl(&mut self) -> Result<Stmt, (String, (usize, usize))> {
        // Assume current_token is Class or Actor
        self.advance();
        let name = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err(("Expected class name".to_string(), self.current_span)),
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        let mut implements = None;
        if self.match_token(Token::Implement) {
            implements = Some(self.parse_type_annotation()?);
        }

        if !self.match_token(Token::LBrace) {
            return Err(("Expected '{' before class body".to_string(), self.current_span));
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            match self.parse_stmt() {
                Ok(stmt) => {
                    match stmt {
                        Stmt::VarDecl { .. } => fields.push(stmt),
                        Stmt::FuncDecl { .. } => methods.push(stmt),
                        _ => {
                            self.errors.push(("Classes can only contain fields and methods".to_string(), self.current_span));
                            self.synchronize();
                        }
                    }
                }
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err(("Expected '}' after class body".to_string(), self.current_span));
        }

        Ok(Stmt::ClassDecl {
            name,
            generic_params,
            fields,
            methods,
            implements,
        })
    }

    fn parse_actor_decl(&mut self) -> Result<Stmt, (String, (usize, usize))> {
        self.advance();
        let name = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err(("Expected actor name".to_string(), self.current_span)),
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        let mut implements = None;
        if self.match_token(Token::Implement) {
            implements = Some(self.parse_type_annotation()?);
        }

        if !self.match_token(Token::LBrace) {
            return Err(("Expected '{' before actor body".to_string(), self.current_span));
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            match self.parse_stmt() {
                Ok(stmt) => {
                    match stmt {
                        Stmt::VarDecl { .. } => fields.push(stmt),
                        Stmt::FuncDecl { .. } => methods.push(stmt),
                        _ => {
                            self.errors.push(("Actors can only contain fields and methods".to_string(), self.current_span));
                            self.synchronize();
                        }
                    }
                }
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err(("Expected '}' after actor body".to_string(), self.current_span));
        }

        Ok(Stmt::ActorDecl {
            name,
            generic_params,
            fields,
            methods,
            implements,
        })
    }

    fn parse_interface_decl(&mut self) -> Result<Stmt, (String, (usize, usize))> {
        self.advance(); // consume interface

        let name = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err(("Expected interface name".to_string(), self.current_span)),
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        if !self.match_token(Token::LBrace) {
            return Err(("Expected '{' before interface body".to_string(), self.current_span));
        }

        let mut methods = Vec::new();

        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let is_async = self.match_token(Token::Async);
            
            if !self.match_token(Token::Func) {
                return Err(("Expected 'func' in interface definition".to_string(), self.current_span));
            }

            let method_name = match &self.current_token {
                Token::Ident(id) => id.clone(),
                _ => return Err(("Expected function name".to_string(), self.current_span)),
            };
            self.advance();

            if !self.match_token(Token::LParen) {
                return Err(("Expected '(' after function name".to_string(), self.current_span));
            }

            let mut params = Vec::new();
            if self.current_token != Token::RParen {
                loop {
                    let param_name = match &self.current_token {
                        Token::Ident(id) => id.clone(),
                        _ => return Err(("Expected parameter name".to_string(), self.current_span)),
                    };
                    self.advance();

                    if !self.match_token(Token::Colon) {
                        return Err(("Expected ':' after parameter name".to_string(), self.current_span));
                    }

                    let param_type = self.parse_type_annotation()?;

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
                return Err(("Expected ')' after parameters".to_string(), self.current_span));
            }

            let mut return_type = None;
            if self.match_token(Token::Arrow) {
                return_type = Some(self.parse_type_annotation()?);
            }

            let mut body = vec![];
            if self.current_token == Token::LBrace {
                self.advance(); // consume '{'
                while self.current_token != Token::RBrace && self.current_token != Token::Eof {
                    match self.parse_stmt() {
                        Ok(stmt) => body.push(stmt),
                        Err(e) => {
                            self.errors.push(e);
                            self.synchronize();
                        }
                    }
                }
                self.match_token(Token::RBrace);
            } else {
                self.match_token(Token::Semi);
            }

            methods.push(Stmt::FuncDecl {
                name: method_name,
                generic_params: None,
                params,
                return_type,
                body,
                is_async,
                is_static: false, // Interfaces methods are inherently non-static for now
                visibility: Visibility::Public,
                span: (0, 0),
            });
        }

        if !self.match_token(Token::RBrace) {
            return Err(("Expected '}' after interface body".to_string(), self.current_span));
        }

        Ok(Stmt::InterfaceDecl {
            name,
            generic_params,
            methods,
        })
    }

    fn parse_enum_decl(&mut self) -> Result<Stmt, (String, (usize, usize))> {
        self.advance(); // consume enum
        
        let name = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err(("Expected enum name".to_string(), self.current_span)),
        };
        self.advance();
        
        let generic_params = if self.match_token(Token::Less) {
            let mut params = Vec::new();
            while self.current_token != Token::Greater && self.current_token != Token::Eof {
                if let Token::Ident(id) = &self.current_token {
                    params.push(id.clone());
                    self.advance();
                } else {
                    return Err(("Expected generic parameter name".to_string(), self.current_span));
                }
                
                if self.match_token(Token::Comma) {
                    continue;
                }
            }
            if !self.match_token(Token::Greater) {
                return Err(("Expected '>' after generic parameters".to_string(), self.current_span));
            }
            Some(params)
        } else {
            None
        };
        
        if !self.match_token(Token::LBrace) {
            return Err(("Expected '{' after enum name".to_string(), self.current_span));
        }
        
        let mut variants = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let variant_name = match &self.current_token {
                Token::Ident(id) => id.clone(),
                _ => return Err(("Expected enum variant name".to_string(), self.current_span)),
            };
            self.advance();
            
            let fields = if self.match_token(Token::LParen) {
                let mut variant_fields = Vec::new();
                while self.current_token != Token::RParen && self.current_token != Token::Eof {
                    variant_fields.push(self.parse_type_annotation()?);
                    if self.match_token(Token::Comma) {
                        continue;
                    }
                }
                if !self.match_token(Token::RParen) {
                    return Err(("Expected ')' after enum variant fields".to_string(), self.current_span));
                }
                Some(variant_fields)
            } else {
                None
            };
            
            use pace_ast::EnumVariant;
            variants.push(EnumVariant { name: variant_name, fields });
            
            // Variants can optionally end with a comma
            self.match_token(Token::Comma);
        }
        
        if !self.match_token(Token::RBrace) {
            return Err(("Expected '}' after enum body".to_string(), self.current_span));
        }
        
        Ok(Stmt::EnumDecl { name, generic_params, variants })
    }

    fn parse_struct_decl(&mut self) -> Result<Stmt, (String, (usize, usize))> {
        self.advance(); // consume struct

        let name = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err(("Expected struct name".to_string(), self.current_span)),
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        if !self.match_token(Token::LBrace) {
            return Err(("Expected '{' before struct body".to_string(), self.current_span));
        }

        let mut fields = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let stmt = self.parse_stmt()?;
            match stmt {
                Stmt::VarDecl { .. } => fields.push(stmt),
                _ => return Err(("Structs can only contain fields".to_string(), self.current_span)),
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err(("Expected '}' after struct body".to_string(), self.current_span));
        }

        Ok(Stmt::StructDecl {
            name,
            generic_params,
            fields,
        })
    }

    fn parse_var_decl(&mut self, is_mutable: bool, is_static: bool) -> Result<Stmt, (String, (usize, usize))> {
        let start_pos = self.current_span.0;
        self.advance(); // consume let/var
        let name = match &self.current_token {
            Token::Ident(id) => id.clone(),
            _ => return Err(("Expected identifier".to_string(), self.current_span)),
        };
        self.advance();

        let mut type_annotation = None;
        if self.match_token(Token::Colon) {
            type_annotation = Some(self.parse_type_annotation()?);
        }

        let mut initializer = None;
        if self.match_token(Token::Eq) {
            initializer = Some(self.parse_expr()?);
        }

        if !self.match_token(Token::Semi) {
            return Err(("Expected ';' after variable declaration".to_string(), self.current_span));
        }

        Ok(Stmt::VarDecl {
            name,
            is_mutable,
            type_annotation,
            is_static,
            initializer,
            span: (start_pos, (self.current_span.0 + self.current_span.1).saturating_sub(start_pos)),
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, (String, (usize, usize))> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, (String, (usize, usize))> {
        let expr = self.parse_null_coalesce()?;
        
        if self.match_token(Token::Eq) {
            let value = self.parse_assignment()?; // Right-associative
            
            match expr {
                Expr::Identifier(_) | Expr::MemberAccess { .. } => {
                    return Ok(Expr::Assign {
                        target: Box::new(expr),
                        value: Box::new(value),
                    });
                }
                _ => return Err(("Invalid assignment target".to_string(), self.current_span)),
            }
        }
        
        Ok(expr)
    }

    fn parse_null_coalesce(&mut self) -> Result<Expr, (String, (usize, usize))> {
        let mut expr = self.parse_logical_or()?;
        while self.match_token(Token::QuestionQuestion) {
            let right = self.parse_logical_or()?;
            expr = Expr::NullCoalesce {
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_logical_or(&mut self) -> Result<Expr, (String, (usize, usize))> {
        let mut expr = self.parse_logical_and()?;
        while self.match_token(Token::PipePipe) {
            let right = self.parse_logical_and()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::Or,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, (String, (usize, usize))> {
        let mut expr = self.parse_equality()?;
        while self.match_token(Token::AndAnd) {
            let right = self.parse_equality()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::And,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, (String, (usize, usize))> {
        let mut expr = self.parse_relational()?;
        while self.current_token == Token::EqEq || self.current_token == Token::NotEq {
            let op = if self.current_token == Token::EqEq { BinaryOp::Eq } else { BinaryOp::NotEq };
            self.advance();
            let right = self.parse_relational()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_relational(&mut self) -> Result<Expr, (String, (usize, usize))> {
        let mut expr = self.parse_term()?;
        while matches!(self.current_token, Token::Less | Token::LessEq | Token::Greater | Token::GreaterEq) {
            let op = match self.current_token {
                Token::Less => BinaryOp::Less,
                Token::LessEq => BinaryOp::LessEq,
                Token::Greater => BinaryOp::Greater,
                Token::GreaterEq => BinaryOp::GreaterEq,
                _ => unreachable!(),
            };
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

    fn parse_term(&mut self) -> Result<Expr, (String, (usize, usize))> {
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

    fn parse_factor(&mut self) -> Result<Expr, (String, (usize, usize))> {
        let mut expr = self.parse_unary()?;
        while self.current_token == Token::Star || self.current_token == Token::Slash || self.current_token == Token::Mod {
            let op = if self.current_token == Token::Star { 
                BinaryOp::Mul 
            } else if self.current_token == Token::Slash { 
                BinaryOp::Div 
            } else { 
                BinaryOp::Mod 
            };
            self.advance();
            let right = self.parse_unary()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, (String, (usize, usize))> {
        if self.match_token(Token::Await) {
            let expr = self.parse_unary()?;
            Ok(Expr::Await(Box::new(expr)))
        } else {
            self.parse_postfix()
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, (String, (usize, usize))> {
        let mut expr = self.parse_primary()?;
        
        loop {
            if self.match_token(Token::Bang) {
                expr = Expr::Unwrap(Box::new(expr));
            } else if self.match_token(Token::Question) {
                expr = Expr::Try(Box::new(expr));
            } else if self.match_token(Token::Dot) {
                let property = match &self.current_token {
                    Token::Ident(id) => id.clone(),
                    _ => return Err(("Expected property name after '.'".to_string(), self.current_span)),
                };
                self.advance();
                expr = Expr::MemberAccess {
                    object: Box::new(expr),
                    property,
                    computed_class: None,
                    is_static_operator: false,
                };
            } else if self.match_token(Token::ColonColon) {
                let property = match &self.current_token {
                    Token::Ident(id) => id.clone(),
                    _ => return Err(("Expected variant name after '::'".to_string(), self.current_span)),
                };
                self.advance();
                expr = Expr::MemberAccess {
                    object: Box::new(expr),
                    property,
                    computed_class: None,
                    is_static_operator: true,
                };
            } else if self.match_token(Token::QuestionDot) {
                let property = match &self.current_token {
                    Token::Ident(id) => id.clone(),
                    _ => return Err(("Expected property name after '?.'".to_string(), self.current_span)),
                };
                self.advance();
                expr = Expr::OptionalMemberAccess {
                    object: Box::new(expr),
                    property,
                };
            } else if self.current_token == Token::Less {
                let backup_lexer = self.lexer.clone();
                let backup_token = self.current_token.clone();
                let backup_span = self.current_span;
                let backup_errors = self.errors.clone();
                
                self.advance();
                let mut generic_args = Vec::new();
                let mut success = false;
                
                if self.current_token != Token::Greater {
                    loop {
                        match self.parse_type_annotation() {
                            Ok(ty) => generic_args.push(ty),
                            Err(_) => break,
                        }
                        if self.match_token(Token::Comma) {
                            continue;
                        }
                        break;
                    }
                }
                
                if self.match_token(Token::Greater) {
                    success = true;
                    expr = Expr::GenericInstantiation {
                        callee: Box::new(expr),
                        generic_args,
                    };
                }
                
                if !success {
                    // Backtrack, it's a Less-Than binary operator!
                    self.lexer = backup_lexer;
                    self.current_token = backup_token;
                    self.current_span = backup_span;
                    self.errors = backup_errors;
                    break;
                }
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
                    return Err(("Expected ')' after arguments".to_string(), self.current_span));
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

    fn parse_primary(&mut self) -> Result<Expr, (String, (usize, usize))> {
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
                let span = self.current_span;
                self.advance();
                Self::parse_interpolated_string(v, span)
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
                    return Err(("Expected ')'".to_string(), self.current_span));
                }
                Ok(expr)
            }
            _ => Err((format!("Unexpected token: {:?}", self.current_token), self.current_span)),
        }
    }

    fn parse_interpolated_string(s: String, base_span: (usize, usize)) -> Result<Expr, (String, (usize, usize))> {
        let mut parts = Vec::new();
        let mut current_text = String::new();
        let mut chars = s.chars().peekable();
        
        while let Some(c) = chars.next() {
            if c == '$' && chars.peek() == Some(&'{') {
                chars.next(); // consume '{'
                if !current_text.is_empty() {
                    parts.push(Expr::StringLiteral(current_text.clone()));
                    current_text.clear();
                }
                
                let mut expr_str = String::new();
                let mut depth = 1;
                for inner_c in chars.by_ref() {
                    if inner_c == '{' { depth += 1; }
                    if inner_c == '}' {
                        depth -= 1;
                        if depth == 0 { break; }
                    }
                    expr_str.push(inner_c);
                }
                
                if depth > 0 {
                    return Err(("Unclosed string interpolation block, expected '}'".to_string(), base_span));
                }
                
                let mut nested_parser = Parser::new(&expr_str);
                let expr = nested_parser.parse_expr().map_err(|(msg, _)| {
                    (format!("In interpolated string: {}", msg), base_span)
                })?;
                
                if nested_parser.current_token != Token::Eof {
                    return Err(("Unexpected tokens in interpolated string".to_string(), base_span));
                }
                
                parts.push(expr);
            } else {
                current_text.push(c);
            }
        }
        
        if !current_text.is_empty() {
            parts.push(Expr::StringLiteral(current_text));
        }
        
        if parts.len() == 1 {
            if let Expr::StringLiteral(_) = parts[0] {
                return Ok(parts.pop().unwrap());
            }
        } else if parts.is_empty() {
            return Ok(Expr::StringLiteral(String::new()));
        }
        
        Ok(Expr::InterpolatedString(parts))
    }
}
