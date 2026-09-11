use std::collections::HashMap;
use pace_hir::{Decl, Expr, HirId, Program, Stmt};

use crate::ty::Ty;

use pace_errors::{Reporter, Diagnostic, ErrorCode};

pub struct TypeChecker {
    pub env: HashMap<HirId, Ty>,
    pub mutability_env: HashMap<HirId, bool>, // true = mutable, false = immutable
    pub methods_env: HashMap<String, Ty>,
    pub struct_defs: HashMap<HirId, Vec<(String, Ty)>>,
    pub class_defs: HashMap<HirId, Vec<(String, Ty)>>,
    pub static_fields_env: HashMap<String, Ty>, // format: "{class_name}_{field_name}"
    pub const_env: HashMap<String, Ty>,
    pub named_types: HashMap<String, (HirId, bool)>, // maps name like "User" to (HirId, is_class)
    pub declared_bindings: Vec<(HirId, String, pace_span::Span)>,
    pub used_bindings: std::collections::HashSet<HirId>,
    pub initialized_bindings: std::collections::HashSet<HirId>,
    pub local_types: HashMap<HirId, Ty>, // Persisted types of all variables
    pub reporter: Reporter,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
            mutability_env: HashMap::new(),
            methods_env: HashMap::new(),
            struct_defs: HashMap::new(),
            class_defs: HashMap::new(),
            static_fields_env: HashMap::new(),
            const_env: HashMap::new(),
            named_types: HashMap::new(),
            declared_bindings: Vec::new(),
            used_bindings: std::collections::HashSet::new(),
            initialized_bindings: std::collections::HashSet::new(),
            local_types: HashMap::new(),
            reporter: Reporter::new(),
        }
    }

    pub fn resolve_type(&self, ast_ty: &pace_ast::Type) -> Result<Ty, String> {
        match ast_ty {
            pace_ast::Type::Named(id) => {
                match id.name.as_str() {
                    "Int" | "int" => Ok(Ty::Int),
                    "Float" | "float" => Ok(Ty::Float),
                    "String" | "string" => Ok(Ty::String),
                    "Bool" | "bool" => Ok(Ty::Bool),
                    "Void" | "void" => Ok(Ty::Void),
                    other => {
                        if let Some(&(hir_id, is_class)) = self.named_types.get(other) {
                            if is_class {
                                return Ok(Ty::Class(hir_id));
                            } else {
                                return Ok(Ty::Struct(hir_id));
                            }
                        }
                        Err(format!("Unknown type: {}", other))
                    }
                }
            }
            pace_ast::Type::Optional(inner_ty, _) => {
                let inner = self.resolve_type(inner_ty)?;
                Ok(Ty::Optional(Box::new(inner)))
            }
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
                Decl::Struct { id, name, static_fields, const_fields, methods, .. } => {
                    self.named_types.insert(name.clone(), (*id, false));
                    for (sf_name, sf_ty, _) in static_fields {
                         let resolved_ty = self.resolve_type(sf_ty).unwrap_or(Ty::Int);
                         self.static_fields_env.insert(format!("{}_{}", name, sf_name), resolved_ty);
                    }
                    for (cf_name, cf_ty, _) in const_fields {
                         let resolved_ty = self.resolve_type(cf_ty).unwrap_or(Ty::Int);
                         self.const_env.insert(format!("{}_{}", name, cf_name), resolved_ty);
                    }
                    for method in methods {
                        if let Decl::Function { name: m_name, params, return_type, id: m_id, .. } = method {
                            let mut param_tys = Vec::new();
                            for (_, _, pty) in params {
                                param_tys.push(self.resolve_type(pty).unwrap_or(Ty::Int));
                            }
                            let ret_ty = if let Some(r) = return_type {
                                self.resolve_type(r).unwrap_or(Ty::Void)
                            } else {
                                Ty::Void
                            };
                            self.env.insert(*m_id, Ty::Function(param_tys.clone(), Box::new(ret_ty.clone())));
                            self.methods_env.insert(m_name.clone(), Ty::Function(param_tys, Box::new(ret_ty)));
                        }
                    }
                }
                Decl::Class { id, name, static_fields, const_fields, methods, .. } => {
                    self.named_types.insert(name.clone(), (*id, true));
                    for (sf_name, sf_ty, _) in static_fields {
                         let resolved_ty = self.resolve_type(sf_ty).unwrap_or(Ty::Int);
                         self.static_fields_env.insert(format!("{}_{}", name, sf_name), resolved_ty);
                    }
                    for (cf_name, cf_ty, _) in const_fields {
                         let resolved_ty = self.resolve_type(cf_ty).unwrap_or(Ty::Int);
                         self.const_env.insert(format!("{}_{}", name, cf_name), resolved_ty);
                    }
                    for method in methods {
                        if let Decl::Function { name: m_name, params, return_type, id: m_id, .. } = method {
                            let mut param_tys = Vec::new();
                            for (_, _, pty) in params {
                                param_tys.push(self.resolve_type(pty).unwrap_or(Ty::Int));
                            }
                            let ret_ty = if let Some(r) = return_type {
                                self.resolve_type(r).unwrap_or(Ty::Void)
                            } else {
                                Ty::Void
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
                    for (fname, fty, _) in fields {
                        resolved_fields.push((fname.clone(), self.resolve_type(fty)?));
                    }
                    self.struct_defs.insert(*id, resolved_fields);
                }
                Decl::Class { id, fields, .. } => {
                    let mut resolved_fields = Vec::new();
                    for (fname, fty, _) in fields {
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
                    Ty::Void
                };
                self.env.insert(*id, Ty::Function(param_tys, Box::new(ret_ty)));
            }
        }

        for decl in &program.declarations {
            self.check_decl(decl)?;
        }
        
        // Pass 4: Unused Variables and Functions Linter Sweep
        for (id, name, span) in &self.declared_bindings {
            let is_compiler_generated = self.named_types.keys().any(|k| name.starts_with(&format!("{}_", k)));
            if !self.used_bindings.contains(&id) && !name.starts_with('_') && name != "main" && !name.ends_with("_init") && !is_compiler_generated {
                self.reporter.report(Diagnostic::warning(format!("unused variable or function: `{}`", name))
                    .with_span(*span)
                    .with_code(ErrorCode::UnusedVariable)
                    .with_hint(format!("if this is intentional, prefix it with an underscore: `_{}`", name)));
            }
        }
        
        Ok(())
    }

    pub fn check_block(&mut self, block: &pace_hir::Block, expected_ret_ty: Option<&Ty>) -> Result<(), String> {
        let outer_env = self.env.clone();
        for stmt in &block.statements {
            match stmt {
                pace_hir::Stmt::Let { id, name, ty: explicit_ty, value, span } => {
                    let mut ty = if let Some(val) = value {
                        self.initialized_bindings.insert(*id);
                        self.check_expr(val)?
                    } else {
                        // Uninitialized variable
                        let explicit = explicit_ty.as_ref().unwrap(); // Guaranteed by parser
                        let expected = self.resolve_type(explicit)?;
                        if let Ty::Optional(_) = expected {
                            self.initialized_bindings.insert(*id); // Implicitly initialized to null
                        }
                        expected
                    };
                    
                    if let Some(explicit) = explicit_ty {
                        let expected = self.resolve_type(explicit)?;
                        if ty != expected {
                            if expected != Ty::Optional(Box::new(ty.clone())) {
                                self.reporter.report(pace_errors::Diagnostic::error(format!("Type mismatch: expected {:?}, got {:?}", expected, ty)).with_span(explicit.span()));
                                return Err("Type mismatch".to_string());
                            }
                            ty = expected; // Promote to Optional
                        }
                    }
                    self.env.insert(*id, ty.clone());
                    self.local_types.insert(*id, ty.clone());
                    self.mutability_env.insert(*id, false); // Let is immutable
                    self.declared_bindings.push((*id, name.clone(), *span));
                }
                pace_hir::Stmt::Var { id, name, ty: explicit_ty, value, span } => {
                    let mut ty = if let Some(val) = value {
                        self.initialized_bindings.insert(*id);
                        self.check_expr(val)?
                    } else {
                        // Uninitialized variable
                        let explicit = explicit_ty.as_ref().unwrap(); // Guaranteed by parser
                        let expected = self.resolve_type(explicit)?;
                        if let Ty::Optional(_) = expected {
                            self.initialized_bindings.insert(*id); // Implicitly initialized to null
                        }
                        expected
                    };
                    
                    if let Some(explicit) = explicit_ty {
                        let expected = self.resolve_type(explicit)?;
                        if ty != expected {
                            if expected != Ty::Optional(Box::new(ty.clone())) {
                                self.reporter.report(pace_errors::Diagnostic::error(format!("Type mismatch: expected {:?}, got {:?}", expected, ty)).with_span(explicit.span()));
                                return Err("Type mismatch".to_string());
                            }
                            ty = expected; // Promote to Optional
                        }
                    }
                    self.env.insert(*id, ty.clone());
                    self.local_types.insert(*id, ty.clone());
                    self.mutability_env.insert(*id, true); // Var is mutable
                    self.declared_bindings.push((*id, name.clone(), *span));
                }
                pace_hir::Stmt::ExprStmt(expr, _) => {
                    self.check_expr(expr)?;
                }
                pace_hir::Stmt::Return(expr, span) => {
                    let ret_ty = if let Some(e) = expr {
                        self.check_expr(e)?
                    } else {
                        Ty::Void
                    };
                    
                    if let Some(expected) = expected_ret_ty {
                        if ret_ty != *expected {
                            self.reporter.report(Diagnostic::error(format!("Type mismatch: function expects to return {:?}, but returned {:?}", expected, ret_ty))
                                .with_span(*span)
                                .with_code(ErrorCode::TypeMismatch));
                        }
                    }
                }
            }
        }
        self.env = outer_env;
        Ok(())
    }

    pub fn check_decl(&mut self, decl: &Decl) -> Result<(), String> {
        match decl {
            Decl::Let { id, name, ty: explicit_ty, value, span } => {
                let mut ty = if let Some(val) = value {
                    self.initialized_bindings.insert(*id);
                    self.check_expr(val)?
                } else {
                    let explicit = explicit_ty.as_ref().unwrap();
                    let expected = self.resolve_type(explicit)?;
                    if let Ty::Optional(_) = expected {
                        self.initialized_bindings.insert(*id);
                    }
                    expected
                };
                
                if let Some(explicit) = explicit_ty {
                    let expected = self.resolve_type(explicit)?;
                    if ty != expected {
                        if expected != Ty::Optional(Box::new(ty.clone())) {
                            self.reporter.report(pace_errors::Diagnostic::error(format!("Type mismatch: expected {:?}, got {:?}", expected, ty)).with_span(explicit.span()));
                            return Err("Type mismatch".to_string());
                        }
                        ty = expected;
                    }
                }
                self.env.insert(*id, ty.clone());
                self.local_types.insert(*id, ty.clone());
                self.mutability_env.insert(*id, false); // Let is immutable
                self.declared_bindings.push((*id, name.clone(), *span));
                Ok(())
            }
            Decl::Var { id, name, ty: explicit_ty, value, span } => {
                let mut ty = if let Some(val) = value {
                    self.initialized_bindings.insert(*id);
                    self.check_expr(val)?
                } else {
                    let explicit = explicit_ty.as_ref().unwrap();
                    let expected = self.resolve_type(explicit)?;
                    if let Ty::Optional(_) = expected {
                        self.initialized_bindings.insert(*id);
                    }
                    expected
                };
                
                if let Some(explicit) = explicit_ty {
                    let expected = self.resolve_type(explicit)?;
                    if ty != expected {
                        if expected != Ty::Optional(Box::new(ty.clone())) {
                            self.reporter.report(pace_errors::Diagnostic::error(format!("Type mismatch: expected {:?}, got {:?}", expected, ty)).with_span(explicit.span()));
                            return Err("Type mismatch".to_string());
                        }
                        ty = expected;
                    }
                }
                self.env.insert(*id, ty.clone());
                self.local_types.insert(*id, ty.clone());
                self.mutability_env.insert(*id, true); // Var is mutable
                self.declared_bindings.push((*id, name.clone(), *span));
                Ok(())
            }
            Decl::Const { id, name, ty: explicit_ty, value, span } => {
                let mut ty = self.check_expr(value)?;
                if let Some(explicit) = explicit_ty {
                    let expected = self.resolve_type(explicit)?;
                    if ty != expected {
                        if expected != Ty::Optional(Box::new(ty.clone())) {
                            self.reporter.report(pace_errors::Diagnostic::error(format!("Type mismatch: expected {:?}, got {:?}", expected, ty)).with_span(explicit.span()));
                            return Err("Type mismatch".to_string());
                        }
                        ty = expected;
                    }
                }
                self.env.insert(*id, ty);
                self.mutability_env.insert(*id, false);
                self.declared_bindings.push((*id, name.clone(), *span));
                Ok(())
            }
            Decl::Struct { id, methods, static_fields, const_fields, .. } => {
                self.env.insert(*id, Ty::Struct(*id));
                for (_, sf_ty, sf_expr) in static_fields {
                    let expected_ty = self.resolve_type(sf_ty)?;
                    let expr_ty = self.check_expr(sf_expr)?;
                    if expected_ty != expr_ty {
                        self.reporter.report(Diagnostic::error(format!("Type mismatch: expected {:?}, got {:?}", expected_ty, expr_ty))
                            .with_span(sf_expr.span())
                            .with_code(ErrorCode::TypeMismatch));
                        return Err("Type mismatch".to_string());
                    }
                }
                for (_, cf_ty, cf_expr) in const_fields {
                    let expected_ty = self.resolve_type(cf_ty)?;
                    let expr_ty = self.check_expr(cf_expr)?;
                    if expected_ty != expr_ty {
                        self.reporter.report(Diagnostic::error(format!("Type mismatch: expected {:?}, got {:?}", expected_ty, expr_ty))
                            .with_span(cf_expr.span())
                            .with_code(ErrorCode::TypeMismatch));
                        return Err("Type mismatch".to_string());
                    }
                }
                for method in methods {
                    self.check_decl(method)?;
                }
                Ok(())
            }
            Decl::Class { id, methods, static_fields, const_fields, .. } => {
                self.env.insert(*id, Ty::Class(*id));
                for (_, sf_ty, sf_expr) in static_fields {
                    let expected_ty = self.resolve_type(sf_ty)?;
                    let expr_ty = self.check_expr(sf_expr)?;
                    if expected_ty != expr_ty {
                        self.reporter.report(Diagnostic::error(format!("Type mismatch: expected {:?}, got {:?}", expected_ty, expr_ty))
                            .with_span(sf_expr.span())
                            .with_code(ErrorCode::TypeMismatch));
                        return Err("Type mismatch".to_string());
                    }
                }
                for (_, cf_ty, cf_expr) in const_fields {
                    let expected_ty = self.resolve_type(cf_ty)?;
                    let expr_ty = self.check_expr(cf_expr)?;
                    if expected_ty != expr_ty {
                        self.reporter.report(Diagnostic::error(format!("Type mismatch: expected {:?}, got {:?}", expected_ty, expr_ty))
                            .with_span(cf_expr.span())
                            .with_code(ErrorCode::TypeMismatch));
                        return Err("Type mismatch".to_string());
                    }
                }
                for method in methods {
                    self.check_decl(method)?;
                }
                Ok(())
            }
            Decl::Expr(expr, _) => {
                self.check_expr(expr)?;
                Ok(())
            }
            Decl::Function { id, name, params, return_type, body, span, .. } => {
                if name.contains('_') && name != "main" && !name.ends_with("_init") {
                    // Only warn for functions not generated by the compiler
                    let is_compiler_generated = self.named_types.keys().any(|k| name.starts_with(k));
                    if !is_compiler_generated {
                        self.reporter.report(Diagnostic::warning(format!("Function '{}' should use camelCase, not snake_case", name))
                            .with_span(*span)
                            .with_code(ErrorCode::SnakeCaseName)
                            .with_hint("Rename to camelCase"));
                    }
                }
                self.declared_bindings.push((*id, name.clone(), *span));
                
                // Definite assignment check for initializers
                if name.ends_with("_init") {
                    let type_name = name.trim_end_matches("_init");
                    let mut assigned_fields = std::collections::HashSet::new();
                    let self_id = params.iter().find(|(_, name, _)| name == "self").map(|(id, _, _)| *id);
                    
                    // Simple analysis: collect all `self.field = value` assignments in the top-level block
                    for stmt in &body.statements {
                        if let Stmt::ExprStmt(Expr::Assign { target, .. }, _) = stmt {
                            if let Expr::MemberAccess { object, member, .. } = &**target {
                                if let Expr::Ident(obj_id, _) = &**object {
                                    if Some(*obj_id) == self_id {
                                        assigned_fields.insert(member.clone());
                                    }
                                }
                            }
                        }
                    }
                    
                    if let Some(&(hir_id, is_class)) = self.named_types.get(type_name) {
                        let fields = if is_class {
                            self.class_defs.get(&hir_id)
                        } else {
                            self.struct_defs.get(&hir_id)
                        };
                        
                        if let Some(fields) = fields {
                            for (fname, _) in fields {
                                if !assigned_fields.contains(fname) {
                                    self.reporter.report(Diagnostic::error(format!("Field '{}' must be initialized", fname))
                                        .with_span(*span)
                                        .with_hint(format!("Assign a value to 'self.{}' in the init method, or initialize it inline", fname)));
                                    return Err("Definite assignment error".to_string());
                                }
                            }
                        }
                    }
                }
                
                let outer_env = self.env.clone();
                for (param_id, _, pty) in params {
                    let ty = self.resolve_type(pty)?;
                    self.env.insert(*param_id, ty);
                }
                let ret_ty = if let Some(rty) = &return_type {
                    self.resolve_type(rty)?
                } else {
                    Ty::Void
                };
                
                let returns_exhaustively = self.check_exhaustive_return(body);
                if !returns_exhaustively && ret_ty != Ty::Void {
                    self.reporter.report(Diagnostic::error(format!("Function '{}' expects to return {:?}, but does not exhaustively return a value", name, ret_ty))
                        .with_span(*span)
                        .with_code(ErrorCode::NonExhaustiveReturn));
                }
                
                self.check_block(body, Some(&ret_ty))?;
                self.env = outer_env;
                Ok(())
            }
        }
    }

    pub fn check_exhaustive_return(&self, block: &pace_hir::Block) -> bool {
        for stmt in &block.statements {
            match stmt {
                pace_hir::Stmt::Return(..) => return true,
                pace_hir::Stmt::ExprStmt(expr, _) => {
                    if let pace_hir::Expr::If { then_block, else_block, .. } = expr {
                        let then_returns = self.check_exhaustive_return(then_block);
                        let else_returns = else_block.as_ref().map_or(false, |b| self.check_exhaustive_return(b));
                        if then_returns && else_returns {
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<Ty, String> {
        match expr {
            Expr::IntLiteral(..) => Ok(Ty::Int),
            Expr::FloatLiteral(..) => Ok(Ty::Float),
            Expr::BoolLiteral(..) => Ok(Ty::Bool),
            Expr::StringLiteral(..) => Ok(Ty::String),
            Expr::Ident(id, span) => {
                self.used_bindings.insert(*id);
                if !self.initialized_bindings.contains(id) {
                    if let Some((_, name, _)) = self.declared_bindings.iter().find(|(d_id, _, _)| d_id == id) {
                        self.reporter.report(Diagnostic::error(format!("Non-nullable variable '{}' must be assigned before it can be used", name))
                            .with_span(*span)
                            .with_code(ErrorCode::UninitializedVariable));
                    }
                }
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
                        let mut struct_name = "";
                        for (name, &(nid, _)) in &self.named_types {
                            if nid == hir_id { struct_name = name; break; }
                        }
                        
                        let fields = self.struct_defs.get(&hir_id).ok_or("Struct definition not found")?;
                        for (fname, fty) in fields {
                            if fname == member {
                                return Ok(fty.clone());
                            }
                        }
                        
                        // Check if it's a static field
                        if let Some(sfty) = self.static_fields_env.get(&format!("{}_{}", struct_name, member)) {
                            return Ok(sfty.clone());
                        }
                        
                        // Check if it's a method
                        let method_name = format!("{}_{}", struct_name, member);
                        if let Some(mty) = self.methods_env.get(&method_name) {
                            return Ok(mty.clone());
                        }
                        
                        Err(format!("Struct has no member '{}'", member))
                    }
                    Ty::Class(hir_id) => {
                        let mut class_name = "";
                        for (name, &(nid, _)) in &self.named_types {
                            if nid == hir_id { class_name = name; break; }
                        }
                        
                        let fields = self.class_defs.get(&hir_id).ok_or("Class definition not found")?;
                        for (fname, fty) in fields {
                            if fname == member {
                                return Ok(fty.clone());
                            }
                        }
                        
                        // Check if it's a static field
                        if let Some(sfty) = self.static_fields_env.get(&format!("{}_{}", class_name, member)) {
                            return Ok(sfty.clone());
                        }
                        
                        // Check if it's a method
                        let method_name = format!("{}_{}", class_name, member);
                        if let Some(mty) = self.methods_env.get(&method_name) {
                            return Ok(mty.clone());
                        }
                        
                        Err(format!("Class has no member '{}'", member))
                    }
                    _ => Err(format!("Cannot access member '{}' on type {:?}", member, obj_ty)),
                }
            }
            Expr::Call { callee, args, span } => {
                let callee_ty = self.check_expr(callee)?;
                
                if let Ty::Struct(id) = callee_ty {
                    let mut struct_name = "";
                    for (name, &(nid, _)) in &self.named_types {
                        if nid == id { struct_name = name; break; }
                    }
                    let init_name = format!("{}_init", struct_name);
                    
                    if let Some(Ty::Function(param_tys, _)) = self.methods_env.get(&init_name).cloned() {
                        if args.len() != param_tys.len() - 1 {
                            self.reporter.report(Diagnostic::error(format!("{} takes {} arguments, got {}", init_name, param_tys.len() - 1, args.len())).with_span(*span).with_code(ErrorCode::ArityMismatch));
                            return Err(format!("{} takes {} arguments, got {}", init_name, param_tys.len() - 1, args.len()));
                        }
                        for (i, (_, fexpr)) in args.iter().enumerate() {
                            let fty = self.check_expr(fexpr)?;
                            if fty != param_tys[i + 1] {
                                self.reporter.report(Diagnostic::error(format!("Argument {} expects type {:?}, got {:?}", i, param_tys[i + 1], fty)).with_span(*span).with_code(ErrorCode::TypeMismatch));
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
                                    self.reporter.report(Diagnostic::error(format!("Field '{}' expects type {:?}, got {:?}", fname, expected_ty, fty)).with_span(*span).with_code(ErrorCode::TypeMismatch));
                                    return Err(format!("Field '{}' expects type {:?}, got {:?}", fname, expected_ty, fty));
                                }
                            } else {
                                self.reporter.report(Diagnostic::error(format!("Unknown field '{}' in instantiation", fname)).with_span(*span).with_code(ErrorCode::UnknownField));
                                return Err(format!("Unknown field '{}' in instantiation", fname));
                            }
                        } else {
                            self.reporter.report(Diagnostic::error("Struct instantiation requires named arguments").with_span(*span).with_code(ErrorCode::InvalidArguments));
                            return Err("Struct instantiation requires named arguments".to_string());
                        }
                    }
                    def_map.retain(|_, ty| !matches!(ty, Ty::Optional(_)));
                    if !def_map.is_empty() {
                        self.reporter.report(Diagnostic::error(format!("Missing fields in struct instantiation: {:?}", def_map.keys())).with_span(*span).with_code(ErrorCode::MissingFields));
                        return Err(format!("Missing fields in struct instantiation: {:?}", def_map.keys()));
                    }
                    return Ok(Ty::Struct(id));
                }
                
                if let Ty::Class(id) = callee_ty {
                    let mut class_name = "";
                    for (name, &(nid, _)) in &self.named_types {
                        if nid == id { class_name = name; break; }
                    }
                    let init_name = format!("{}_init", class_name);
                    
                    if let Some(Ty::Function(param_tys, _)) = self.methods_env.get(&init_name).cloned() {
                        if args.len() != param_tys.len() - 1 {
                            self.reporter.report(Diagnostic::error(format!("{} takes {} arguments, got {}", init_name, param_tys.len() - 1, args.len())).with_span(*span).with_code(ErrorCode::ArityMismatch));
                            return Err(format!("{} takes {} arguments, got {}", init_name, param_tys.len() - 1, args.len()));
                        }
                        for (i, (_, fexpr)) in args.iter().enumerate() {
                            let fty = self.check_expr(fexpr)?;
                            if fty != param_tys[i + 1] {
                                self.reporter.report(Diagnostic::error(format!("Argument {} expects type {:?}, got {:?}", i, param_tys[i + 1], fty)).with_span(*span).with_code(ErrorCode::TypeMismatch));
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
                                    self.reporter.report(Diagnostic::error(format!("Field '{}' expects type {:?}, got {:?}", fname, expected_ty, fty)).with_span(*span).with_code(ErrorCode::TypeMismatch));
                                    return Err(format!("Field '{}' expects type {:?}, got {:?}", fname, expected_ty, fty));
                                }
                            } else {
                                self.reporter.report(Diagnostic::error(format!("Unknown field '{}' in instantiation", fname)).with_span(*span).with_code(ErrorCode::UnknownField));
                                return Err(format!("Unknown field '{}' in instantiation", fname));
                            }
                        } else {
                            self.reporter.report(Diagnostic::error("Class instantiation requires named arguments").with_span(*span).with_code(ErrorCode::InvalidArguments));
                            return Err("Class instantiation requires named arguments".to_string());
                        }
                    }
                    def_map.retain(|_, ty| !matches!(ty, Ty::Optional(_)));
                    if !def_map.is_empty() {
                        self.reporter.report(Diagnostic::error(format!("Missing fields in class instantiation: {:?}", def_map.keys())).with_span(*span).with_code(ErrorCode::MissingFields));
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
                self.check_block(then_block, None)?;
                if let Some(else_b) = else_block {
                    self.check_block(else_b, None)?;
                }
                Ok(Ty::Int)
            }
            Expr::While { cond, body, .. } => {
                self.check_expr(cond)?;
                self.check_block(body, None)?;
                Ok(Ty::Int)
            }
            Expr::Assign { target, value, span } => {
                // Determine target type without checking initialization for Idents
                let target_ty = match &**target {
                    Expr::Ident(id, _) => {
                        self.used_bindings.insert(*id);
                        self.env.get(id).cloned().ok_or(format!("Cannot infer type for unbound variable"))?
                    },
                    _ => self.check_expr(target)?,
                };
                
                // Mutability check
                if let Expr::Ident(id, _) = &**target {
                    let was_initialized = self.initialized_bindings.contains(id);
                    self.initialized_bindings.insert(*id); // Mark as initialized upon assignment
                    if let Some(&is_mut) = self.mutability_env.get(id) {
                        if !is_mut && was_initialized {
                            self.reporter.report(Diagnostic::error("Cannot reassign immutable variable")
                                .with_span(*span)
                                .with_code(ErrorCode::ImmutableAssignment)
                                .with_hint("Declare this variable with 'var' instead of 'let' to make it mutable"));
                        }
                    }
                } else if let Expr::MemberAccess { object, .. } = &**target {
                    if let Expr::Ident(id, _) = &**object {
                        if let Some(&is_mut) = self.mutability_env.get(id) {
                            if !is_mut {
                                self.reporter.report(Diagnostic::error("Cannot mutate field of immutable variable")
                                    .with_span(*span)
                                    .with_code(ErrorCode::ImmutableAssignment)
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
