use crate::arena::{ExprId, HirArena, StmtId};
use crate::expr::Expr;
use crate::stmt::{EnumVariant, Param, Pattern, Stmt};

pub struct HirBuilder<'a> {
    pub ast_arena: &'a pace_ast::arena::AstArena,
    pub hir_arena: HirArena,
}

impl<'a> HirBuilder<'a> {
    pub fn new(ast_arena: &'a pace_ast::arena::AstArena) -> Self {
        Self {
            ast_arena,
            hir_arena: HirArena::new(),
        }
    }

    pub fn build(mut self, ast_stmts: &[pace_ast::arena::StmtId]) -> (HirArena, Vec<StmtId>) {
        let stmts = self.lower_stmts(ast_stmts);
        (self.hir_arena, stmts)
    }

    fn lower_exprs(&mut self, ast_exprs: &[pace_ast::arena::ExprId]) -> Vec<ExprId> {
        ast_exprs
            .iter()
            .map(|&expr_id| self.lower_expr(expr_id))
            .collect()
    }

    fn lower_stmts(&mut self, ast_stmts: &[pace_ast::arena::StmtId]) -> Vec<StmtId> {
        ast_stmts
            .iter()
            .map(|&stmt_id| self.lower_stmt(stmt_id))
            .collect()
    }

    pub fn lower_expr(&mut self, ast_expr_id: pace_ast::arena::ExprId) -> ExprId {
        let ast_expr = self.ast_arena.get_expr(ast_expr_id);
        let span = self.ast_arena.get_expr_span(ast_expr_id);

        let hir_expr = match ast_expr {
            pace_ast::Expr::IntLiteral(val) => Expr::IntLiteral(*val),
            pace_ast::Expr::FloatLiteral(val) => Expr::FloatLiteral(*val),
            pace_ast::Expr::StringLiteral(val) => Expr::StringLiteral(*val),
            pace_ast::Expr::InterpolatedString(parts) => {
                let hir_parts = self.lower_exprs(parts);
                Expr::InterpolatedString(hir_parts)
            }
            pace_ast::Expr::BoolLiteral(val) => Expr::BoolLiteral(*val),
            pace_ast::Expr::Null => Expr::Null,
            pace_ast::Expr::Identifier(name, _span) => Expr::Identifier(*name),
            pace_ast::Expr::GenericInstantiation {
                callee,
                generic_args,
            } => {
                let callee_id = self.lower_expr(*callee);
                Expr::GenericInstantiation {
                    callee: callee_id,
                    generic_args: generic_args.clone(),
                }
            }
            pace_ast::Expr::Unary { op, expr } => {
                let expr_id = self.lower_expr(*expr);
                Expr::Unary { op: op.clone(), expr: expr_id }
            }
            pace_ast::Expr::Binary { left, op, right } => {
                let left_id = self.lower_expr(*left);
                let right_id = self.lower_expr(*right);
                Expr::Binary {
                    left: left_id,
                    op: op.clone(),
                    right: right_id,
                }
            }
            pace_ast::Expr::Call { callee, args } => {
                let callee_id = self.lower_expr(*callee);
                let arg_ids = self.lower_exprs(args);
                Expr::Call {
                    callee: callee_id,
                    args: arg_ids,
                }
            }
            pace_ast::Expr::Assign { target, value } => {
                let target_id = self.lower_expr(*target);
                let value_id = self.lower_expr(*value);
                Expr::Assign {
                    target: target_id,
                    value: value_id,
                }
            }
            pace_ast::Expr::MemberAccess {
                object,
                property,
                computed_class,
                is_static_operator,
            } => {
                let object_id = self.lower_expr(*object);
                Expr::MemberAccess {
                    object: object_id,
                    property: *property,
                    computed_class: computed_class.clone(),
                    is_static_operator: *is_static_operator,
                }
            }
            pace_ast::Expr::Unwrap(expr) => {
                let expr_id = self.lower_expr(*expr);
                Expr::Unwrap(expr_id)
            }
            pace_ast::Expr::OptionalMemberAccess { object, property } => {
                let object_id = self.lower_expr(*object);
                Expr::OptionalMemberAccess {
                    object: object_id,
                    property: *property,
                }
            }
            pace_ast::Expr::NullCoalesce { left, right } => {
                let left_id = self.lower_expr(*left);
                let right_id = self.lower_expr(*right);
                Expr::NullCoalesce {
                    left: left_id,
                    right: right_id,
                }
            }
            pace_ast::Expr::Try(expr) => {
                let expr_id = self.lower_expr(*expr);
                Expr::Try(expr_id)
            }
            pace_ast::Expr::Await(expr) => {
                let expr_id = self.lower_expr(*expr);
                Expr::Await(expr_id)
            }
            pace_ast::Expr::Closure {
                params,
                return_type,
                body,
            } => {
                let body_id = self.lower_expr(*body);
                Expr::Closure {
                    params: params.clone(),
                    return_type: return_type.clone(),
                    body: body_id,
                }
            }
            pace_ast::Expr::Block(stmts) => {
                let stmt_ids = self.lower_stmts(stmts);
                Expr::Block(stmt_ids)
            }
        };

        self.hir_arena.alloc_expr(hir_expr, span)
    }

