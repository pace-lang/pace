use std::collections::HashMap;
use pace_hir::{Expr, Decl, Program, HirId};
use pace_ast::BinaryOp;
use crate::ty::Ty;

pub struct TypeChecker {
    pub env: HashMap<HirId, Ty>,
    pub struct_defs: HashMap<HirId, Vec<(String, Ty)>>,
    pub class_defs: HashMap<HirId, Vec<(String, Ty)>>,
    pub named_types: HashMap<String, HirId>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
            struct_defs: HashMap::new(),
            class_defs: HashMap::new(),
            named_types: HashMap::new(),
        }
    }

    pub fn resolve_type(&self, ast_ty: &pace_ast::Type) -> Result<Ty, String> {
        match ast_ty {
            pace_ast::Type::Named(id) => {
                match id.name.as_str() {
                    "Int" => Ok(Ty::Int),
                    "Float" => Ok(Ty::Float),
                    "String" => Ok(Ty::String),
                    "Bool" => Ok(Ty::Bool),
                    other => {
                        if let Some(&hir_id) = self.named_types.get(other) {
                            if self.struct_defs.contains_key(&hir_id) {
                                return Ok(Ty::Struct(hir_id));
                            }
                            if self.class_defs.contains_key(&hir_id) {
                                return Ok(Ty::Class(hir_id));
                            }
                        }
                        Err(format!("Unknown type: {}", other))
                    }
                }
            }
            pace_ast::Type::Optional(_, _) => Err("Optional types not yet supported".to_string()),
        }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), String> {
        // Pass 1: Register top-level structures and classes names
        for decl in &program.declarations {
            match decl {
                Decl::Struct { id, name, .. } => {
                    self.named_types.insert(name.clone(), *id);
                }
                Decl::Class { id, name, .. } => {
                    self.named_types.insert(name.clone(), *id);
                }
                _ => {}
            }
        }

        // Pass 2: Register struct/class fields
        for decl in &program.declarations {
            match decl {
                Decl::Struct { id, fields, .. } => {
                    let mut resolved_fields = Vec::new();
                    for (fname, fty) in fields {
                        resolved_fields.push((fname.clone(), self.resolve_type(fty)?));
                    }
                    self.struct_defs.insert(*id, resolved_fields);
                }
                Decl::Class { id, fields, .. } => {
                    let mut resolved_fields = Vec::new();
                    for (fname, fty) in fields {
                        resolved_fields.push((fname.clone(), self.resolve_type(fty)?));
                    }
                    self.class_defs.insert(*id, resolved_fields);
                }
                _ => {}
            }
        }

        // Pass 3: Register functions
        for decl in &program.declarations {
            if let Decl::Function { id, params, return_type, .. } = decl {
                let mut param_tys = Vec::new();
                for (_, _, pty) in params {
                    param_tys.push(self.resolve_type(pty)?); 
                }
                let ret_ty = if let Some(rty) = return_type {
                    self.resolve_type(rty)?
                } else {
                    Ty::Int // Defaulting for now
                };
                self.env.insert(*id, Ty::Function(param_tys, Box::new(ret_ty)));
            }
        }

        for decl in &program.declarations {
            self.check_decl(decl)?;
        }
        Ok(())
    }

    pub fn check_block(&mut self, block: &pace_hir::Block) -> Result<(), String> {
        let outer_env = self.env.clone();
        for stmt in &block.statements {
            match stmt {
                pace_hir::Stmt::Let { id, value, .. } => {
                    let ty = self.check_expr(value)?;
                    self.env.insert(*id, ty);
                }
                pace_hir::Stmt::ExprStmt(expr, _) => {
                    self.check_expr(expr)?;
                }
                pace_hir::Stmt::Return(expr, _) => {
                    if let Some(e) = expr {
                        self.check_expr(e)?;
                    }
                }
            }
        }
        self.env = outer_env;
        Ok(())
    }

    pub fn check_decl(&mut self, decl: &Decl) -> Result<(), String> {
        match decl {
            Decl::Let { id, value, .. } => {
                let ty = self.check_expr(value)?;
                self.env.insert(*id, ty);
                Ok(())
            }
            Decl::Struct { id, .. } => {
                self.env.insert(*id, Ty::Struct(*id));
                Ok(())
            }
            Decl::Class { id, .. } => {
                self.env.insert(*id, Ty::Class(*id));
                Ok(())
            }
            Decl::Expr(expr, _) => {
                self.check_expr(expr)?;
                Ok(())
            }
            Decl::Function { params, body, .. } => {
                let outer_env = self.env.clone();
                for (param_id, _, pty) in params {
                    let ty = self.resolve_type(pty)?;
                    self.env.insert(*param_id, ty);
                }
                self.check_block(body)?;
                self.env = outer_env;
                Ok(())
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<Ty, String> {
        match expr {
            Expr::IntLiteral(..) => Ok(Ty::Int),
            Expr::StringLiteral(..) => Ok(Ty::String),
            Expr::Ident(id, span) => {
                self.env.get(id).cloned().ok_or(format!("Cannot infer type for unbound variable at {:?}", span))
            }
            Expr::Binary { left, op, right, .. } => {
                let left_ty = self.check_expr(left)?;
                let right_ty = self.check_expr(right)?;

                match op {
                    pace_ast::BinaryOp::Add | pace_ast::BinaryOp::Sub |
                    pace_ast::BinaryOp::Mul | pace_ast::BinaryOp::Div => {
                        if left_ty == Ty::Int && right_ty == Ty::Int {
                            Ok(Ty::Int)
                        } else if left_ty == Ty::Float && right_ty == Ty::Float {
                            Ok(Ty::Float)
                        } else {
                            Err(format!("Type mismatch in binary operation: {:?} and {:?}", left_ty, right_ty))
                        }
                    }
                    pace_ast::BinaryOp::EqEq | pace_ast::BinaryOp::NotEq |
                    pace_ast::BinaryOp::Gt | pace_ast::BinaryOp::Lt |
                    pace_ast::BinaryOp::GtEq | pace_ast::BinaryOp::LtEq => {
                        Ok(Ty::Int) // Boolean represented as Int in MVP
                    }
                }
            }
            Expr::MemberAccess { object, member, .. } => {
                let obj_ty = self.check_expr(object)?;
                match obj_ty {
                    Ty::Struct(hir_id) => {
                        let fields = self.struct_defs.get(&hir_id).ok_or("Struct definition not found")?;
                        for (fname, fty) in fields {
                            if fname == member {
                                return Ok(fty.clone());
                            }
                        }
                        Err(format!("Struct has no member '{}'", member))
                    }
                    Ty::Class(hir_id) => {
                        let fields = self.class_defs.get(&hir_id).ok_or("Class definition not found")?;
                        for (fname, fty) in fields {
                            if fname == member {
                                return Ok(fty.clone());
                            }
                        }
                        Err(format!("Class has no member '{}'", member))
                    }
                    _ => Err(format!("Cannot access member '{}' on type {:?}", member, obj_ty)),
                }
            }
            Expr::Call { callee, args, .. } => {
                let callee_ty = self.check_expr(callee)?;
                for arg in args {
                    self.check_expr(arg)?;
                }
                if let Ty::Function(_, ret_ty) = callee_ty {
                    return Ok(*ret_ty);
                }
                Ok(Ty::Int) // Fallback for MVP
            }
            Expr::BuiltinCall(name, args, _) => {
                let mut arg_types = Vec::new();
                for arg in args {
                    arg_types.push(self.check_expr(arg)?);
                }
                
                if name == "print" || name == "println" {
                    if arg_types.len() != 1 {
                        return Err(format!("{} takes exactly 1 argument", name));
                    }
                    Ok(Ty::Int)
                } else {
                    Err(format!("Unknown builtin: {}", name))
                }
            }
            Expr::If { cond, then_block, else_block, .. } => {
                self.check_expr(cond)?;
                self.check_block(then_block)?;
                if let Some(else_b) = else_block {
                    self.check_block(else_b)?;
                }
                Ok(Ty::Int)
            }
            Expr::While { cond, body, .. } => {
                self.check_expr(cond)?;
                self.check_block(body)?;
                Ok(Ty::Int)
            }
            Expr::Assign { target, value, .. } => {
                let target_ty = self.check_expr(target)?;
                let val_ty = self.check_expr(value)?;
                if target_ty != val_ty {
                    return Err(format!("Type mismatch in assignment: expected {:?}, got {:?}", target_ty, val_ty));
                }
                Ok(target_ty)
            }
            Expr::Instantiate { name, id, fields, .. } => {
                let is_struct = self.struct_defs.contains_key(id);
                let is_class = self.class_defs.contains_key(id);
                
                let def_fields = if is_struct {
                    self.struct_defs.get(id).unwrap().clone()
                } else if is_class {
                    self.class_defs.get(id).unwrap().clone()
                } else {
                    return Err(format!("'{}' is not a struct or class", name));
                };

                for (fname, expr) in fields {
                    let expr_ty = self.check_expr(expr)?;
                    let mut found = false;
                    for (dfname, dfty) in &def_fields {
                        if fname == dfname {
                            if *dfty != expr_ty {
                                return Err(format!("Field '{}' expects type {:?}, got {:?}", fname, dfty, expr_ty));
                            }
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Err(format!("Type '{}' has no field '{}'", name, fname));
                    }
                }
                
                if is_struct {
                    Ok(Ty::Struct(*id))
                } else {
                    Ok(Ty::Class(*id))
                }
            }
        }
    }
}
