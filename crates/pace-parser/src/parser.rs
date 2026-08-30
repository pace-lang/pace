use crate::lexer::{Lexer, Token};
use pace_ast::{BinaryOp, Expr, Param, Stmt, TypeAnnotation, Visibility, arena::AstArena};

pub struct Parser<'a, 'b> {
    pub lexer: Lexer<'a>,
    pub current_token: Token<'a>,
    pub current_span: (usize, usize),
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
            current_span,
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
        self.current_span = span;
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

    fn parse_stmt(&mut self) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        let mut doc_comment = None;
        while let Token::DocComment(c) = &self.current_token {
            let existing = doc_comment.get_or_insert_with(String::new);
            if !existing.is_empty() {
                existing.push('\n');
            }
            existing.push_str(c);
            self.advance();
        }

        let visibility = if self.match_token(Token::Private) {
            Visibility::Private
        } else {
            Visibility::Public // Default is public
        };

        let is_async = self.match_token(Token::Async);
        let is_static = self.match_token(Token::Static);

        match self.current_token {
            Token::Let => self.parse_var_decl(false, is_static, visibility), // doc_comments on vars omitted for now
            Token::Var => self.parse_var_decl(true, is_static, visibility),
            Token::Func => self.parse_func_decl(is_async, visibility, is_static, doc_comment.as_deref().map(ustr::Ustr::from)),
            Token::Class => self.parse_class_decl(doc_comment.as_deref().map(ustr::Ustr::from)),
            Token::Actor => self.parse_actor_decl(doc_comment.as_deref().map(ustr::Ustr::from)),
            Token::Interface => self.parse_interface_decl(doc_comment.as_deref().map(ustr::Ustr::from)),
            Token::Struct => self.parse_struct_decl(doc_comment.as_deref().map(ustr::Ustr::from)),
            Token::Enum => self.parse_enum_decl(doc_comment.as_deref().map(ustr::Ustr::from)),
            Token::If => self.parse_if_stmt(),
            Token::While => self.parse_while_stmt(),
            Token::Loop => self.parse_loop_stmt(),
            Token::For => self.parse_for_stmt(),
            Token::Match => self.parse_match_stmt(),
            Token::Import => self.parse_import_stmt(),
            Token::Export => self.parse_export_stmt(),
            Token::LBrace => self.parse_block(),
            Token::Return => {
                self.advance();
                let expr = if self.current_token != Token::Semi {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                if !self.match_token(Token::Semi) {
                    return Err(pace_errors::SyntaxError {
                        message: "Expected ';' after return".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
                Ok(self.alloc_stmt(Stmt::Return(expr)))
            }
            _ => {
                let expr = self.parse_expr()?;
                if !self.match_token(Token::Semi) {
                    if let pace_ast::Expr::Call { callee, .. } = self.arena.get_expr(expr)
                        && (self.current_token == Token::LBrace || self.current_token == Token::Arrow)
                            && let Expr::Identifier(name, _) = self.arena.get_expr(*callee) {
                                return Err(pace_errors::SyntaxError {
                                    message: format!(
                                        "Functions and methods must be prefixed with 'func'. Did you forget 'func' before '{}'?",
                                        name
                                    ),
                                    src: miette::NamedSource::new(
                                        self.file_name.clone(),
                                        self.src.to_string(),
                                    ),
                                    span: self.current_span,
                                });
                            }
                    return Err(pace_errors::SyntaxError {
                        message: "Expected ';' after expression".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
                Ok(self.alloc_stmt(Stmt::Expr(expr)))
            }
        }
    }

    fn parse_generic_params(&mut self) -> Result<Option<Vec<ustr::Ustr>>, pace_errors::SyntaxError> {
        if self.match_token(Token::Less) {
            let mut params = Vec::new();
            while self.current_token != Token::Greater && self.current_token != Token::Eof {
                if let Token::Ident(id) = &self.current_token {
                    params.push((*id).into());
                    self.advance();
                } else {
                    return Err(pace_errors::SyntaxError {
                        message: "Expected generic parameter name".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
                if !self.match_token(Token::Comma) {
                    break;
                }
            }
            if !self.match_token(Token::Greater) {
                return Err(pace_errors::SyntaxError {
                    message: "Expected '>' after generic parameters".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
            Ok(Some(params))
        } else {
            Ok(None)
        }
    }

    fn parse_type_annotation(&mut self) -> Result<TypeAnnotation, pace_errors::SyntaxError> {
        // Parse function types like (Int, String) -> Bool
        if self.match_token(Token::LParen) {
            let mut params = Vec::new();
            while self.current_token != Token::RParen && self.current_token != Token::Eof {
                params.push(self.parse_type_annotation()?);
                if !self.match_token(Token::Comma) {
                    break;
                }
            }
            if !self.match_token(Token::RParen) {
                return Err(pace_errors::SyntaxError {
                    message: "Expected ')' after function type parameters".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
            if !self.match_token(Token::Arrow) {
                return Err(pace_errors::SyntaxError {
                    message: "Expected '->' after function parameters".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
            let return_type = self.parse_type_annotation()?;

            return Ok(TypeAnnotation {
                module_prefix: None,
                name: "Function".to_string().into(),
                args: vec![],
                is_nullable: false,
                is_function: true,
                function_params: Some(params),
                function_return: Some(Box::new(return_type)),
            });
        }

        let mut name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected type name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let mut module_prefix = None;
        if self.match_token(Token::Dot) {
            module_prefix = Some(name);
            name = match &self.current_token {
                Token::Ident(id) => *id,
                _ => {
                    return Err(pace_errors::SyntaxError {
                        message: "Expected type name after '.'".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
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
                return Err(pace_errors::SyntaxError {
                    message: "Expected '>' after generic arguments".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        }

        let is_nullable = self.match_token(Token::Question);

        Ok(TypeAnnotation {
            module_prefix: module_prefix.map(ustr::Ustr::from),
            name: name.into(),
            args,
            is_nullable,
            is_function: false,
            function_params: None,
            function_return: None,
        })
    }

    fn parse_block(&mut self) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
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
            return Err(pace_errors::SyntaxError {
                message: "Expected '}' after block".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }
        Ok(self.alloc_stmt(Stmt::Block(stmts)))
    }

    fn parse_if_stmt(&mut self) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume if
        let condition = self.parse_expr()?;

        let then_branch = self.parse_stmt()?;

        let mut else_branch = None;
        if self.match_token(Token::Else) {
            else_branch = Some(self.parse_stmt()?);
        }

        Ok(self.alloc_stmt(Stmt::If {
            condition,
            then_branch,
            else_branch,
        }))
    }

    fn parse_while_stmt(&mut self) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume while
        let condition = self.parse_expr()?;
        let body = self.parse_stmt()?;
        Ok(self.alloc_stmt(Stmt::While { condition, body }))
    }

    fn parse_loop_stmt(&mut self) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume loop
        let body = self.parse_stmt()?;
        Ok(self.alloc_stmt(Stmt::Loop { body }))
    }

    fn parse_for_stmt(&mut self) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume for
        let item = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected identifier in for loop".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        if !self.match_token(Token::In) {
            return Err(pace_errors::SyntaxError {
                message: "Expected 'in' in for loop".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let iterable = self.parse_expr()?;
        let body = self.parse_stmt()?;

        Ok(self.alloc_stmt(Stmt::ForIn {
            item: item.into(),
            iterable,
            body,
        }))
    }

    fn parse_pattern(&mut self) -> Result<pace_ast::Pattern, pace_errors::SyntaxError> {
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
                        return Err(pace_errors::SyntaxError {
                            message: "Expected '>' after generic arguments in pattern".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                    }
                    generic_args = Some(args);
                }

                // Could be a variable, or an Enum variant like `Some(x)` or `Option::Some(x)`
                // Let's check for `::`
                let mut enum_name = None;
                let mut variant_name = name;

                if self.current_token == Token::ColonColon {
                    self.advance();
                    if let Token::Ident(v_name) = self.current_token.clone() {
                        self.advance();
                        enum_name = Some(name);
                        variant_name = v_name;
                    } else {
                        return Err(pace_errors::SyntaxError {
                            message: "Expected variant name after :: in pattern".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
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
                        return Err(pace_errors::SyntaxError {
                            message: "Expected ')' after pattern fields".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                    }
                    Ok(pace_ast::Pattern::Variant {
                        enum_name: enum_name.map(ustr::Ustr::from),
                        variant_name: variant_name.into(),
                        fields: Some(fields),
                        generic_args,
                    })
                } else if enum_name.is_some() || variant_name.chars().next().unwrap().is_uppercase()
                {
                    // It's a variant without fields (like `None`)
                    Ok(pace_ast::Pattern::Variant {
                        enum_name: enum_name.map(ustr::Ustr::from),
                        variant_name: variant_name.into(),
                        fields: None,
                        generic_args,
                    })
                } else {
                    // Lowercase without parenthesis -> Variable binding
                    Ok(pace_ast::Pattern::Variable(variant_name.into(), self.current_span.into()))
                }
            }
            Token::Int(_) | Token::Float(_) | Token::String(_) | Token::Bool(_) => {
                let expr = self.parse_primary()?;
                Ok(pace_ast::Pattern::Literal(expr))
            }
            _ => Err(pace_errors::SyntaxError {
                message: "Expected pattern".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            }),
        }
    }

    fn parse_match_stmt(&mut self) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume match
        let expr = self.parse_expr()?;

        if !self.match_token(Token::LBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '{' before match arms".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut arms = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let pattern = self.parse_pattern()?;
            if !self.match_token(Token::FatArrow) && !self.match_token(Token::Arrow) {
                return Err(pace_errors::SyntaxError {
                    message: "Expected '=>' after match pattern".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
            let body = self.parse_stmt()?;
            arms.push((pattern, body));

            // Optional comma after arm
            if self.current_token == Token::Comma {
                self.advance();
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '}' after match arms".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::Match { expr, arms }))
    }

    fn parse_import_stmt(&mut self) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume 'import'

        let path = match &self.current_token {
            Token::String(s) => {
                let p = s.clone();
                self.advance();
                p
            }
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Import path must be a quoted string".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };

        let mut alias = None;
        let mut show = None;
        let mut hide = None;

        if self.match_token(Token::As) {
            alias = Some(match &self.current_token {
                Token::Ident(id) => *id,
                _ => {
                    return Err(pace_errors::SyntaxError {
                        message: "Expected alias identifier after 'as'".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
            });
            self.advance();
        }

        if self.match_token(Token::Show) {
            let mut items = Vec::new();
            loop {
                match &self.current_token {
                    Token::Ident(id) => {
                        items.push(*id);
                        self.advance();
                    }
                    _ => {
                        return Err(pace_errors::SyntaxError {
                            message: "Expected identifier to show".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                    }
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
                        items.push(*id);
                        self.advance();
                    }
                    _ => {
                        return Err(pace_errors::SyntaxError {
                            message: "Expected identifier to hide".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                    }
                }
                if self.match_token(Token::Comma) {
                    continue;
                }
                break;
            }
            hide = Some(items);
        }

        if !self.match_token(Token::Semi) {
            return Err(pace_errors::SyntaxError {
                message: "Expected ';' after import statement".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::Import {
            path: path.into(),
            alias: alias.map(ustr::Ustr::from),
            show: show.map(|v| v.iter().map(|s| ustr::Ustr::from(s)).collect()),
            hide: hide.map(|v| v.iter().map(|s| ustr::Ustr::from(s)).collect()),
        }))
    }

    fn parse_export_stmt(&mut self) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume 'export'

        let path = match &self.current_token {
            Token::String(s) => {
                let p = s.clone();
                self.advance();
                p
            }
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Export path must be a quoted string".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };

        if !self.match_token(Token::Semi) {
            return Err(pace_errors::SyntaxError {
                message: "Expected ';' after export statement".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::Export { path: path.into() }))
    }

    fn parse_func_decl(
        &mut self,
        is_async: bool,
        visibility: Visibility,
        is_static: bool,
        doc_comment: Option<ustr::Ustr>,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        let start_pos = self.current_span.0;
        self.advance(); // consume func

        let name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected function name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        if !self.match_token(Token::LParen) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '(' after function name".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut params = Vec::new();
        if self.current_token != Token::RParen {
            loop {
                let param_name = match &self.current_token {
                    Token::Ident(id) => *id,
                    _ => {
                        return Err(pace_errors::SyntaxError {
                            message: "Expected parameter name".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                    }
                };
                self.advance();

                if !self.match_token(Token::Colon) {
                    return Err(pace_errors::SyntaxError {
                        message: "Expected ':' after parameter name".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }

                let param_type = self.parse_type_annotation()?;

                params.push(Param {
                    name: param_name.into(),
                    type_annotation: param_type,
                });

                if !self.match_token(Token::Comma) {
                    break;
                }
            }
        }

        if !self.match_token(Token::RParen) {
            return Err(pace_errors::SyntaxError {
                message: "Expected ')' after parameters".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut return_type = None;
        if self.match_token(Token::Arrow) {
            return_type = Some(self.parse_type_annotation()?);
        }

        if !self.match_token(Token::LBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '{' before function body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
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
            return Err(pace_errors::SyntaxError {
                message: "Expected '}' after function body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::FuncDecl {
            name: name.into(),
            generic_params,
            params,
            return_type,
            body,
            is_async,
            is_static,
            visibility,
            doc_comment,
            span: pace_ast::Span::new(
                start_pos,
                (self.current_span.0 + self.current_span.1).saturating_sub(start_pos),
            ),
        }))
    }

    fn parse_class_decl(
        &mut self,
        doc_comment: Option<ustr::Ustr>,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        // Assume current_token is Class or Actor
        self.advance();
        let name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected class name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        let mut implements = None;
        if self.match_token(Token::Implement) {
            implements = Some(self.parse_type_annotation()?);
        }

        if !self.match_token(Token::LBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '{' before class body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            match self.parse_stmt() {
                Ok(stmt) => match self.arena.get_stmt(stmt) {
                    Stmt::VarDecl { .. } => fields.push(stmt),
                    Stmt::FuncDecl { .. } => methods.push(stmt),
                    Stmt::Expr(expr_id) => {
                        if let pace_ast::Expr::Call { callee, .. } = self.arena.get_expr(*expr_id) {
                            if let pace_ast::Expr::Identifier(name, _) = self.arena.get_expr(*callee) {
                                self.errors.push(pace_errors::SyntaxError { message: format!("Methods must be prefixed with 'func'. Did you forget 'func' before '{}'?", name), src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()), span: self.current_span });
                            } else {
                                self.errors.push(pace_errors::SyntaxError {
                                    message: "Classes can only contain fields and methods".to_string(),
                                    src: miette::NamedSource::new(
                                        self.file_name.clone(),
                                        self.src.to_string(),
                                ),
                                span: self.current_span,
                            });
                        }
                        }
                        self.synchronize();
                    }
                    _ => {
                        self.errors.push(pace_errors::SyntaxError {
                            message: "Classes can only contain fields and methods".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                        self.synchronize();
                    }
                },
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '}' after class body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::ClassDecl {
            name: name.into(),
            generic_params,
            fields,
            methods,
            implements,
            doc_comment,
        }))
    }

    fn parse_actor_decl(
        &mut self,
        doc_comment: Option<ustr::Ustr>,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance();
        let name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected actor name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        let mut implements = None;
        if self.match_token(Token::Implement) {
            implements = Some(self.parse_type_annotation()?);
        }

        if !self.match_token(Token::LBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '{' before actor body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            match self.parse_stmt() {
                Ok(stmt) => match self.arena.get_stmt(stmt) {
                    Stmt::VarDecl { .. } => fields.push(stmt),
                    Stmt::FuncDecl { .. } => methods.push(stmt),
                    Stmt::Expr(expr_id) => {
                        if let pace_ast::Expr::Call { callee, .. } = self.arena.get_expr(*expr_id) {
                            if let pace_ast::Expr::Identifier(name, _) = self.arena.get_expr(*callee) {
                                self.errors.push(pace_errors::SyntaxError { message: format!("Methods must be prefixed with 'func'. Did you forget 'func' before '{}'?", name), src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()), span: self.current_span });
                            } else {
                                self.errors.push(pace_errors::SyntaxError {
                                    message: "Actors can only contain fields and methods".to_string(),
                                    src: miette::NamedSource::new(
                                        self.file_name.clone(),
                                        self.src.to_string(),
                                ),
                                span: self.current_span,
                            });
                        }
                        }
                        self.synchronize();
                    }
                    _ => {
                        self.errors.push(pace_errors::SyntaxError {
                            message: "Actors can only contain fields and methods".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                        self.synchronize();
                    }
                },
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '}' after actor body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::ActorDecl {
            name: name.into(),
            generic_params,
            fields,
            methods,
            implements,
            doc_comment,
        }))
    }

    fn parse_interface_decl(
        &mut self,
        doc_comment: Option<ustr::Ustr>,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume interface

        let name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected interface name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        if !self.match_token(Token::LBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '{' before interface body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut methods = Vec::new();

        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let is_async = self.match_token(Token::Async);

            if !self.match_token(Token::Func) {
                return Err(pace_errors::SyntaxError {
                    message: "Expected 'func' in interface definition".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }

            let method_name = match &self.current_token {
                Token::Ident(id) => *id,
                _ => {
                    return Err(pace_errors::SyntaxError {
                        message: "Expected function name".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
            };
            self.advance();

            if !self.match_token(Token::LParen) {
                return Err(pace_errors::SyntaxError {
                    message: "Expected '(' after function name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }

            let mut params = Vec::new();
            if self.current_token != Token::RParen {
                loop {
                    let param_name = match &self.current_token {
                        Token::Ident(id) => *id,
                        _ => {
                            return Err(pace_errors::SyntaxError {
                                message: "Expected parameter name".to_string(),
                                src: miette::NamedSource::new(
                                    self.file_name.clone(),
                                    self.src.to_string(),
                                ),
                                span: self.current_span,
                            });
                        }
                    };
                    self.advance();

                    if !self.match_token(Token::Colon) {
                        return Err(pace_errors::SyntaxError {
                            message: "Expected ':' after parameter name".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                    }

                    let param_type = self.parse_type_annotation()?;

                    params.push(Param {
                        name: param_name.into(),
                        type_annotation: param_type,
                    });

                    if !self.match_token(Token::Comma) {
                        break;
                    }
                }
            }

            if !self.match_token(Token::RParen) {
                return Err(pace_errors::SyntaxError {
                    message: "Expected ')' after parameters".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
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

            methods.push(self.alloc_stmt(Stmt::FuncDecl {
                name: method_name.into(),
                generic_params: None,
                params,
                return_type,
                body,
                is_async,
                is_static: false, // Interfaces methods are inherently non-static for now
                visibility: Visibility::Public,
                doc_comment: None,
                span: pace_ast::Span::default(),
            }));
        }

        if !self.match_token(Token::RBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '}' after interface body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::InterfaceDecl {
            name: name.into(),
            generic_params,
            methods,
            doc_comment,
        }))
    }

    fn parse_enum_decl(
        &mut self,
        doc_comment: Option<ustr::Ustr>,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume enum

        let name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected enum name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let generic_params = if self.match_token(Token::Less) {
            let mut params = Vec::new();
            while self.current_token != Token::Greater && self.current_token != Token::Eof {
                if let Token::Ident(id) = &self.current_token {
                    params.push((*id).into());
                    self.advance();
                } else {
                    return Err(pace_errors::SyntaxError {
                        message: "Expected generic parameter name".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }

                if self.match_token(Token::Comma) {
                    continue;
                }
            }
            if !self.match_token(Token::Greater) {
                return Err(pace_errors::SyntaxError {
                    message: "Expected '>' after generic parameters".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
            Some(params)
        } else {
            None
        };

        if !self.match_token(Token::LBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '{' after enum name".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut variants = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let variant_name = match &self.current_token {
                Token::Ident(id) => *id,
                _ => {
                    return Err(pace_errors::SyntaxError {
                        message: "Expected enum variant name".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
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
                    return Err(pace_errors::SyntaxError {
                        message: "Expected ')' after enum variant fields".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
                Some(variant_fields)
            } else {
                None
            };

            use pace_ast::EnumVariant;
            variants.push(EnumVariant {
                name: variant_name.into(),
                fields,
            });

            // Variants can optionally end with a comma
            self.match_token(Token::Comma);
        }

        if !self.match_token(Token::RBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '}' after enum body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::EnumDecl {
            name: name.into(),
            generic_params,
            variants,
            doc_comment,
        }))
    }

    fn parse_struct_decl(
        &mut self,
        doc_comment: Option<ustr::Ustr>,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        self.advance(); // consume struct

        let name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected struct name".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
        };
        self.advance();

        let generic_params = self.parse_generic_params()?;

        if !self.match_token(Token::LBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '{' before struct body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        let mut fields = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let stmt = self.parse_stmt()?;
            match self.arena.get_stmt(stmt) {
                Stmt::VarDecl { .. } => fields.push(stmt),
                _ => {
                    return Err(pace_errors::SyntaxError {
                        message: "Structs can only contain fields".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
            }
        }

        if !self.match_token(Token::RBrace) {
            return Err(pace_errors::SyntaxError {
                message: "Expected '}' after struct body".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::StructDecl {
            name: name.into(),
            generic_params,
            fields,
            doc_comment,
        }))
    }

    fn parse_var_decl(
        &mut self,
        is_mutable: bool,
        is_static: bool,
        visibility: Visibility,
    ) -> Result<pace_ast::arena::StmtId, pace_errors::SyntaxError> {
        let start_pos = self.current_span.0;
        self.advance(); // consume let/var
        let name = match &self.current_token {
            Token::Ident(id) => *id,
            _ => {
                return Err(pace_errors::SyntaxError {
                    message: "Expected identifier".to_string(),
                    src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                    span: self.current_span,
                });
            }
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
            return Err(pace_errors::SyntaxError {
                message: "Expected ';' after variable declaration".to_string(),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            });
        }

        Ok(self.alloc_stmt(Stmt::VarDecl {
            name: name.into(),
            is_mutable,
            type_annotation,
            is_static,
            visibility,
            initializer,
            span: pace_ast::Span::new(
                start_pos,
                (self.current_span.0 + self.current_span.1).saturating_sub(start_pos),
            ),
        }))
    }

    fn parse_expr(&mut self) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let expr = self.parse_null_coalesce()?;

        if self.match_token(Token::Eq) {
            let value = self.parse_assignment()?; // Right-associative

            match self.arena.get_expr(expr) {
                Expr::Identifier(_, _) | Expr::MemberAccess { .. } => {
                    return Ok(self.alloc_expr(Expr::Assign {
                        target: expr,
                        value,
                    }));
                }
                _ => {
                    return Err(pace_errors::SyntaxError {
                        message: "Invalid assignment target".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
            }
        }

        Ok(expr)
    }

    fn parse_null_coalesce(&mut self) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_logical_or()?;
        while self.match_token(Token::QuestionQuestion) {
            let right = self.parse_logical_or()?;
            expr = self.alloc_expr(Expr::NullCoalesce {
                left: expr,
                right,
            });
        }
        Ok(expr)
    }

    fn parse_logical_or(&mut self) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_logical_and()?;
        while self.match_token(Token::PipePipe) {
            let right = self.parse_logical_and()?;
            expr = self.alloc_expr(Expr::Binary {
                left: expr,
                op: BinaryOp::Or,
                right,
            });
        }
        Ok(expr)
    }

    fn parse_logical_and(&mut self) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_equality()?;
        while self.match_token(Token::AndAnd) {
            let right = self.parse_equality()?;
            expr = self.alloc_expr(Expr::Binary {
                left: expr,
                op: BinaryOp::And,
                right,
            });
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_relational()?;
        while self.current_token == Token::EqEq || self.current_token == Token::NotEq {
            let op = if self.current_token == Token::EqEq {
                BinaryOp::Eq
            } else {
                BinaryOp::NotEq
            };
            self.advance();
            let right = self.parse_relational()?;
            expr = self.alloc_expr(Expr::Binary {
                left: expr,
                op,
                right,
            });
        }
        Ok(expr)
    }

    fn parse_relational(&mut self) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_term()?;
        while matches!(
            self.current_token,
            Token::Less | Token::LessEq | Token::Greater | Token::GreaterEq
        ) {
            let op = match self.current_token {
                Token::Less => BinaryOp::Less,
                Token::LessEq => BinaryOp::LessEq,
                Token::Greater => BinaryOp::Greater,
                Token::GreaterEq => BinaryOp::GreaterEq,
                _ => unreachable!(),
            };
            self.advance();
            let right = self.parse_term()?;
            expr = self.alloc_expr(Expr::Binary {
                left: expr,
                op,
                right,
            });
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_factor()?;
        while self.current_token == Token::Plus || self.current_token == Token::Minus {
            let op = if self.current_token == Token::Plus {
                BinaryOp::Add
            } else {
                BinaryOp::Sub
            };
            self.advance();
            let right = self.parse_factor()?;
            expr = self.alloc_expr(Expr::Binary {
                left: expr,
                op,
                right,
            });
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_unary()?;
        while self.current_token == Token::Star
            || self.current_token == Token::Slash
            || self.current_token == Token::Mod
        {
            let op = if self.current_token == Token::Star {
                BinaryOp::Mul
            } else if self.current_token == Token::Slash {
                BinaryOp::Div
            } else {
                BinaryOp::Mod
            };
            self.advance();
            let right = self.parse_unary()?;
            expr = self.alloc_expr(Expr::Binary {
                left: expr,
                op,
                right,
            });
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        if self.match_token(Token::Await) {
            let expr = self.parse_unary()?;
            Ok(self.alloc_expr(Expr::Await(expr)))
        } else {
            self.parse_postfix()
        }
    }

    fn parse_postfix(&mut self) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.match_token(Token::Bang) {
                expr = self.alloc_expr(Expr::Unwrap(expr));
            } else if self.match_token(Token::Question) {
                expr = self.alloc_expr(Expr::Try(expr));
            } else if self.match_token(Token::Dot) {
                let property = match &self.current_token {
                    Token::Ident(id) => *id,
                    _ => {
                        return Err(pace_errors::SyntaxError {
                            message: "Expected property name after '.'".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                    }
                };
                self.advance();
                expr = self.alloc_expr(Expr::MemberAccess {
                    object: expr,
                    property: property.into(),
                    computed_class: None,
                    is_static_operator: false,
                });
            } else if self.match_token(Token::ColonColon) {
                let property = match &self.current_token {
                    Token::Ident(id) => *id,
                    _ => {
                        return Err(pace_errors::SyntaxError {
                            message: "Expected variant name after '::'".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                    }
                };
                self.advance();
                expr = self.alloc_expr(Expr::MemberAccess {
                    object: expr,
                    property: property.into(),
                    computed_class: None,
                    is_static_operator: true,
                });
            } else if self.match_token(Token::QuestionDot) {
                let property = match &self.current_token {
                    Token::Ident(id) => *id,
                    _ => {
                        return Err(pace_errors::SyntaxError {
                            message: "Expected property name after '?.'".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                    }
                };
                self.advance();
                expr = self.alloc_expr(Expr::OptionalMemberAccess {
                    object: expr,
                    property: property.into(),
                });
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
                    expr = self.alloc_expr(Expr::GenericInstantiation {
                        callee: expr,
                        generic_args,
                    });
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
                    if self.current_token == Token::Colon {
                        return Err(pace_errors::SyntaxError { message: "Expected ')' after arguments. (Note: if you are declaring a function, ensure it is prefixed with the 'func' keyword)".to_string(), src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()), span: self.current_span });
                    }
                    return Err(pace_errors::SyntaxError {
                        message: "Expected ')' after arguments".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
                expr = self.alloc_expr(Expr::Call {
                    callee: expr,
                    args,
                });
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        match &self.current_token {
            Token::Int(n) => {
                let v = *n;
                self.advance();
                Ok(self.alloc_expr(Expr::IntLiteral(v)))
            }
            Token::Float(f) => {
                let v = *f;
                self.advance();
                Ok(self.alloc_expr(Expr::FloatLiteral(v)))
            }
            Token::String(s) => {
                let v = s.clone();
                let span = self.current_span;
                self.advance();
                self.parse_interpolated_string(v, span)
            }
            Token::Bool(b) => {
                let v = *b;
                self.advance();
                Ok(self.alloc_expr(Expr::BoolLiteral(v)))
            }
            Token::Null => {
                self.advance();
                Ok(self.alloc_expr(Expr::Null))
            }
            Token::Ident(id) => {
                let v = *id;
                let span = self.current_span;
                self.advance();
                Ok(self.alloc_expr(Expr::Identifier(v.into(), span.into())))
            }
            Token::LParen => {
                let _start_span = self.current_span;

                // Try parsing as a closure first by checking if it matches closure parameter syntax
                // To avoid messing up the main parser state if it fails, we can clone it
                let mut dummy_arena = pace_ast::arena::AstArena::new();
                let mut lookahead = Parser {
                    file_name: self.file_name.clone(),
                    src: self.src,
                    arena: &mut dummy_arena,
                    lexer: self.lexer.clone(),
                    current_token: self.current_token.clone(),
                    current_span: self.current_span,
                    errors: vec![],
                };

                lookahead.advance(); // consume '('
                let mut is_closure = false;
                if lookahead.current_token == Token::RParen {
                    lookahead.advance();
                    if lookahead.current_token == Token::FatArrow
                        || lookahead.current_token == Token::Arrow
                    {
                        is_closure = true;
                    }
                } else if let Token::Ident(_) = lookahead.current_token {
                    lookahead.advance();
                    if lookahead.current_token == Token::Colon {
                        is_closure = true; // (ident: type) -> definitely a closure
                    }
                }

                if is_closure {
                    self.advance(); // consume '('
                    let mut params = Vec::new();
                    while self.current_token != Token::RParen && self.current_token != Token::Eof {
                        let param_name = match &self.current_token {
                            Token::Ident(id) => *id,
                            _ => {
                                return Err(pace_errors::SyntaxError {
                                    message: "Expected parameter name in closure".to_string(),
                                    src: miette::NamedSource::new(
                                        self.file_name.clone(),
                                        self.src.to_string(),
                                    ),
                                    span: self.current_span,
                                });
                            }
                        };
                        self.advance();
                        if !self.match_token(Token::Colon) {
                            return Err(pace_errors::SyntaxError {
                                message: "Expected ':' after closure parameter name".to_string(),
                                src: miette::NamedSource::new(
                                    self.file_name.clone(),
                                    self.src.to_string(),
                                ),
                                span: self.current_span,
                            });
                        }
                        let param_ty = self.parse_type_annotation()?;
                        params.push((ustr::Ustr::from(param_name), param_ty));
                        if !self.match_token(Token::Comma) {
                            break;
                        }
                    }
                    if !self.match_token(Token::RParen) {
                        return Err(pace_errors::SyntaxError {
                            message: "Expected ')' after closure parameters".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                    }

                    let mut return_type = None;
                    if self.match_token(Token::Arrow) {
                        return_type = Some(self.parse_type_annotation()?);
                    }

                    if !self.match_token(Token::FatArrow) {
                        return Err(pace_errors::SyntaxError {
                            message: "Expected '=>' for closure body".to_string(),
                            src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                            span: self.current_span,
                        });
                    }

                    let body = if self.current_token == Token::LBrace {
                        let block = self.parse_block()?;
                        let stmts = match self.arena.get_stmt(block).clone() {
                            Stmt::Block(stmts) => stmts,
                            _ => vec![], // Shouldn't happen
                        };
                        self.alloc_expr(Expr::Block(stmts))
                    } else {
                        self.parse_expr()?
                    };

                    return Ok(self.alloc_expr(Expr::Closure {
                        params,
                        return_type,
                        body,
                    }));
                }

                self.advance(); // consume '(' as normal group
                let expr = self.parse_expr()?;
                if !self.match_token(Token::RParen) {
                    return Err(pace_errors::SyntaxError {
                        message: "Expected ')'".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: self.current_span,
                    });
                }
                Ok(expr)
            }
            _ => Err(pace_errors::SyntaxError {
                message: format!("Unexpected token: {:?}", self.current_token),
                src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                span: self.current_span,
            }),
        }
    }

    fn parse_interpolated_string(
        &mut self,
        s: String,
        base_span: (usize, usize),
    ) -> Result<pace_ast::arena::ExprId, pace_errors::SyntaxError> {
        let mut parts = Vec::new();
        let mut current_text = String::new();
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '$' && chars.peek() == Some(&'{') {
                chars.next(); // consume '{'
                if !current_text.is_empty() {
                    parts.push(self.alloc_expr(Expr::StringLiteral(current_text.clone().into())));
                    current_text.clear();
                }

                let mut expr_str = String::new();
                let mut depth = 1;
                for inner_c in chars.by_ref() {
                    if inner_c == '{' {
                        depth += 1;
                    }
                    if inner_c == '}' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    expr_str.push(inner_c);
                }

                if depth > 0 {
                    return Err(pace_errors::SyntaxError {
                        message: "Unclosed string interpolation block, expected '}'".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: base_span,
                    });
                }

                let mut nested_parser = Parser::new_with_arena(&expr_str, &self.file_name, self.arena);
                let expr = nested_parser
                    .parse_expr()
                    .map_err(|err| pace_errors::SyntaxError {
                        message: format!("In interpolated string: {}", err.message),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: base_span,
                    })?;

                if nested_parser.current_token != Token::Eof {
                    return Err(pace_errors::SyntaxError {
                        message: "Unexpected tokens in interpolated string".to_string(),
                        src: miette::NamedSource::new(self.file_name.clone(), self.src.to_string()),
                        span: base_span,
                    });
                }

                self.errors.extend(nested_parser.errors);
                parts.push(expr);
            } else {
                current_text.push(c);
            }
        }

        if !current_text.is_empty() {
            parts.push(self.alloc_expr(Expr::StringLiteral(current_text.into())));
        }

        if parts.len() == 1 {
            if let Expr::StringLiteral(_) = self.arena.get_expr(parts[0]) {
                return Ok(parts.pop().unwrap());
            }
        } else if parts.is_empty() {
            return Ok(self.alloc_expr(Expr::StringLiteral(String::new().into())));
        }

        Ok(self.alloc_expr(Expr::InterpolatedString(parts)))
    }
}
