use crate::mir::*;
use pace_hir::{Expr, HirId};
use pace_ty::{Ty, TypeChecker};
use std::collections::HashMap;

fn tc_get_type(tc: &TypeChecker, ty: &pace_ast::Type) -> Option<Ty> {
    tc.get_type(ty).ok().or_else(|| {
        if let pace_ast::Type::Named(id) = ty {
            let suffix = format!("_{}", id.name);
            for (mangled, &(hir_id, kind)) in &tc.named_types {
                if mangled == id.name || mangled.ends_with(&suffix) {
                    if kind == 1 {
                        return Some(Ty::Class(hir_id));
                    }
                    if kind == 0 {
                        return Some(Ty::Struct(hir_id));
                    }
                    return Some(Ty::Enum(hir_id));
                }
            }
        }
        None
    })
}

fn get_mangled_name(tc: &TypeChecker, name: &str) -> String {
    if name == "main" {
        return name.to_string();
    }
    for k in tc.global_functions.keys().chain(tc.methods_env.keys()) {
        if k == name || k.ends_with(&format!("_{}", name)) {
            return k.clone();
        }
    }
    name.to_string()
}

pub struct MirBuilder<'a> {
    pub blocks: Vec<BasicBlock>,
    pub current_block: BasicBlockId,
    pub locals: Vec<Ty>,
    pub hir_to_local: HashMap<HirId, Local>,
    pub global_fns: HashMap<String, String>,
    pub struct_defs: &'a HashMap<HirId, Vec<pace_ty::ResolvedField>>,
    pub class_defs: &'a HashMap<HirId, Vec<pace_ty::ResolvedField>>,
    pub class_vtables: &'a HashMap<HirId, Vec<pace_ty::VTableEntry>>,
    pub enum_defs: &'a HashMap<HirId, Vec<pace_hir::EnumVariant>>,
    pub global_env: &'a HashMap<pace_hir::HirId, Ty>,
    pub local_types: &'a HashMap<pace_hir::HirId, Ty>,
    pub named_types: &'a HashMap<String, (pace_hir::HirId, u8)>,
    pub static_fields_env: &'a HashMap<String, Ty>,
    pub methods_env: &'a HashMap<String, Ty>,
    pub current_expected_ty: Option<Ty>,
    pub current_self_local: Option<Local>,
    pub class_parents: &'a HashMap<HirId, HirId>,
    pub global_functions_env: &'a HashMap<String, Ty>,
    pub resolved_global_names: &'a HashMap<pace_hir::HirId, String>,
    pub closure_functions: Vec<MirFunction>,
    pub closure_counter: usize,
}

