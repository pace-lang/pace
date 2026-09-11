use std::collections::HashMap;
use pace_hir::{Expr, Decl, Program, HirId};
use pace_ast::BinaryOp;
use crate::ty::Ty;

use pace_errors::{Reporter, Diagnostic};

pub struct TypeChecker {
    pub env: HashMap<HirId, Ty>,
    pub mutability_env: HashMap<HirId, bool>,
    pub named_types: HashMap<String, HirId>, // struct/class names to HirId
    pub struct_defs: HashMap<HirId, Vec<(String, Ty)>>,
    pub class_defs: HashMap<HirId, Vec<(String, Ty)>>,
    pub methods_env: HashMap<String, Ty>,
    pub reporter: Reporter,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
            mutability_env: HashMap::new(),
            named_types: HashMap::new(),
            struct_defs: HashMap::new(),
            class_defs: HashMap::new(),
            methods_env: HashMap::new(),
            reporter: Reporter::new(),
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
        let mut has_main = false;
        
        for decl in &program.declarations {
            if let Decl::Function { name, .. } = decl {
                if name == "main" {
                    has_main = true;
                }
            } else if let Decl::Expr(_, span) = decl {
                self.reporter.report(Diagnostic::error("Top-level executable statements are not allowed")
                    .with_span(*span)
                    .with_hint("Wrap this code in a 'fn main()' block"));
            }
        }
        
        // MVP: Assume it's an executable if it's not a library. 
        // For now we will warn if main is missing instead of error, so we don't break old tests without main yet.
        if !has_main {
            self.reporter.report(Diagnostic::warning("No 'main' function found")
                .with_hint("Executables must have an entry point 'fn main()'"));
        }
        
