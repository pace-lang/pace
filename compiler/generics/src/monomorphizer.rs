use crate::substitution::TypeSubstitution;
use ast::{Expr, ExprKind, Stmt, StmtKind};

pub struct Monomorphizer<'a> {
    pub arena: &'a bumpalo::Bump,
    pub subst: &'a TypeSubstitution<'a>,
    mangled_name: session::Symbol,
}

impl<'a> Monomorphizer<'a> {
    pub fn new(
        arena: &'a bumpalo::Bump,
        subst: &'a TypeSubstitution<'a>,
        mangled_name: session::Symbol,
    ) -> Self {
        Self {
            arena,
            subst,
            mangled_name,
        }
    }

    pub fn monomorphize_stmt(&self, stmt: &Stmt<'a>) -> Stmt<'a> {
        let kind = match &stmt.kind {
            StmtKind::Class {
                name: _,
                type_params: _,
                implements,
                methods,
                fields,
                is_private,
            } => {
                let new_methods = methods.iter().map(|m| self.monomorphize_stmt(m)).collect();
                let new_fields = fields.iter().map(|f| self.monomorphize_stmt(f)).collect();
                let new_implements = implements
                    .iter()
                    .map(|ty| self.subst.substitute(ty))
                    .collect();

                StmtKind::Class {
                    name: self.mangled_name,
                    type_params: Vec::new(), // Erase generic parameters
                    implements: new_implements,
                    methods: new_methods,
                    fields: new_fields,
                    is_private: *is_private,
                }
            }
            StmtKind::Enum {
                name: _,
                type_params: _,
                variants,
                methods,
                is_private,
            } => {
                let new_methods = methods.iter().map(|m| self.monomorphize_stmt(m)).collect();

                let new_variants = variants
                    .iter()
                    .map(|v| {
                        let new_fields = v.fields.as_ref().map(|fields| {
                            fields
                                .iter()
                                .map(|f| ast::stmt::EnumField {
                                    name: f.name,
                                    ty: self.subst.substitute(&f.ty),
                                })
                                .collect()
                        });
                        ast::stmt::EnumVariant {
                            name: v.name,
                            fields: new_fields,
                        }
                    })
                    .collect();

                StmtKind::Enum {
                    name: self.mangled_name,
                    type_params: Vec::new(),
                    variants: new_variants,
                    methods: new_methods,
                    is_private: *is_private,
                }
            }
            StmtKind::Func {
                name,
                type_params: _,
                params,
                return_type,
                body,
                is_private,
                is_async,
            } => {
                // If this is a standalone function, we rename it. If it's a method inside a class, we keep the name!
                // We'll rename it ONLY if it's the top-level generic function being monomorphized.
                // Wait, if it's a method, we shouldn't rename it to the class's mangled name.
                // For simplicity, we assume we only rename the top-level entity if it's a Func.

                let new_params: Vec<_> = params
                    .iter()
                    .map(|(n, t)| (*n, self.subst.substitute(t)))
                    .collect();
                let new_return = return_type.as_ref().map(|t| self.subst.substitute(t));
                let new_body = self.subst.arena.alloc(self.monomorphize_stmt(body));

                StmtKind::Func {
                    name: *name, // We might need to override the name outside
                    type_params: Vec::new(),
                    params: new_params,
                    return_type: new_return,
                    body: new_body,
                    is_private: *is_private,
                    is_async: *is_async,
                }
            }
            StmtKind::ForeignFunc {
                name: _name,
                base_name,
                type_params: _,
                params,
                return_type,
                is_private,
            } => {
                let new_params = params
                    .iter()
                    .map(|(n, t)| (*n, self.subst.substitute(t)))
                    .collect();
                let new_return = return_type.as_ref().map(|t| self.subst.substitute(t));
                StmtKind::ForeignFunc {
                    name: self.mangled_name, // Ensure foreign functions get mangled names too!
                    base_name: *base_name,
                    type_params: Vec::new(),
                    params: new_params,
                    return_type: new_return,
                    is_private: *is_private,
                }
            }
            StmtKind::Let {
                name,
                type_annotation,
                initializer,
                is_private,
            } => StmtKind::Let {
                name: *name,
                type_annotation: type_annotation.as_ref().map(|t| self.subst.substitute(t)),
                initializer: initializer
                    .as_ref()
                    .map(|e| &*self.subst.arena.alloc(self.monomorphize_expr(e))),
                is_private: *is_private,
            },
            StmtKind::Var {
                name,
                type_annotation,
                initializer,
                is_weak,
                is_private,
            } => StmtKind::Var {
                name: *name,
                type_annotation: type_annotation.as_ref().map(|t| self.subst.substitute(t)),
                initializer: initializer
                    .as_ref()
                    .map(|e| &*self.subst.arena.alloc(self.monomorphize_expr(e))),
                is_weak: *is_weak,
                is_private: *is_private,
            },
            StmtKind::Expression(expr) => {
                StmtKind::Expression(self.subst.arena.alloc(self.monomorphize_expr(expr)))
            }
            StmtKind::Block(stmts) => {
                StmtKind::Block(stmts.iter().map(|s| self.monomorphize_stmt(s)).collect())
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => StmtKind::If {
                condition: self.subst.arena.alloc(self.monomorphize_expr(condition)),
                then_branch: self.subst.arena.alloc(self.monomorphize_stmt(then_branch)),
                else_branch: else_branch
                    .as_ref()
                    .map(|s| &*self.subst.arena.alloc(self.monomorphize_stmt(s))),
            },
            StmtKind::While { condition, body } => StmtKind::While {
                condition: self.subst.arena.alloc(self.monomorphize_expr(condition)),
                body: self.subst.arena.alloc(self.monomorphize_stmt(body)),
            },
            StmtKind::Return { value } => StmtKind::Return {
                value: value
                    .as_ref()
                    .map(|e| &*self.subst.arena.alloc(self.monomorphize_expr(e))),
            },
            StmtKind::Extension {
                target_type,
                type_params: _,
                methods,
            } => StmtKind::Extension {
                target_type: self.subst.substitute(target_type),
                type_params: Vec::new(),
                methods: methods.iter().map(|m| self.monomorphize_stmt(m)).collect(),
            },
            _ => stmt.kind.clone(), // Fallback for Interface, ForeignFunc which shouldn't have generics inside
        };

        Stmt {
            kind,
            span: stmt.span,
        }
    }

    fn monomorphize_expr(&self, expr: &Expr<'a>) -> Expr<'a> {
        let kind = match &expr.kind {
            ExprKind::Binary(left, op, right) => ExprKind::Binary(
                self.subst.arena.alloc(self.monomorphize_expr(left)),
                op.clone(),
                self.subst.arena.alloc(self.monomorphize_expr(right)),
            ),
            ExprKind::Unary(op, inner) => ExprKind::Unary(
                op.clone(),
                self.subst.arena.alloc(self.monomorphize_expr(inner)),
            ),
            ExprKind::Grouping(inner) => {
                ExprKind::Grouping(self.subst.arena.alloc(self.monomorphize_expr(inner)))
            }
            ExprKind::Call {
                callee,
                type_args,
                arguments,
            } => ExprKind::Call {
                callee: self.subst.arena.alloc(self.monomorphize_expr(callee)),
                type_args: type_args.iter().map(|t| self.subst.substitute(t)).collect(),
                arguments: arguments
                    .iter()
                    .map(|e| self.monomorphize_expr(e))
                    .collect(),
            },
            ExprKind::Get { object, name } => ExprKind::Get {
                object: self.subst.arena.alloc(self.monomorphize_expr(object)),
                name: *name,
            },
            ExprKind::Set {
                object,
                name,
                value,
            } => ExprKind::Set {
                object: self.subst.arena.alloc(self.monomorphize_expr(object)),
                name: *name,
                value: self.subst.arena.alloc(self.monomorphize_expr(value)),
            },
            ExprKind::Assign { name, value } => ExprKind::Assign {
                name: *name,
                value: self.subst.arena.alloc(self.monomorphize_expr(value)),
            },
            ExprKind::ForceUnwrap(inner) => {
                ExprKind::ForceUnwrap(self.subst.arena.alloc(self.monomorphize_expr(inner)))
            }
            ExprKind::PostfixTry(inner) => {
                ExprKind::PostfixTry(self.subst.arena.alloc(self.monomorphize_expr(inner)))
            }
            ExprKind::OptionalGet { object, name } => ExprKind::OptionalGet {
                object: self.subst.arena.alloc(self.monomorphize_expr(object)),
                name: *name,
            },
            ExprKind::Array(elements) => {
                ExprKind::Array(elements.iter().map(|e| self.monomorphize_expr(e)).collect())
            }
            ExprKind::ArrayRepeat { value, count } => ExprKind::ArrayRepeat {
                value: self.subst.arena.alloc(self.monomorphize_expr(value)),
                count: self.subst.arena.alloc(self.monomorphize_expr(count)),
            },
            ExprKind::IndexGet { object, index } => ExprKind::IndexGet {
                object: self.subst.arena.alloc(self.monomorphize_expr(object)),
                index: self.subst.arena.alloc(self.monomorphize_expr(index)),
            },
            ExprKind::IndexSet {
                object,
                index,
                value,
            } => ExprKind::IndexSet {
                object: self.subst.arena.alloc(self.monomorphize_expr(object)),
                index: self.subst.arena.alloc(self.monomorphize_expr(index)),
                value: self.subst.arena.alloc(self.monomorphize_expr(value)),
            },
            _ => expr.kind.clone(), // Variables, Literals, SelfRef
        };

        Expr {
            kind,
            span: expr.span,
        }
    }
}