impl<'a> MirBuilder<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        global_fns: HashMap<String, String>,
        struct_defs: &'a HashMap<HirId, Vec<pace_ty::ResolvedField>>,
        class_defs: &'a HashMap<pace_hir::HirId, Vec<pace_ty::ResolvedField>>,
        class_vtables: &'a HashMap<HirId, Vec<pace_ty::VTableEntry>>,
        enum_defs: &'a HashMap<pace_hir::HirId, Vec<pace_hir::EnumVariant>>,
        global_env: &'a HashMap<pace_hir::HirId, Ty>,
        local_types: &'a HashMap<pace_hir::HirId, Ty>,
        named_types: &'a HashMap<String, (pace_hir::HirId, u8)>,
        static_fields_env: &'a HashMap<String, Ty>,
        methods_env: &'a HashMap<String, Ty>,
        class_parents: &'a HashMap<HirId, HirId>,
        global_functions_env: &'a HashMap<String, Ty>,
        resolved_global_names: &'a HashMap<pace_hir::HirId, String>,
    ) -> Self {
        let initial_block = BasicBlock {
            statements: Vec::new(),
            terminator: None,
        };
        MirBuilder {
            blocks: vec![initial_block],
            current_block: BasicBlockId(0),
            locals: Vec::new(),
            hir_to_local: HashMap::new(),
            current_expected_ty: None,
            current_self_local: None,
            global_fns,
            struct_defs,
            class_defs,
            class_vtables,
            enum_defs,
            global_env,
            local_types,
            named_types,
            static_fields_env,
            methods_env,
            class_parents,
            global_functions_env,
            resolved_global_names,
            closure_functions: Vec::new(),
            closure_counter: 0,
        }
    }

    pub fn new_local(&mut self, ty: Ty) -> Local {
        let l = Local(self.locals.len() as u32);
        self.locals.push(ty);
        l
    }

    pub fn new_block(&mut self) -> BasicBlockId {
        let id = BasicBlockId(self.blocks.len() as u32);
        self.blocks.push(BasicBlock {
            statements: Vec::new(),
            terminator: None,
        });
        id
    }

    pub fn build_block(&mut self, block: &pace_hir::Block) {
        for stmt in &block.statements {
            match stmt {
                pace_hir::Stmt::Let { id, value, .. } | pace_hir::Stmt::Var { id, value, .. } => {
                    let var_ty = self
                        .local_types
                        .get(id)
                        .expect("Variable type not found in local_types environment")
                        .clone();
                    if let Some(val) = value {
                        let prev = self.current_expected_ty.take();
                        self.current_expected_ty = Some(var_ty.clone());
                        let rval_local = self.build_expr(val);
                        self.current_expected_ty = prev;
                        let var_local = self.new_local(var_ty.clone());
                        self.hir_to_local.insert(*id, var_local);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(var_local),
                            Rvalue::Use(rval_local),
                        ));
                        if matches!(var_ty, Ty::Class(_)) {
                            self.push_stmt(Statement::Retain(Lvalue::Local(var_local)));
                        }
                    } else {
                        // Uninitialized variable
                        let var_ty = self
                            .local_types
                            .get(id)
                            .expect("Variable type not found in local_types environment")
                            .clone();
                        let var_local = self.new_local(var_ty.clone());
                        self.hir_to_local.insert(*id, var_local);
                        // C backend automatically zeroes/nulls locals upon declaration
                    }
                }
                pace_hir::Stmt::ExprStmt(expr, _) => {
                    self.build_expr(expr);
                }
                pace_hir::Stmt::Return(expr, _) => {
                    let local = match expr {
                        Some(e) => self.build_expr(e),
                        None => {
                            let temp = self.new_local(Ty::Int);
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(temp),
                                Rvalue::Constant(Constant::Int("0".to_string())),
                            ));
                            temp
                        }
                    };

                    let current_bb = self.current_block.0 as usize;
                    self.blocks[current_bb].terminator = Some(Terminator::Return(local));
                    break;
                }
            }
        }
    }

    fn push_stmt(&mut self, stmt: Statement) {
        let idx = self.current_block.0 as usize;
        self.blocks[idx].statements.push(stmt);
    }

    pub fn build_expr(&mut self, expr: &Expr) -> Local {
        match expr {
            Expr::Super(_) => {
                let local = self
                    .current_self_local
                    .expect("Super used outside of a method");
                let temp = self.new_local(self.locals[local.0 as usize].clone());
                self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::Use(local)));
                temp
            }
            Expr::Closure { params, return_type: _, body, span: _ } => {
                self.closure_counter += 1;
                let closure_name = format!("__closure_{}", self.closure_counter);
                
                let mut fn_builder = MirBuilder::new(
                    self.global_fns.clone(),
                    self.struct_defs,
                    self.class_defs,
                    self.class_vtables,
                    self.enum_defs,
                    self.global_env,
                    self.local_types,
                    self.named_types,
                    self.static_fields_env,
                    self.methods_env,
                    self.class_parents,
                    self.global_functions_env,
                    self.resolved_global_names,
                );
                
                let mut mir_params = Vec::new();
                let mut param_tys = Vec::new();
                for param in params {
                    let ty = self.local_types.get(&param.id).cloned().unwrap_or(Ty::Int);
                    param_tys.push(ty.clone());
                    let local = fn_builder.new_local(ty);
                    fn_builder.hir_to_local.insert(param.id, local);
                    mir_params.push(local);
                }
                
                let body_local = fn_builder.build_expr(body);
                let ret_ty = fn_builder.locals[body_local.0 as usize].clone();
                let ret_local = fn_builder.new_local(ret_ty.clone());
                fn_builder.push_stmt(Statement::Assign(Lvalue::Local(ret_local), Rvalue::Use(body_local)));
                if fn_builder.blocks[fn_builder.current_block.0 as usize].terminator.is_none() {
                    fn_builder.blocks[fn_builder.current_block.0 as usize].terminator = Some(Terminator::Return(ret_local));
                }
                
                let fn_body = fn_builder.finish(&mir_params);
                
                self.closure_functions.push(MirFunction {
                    name: closure_name.clone(),
                    params: mir_params,
                    return_type: ret_ty.clone(),
                    body: fn_body,
                });
                
                let closure_ty = Ty::Closure(param_tys, Box::new(ret_ty));
                let temp = self.new_local(closure_ty);
                self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::GlobalRead(closure_name)));
                temp
            }
            Expr::Ident(id, name, _, _) => {
                if let Some(Ty::Enum(enum_id)) = self.global_env.get(id)
                    && let Some(variants) = self.enum_defs.get(enum_id)
                {
                    for v in variants {
                        if v.id == *id {
                            let temp = self.new_local(Ty::Enum(*enum_id));
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(temp),
                                Rvalue::InstantiateEnum(*enum_id, v.name.to_string(), vec![]),
                            ));
                            return temp;
                        }
                    }
                }

                let keys: Vec<_> = self.named_types.keys().collect();
                let local = *self.hir_to_local.get(id).unwrap_or_else(|| {
                    panic!(
                        "Local not found for id {:?} name {}. Named types: {:?}",
                        id, name, keys
                    )
                });
                let temp = self.new_local(self.locals[local.0 as usize].clone());
                self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::Use(local)));
                if matches!(self.locals[temp.0 as usize], Ty::Class(_)) {
                    self.push_stmt(Statement::Retain(Lvalue::Local(temp)));
                }
                temp
            }
            Expr::Null(_) => {
                // In C, we can just represent null as 0 or a special Option struct
                // For simplicity, we just use Void as the inner type
                let local = self.new_local(Ty::Optional(Box::new(Ty::Void)));
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(local),
                    Rvalue::Constant(Constant::Int("0".to_string())), // Represent null as 0 in MIR for MVP
                ));
                local
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                let lhs = self.build_expr(left);

                // For short-circuiting like ??, &&, ||, we should ideally use basic blocks
                // but for MVP, we just emit a binary op if C supports it, or lower to ternary
                let rhs = self.build_expr(right);

                let ret_ty = if *op == pace_ast::BinaryOp::NullCoalesce {
                    self.locals[rhs.0 as usize].clone()
                } else if *op == pace_ast::BinaryOp::And || *op == pace_ast::BinaryOp::Or {
                    Ty::Bool
                } else {
                    Ty::Int
                };

                let temp = self.new_local(ret_ty);
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(temp),
                    Rvalue::BinaryOp(*op, lhs, rhs),
                ));
                temp
            }
            Expr::IntLiteral(val, _) => {
                let local = self.new_local(Ty::Int);
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(local),
                    Rvalue::Constant(Constant::Int(val.clone())),
                ));
                local
            }
            Expr::FloatLiteral(val, _) => {
                let local = self.new_local(Ty::Float);
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(local),
                    Rvalue::Constant(Constant::Float(val.clone())),
                ));
                local
            }
            Expr::BoolLiteral(val, _) => {
                let local = self.new_local(Ty::Bool);
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(local),
                    Rvalue::Constant(Constant::Bool(*val)),
                ));
                local
            }
            Expr::StringLiteral(val, _) => {
                let temp = self.new_local(Ty::String);
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(temp),
                    Rvalue::Constant(Constant::String(val.clone())),
                ));
                temp
            }
            Expr::InterpolatedString(exprs, _) => {
                let mut args = Vec::new();
                for expr in exprs {
                    args.push(self.build_expr(expr));
                }
                let temp = self.new_local(Ty::String);
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(temp),
                    Rvalue::BuiltinCall("interpolate_string".to_string(), args),
                ));
                temp
            }
            Expr::OptionalMemberAccess { object, member, .. } => {
                let obj = self.build_expr(object);
                let obj_ty = self.locals[obj.0 as usize].clone();

                let _inner_ty = if let Ty::Optional(inner) = obj_ty {
                    *inner
                } else {
                    Ty::Void // Fallback
                };

                // We mock the field access type. In a real compiler we'd resolve it fully here.
                let temp = self.new_local(Ty::Optional(Box::new(Ty::Void))); // We just use Option<Void> as a placeholder, Codegen will emit properly
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(temp),
                    Rvalue::OptionalFieldAccess(obj, member.to_string()),
                ));
                temp
            }
            Expr::MemberAccess { object, member, .. } => {
                // If the object is a type (class/struct), it's a static access
                if let Expr::Ident(id, _name, generic_args, _) = &**object {
                    let is_type_name = self
                        .resolved_global_names
                        .get(id)
                        .is_some_and(|m| self.named_types.contains_key(m))
                        || self
                            .named_types
                            .keys()
                            .any(|k| k.ends_with(&format!("_{}", _name)) || k == _name)
                        || generic_args.is_some();
                    let mut ty_opt = self.global_env.get(id).cloned();
                    if ty_opt.is_none() {
                        ty_opt = self.local_types.get(id).cloned();
                    }
                    if let Some(ty) = ty_opt.filter(|_| is_type_name) {
                        if matches!(ty, Ty::Struct(_) | Ty::Class(_)) {
                            let mut type_name = "";
                            let nid = match ty {
                                Ty::Struct(i) => i,
                                Ty::Class(i) => i,
                                _ => unreachable!(),
                            };
                            for (name, &(tid, _)) in self.named_types {
                                if nid == tid {
                                    type_name = name;
                                    break;
                                }
                            }

                            let static_name = format!("{}_{}", type_name, member);

                            let mut field_ty = Ty::Int;
                            if let Some(sfty) = self.static_fields_env.get(&static_name) {
                                field_ty = sfty.clone();
                            } else if let Some(mty) = self.methods_env.get(&static_name) {
                                field_ty = mty.clone();
                            }

                            let temp = self.new_local(field_ty);
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(temp),
                                Rvalue::GlobalRead(static_name),
                            ));
                            return temp;
                        } else if let Ty::Enum(enum_id) = ty {
                            let temp = self.new_local(Ty::Enum(enum_id));
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(temp),
                                Rvalue::InstantiateEnum(enum_id, member.to_string(), vec![]),
                            ));
                            return temp;
                        }
                    }
                }

                let obj_local = self.build_expr(object);
                let obj_ty = self.locals[obj_local.0 as usize].clone();
                let field_ty = match obj_ty {
                    Ty::Struct(id) => {
                        let mut ft = Ty::Int;
                        if let Some(fields) = self.struct_defs.get(&id) {
                            for pace_ty::ResolvedField { name: n, ty: t, .. } in fields {
                                if n == member {
                                    ft = t.clone();
                                    break;
                                }
                            }
                        }
                        ft
                    }
                    Ty::Class(id) => {
                        let mut ft = Ty::Int;
                        if let Some(fields) = self.class_defs.get(&id) {
                            for pace_ty::ResolvedField { name: n, ty: t, .. } in fields {
                                if n == member {
                                    ft = t.clone();
                                    break;
                                }
                            }
                        }
                        ft
                    }
                    Ty::Enum(id) => {
                        let mut ft = Ty::Int;
                        if let Some(variants) = self.enum_defs.get(&id) {
                            for v in variants {
                                if v.name == *member {
                                    ft = Ty::Enum(id);
                                    break;
                                }
                            }
                        }
                        ft
                    }
                    _ => Ty::Int,
                };

                let temp = self.new_local(field_ty.clone());
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(temp),
                    Rvalue::FieldAccess(obj_local, member.to_string()),
                ));
                if matches!(field_ty, Ty::Class(_)) {
                    self.push_stmt(Statement::Retain(Lvalue::Local(temp))); // MVP: Retain classes when read from fields
                }
                temp
            }
            Expr::Call { callee, args, .. } => {
                let mut is_instantiation = false;
                let mut inst_ty = None;
                let mut struct_name = String::new();
                let mut is_global = false;
                let mut global_name = String::new();

                if let Expr::Ident(id, name, _, _) = &**callee {
                    if let Some(mangled) = self.resolved_global_names.get(id) {
                        if let Some(&(nid, kind)) = self.named_types.get(mangled) {
                            if kind == 0 || kind == 1 {
                                is_instantiation = true;
                                inst_ty = Some(if kind == 0 {
                                    Ty::Struct(nid)
                                } else {
                                    Ty::Class(nid)
                                });
                                struct_name = mangled.clone();
                            } else {
                                is_global = true;
                                global_name = mangled.clone();
                            }
                        } else if let Some(ty) = self.local_types.get(id) {
                            if let Ty::Struct(nid) | Ty::Class(nid) = ty {
                                is_instantiation = true;
                                inst_ty = Some(ty.clone());
                                for (n, &(tid, _)) in self.named_types {
                                    if *nid == tid {
                                        struct_name = n.clone();
                                        break;
                                    }
                                }
                            } else {
                                is_global = true;
                                global_name = mangled.clone();
                            }
                        } else {
                            is_global = true;
                            global_name = mangled.clone();
                        }
                    } else if let Some(name) = self.global_fns.get(&name.to_string()) {
                        is_global = true;
                        global_name = name.clone();
                    } else if let Some(ty) = self.global_env.get(id) {
                        if matches!(ty, Ty::Struct(_) | Ty::Class(_)) {
                            is_instantiation = true;
                            inst_ty = Some(ty.clone());
                            let nid = match ty {
                                Ty::Struct(i) => i,
                                Ty::Class(i) => i,
                                _ => unreachable!(),
                            };
                            for (name, &(tid, _)) in self.named_types {
                                if *nid == tid {
                                    struct_name = name.clone();
                                    break;
                                }
                            }
                        } else if let Ty::Function(_, ret) = ty
                            && let Ty::Enum(eid) = **ret
                        {
                            is_global = true;
                            if let Some(variants) = self.enum_defs.get(&eid) {
                                for v in variants {
                                    if v.id == *id {
                                        global_name = v.name.to_string();
                                    }
                                }
                            }
                        }
                    } else if let Some(&(nid, kind)) = self.named_types.get(&name.to_string()) {
                        if kind == 0 || kind == 1 {
                            is_instantiation = true;
                            inst_ty = Some(if kind == 0 {
                                Ty::Struct(nid)
                            } else {
                                Ty::Class(nid)
                            });
                            struct_name = name.to_string();
                        }
                    } else if let Some(expected) = &self.current_expected_ty
                        && let Ty::Struct(nid) | Ty::Class(nid) = expected
                    {
                        is_instantiation = true;
                        inst_ty = Some(expected.clone());
                        for (t_name, &(tid, _)) in self.named_types {
                            if *nid == tid {
                                struct_name = t_name.clone();
                                break;
                            }
                        }
                    }
                } else if let Expr::MemberAccess { object, member, .. } = &**callee {
                    let mut is_static_method = false;
                    if let Expr::Ident(id, name, generic_args, _) = &**object {
                        let is_type_name = self
                            .resolved_global_names
                            .get(id)
                            .is_some_and(|m| self.named_types.contains_key(m))
                            || generic_args.is_some();
                        if is_type_name {
                            is_static_method = true;
                            is_global = true;
                            if let Some(mangled) = self.resolved_global_names.get(id) {
                                global_name = format!("{}_{}", mangled, member);
                            } else {
                                global_name = format!("{}_{}", name, member);
                            }
                        } else if let Some(mangled) = self.resolved_global_names.get(id) {
                            // Check if it's a module alias mapping to a global function or method
                            let possible_global = format!("{}_{}", mangled, member);
                            if self.global_functions_env.contains_key(&possible_global)
                                || self.methods_env.contains_key(&possible_global)
                            {
                                is_static_method = true;
                                is_global = true;
                                global_name = possible_global;
                            } else {
                                // Might be a generic instantiation from a module alias
                                let mut found_type = false;
                                for tname in self.named_types.keys() {
                                    if tname == &possible_global {
                                        found_type = true;
                                        break;
                                    }
                                }
                                if found_type {
                                    is_static_method = true;
                                    is_global = true;
                                    global_name = possible_global;
                                }
                            }
                        }
                        let mut ty_opt = self.global_env.get(id).cloned();
                        if ty_opt.is_none() {
                            ty_opt = self.local_types.get(id).cloned();
                        }
                        if let Some(ty) = ty_opt.filter(|_| is_type_name) {
                            if matches!(ty, Ty::Struct(_) | Ty::Class(_)) {
                                let mut type_name = "";
                                let nid = match ty {
                                    Ty::Struct(i) => i,
                                    Ty::Class(i) => i,
                                    _ => unreachable!(),
                                };
                                for (name, &(tid, _)) in self.named_types {
                                    if nid == tid {
                                        type_name = name;
                                        break;
                                    }
                                }
                                let static_name = format!("{}_{}", type_name, member);
                                if self.methods_env.contains_key(&static_name) {
                                    is_global = true;
                                    global_name = static_name;
                                    is_static_method = true;
                                }
                            } else if let Ty::Enum(_) = ty {
                                is_global = true;
                                global_name = member.to_string();
                                is_static_method = true;
                            }
                        } else if let Some(&(nid, kind)) = self.named_types.get(&name.to_string()) {
                            if kind == 0 || kind == 1 {
                                let mut type_name = "";
                                for (name, &(tid, _)) in self.named_types {
                                    if nid == tid {
                                        type_name = name;
                                        break;
                                    }
                                }
                                let static_name = format!("{}_{}", type_name, member);
                                if self.methods_env.contains_key(&static_name) {
                                    is_global = true;
                                    global_name = static_name;
                                    is_static_method = true;
                                }
                            } else if kind == 2 {
                                is_global = true;
                                global_name = member.to_string();
                                is_static_method = true;
                            }
                        } else if let Some(expected) = &self.current_expected_ty {
                            if let Ty::Enum(_eid) = expected {
                                is_global = true;
                                global_name = member.to_string();
                                is_static_method = true;
                            } else if let Ty::Struct(nid) | Ty::Class(nid) = expected {
                                let mut type_name = "";
                                for (name, &(tid, _)) in self.named_types {
                                    if *nid == tid {
                                        type_name = name;
                                        break;
                                    }
                                }
                                let static_name = format!("{}_{}", type_name, member);
                                if self.methods_env.contains_key(&static_name) {
                                    is_global = true;
                                    global_name = static_name;
                                    is_static_method = true;
                                }
                            }
                        }
                    }

                    if !is_static_method {
                        // It's an instance method
                        let is_super = matches!(&**object, Expr::Super(_));
                        let obj_local = self.build_expr(object);
                        let obj_ty = self.locals[obj_local.0 as usize].clone();
                        if let Ty::Struct(id) | Ty::Class(id) = obj_ty {
                            let mut target_id = id;
                            if is_super && let Some(&parent_id) = self.class_parents.get(&id) {
                                target_id = parent_id;
                            }
                            let mut type_name = "";
                            for (name, &(tid, _)) in self.named_types {
                                if target_id == tid {
                                    type_name = name;
                                    break;
                                }
                            }
                            let method_name = format!("{}_{}", type_name, member);
                            if self.methods_env.contains_key(&method_name) {
                                global_name = method_name;

                                // Insert the object as the first argument (self)
                                let mut new_arg_locals = vec![obj_local];
                                for pace_hir::HirCallArg { expr: arg, .. } in args {
                                    new_arg_locals.push(self.build_expr(arg));
                                }

                                let mut ret_ty = Ty::Int;
                                if let Some(Ty::Function(_, ret)) =
                                    self.methods_env.get(&global_name)
                                {
                                    ret_ty = *ret.clone();
                                }

                                if matches!(obj_ty, Ty::Class(_))
                                    && !is_super
                                    && let Some(vtable) = self.class_vtables.get(&id)
                                    && let Some(vtable_idx) =
                                        vtable.iter().position(|v| &v.name == member)
                                {
                                    let temp = self.new_local(ret_ty);
                                    // The arguments are [arg1, arg2, ...] where arg1 is NOT `obj_local` in VirtualCall because VirtualCall takes `obj_local` separately.
                                    // Wait, `new_arg_locals` has `obj_local` as its first element.
                                    // `VirtualCall`'s `args` vector should NOT include `self`? Actually, it's easier to include `self` in `args` just like `GlobalCall`!
                                    // Let's pass `new_arg_locals` but skip the first element if we pass `obj_local` separately! Or just pass `new_arg_locals[1..].to_vec()`.
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::VirtualCall(
                                            vtable_idx,
                                            obj_local,
                                            new_arg_locals[1..].to_vec(),
                                        ),
                                    ));
                                    return temp;
                                }

                                let temp = self.new_local(ret_ty);
                                self.push_stmt(Statement::Assign(
                                    Lvalue::Local(temp),
                                    Rvalue::GlobalCall(global_name, new_arg_locals),
                                ));
                                return temp;
                            }
                        }
                    }
                }

                let mut arg_locals = Vec::new();
                for pace_hir::HirCallArg { expr: arg, .. } in args {
                    arg_locals.push(self.build_expr(arg));
                }

                if is_instantiation {
                    let ty = inst_ty.unwrap();
                    let init_name = format!("{}_init", struct_name);
                    let mut has_init = false;
                    for fn_name in self.global_fns.values() {
                        if *fn_name == init_name {
                            has_init = true;
                            break;
                        }
                    }

                    let temp = self.new_local(ty.clone());

                    if has_init {
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(temp),
                            Rvalue::Instantiate(ty.clone(), vec![]),
                        ));

                        let mut call_args = vec![temp];
                        call_args.extend(arg_locals);

                        let ret_dummy = self.new_local(Ty::Void);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(ret_dummy),
                            Rvalue::GlobalCall(init_name, call_args),
                        ));
                    } else {
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(temp),
                            Rvalue::Instantiate(ty, arg_locals),
                        ));
                    }
                    return temp;
                }

                if is_global {
                    let mut ret_ty = Ty::Int;
                    let mut is_enum_variant = false;
                    let mut enum_id = None;

                    if let Some(Ty::Function(_, ret)) = self.global_functions_env.get(&global_name)
                    {
                        if let Ty::Enum(eid) = **ret {
                            is_enum_variant = true;
                            enum_id = Some(eid);
                        } else {
                            ret_ty = *ret.clone();
                        }
                    } else if let Some(Ty::Function(_, ret)) = self.methods_env.get(&global_name) {
                        if let Ty::Enum(eid) = **ret {
                            is_enum_variant = true;
                            enum_id = Some(eid);
                        } else {
                            ret_ty = *ret.clone();
                        }
                    }

                    if let Expr::MemberAccess { object, .. } = &**callee
                        && let Expr::Ident(id, name, _, _) = &**object
                    {
                        if let Some(Ty::Enum(eid)) = self.global_env.get(id) {
                            is_enum_variant = true;
                            enum_id = Some(*eid);
                        } else if let Some(&(eid, 2)) = self.named_types.get(&name.to_string()) {
                            is_enum_variant = true;
                            enum_id = Some(eid);
                        }
                    }

                    if let Some(expected) = &self.current_expected_ty
                        && let Ty::Enum(eid) = expected
                    {
                        is_enum_variant = true;
                        enum_id = Some(*eid);
                    }

                    if is_enum_variant {
                        let temp = self.new_local(Ty::Enum(enum_id.unwrap()));
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(temp),
                            Rvalue::InstantiateEnum(
                                enum_id.unwrap(),
                                global_name.clone(),
                                arg_locals,
                            ),
                        ));
                        return temp;
                    }

                    let temp = self.new_local(ret_ty);
                    self.push_stmt(Statement::Assign(
                        Lvalue::Local(temp),
                        Rvalue::GlobalCall(global_name, arg_locals),
                    ));
                    return temp;
                }

                let callee_local = self.build_expr(callee);
                let temp = self.new_local(Ty::Int); // Placeholder MVP return ty
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(temp),
                    Rvalue::Call(callee_local, arg_locals),
                ));
                temp
            }
            Expr::BuiltinCall(name, args, _) => {
                let mut arg_locals = Vec::new();
                for arg in args {
                    arg_locals.push(self.build_expr(arg));
                }
                let temp = self.new_local(Ty::Int);
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(temp),
                    Rvalue::BuiltinCall(name.to_string(), arg_locals),
                ));
                temp
            }
            Expr::If {
                cond,
                then_block,
                else_block,
                ..
            } => {
                let cond_local = self.build_expr(cond);
                let then_bb = self.new_block();
                let else_bb = self.new_block();
                let merge_bb = self.new_block();

                let current_bb = self.current_block;
                self.blocks[current_bb.0 as usize].terminator = Some(Terminator::Branch {
                    cond: cond_local,
                    then_block: then_bb,
                    else_block: else_bb,
                });

                self.current_block = then_bb;
                self.build_block(then_block);
                if self.blocks[self.current_block.0 as usize]
                    .terminator
                    .is_none()
                {
                    self.blocks[self.current_block.0 as usize].terminator =
                        Some(Terminator::Goto(merge_bb));
                }

                self.current_block = else_bb;
                if let Some(eb) = else_block {
                    self.build_block(eb);
                }
                if self.blocks[self.current_block.0 as usize]
                    .terminator
                    .is_none()
                {
                    self.blocks[self.current_block.0 as usize].terminator =
                        Some(Terminator::Goto(merge_bb));
                }

                self.current_block = merge_bb;
                self.new_local(Ty::Int)
            }
            Expr::While { cond, body, .. } => {
                let cond_bb = self.new_block();
                let body_bb = self.new_block();
                let merge_bb = self.new_block();

                let current_bb = self.current_block;
                self.blocks[current_bb.0 as usize].terminator = Some(Terminator::Goto(cond_bb));

                self.current_block = cond_bb;
                let cond_local = self.build_expr(cond);
                self.blocks[self.current_block.0 as usize].terminator = Some(Terminator::Branch {
                    cond: cond_local,
                    then_block: body_bb,
                    else_block: merge_bb,
                });

                self.current_block = body_bb;
                self.build_block(body);
                if self.blocks[self.current_block.0 as usize]
                    .terminator
                    .is_none()
                {
                    self.blocks[self.current_block.0 as usize].terminator =
                        Some(Terminator::Goto(cond_bb));
                }

                self.current_block = merge_bb;
                self.new_local(Ty::Int)
            }
            Expr::Match { subject, arms, .. } => {
                let subject_local = self.build_expr(subject);
                let temp = self.new_local(Ty::Int); // MVP return Ty

                let tag_local = self.new_local(Ty::Int);
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(tag_local),
                    Rvalue::EnumTag(subject_local),
                ));

                let merge_bb = self.new_block();
                let mut current_bb = self.current_block;

                for arm in arms {
                    let next_arm_bb = self.new_block();
                    let body_bb = self.new_block();

                    let mut arm_tag = 0;
                    if let pace_hir::Pattern::Variant { name, .. } = &arm.pattern {
                        let subject_ty = self.locals[subject_local.0 as usize].clone();
                        if let Ty::Enum(enum_id) = subject_ty
                            && let Some(variants) = self.enum_defs.get(&enum_id)
                        {
                            for (i, v) in variants.iter().enumerate() {
                                if v.name == *name {
                                    arm_tag = i as i32;
                                    break;
                                }
                            }
                        }
                    } else if let pace_hir::Pattern::Ident(id, _, _) = &arm.pattern {
                        let subject_ty = self.locals[subject_local.0 as usize].clone();
                        let f_local = self.new_local(subject_ty);
                        self.hir_to_local.insert(*id, f_local);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(f_local),
                            Rvalue::Use(subject_local),
                        ));

                        self.blocks[current_bb.0 as usize].terminator =
                            Some(Terminator::Goto(body_bb));
                        self.current_block = body_bb;
                        let arm_local = self.build_expr(&arm.body);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(temp),
                            Rvalue::Use(arm_local),
                        ));
                        if self.blocks[self.current_block.0 as usize]
                            .terminator
                            .is_none()
                        {
                            self.blocks[self.current_block.0 as usize].terminator =
                                Some(Terminator::Goto(merge_bb));
                        }
                        current_bb = next_arm_bb;
                        self.current_block = next_arm_bb;
                        continue;
                    } else if let pace_hir::Pattern::CatchAll(_) = &arm.pattern {
                        self.blocks[current_bb.0 as usize].terminator =
                            Some(Terminator::Goto(body_bb));
                        self.current_block = body_bb;
                        let arm_local = self.build_expr(&arm.body);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(temp),
                            Rvalue::Use(arm_local),
                        ));
                        if self.blocks[self.current_block.0 as usize]
                            .terminator
                            .is_none()
                        {
                            self.blocks[self.current_block.0 as usize].terminator =
                                Some(Terminator::Goto(merge_bb));
                        }
                        current_bb = next_arm_bb;
                        self.current_block = next_arm_bb;
                        continue;
                    }

                    let cond_local = self.new_local(Ty::Int);
                    let tag_const_local = self.new_local(Ty::Int);
                    self.push_stmt(Statement::Assign(
                        Lvalue::Local(tag_const_local),
                        Rvalue::Constant(Constant::Int(arm_tag.to_string())),
                    ));
                    self.push_stmt(Statement::Assign(
                        Lvalue::Local(cond_local),
                        Rvalue::BinaryOp(pace_ast::BinaryOp::EqEq, tag_local, tag_const_local),
                    ));

                    self.blocks[current_bb.0 as usize].terminator = Some(Terminator::Branch {
                        cond: cond_local,
                        then_block: body_bb,
                        else_block: next_arm_bb,
                    });

                    self.current_block = body_bb;

                    if let pace_hir::Pattern::Variant { name, fields, .. } = &arm.pattern
                        && let Some(pfields) = fields
                    {
                        let subject_ty = self.locals[subject_local.0 as usize].clone();
                        if let Ty::Enum(enum_id) = subject_ty
                            && let Some(variants) = self.enum_defs.get(&enum_id)
                            && let Some(v) = variants.iter().find(|v| v.name == *name)
                            && let Some(vfields) = &v.fields
                        {
                            for (i, (pf_id, _, _)) in pfields.iter().enumerate() {
                                if i < vfields.len() {
                                    let fty =
                                        self.local_types.get(pf_id).unwrap_or(&Ty::Int).clone();
                                    let f_local = self.new_local(fty);
                                    self.hir_to_local.insert(*pf_id, f_local);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(f_local),
                                        Rvalue::EnumFieldAccess(
                                            subject_local,
                                            name.to_string(),
                                            vfields[i].0.to_string(),
                                        ),
                                    ));
                                }
                            }
                        }
                    }

                    let arm_local = self.build_expr(&arm.body);
                    self.push_stmt(Statement::Assign(
                        Lvalue::Local(temp),
                        Rvalue::Use(arm_local),
                    ));

                    if self.blocks[self.current_block.0 as usize]
                        .terminator
                        .is_none()
                    {
                        self.blocks[self.current_block.0 as usize].terminator =
                            Some(Terminator::Goto(merge_bb));
                    }

                    current_bb = next_arm_bb;
                    self.current_block = next_arm_bb;
                }

                if self.blocks[current_bb.0 as usize].terminator.is_none() {
                    self.blocks[current_bb.0 as usize].terminator =
                        Some(Terminator::Goto(merge_bb));
                }

                self.current_block = merge_bb;
                temp
            }
            Expr::Assign { target, value, .. } => {
                let mut is_static_assign = false;
                let mut static_name = String::new();
                let lvalue = match &**target {
                    Expr::Ident(id, _name, _, _) => {
                        Lvalue::Local(*self.hir_to_local.get(id).unwrap())
                    }
                    Expr::MemberAccess { object, member, .. } => {
                        if let Expr::Ident(id, name, generic_args, _) = &**object {
                            let is_type_name = self
                                .resolved_global_names
                                .get(id)
                                .is_some_and(|m| self.named_types.contains_key(m))
                                || self
                                    .named_types
                                    .keys()
                                    .any(|k| k.ends_with(&format!("_{}", name)) || k == name)
                                || generic_args.is_some();
                            if is_type_name {
                                let mut ty_opt = self.global_env.get(id).cloned();
                                if ty_opt.is_none() {
                                    ty_opt = self.local_types.get(id).cloned();
                                }
                                if let Some(ty) = ty_opt
                                    && matches!(ty, Ty::Struct(_) | Ty::Class(_))
                                {
                                    let mut type_name = "";
                                    let nid = match ty {
                                        Ty::Struct(i) => i,
                                        Ty::Class(i) => i,
                                        _ => unreachable!(),
                                    };
                                    for (n, &(tid, _)) in self.named_types {
                                        if nid == tid {
                                            type_name = n;
                                            break;
                                        }
                                    }
                                    static_name = format!("{}_{}", type_name, member);
                                    is_static_assign = true;
                                }
                            }
                        }

                        if is_static_assign {
                            // Dummy Lvalue, not used for static assign
                            let temp = self.new_local(Ty::Int);
                            Lvalue::Local(temp)
                        } else {
                            let obj_local = self.build_expr(object);
                            Lvalue::FieldAccess(obj_local, member.to_string())
                        }
                    }
                    _ => panic!("Invalid assignment target"),
                };
                if is_static_assign {
                    let prev = self.current_expected_ty.take();
                    // We could lookup the exact static field type, but we can default to Int for expected
                    self.current_expected_ty = Some(Ty::Int);
                    let rval = self.build_expr(value);
                    self.current_expected_ty = prev;
                    self.push_stmt(Statement::GlobalWrite(static_name, rval));
                    return rval;
                }

                let lval_ty = match &lvalue {
                    Lvalue::Local(l) => self.locals[l.0 as usize].clone(),
                    Lvalue::FieldAccess(obj, member) => {
                        let mut field_ty = Ty::Int;
                        let obj_ty = self.locals[obj.0 as usize].clone();
                        let mut found = false;
                        if let Ty::Struct(id) | Ty::Class(id) = obj_ty {
                            let defs = if matches!(obj_ty, Ty::Struct(_)) {
                                self.struct_defs.get(&id)
                            } else {
                                self.class_defs.get(&id)
                            };
                            if let Some(fields) = defs {
                                for pace_ty::ResolvedField { name: n, ty: t, .. } in fields {
                                    if n == member {
                                        field_ty = t.clone();
                                        found = true;
                                        break;
                                    }
                                }
                            }
                        }
                        if !found {
                            panic!("Field not found");
                        }
                        field_ty
                    }
                    Lvalue::EnumFieldAccess(_, _, _) => panic!("Cannot assign to enum field"),
                };
                let prev = self.current_expected_ty.take();
                self.current_expected_ty = Some(lval_ty.clone());
                let rval = self.build_expr(value);
                self.current_expected_ty = prev;
                if matches!(lval_ty, Ty::Class(_)) {
                    self.push_stmt(Statement::Release(lvalue.clone()));
                }
                self.push_stmt(Statement::Assign(lvalue.clone(), Rvalue::Use(rval)));
                if matches!(lval_ty, Ty::Class(_)) {
                    self.push_stmt(Statement::Retain(lvalue));
                }
                rval
            }
        }
    }

    pub fn build_program(program: &pace_hir::Program, tc: &mut pace_ty::TypeChecker) -> MirProgram {
        let mut declarations: Vec<pace_hir::Decl> = Vec::new();
        for module in program.modules.values() {
            for decl in &module.declarations {
                declarations.push(decl.clone());
            }
        }

        let mut global_fns = HashMap::new();
        for decl in declarations.iter().chain(tc.instantiated_generics.iter()) {
            if let pace_hir::Decl::Function { name, .. } = decl {
                global_fns.insert(name.to_string(), name.to_string());
            } else if let pace_hir::Decl::Struct { methods, .. } = decl {
                for method in methods {
                    if let pace_hir::Decl::Function { name, .. } = method {
                        global_fns.insert(name.to_string(), name.to_string());
                    }
                }
            } else if let pace_hir::Decl::Class { methods, .. } = decl {
                for method in methods {
                    if let pace_hir::Decl::Function { name, .. } = method {
                        global_fns.insert(name.to_string(), name.to_string());
                    }
                }
            }
        }

        let mut functions = Vec::new();
        let mut global_vars = Vec::new();
        let mut main_builder = MirBuilder::new(
            global_fns.clone(),
            &tc.struct_defs,
            &tc.class_defs,
            &tc.class_vtables,
            &tc.enum_defs,
            &tc.env,
            &tc.local_types,
            &tc.named_types,
            &tc.static_fields_env,
            &tc.methods_env,
            &tc.class_parents,
            &tc.global_functions,
            &tc.resolved_global_names,
        );
        let mut _main_last_local = Local(0);

        for decl in declarations.iter().chain(tc.instantiated_generics.iter()) {
            match decl {
                pace_hir::Decl::Let { id, value, .. } | pace_hir::Decl::Var { id, value, .. } => {
                    if let Some(val) = value {
                        let rval_local = main_builder.build_expr(val);
                        let var_ty = main_builder.locals[rval_local.0 as usize].clone();
                        let var_local = main_builder.new_local(var_ty.clone());
                        main_builder.hir_to_local.insert(*id, var_local);
                        main_builder.push_stmt(Statement::Assign(
                            Lvalue::Local(var_local),
                            Rvalue::Use(rval_local),
                        ));
                        if matches!(var_ty, Ty::Class(_)) {
                            main_builder.push_stmt(Statement::Retain(Lvalue::Local(var_local)));
                        }
                        _main_last_local = rval_local;
                    } else {
                        // Uninitialized global variable
                        let var_ty = main_builder
                            .local_types
                            .get(id)
                            .expect("Type not found in local_types")
                            .clone();
                        let var_local = main_builder.new_local(var_ty.clone());
                        main_builder.hir_to_local.insert(*id, var_local);
                        // No assignment, C handles initialization
                    }
                }
                pace_hir::Decl::Const {
                    id, name, value, ..
                } => {
                    let global_name = format!("{}", name);
                    let ty = tc.env.get(id).cloned().unwrap_or(Ty::Int);
                    global_vars.push((global_name.clone(), ty.clone()));
                    let rval_local = main_builder.build_expr(value);
                    main_builder.push_stmt(Statement::GlobalWrite(global_name, rval_local));
                }
                pace_hir::Decl::Struct {
                    id,
                    name,
                    static_fields,
                    const_fields,
                    methods,
                    generic_params,
                    ..
                }
                | pace_hir::Decl::Class {
                    id,
                    name,
                    methods,
                    static_fields,
                    const_fields,
                    generic_params,
                    ..
                } => {
                    let mut mangled_name = name.to_string();
                    for (k, &(tid, _)) in &tc.named_types {
                        if tid == *id {
                            mangled_name = k.clone();
                            break;
                        }
                    }
                    if generic_params.is_some() {
                        continue;
                    }

                    for pace_hir::HirStaticFieldDef {
                        name: sf_name,
                        ty: sf_ty,
                        value: sf_expr,
                        ..
                    } in static_fields
                    {
                        let global_name = format!("{}_{}", mangled_name, sf_name);
                        let ty = tc_get_type(tc, sf_ty).unwrap_or(Ty::Int);
                        global_vars.push((global_name.clone(), ty.clone()));

                        let rval_local = main_builder.build_expr(sf_expr);
                        main_builder.push_stmt(Statement::GlobalWrite(global_name, rval_local));
                    }
                    for pace_hir::HirConstFieldDef {
                        name: cf_name,
                        ty: cf_ty,
                        value: cf_expr,
                        ..
                    } in const_fields
                    {
                        let global_name = format!("{}_{}", mangled_name, cf_name);
                        let ty = tc_get_type(tc, cf_ty).unwrap_or(Ty::Int);
                        global_vars.push((global_name.clone(), ty.clone()));

                        let rval_local = main_builder.build_expr(cf_expr);
                        main_builder.push_stmt(Statement::GlobalWrite(global_name, rval_local));
                    }
                    for method in methods {
                        if let pace_hir::Decl::Function {
                            name,
                            params,
                            return_type,
                            body,
                            ..
                        } = method
                        {
                            let mut fn_builder = MirBuilder::new(
                                global_fns.clone(),
                                &tc.struct_defs,
                                &tc.class_defs,
                                &tc.class_vtables,
                                &tc.enum_defs,
                                &tc.env,
                                &tc.local_types,
                                &tc.named_types,
                                &tc.static_fields_env,
                                &tc.methods_env,
                                &tc.class_parents,
                                &tc.global_functions,
                                &tc.resolved_global_names,
                            );
                            let mut mir_params = Vec::new();
                            for (param_id, param_name, pty) in params {
                                let ty = tc_get_type(tc, pty).unwrap_or(Ty::Int);
                                let local = fn_builder.new_local(ty);
                                if param_name == "self" {
                                    fn_builder.current_self_local = Some(local);
                                }
                                fn_builder.hir_to_local.insert(*param_id, local);
                                mir_params.push(local);
                            }

                            let ret_ty = if let Some(rty) = return_type {
                                tc_get_type(tc, rty).unwrap_or(Ty::Void)
                            } else {
                                Ty::Void
                            };
                            fn_builder.build_block(body);
                            let extracted_closures = std::mem::take(&mut fn_builder.closure_functions);
                            let fn_body = fn_builder.finish(&mir_params);

                            functions.push(MirFunction {
                                name: get_mangled_name(tc, name),
                                params: mir_params,
                                return_type: ret_ty,
                                body: fn_body,
                            });
                            functions.extend(extracted_closures);
                        }
                    }
                }
                pace_hir::Decl::Expr(expr, _) => {
                    let rval_local = main_builder.build_expr(expr);
                    _main_last_local = rval_local;
                }
                pace_hir::Decl::Function {
                    name,
                    id: func_id,
                    params,
                    return_type,
                    body,
                    generic_params,
                    ..
                } => {
                    if generic_params.is_some() {
                        continue;
                    }
                    let mut fn_builder = MirBuilder::new(
                        global_fns.clone(),
                        &tc.struct_defs,
                        &tc.class_defs,
                        &tc.class_vtables,
                        &tc.enum_defs,
                        &tc.env,
                        &tc.local_types,
                        &tc.named_types,
                        &tc.static_fields_env,
                        &tc.methods_env,
                        &tc.class_parents,
                        &tc.global_functions,
                        &tc.resolved_global_names,
                    );
                    let mut mir_params = Vec::new();
                    for (param_id, param_name, pty) in params {
                        let ty = tc_get_type(tc, pty).unwrap_or(Ty::Int);
                        let local = fn_builder.new_local(ty);
                        if param_name == "self" {
                            fn_builder.current_self_local = Some(local);
                        }
                        fn_builder.hir_to_local.insert(*param_id, local);
                        mir_params.push(local);
                    }

                    let ret_ty = if let Some(rty) = return_type {
                        tc_get_type(tc, rty).unwrap_or(Ty::Void)
                    } else {
                        Ty::Void
                    };
                    fn_builder.build_block(body);
                    let extracted_closures = std::mem::take(&mut fn_builder.closure_functions);
                    let fn_body = fn_builder.finish(&mir_params);
                    functions.push(MirFunction {
                        name: tc
                            .resolved_global_names
                            .get(func_id)
                            .cloned()
                            .unwrap_or_else(|| get_mangled_name(tc, name)),
                        params: mir_params,
                        return_type: ret_ty,
                        body: fn_body,
                    });
                    functions.extend(extracted_closures);
                }
                pace_hir::Decl::Enum { .. } => {}
                pace_hir::Decl::Trait { .. } => {}
                pace_hir::Decl::Import { .. } => {}
            }
        }
        
        let extracted_closures = std::mem::take(&mut main_builder.closure_functions);
        functions.extend(extracted_closures);

        let mut resolved_enum_defs = std::collections::HashMap::new();
        for (id, variants) in &tc.enum_defs {
            let mut res_variants = Vec::new();
            for v in variants {
                let res_fields = v.fields.as_ref().map(|fields| {
                    fields
                        .iter()
                        .map(|(name, ty)| {
                            (name.to_string(), tc_get_type(tc, ty).unwrap_or(Ty::Int))
                        })
                        .collect()
                });
                res_variants.push((v.name.to_string(), res_fields));
            }
            resolved_enum_defs.insert(*id, res_variants);
        }

        MirProgram {
            functions,
            main_body: main_builder.finish(&[]),
            struct_defs: tc.struct_defs.clone(),
            class_defs: tc.class_defs.clone(),
            class_vtables: tc.class_vtables.clone(),
            enum_defs: resolved_enum_defs,
            global_vars,
        }
    }
}

impl<'a> MirBuilder<'a> {
    pub fn finish(mut self, params: &[Local]) -> MirBody {
        let current_bb = self.current_block.0 as usize;

        if self.blocks[current_bb].terminator.is_none() {
            let temp = self.new_local(Ty::Int);
            self.blocks[current_bb].statements.push(Statement::Assign(
                Lvalue::Local(temp),
                Rvalue::Constant(Constant::Int("0".to_string())),
            ));
            self.blocks[current_bb].terminator = Some(Terminator::Return(temp));
        }

        let mut return_locals = Vec::new();
        for block in &self.blocks {
            if let Some(Terminator::Return(r)) = &block.terminator {
                return_locals.push(*r);
            }
        }

        // ARC: Release all Class locals at the end, except return_local and params
        for (i, ty) in self.locals.iter().enumerate() {
            let local = Local(i as u32);
            if matches!(ty, Ty::Class(_))
                && !return_locals.contains(&local)
                && !params.contains(&local)
            {
                self.blocks[current_bb]
                    .statements
                    .push(Statement::Release(Lvalue::Local(local)));
            }
        }

        MirBody {
            blocks: self.blocks,
            locals: self.locals,
        }
    }
}
