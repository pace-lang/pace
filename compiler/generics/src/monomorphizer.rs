use ast::{Stmt, StmtKind, Expr, ExprKind};
use crate::substitution::TypeSubstitution;

pub struct Monomorphizer<'a> {
    subst: &'a TypeSubstitution,
    mangled_name: String,
}

impl<'a> Monomorphizer<'a> {
    pub fn new(subst: &'a TypeSubstitution, mangled_name: String) -> Self {
        Self { subst, mangled_name }
    }

    pub fn monomorphize_stmt(&self, stmt: &Stmt) -> Stmt {
        let kind = match &stmt.kind {
            StmtKind::Class { name: _, type_params: _, implements, methods, fields, is_private } => {
                let new_methods = methods.iter().map(|m| self.monomorphize_stmt(m)).collect();
                let new_fields = fields.iter().map(|f| self.monomorphize_stmt(f)).collect();
                
                StmtKind::Class {
                    name: self.mangled_name.clone(),
                    type_params: Vec::new(), // Erase generic parameters
                    implements: implements.clone(), // Ignore interfaces for now
                    methods: new_methods,
                    fields: new_fields,
                 is_private: *is_private,
                }
            }
            StmtKind::Func { name, type_params: _, params, return_type, body, is_private } => {
                // If this is a standalone function, we rename it. If it's a method inside a class, we keep the name!
                // We'll rename it ONLY if it's the top-level generic function being monomorphized.
                // Wait, if it's a method, we shouldn't rename it to the class's mangled name.
                // For simplicity, we assume we only rename the top-level entity if it's a Func.
                
                let new_params = params.iter().map(|(n, t)| (n.clone(), self.subst.substitute(t))).collect();
                let new_return = return_type.as_ref().map(|t| self.subst.substitute(t));
                let new_body = Box::new(self.monomorphize_stmt(body));
                
                StmtKind::Func {
                    name: name.clone(), // We might need to override the name outside
                    type_params: Vec::new(),
                    params: new_params,
                    return_type: new_return,
                    body: new_body,
                 is_private: *is_private,
                }
            }
            StmtKind::ForeignFunc { name: _name, type_params: _, params, return_type, is_private } => {
                let new_params = params.iter().map(|(n, t)| (n.clone(), self.subst.substitute(t))).collect();
                let new_return = return_type.as_ref().map(|t| self.subst.substitute(t));
                StmtKind::ForeignFunc {
                    name: self.mangled_name.clone(), // Ensure foreign functions get mangled names too!
                    type_params: Vec::new(),
                    params: new_params,
                    return_type: new_return,
                    is_private: *is_private,
                }
            }
            StmtKind::Let { name, type_annotation, initializer, is_private } => {
                StmtKind::Let {
                    name: name.clone(),
                    type_annotation: type_annotation.as_ref().map(|t| self.subst.substitute(t)),
                    initializer: initializer.as_ref().map(|e| self.monomorphize_expr(e)),
                 is_private: *is_private,
                }
            }
            StmtKind::Var { name, type_annotation, initializer, is_weak, is_private } => {
                StmtKind::Var {
                    name: name.clone(),
                    type_annotation: type_annotation.as_ref().map(|t| self.subst.substitute(t)),
                    initializer: initializer.as_ref().map(|e| self.monomorphize_expr(e)),
                    is_weak: *is_weak,
                 is_private: *is_private,
                }
            }
            StmtKind::Expression(expr) => StmtKind::Expression(self.monomorphize_expr(expr)),
            StmtKind::Block(stmts) => StmtKind::Block(stmts.iter().map(|s| self.monomorphize_stmt(s)).collect()),
            StmtKind::If { condition, then_branch, else_branch } => {
                StmtKind::If {
                    condition: self.monomorphize_expr(condition),
                    then_branch: Box::new(self.monomorphize_stmt(then_branch)),
                    else_branch: else_branch.as_ref().map(|s| Box::new(self.monomorphize_stmt(s))),
                }
            }
            StmtKind::While { condition, body } => {
                StmtKind::While {
                    condition: self.monomorphize_expr(condition),
                    body: Box::new(self.monomorphize_stmt(body)),
                }
            }
            StmtKind::Return { value } => {
                StmtKind::Return {
                    value: value.as_ref().map(|e| self.monomorphize_expr(e)),
                }
            }
            _ => stmt.kind.clone(), // Fallback for Interface, ForeignFunc which shouldn't have generics inside
        };
        
        Stmt { kind, span: stmt.span }
    }

    fn monomorphize_expr(&self, expr: &Expr) -> Expr {
        let kind = match &expr.kind {
            ExprKind::Binary(left, op, right) => {
                ExprKind::Binary(Box::new(self.monomorphize_expr(left)), op.clone(), Box::new(self.monomorphize_expr(right)))
            }
            ExprKind::Unary(op, inner) => {
                ExprKind::Unary(op.clone(), Box::new(self.monomorphize_expr(inner)))
            }
            ExprKind::Grouping(inner) => ExprKind::Grouping(Box::new(self.monomorphize_expr(inner))),
            ExprKind::Call { callee, type_args, arguments } => {
                ExprKind::Call {
                    callee: Box::new(self.monomorphize_expr(callee)),
                    type_args: type_args.iter().map(|t| self.subst.substitute(t)).collect(),
                    arguments: arguments.iter().map(|e| self.monomorphize_expr(e)).collect(),
                }
            }
            ExprKind::Get { object, name } => {
                ExprKind::Get {
                    object: Box::new(self.monomorphize_expr(object)),
                    name: name.clone(),
                }
            }
            ExprKind::Set { object, name, value } => {
                ExprKind::Set {
                    object: Box::new(self.monomorphize_expr(object)),
                    name: name.clone(),
                    value: Box::new(self.monomorphize_expr(value)),
                }
            }
            ExprKind::Assign { name, value } => {
                ExprKind::Assign {
                    name: name.clone(),
                    value: Box::new(self.monomorphize_expr(value)),
                }
            }
            ExprKind::ForceUnwrap(inner) => ExprKind::ForceUnwrap(Box::new(self.monomorphize_expr(inner))),
            ExprKind::OptionalGet { object, name } => {
                ExprKind::OptionalGet {
                    object: Box::new(self.monomorphize_expr(object)),
                    name: name.clone(),
                }
            }
            ExprKind::Array(elements) => ExprKind::Array(elements.iter().map(|e| self.monomorphize_expr(e)).collect()),
            ExprKind::ArrayRepeat { value, count } => {
                ExprKind::ArrayRepeat {
                    value: Box::new(self.monomorphize_expr(value)),
                    count: Box::new(self.monomorphize_expr(count)),
                }
            }
            ExprKind::IndexGet { object, index } => {
                ExprKind::IndexGet {
                    object: Box::new(self.monomorphize_expr(object)),
                    index: Box::new(self.monomorphize_expr(index)),
                }
            }
            ExprKind::IndexSet { object, index, value } => {
                ExprKind::IndexSet {
                    object: Box::new(self.monomorphize_expr(object)),
                    index: Box::new(self.monomorphize_expr(index)),
                    value: Box::new(self.monomorphize_expr(value)),
                }
            }
            _ => expr.kind.clone(), // Variables, Literals, SelfRef
        };
        
        Expr { kind, span: expr.span }
    }
}