        // 1. Gather all top-level types (Structs/Classes/Functions)
        for decl in &program.declarations {
            match decl {
                Decl::Struct { id, name, methods, .. } => {
                    self.named_types.insert(name.clone(), *id);
                    for method in methods {
                        if let Decl::Function { name: m_name, params, return_type, id: m_id, .. } = method {
                            let mut param_tys = Vec::new();
                            for (_, _, pty) in params {
                                param_tys.push(self.resolve_type(pty).unwrap_or(Ty::Int));
                            }
                            let ret_ty = if let Some(r) = return_type {
                                self.resolve_type(r).unwrap_or(Ty::Int)
                            } else {
                                Ty::Int
                            };
                            self.env.insert(*m_id, Ty::Function(param_tys.clone(), Box::new(ret_ty.clone())));
                            self.methods_env.insert(m_name.clone(), Ty::Function(param_tys, Box::new(ret_ty)));
                        }
                    }
                }
                Decl::Class { id, name, methods, .. } => {
                    self.named_types.insert(name.clone(), *id);
                    for method in methods {
                        if let Decl::Function { name: m_name, params, return_type, id: m_id, .. } = method {
                            let mut param_tys = Vec::new();
                            for (_, _, pty) in params {
                                param_tys.push(self.resolve_type(pty).unwrap_or(Ty::Int));
                            }
                            let ret_ty = if let Some(r) = return_type {
                                self.resolve_type(r).unwrap_or(Ty::Int)
                            } else {
                                Ty::Int
                            };
                            self.env.insert(*m_id, Ty::Function(param_tys.clone(), Box::new(ret_ty.clone())));
                            self.methods_env.insert(m_name.clone(), Ty::Function(param_tys, Box::new(ret_ty)));
                        }
                    }
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
                    self.mutability_env.insert(*id, false); // Let is immutable
                }
                pace_hir::Stmt::Var { id, value, .. } => {
                    let ty = self.check_expr(value)?;
                    self.env.insert(*id, ty);
                    self.mutability_env.insert(*id, true); // Var is mutable
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
                self.mutability_env.insert(*id, false); // Let is immutable
                Ok(())
            }
            Decl::Var { id, value, .. } => {
                let ty = self.check_expr(value)?;
                self.env.insert(*id, ty);
                self.mutability_env.insert(*id, true); // Var is mutable
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
            Decl::Function { name, params, body, span, .. } => {
                if name.contains('_') && name != "main" && !name.ends_with("_init") {
                    self.reporter.report(Diagnostic::warning(format!("Function '{}' should use camelCase, not snake_case", name))
                        .with_span(*span)
                        .with_hint("Rename to camelCase"));
                }
                
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
                
                if let Ty::Struct(id) = callee_ty {
                    let mut struct_name = "";
                    for (name, nid) in &self.named_types {
                        if *nid == id { struct_name = name; break; }
                    }
                    let init_name = format!("{}_init", struct_name);
                    
                    if let Some(Ty::Function(param_tys, _)) = self.methods_env.get(&init_name).cloned() {
                        if args.len() != param_tys.len() - 1 {
                            return Err(format!("{} takes {} arguments, got {}", init_name, param_tys.len() - 1, args.len()));
                        }
                        for (i, (_, fexpr)) in args.iter().enumerate() {
                            let fty = self.check_expr(fexpr)?;
                            if fty != param_tys[i + 1] {
                                return Err(format!("Argument {} expects type {:?}, got {:?}", i, param_tys[i + 1], fty));
                            }
                        }
                        return Ok(Ty::Struct(id));
                    }
                    
                    let def_fields = self.struct_defs.get(&id).unwrap().clone();
                    let mut def_map: std::collections::HashMap<_, _> = def_fields.into_iter().collect();
                    for (label, fexpr) in args {
                        let fty = self.check_expr(fexpr)?;
                        if let Some(fname) = label {
                            if let Some(expected_ty) = def_map.remove(fname) {
                                if fty != expected_ty {
                                    return Err(format!("Field '{}' expects type {:?}, got {:?}", fname, expected_ty, fty));
                                }
                            } else {
                                return Err(format!("Unknown field '{}' in instantiation", fname));
                            }
                        } else {
                            return Err("Struct instantiation requires named arguments".to_string());
                        }
                    }
                    if !def_map.is_empty() {
                        return Err(format!("Missing fields in struct instantiation: {:?}", def_map.keys()));
                    }
                    return Ok(Ty::Struct(id));
                }
                
                if let Ty::Class(id) = callee_ty {
                    let mut class_name = "";
                    for (name, nid) in &self.named_types {
                        if *nid == id { class_name = name; break; }
                    }
                    let init_name = format!("{}_init", class_name);
                    
                    if let Some(Ty::Function(param_tys, _)) = self.methods_env.get(&init_name).cloned() {
                        if args.len() != param_tys.len() - 1 {
                            return Err(format!("{} takes {} arguments, got {}", init_name, param_tys.len() - 1, args.len()));
                        }
                        for (i, (_, fexpr)) in args.iter().enumerate() {
                            let fty = self.check_expr(fexpr)?;
                            if fty != param_tys[i + 1] {
                                return Err(format!("Argument {} expects type {:?}, got {:?}", i, param_tys[i + 1], fty));
                            }
                        }
                        return Ok(Ty::Class(id));
                    }

                    let def_fields = self.class_defs.get(&id).unwrap().clone();
                    let mut def_map: std::collections::HashMap<_, _> = def_fields.into_iter().collect();
                    for (label, fexpr) in args {
                        let fty = self.check_expr(fexpr)?;
                        if let Some(fname) = label {
                            if let Some(expected_ty) = def_map.remove(fname) {
                                if fty != expected_ty {
                                    return Err(format!("Field '{}' expects type {:?}, got {:?}", fname, expected_ty, fty));
                                }
                            } else {
                                return Err(format!("Unknown field '{}' in instantiation", fname));
                            }
                        } else {
                            return Err("Class instantiation requires named arguments".to_string());
                        }
                    }
                    if !def_map.is_empty() {
                        return Err(format!("Missing fields in class instantiation: {:?}", def_map.keys()));
                    }
                    return Ok(Ty::Class(id));
                }

                for (_, arg) in args {
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
            Expr::Assign { target, value, span } => {
                let target_ty = self.check_expr(target)?;
                
                // Mutability check
                if let Expr::Ident(id, _) = &**target {
                    if let Some(&is_mut) = self.mutability_env.get(id) {
                        if !is_mut {
                            self.reporter.report(Diagnostic::error("Cannot reassign immutable variable")
                                .with_span(*span)
                                .with_hint("Declare this variable with 'var' instead of 'let' to make it mutable"));
                        }
                    }
                } else if let Expr::MemberAccess { object, .. } = &**target {
                    if let Expr::Ident(id, _) = &**object {
                        if let Some(&is_mut) = self.mutability_env.get(id) {
                            if !is_mut {
                                self.reporter.report(Diagnostic::error("Cannot mutate field of immutable variable")
                                    .with_span(*span)
                                    .with_hint("Declare this variable with 'var' instead of 'let' to make it mutable"));
                            }
                        }
                    }
                }
                
                let val_ty = self.check_expr(value)?;
                if target_ty != val_ty {
                    self.reporter.report(Diagnostic::error(format!("Type mismatch in assignment: expected {:?}, got {:?}", target_ty, val_ty))
                        .with_span(*span));
                    return Ok(target_ty); // Return target type to continue checking gracefully
                }
                Ok(target_ty)
            }
        }
    }
}