    pub fn lower_stmt(&mut self, ast_stmt_id: pace_ast::arena::StmtId) -> StmtId {
        let ast_stmt = self.ast_arena.get_stmt(ast_stmt_id);
        let span = self.ast_arena.get_stmt_span(ast_stmt_id);

        let hir_stmt = match ast_stmt {
            pace_ast::Stmt::Expr(expr) => {
                let expr_id = self.lower_expr(*expr);
                Stmt::Expr(expr_id)
            }
            pace_ast::Stmt::VarDecl {
                name,
                is_mutable,
                type_annotation,
                is_static,
                visibility,
                is_weak: _,
                initializer,
                span: _decl_span,
            } => {
                let init_id = initializer.map(|expr| self.lower_expr(expr));
                Stmt::VarDecl {
                    name: *name,
                    is_mutable: *is_mutable,
                    type_annotation: type_annotation.clone(),
                    is_static: *is_static,
                    visibility: visibility.clone(),
                    initializer: init_id,
                }
            }
            pace_ast::Stmt::Block(stmts) => {
                let stmt_ids = self.lower_stmts(stmts);
                Stmt::Block(stmt_ids)
            }
            pace_ast::Stmt::Return(expr) => {
                let expr_id = expr.map(|e| self.lower_expr(e));
                Stmt::Return(expr_id)
            }
            pace_ast::Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_id = self.lower_expr(*condition);
                let then_id = self.lower_stmt(*then_branch);
                let else_id = else_branch.map(|s| self.lower_stmt(s));
                Stmt::If {
                    condition: cond_id,
                    then_branch: then_id,
                    else_branch: else_id,
                }
            }
            pace_ast::Stmt::FuncDecl {
                name,
                generic_params,
                params,
                return_type,
                body,
                is_async,
                is_static,
                is_extern,
                visibility,
                span: _span,
            } => {
                let body_ids = self.lower_stmts(body);
                let hir_params = params
                    .iter()
                    .map(|p| Param {
                        name: p.name,
                        type_annotation: p.type_annotation.clone(),
                    })
                    .collect();
                Stmt::FuncDecl {
                    name: *name,
                    generic_params: generic_params.clone(),
                    params: hir_params,
                    return_type: return_type.clone(),
                    body: body_ids,
                    is_async: *is_async,
                    is_static: *is_static,
                    is_extern: *is_extern,
                    visibility: visibility.clone(),
                }
            }
            pace_ast::Stmt::ClassDecl {
                name,
                generic_params,
                fields,
                methods,
                implements,
                visibility,
            } => {
                let field_ids = self.lower_stmts(fields);
                let method_ids = self.lower_stmts(methods);
                Stmt::ClassDecl {
                    name: *name,
                    generic_params: generic_params.clone(),
                    fields: field_ids,
                    methods: method_ids,
                    implements: implements.clone(),
                    visibility: visibility.clone(),
                }
            }
            pace_ast::Stmt::ActorDecl {
                name,
                generic_params,
                fields,
                methods,
                implements,
                visibility,
            } => {
                let field_ids = self.lower_stmts(fields);
                let method_ids = self.lower_stmts(methods);
                Stmt::ActorDecl {
                    name: *name,
                    generic_params: generic_params.clone(),
                    fields: field_ids,
                    methods: method_ids,
                    implements: implements.clone(),
                    visibility: visibility.clone(),
                }
            }
            pace_ast::Stmt::InterfaceDecl {
                name,
                generic_params,
                methods,
                visibility,
            } => {
                let method_ids = self.lower_stmts(methods);
                Stmt::InterfaceDecl {
                    name: *name,
                    generic_params: generic_params.clone(),
                    methods: method_ids,
                    visibility: visibility.clone(),
                }
            }
            pace_ast::Stmt::StructDecl {
                name,
                generic_params,
                fields,
                visibility,
            } => {
                let field_ids = self.lower_stmts(fields);
                Stmt::StructDecl {
                    name: *name,
                    generic_params: generic_params.clone(),
                    fields: field_ids,
                    visibility: visibility.clone(),
                }
            }
            pace_ast::Stmt::EnumDecl {
                name,
                generic_params,
                variants,
                visibility,
            } => {
                let hir_variants = variants
                    .iter()
                    .map(|v| EnumVariant {
                        name: v.name,
                        fields: v.fields.clone(),
                    })
                    .collect();
                Stmt::EnumDecl {
                    name: *name,
                    generic_params: generic_params.clone(),
                    variants: hir_variants,
                    visibility: visibility.clone(),
                }
            }
            pace_ast::Stmt::While { condition, body } => {
                let cond_id = self.lower_expr(*condition);
                let body_id = self.lower_stmt(*body);
                Stmt::While {
                    condition: cond_id,
                    body: body_id,
                }
            }
            pace_ast::Stmt::Loop { body } => {
                let body_id = self.lower_stmt(*body);
                Stmt::Loop { body: body_id }
            }
            pace_ast::Stmt::ForIn {
                item,
                iterable,
                body,
            } => {
                let iter_expr_id = self.lower_expr(*iterable);
                let iter_name = ustr::Ustr::from(&format!("__iter_{}", iter_expr_id.0));
                
                let iterator_prop = self.hir_arena.alloc_expr(Expr::MemberAccess {
                    object: iter_expr_id,
                    property: ustr::Ustr::from("iterator"),
                    computed_class: None,
                    is_static_operator: false,
                }, pace_span::Span::default());
                let iterator_call = self.hir_arena.alloc_expr(Expr::Call {
                    callee: iterator_prop,
                    args: vec![],
                }, pace_span::Span::default());
                
                let iter_decl = self.hir_arena.alloc_stmt(Stmt::VarDecl {
                    name: iter_name,
                    is_mutable: false,
                    type_annotation: None,
                    is_static: false,
                    visibility: pace_ast::Visibility::Private,
                    initializer: Some(iterator_call),
                }, pace_span::Span::default());
                
                let iter_ident = self.hir_arena.alloc_expr(Expr::Identifier(iter_name), pace_span::Span::default());
                
                let has_next_prop = self.hir_arena.alloc_expr(Expr::MemberAccess {
                    object: iter_ident,
                    property: ustr::Ustr::from("hasNext"),
                    computed_class: None,
                    is_static_operator: false,
                }, pace_span::Span::default());
                let condition = self.hir_arena.alloc_expr(Expr::Call {
                    callee: has_next_prop,
                    args: vec![],
                }, pace_span::Span::default());
                
                let next_prop = self.hir_arena.alloc_expr(Expr::MemberAccess {
                    object: iter_ident,
                    property: ustr::Ustr::from("next"),
                    computed_class: None,
                    is_static_operator: false,
                }, pace_span::Span::default());
                let next_call = self.hir_arena.alloc_expr(Expr::Call {
                    callee: next_prop,
                    args: vec![],
                }, pace_span::Span::default());
                
                let item_decl = self.hir_arena.alloc_stmt(Stmt::VarDecl {
                    name: *item,
                    is_mutable: false,
                    type_annotation: None,
                    is_static: false,
                    visibility: pace_ast::Visibility::Private,
                    initializer: Some(next_call),
                }, pace_span::Span::default());
                
                let lowered_body = self.lower_stmt(*body);
                let while_body = self.hir_arena.alloc_stmt(Stmt::Block(vec![item_decl, lowered_body]), pace_span::Span::default());
                
                let while_loop = self.hir_arena.alloc_stmt(Stmt::While {
                    condition,
                    body: while_body,
                }, pace_span::Span::default());
                
                Stmt::Block(vec![iter_decl, while_loop])
            }
            pace_ast::Stmt::Match { expr, arms } => {
                let expr_id = self.lower_expr(*expr);
                let hir_arms = arms
                    .iter()
                    .map(|(pat, arm_stmt)| {
                        let hir_pat = self.lower_pattern(pat);
                        let arm_stmt_id = self.lower_stmt(*arm_stmt);
                        (hir_pat, arm_stmt_id)
                    })
                    .collect();
                Stmt::Match {
                    expr: expr_id,
                    arms: hir_arms,
                }
            }
            pace_ast::Stmt::Import {
                path,
                alias,
                show,
                hide,
            } => Stmt::Import {
                path: *path,
                alias: alias.clone(),
                show: show.clone(),
                hide: hide.clone(),
            },
            pace_ast::Stmt::Export { path } => Stmt::Export { path: *path },
            pace_ast::Stmt::Module { name, body } => {
                let body_ids = self.lower_stmts(body);
                Stmt::Module {
                    name: *name,
                    body: body_ids,
                }
            }
        };

        self.hir_arena.alloc_stmt(hir_stmt, span)
    }

    fn lower_pattern(&mut self, ast_pat: &pace_ast::Pattern) -> Pattern {
        match ast_pat {
            pace_ast::Pattern::Wildcard => Pattern::Wildcard,
            pace_ast::Pattern::Literal(expr) => {
                let expr_id = self.lower_expr(*expr);
                Pattern::Literal(expr_id)
            }
            pace_ast::Pattern::Variable(name, _span) => Pattern::Variable(*name),
            pace_ast::Pattern::Variant {
                enum_name,
                variant_name,
                fields,
                generic_args,
            } => {
                let hir_fields = fields.as_ref().map(|f| {
                    f.iter().map(|p| self.lower_pattern(p)).collect()
                });
                Pattern::Variant {
                    enum_name: enum_name.clone(),
                    variant_name: *variant_name,
                    fields: hir_fields,
                    generic_args: generic_args.clone(),
                }
            }
        }
    }
}
