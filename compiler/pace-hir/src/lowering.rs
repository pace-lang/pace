use std::collections::HashMap;
use crate::hir::*;
use pace_ast as ast;

/// Lowers an AST (with string names) into HIR (with unique HirIds).
pub struct LoweringContext {
    next_id: u32,
    scope: HashMap<String, HirId>,
}

impl LoweringContext {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            scope: HashMap::new(),
        }
    }

    fn generate_id(&mut self) -> HirId {
        let id = self.next_id;
        self.next_id += 1;
        HirId(id)
    }

    pub fn lower_program(&mut self, ast: ast::Program) -> Result<Program, String> {
        let mut declarations = Vec::new();
        for decl in ast.declarations {
            if let Some(d) = self.lower_decl(decl)? {
                declarations.push(d);
            }
        }
        Ok(Program { declarations })
    }

    pub fn lower_block(&mut self, block: ast::Block) -> Result<Block, String> {
        let mut statements = Vec::new();
        let outer_scope = self.scope.clone();
        for stmt in block.statements {
            match stmt {
                ast::Stmt::Let { name, ty, value, span } => {
                    let id = self.generate_id();
                    let lowered_val = match value {
                        Some(expr) => Some(self.lower_expr(expr)?),
                        None => None,
                    };
                    self.scope.insert(name.name.clone(), id);
                    statements.push(Stmt::Let { id, name: name.name, ty, value: lowered_val, span });
                }
                ast::Stmt::Var { name, ty, value, span } => {
                    let id = self.generate_id();
                    let lowered_val = match value {
                        Some(expr) => Some(self.lower_expr(expr)?),
                        None => None,
                    };
                    self.scope.insert(name.name.clone(), id);
                    statements.push(Stmt::Var { id, name: name.name, ty, value: lowered_val, span });
                }
                ast::Stmt::ExprStmt(expr, span) => {
                    statements.push(Stmt::ExprStmt(self.lower_expr(expr)?, span));
                }
                ast::Stmt::Return(expr, span) => {
                    let lowered_expr = match expr {
                        Some(e) => Some(self.lower_expr(e)?),
                        None => None,
                    };
                    statements.push(Stmt::Return(lowered_expr, span));
                }
            }
        }
        self.scope = outer_scope;
        Ok(Block { statements, span: block.span })
    }

    fn lower_decl(&mut self, decl: ast::Decl) -> Result<Option<Decl>, String> {
        match decl {
            ast::Decl::Let { name, ty, value, span } => {
                let id = self.generate_id();
                self.scope.insert(name.name.clone(), id);
                let lowered_value = match value {
                    Some(expr) => Some(self.lower_expr(expr)?),
                    None => None,
                };
                Ok(Some(Decl::Let { id, name: name.name, ty, value: lowered_value, span }))
            }
            ast::Decl::Var { name, ty, value, span } => {
                let id = self.generate_id();
                self.scope.insert(name.name.clone(), id);
                let lowered_value = match value {
                    Some(expr) => Some(self.lower_expr(expr)?),
                    None => None,
                };
                Ok(Some(Decl::Var { id, name: name.name, ty, value: lowered_value, span }))
            }
            ast::Decl::Struct { name, fields, static_fields, const_fields, methods, span, .. } => {
                let id = self.generate_id();
                self.scope.insert(name.name.clone(), id);
                let mut lowered_fields = Vec::new();
                for (field_name, field_ty, field_val) in fields {
                    let lowered_val = match field_val {
                        Some(v) => Some(self.lower_expr(v)?),
                        None => None,
                    };
                    lowered_fields.push((field_name.name, field_ty, lowered_val));
                }
                
                let mut lowered_static = Vec::new();
                for (sf_name, sf_ty, sf_val) in static_fields {
                    lowered_static.push((sf_name.name, sf_ty, self.lower_expr(sf_val)?));
                }
                let mut lowered_const = Vec::new();
                for (cf_name, cf_ty, cf_val) in const_fields {
                    lowered_const.push((cf_name.name, cf_ty, self.lower_expr(cf_val)?));
                }
                
                let mut lowered_methods = Vec::new();
                for method in methods {
                    if let ast::Decl::Function { name: m_name, params, return_type, body, span: m_span, is_static } = method {
                        let mut new_params = Vec::new();
                        if !is_static {
                            new_params.push((
                                ast::Ident { name: "self".to_string(), span: m_name.span },
                                ast::Type::Named(name.clone())
                            ));
                        }
                        new_params.extend(params);
                        let m_decl = ast::Decl::Function {
                            name: ast::Ident { name: format!("{}_{}", name.name, m_name.name), span: m_name.span },
                            params: new_params,
                            return_type,
                            body,
                            span: m_span,
                            is_static,
                        };
                        if let Some(lowered) = self.lower_decl(m_decl)? {
                            lowered_methods.push(lowered);
                        }
                    }
                }
                Ok(Some(Decl::Struct { id, name: name.name, fields: lowered_fields, static_fields: lowered_static, const_fields: lowered_const, methods: lowered_methods, span }))
            }
            ast::Decl::Class { name, fields, static_fields, const_fields, methods, span, .. } => {
                let id = self.generate_id();
                self.scope.insert(name.name.clone(), id);
                let mut lowered_fields = Vec::new();
                for (field_name, field_ty, field_val) in fields {
                    let lowered_val = match field_val {
                        Some(v) => Some(self.lower_expr(v)?),
                        None => None,
                    };
                    lowered_fields.push((field_name.name, field_ty, lowered_val));
                }
                
                let mut lowered_static = Vec::new();
                for (sf_name, sf_ty, sf_val) in static_fields {
                    lowered_static.push((sf_name.name, sf_ty, self.lower_expr(sf_val)?));
                }
                let mut lowered_const = Vec::new();
                for (cf_name, cf_ty, cf_val) in const_fields {
                    lowered_const.push((cf_name.name, cf_ty, self.lower_expr(cf_val)?));
                }
                
                let mut lowered_methods = Vec::new();
                for method in methods {
                    if let ast::Decl::Function { name: m_name, params, return_type, body, span: m_span, is_static } = method {
                        let mut new_params = Vec::new();
                        if !is_static {
                            new_params.push((
                                ast::Ident { name: "self".to_string(), span: m_name.span },
                                ast::Type::Named(name.clone())
                            ));
                        }
                        new_params.extend(params);
                        let m_decl = ast::Decl::Function {
                            name: ast::Ident { name: format!("{}_{}", name.name, m_name.name), span: m_name.span },
                            params: new_params,
                            return_type,
                            body,
                            span: m_span,
                            is_static,
                        };
                        if let Some(lowered) = self.lower_decl(m_decl)? {
                            lowered_methods.push(lowered);
                        }
                    }
                }
                Ok(Some(Decl::Class { id, name: name.name, fields: lowered_fields, static_fields: lowered_static, const_fields: lowered_const, methods: lowered_methods, span }))
            }
            ast::Decl::Expr(expr, span) => {
                let lowered = self.lower_expr(expr)?;
                Ok(Some(Decl::Expr(lowered, span)))
            }
            ast::Decl::Enum { name, variants, span } => {
                let id = self.generate_id();
                self.scope.insert(name.name.clone(), id);
                let mut lowered_variants = Vec::new();
                for v in variants {
                    let var_id = self.generate_id();
                    self.scope.insert(v.name.name.clone(), var_id);
                    
                    let mut lowered_fields = None;
                    if let Some(fields) = v.fields {
                        let mut lf = Vec::new();
                        for (fname, fty) in fields {
                            lf.push((fname.name, fty));
                        }
                        lowered_fields = Some(lf);
                    }
                    lowered_variants.push(crate::hir::EnumVariant {
                        name: v.name.name,
                        id: var_id,
                        fields: lowered_fields,
                        span: v.span,
                    });
                }
                Ok(Some(Decl::Enum { id, name: name.name, variants: lowered_variants, span }))
            }
            ast::Decl::Function { name, params, return_type, body, span, is_static } => {
                let id = self.generate_id();
                self.scope.insert(name.name.clone(), id);
                
                let outer_scope = self.scope.clone();
                let mut lowered_params = Vec::new();
                for (param_name, param_ty) in params {
                    let param_id = self.generate_id();
                    self.scope.insert(param_name.name.clone(), param_id);
                    lowered_params.push((param_id, param_name.name, param_ty));
                }
                
                let lowered_body = self.lower_block(body)?;
                self.scope = outer_scope;
                
                Ok(Some(Decl::Function {
                    id,
                    name: name.name,
                    params: lowered_params,
                    return_type,
                    body: lowered_body,
                    is_static,
                    span,
                }))
            }
            _ => Ok(None),
        }
    }

    fn lower_expr(&mut self, expr: ast::Expr) -> Result<Expr, String> {
        match expr {
            ast::Expr::IntLiteral(val, span) => Ok(Expr::IntLiteral(val, span)),
            ast::Expr::FloatLiteral(val, span) => Ok(Expr::FloatLiteral(val, span)),
            ast::Expr::StringLiteral(val, span) => Ok(Expr::StringLiteral(val, span)),
            ast::Expr::Ident(ident) => {
                if ident.name == "true" {
                    return Ok(Expr::BoolLiteral(true, ident.span));
                }
                if ident.name == "false" {
                    return Ok(Expr::BoolLiteral(false, ident.span));
                }
                let id = self.scope.get(&ident.name).copied().ok_or(format!("Undefined variable: {}", ident.name))?;
                Ok(Expr::Ident(id, ident.span))
            }
            ast::Expr::Binary { left, op, right, span } => {
                Ok(Expr::Binary {
                    left: Box::new(self.lower_expr(*left)?),
                    op,
                    right: Box::new(self.lower_expr(*right)?),
                    span,
                })
            }
            ast::Expr::MemberAccess { object, member, span } => {
                Ok(Expr::MemberAccess {
                    object: Box::new(self.lower_expr(*object)?),
                    member: member.name,
                    span,
                })
            }
            ast::Expr::Call { callee, args, span } => {
                if let ast::Expr::Ident(ident) = &*callee {
                    if ident.name == "print" || ident.name == "println" {
                        let mut lowered_args = Vec::new();
                        for (_, arg) in args {
                            lowered_args.push(self.lower_expr(arg)?);
                        }
                        return Ok(Expr::BuiltinCall(ident.name.clone(), lowered_args, span));
                    }
                }
                
                let mut lowered_args = Vec::new();
                for (label, arg) in args {
                    let lowered_label = label.map(|id| id.name);
                    lowered_args.push((lowered_label, self.lower_expr(arg)?));
                }
                Ok(Expr::Call {
                    callee: Box::new(self.lower_expr(*callee)?),
                    args: lowered_args,
                    span,
                })
            }
            ast::Expr::If { cond, then_block, else_block, span } => {
                let lowered_cond = self.lower_expr(*cond)?;
                let lowered_then = self.lower_block(then_block)?;
                let lowered_else = match else_block {
                    Some(b) => Some(self.lower_block(b)?),
                    None => None,
                };
                Ok(Expr::If {
                    cond: Box::new(lowered_cond),
                    then_block: lowered_then,
                    else_block: lowered_else,
                    span,
                })
            }
            ast::Expr::While { cond, body, span } => {
                let lowered_cond = self.lower_expr(*cond)?;
                let lowered_body = self.lower_block(body)?;
                Ok(Expr::While {
                    cond: Box::new(lowered_cond),
                    body: lowered_body,
                    span,
                })
            }
            ast::Expr::Assign { target, value, span } => {
                let lowered_target = self.lower_expr(*target)?;
                let lowered_val = self.lower_expr(*value)?;
                Ok(Expr::Assign {
                    target: Box::new(lowered_target),
                    value: Box::new(lowered_val),
                    span,
                })
            }
            ast::Expr::Match { subject, arms, span } => {
                let lowered_subject = self.lower_expr(*subject)?;
                let mut lowered_arms = Vec::new();
                for arm in arms {
                    let outer_scope = self.scope.clone();
                    
                    let pattern = match arm.pattern {
                        ast::Pattern::Ident(ident) => {
                            let id = self.generate_id();
                            self.scope.insert(ident.name.clone(), id);
                            crate::hir::Pattern::Ident(id, ident.name, ident.span)
                        }
                        ast::Pattern::Variant { name, fields, span: p_span } => {
                            let mut lowered_fields = None;
                            if let Some(f) = fields {
                                let mut lf = Vec::new();
                                for fname in f {
                                    let id = self.generate_id();
                                    self.scope.insert(fname.name.clone(), id);
                                    lf.push((id, fname.name, fname.span));
                                }
                                lowered_fields = Some(lf);
                            }
                            crate::hir::Pattern::Variant { name: name.name, fields: lowered_fields, span: p_span }
                        }
                        ast::Pattern::CatchAll(s) => crate::hir::Pattern::CatchAll(s),
                    };
                    
                    let body = self.lower_expr(arm.body)?;
                    self.scope = outer_scope;
                    
                    lowered_arms.push(crate::hir::MatchArm {
                        pattern,
                        body,
                        span: arm.span,
                    });
                }
                
                Ok(Expr::Match {
                    subject: Box::new(lowered_subject),
                    arms: lowered_arms,
                    span,
                })
            }
        }
    }
}
