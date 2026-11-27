use crate::mir::*;
use pace_hir::{Expr, HirId};
use pace_ty::{Ty, TypeChecker};
use std::collections::HashMap;

fn tc_get_type(tc: &TypeChecker, ty: &pace_ast::Type) -> Option<Ty> {
    if let Ok(t) = tc.get_type(ty) {
        return Some(t);
    }
    match ty {
        pace_ast::Type::Named(id) => {
            let suffix = format!("_{}", id.name);
            for (mangled, &(hir_id, kind)) in &tc.named_types {
                if mangled == id.name || mangled.ends_with(&suffix) {
                    return Some(match kind {
                        1 => Ty::Class(hir_id),
                        0 => Ty::Struct(hir_id),
                        3 => Ty::Actor(hir_id),
                        _ => Ty::Enum(hir_id),
                    });
                }
            }
            None
        }
        pace_ast::Type::Optional(inner, _) => {
            Some(Ty::Optional(Box::new(tc_get_type(tc, inner)?)))
        }
        pace_ast::Type::Generic(base, args, _) => {
            if let pace_ast::Type::Named(id) = base.as_ref() {
                match (id.name.as_str(), args.as_slice()) {
                    ("Option", [a]) => Some(Ty::Optional(Box::new(tc_get_type(tc, a)?))),
                    ("Result", [ok, err]) => Some(Ty::Result(
                        Box::new(tc_get_type(tc, ok)?),
                        Box::new(tc_get_type(tc, err)?),
                    )),
                    ("List", [elem]) => Some(Ty::List(Box::new(tc_get_type(tc, elem)?))),
                    ("Map", [k, v]) => Some(Ty::Map(
                        Box::new(tc_get_type(tc, k)?),
                        Box::new(tc_get_type(tc, v)?),
                    )),
                    ("Set", [elem]) => Some(Ty::Set(Box::new(tc_get_type(tc, elem)?))),
                    _ => None,
                }
            } else {
                None
            }
        }
        pace_ast::Type::Closure(params, ret, _) => {
            let p_tys: Option<Vec<Ty>> = params.iter().map(|p| tc_get_type(tc, p)).collect();
            let r_ty = tc_get_type(tc, ret)?;
            Some(Ty::Function(p_tys?, Box::new(r_ty)))
        }
    }
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
    pub loop_stack: Vec<(BasicBlockId, BasicBlockId)>,
    pub scope_stack: Vec<Vec<Local>>,
    pub scope_managed_locals: std::collections::HashSet<Local>,
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
            loop_stack: Vec::new(),
            scope_stack: Vec::new(),
            scope_managed_locals: std::collections::HashSet::new(),
        }
    }

    pub fn new_local(&mut self, ty: Ty) -> Local {
        let l = Local(self.locals.len() as u32);
        let is_rc = matches!(ty, Ty::Class(_) | Ty::List(_) | Ty::Map(_, _) | Ty::Set(_) | Ty::StringBuilder);
        self.locals.push(ty);
        if is_rc && self.scope_stack.len() > 1 {
            self.scope_stack.last_mut().unwrap().push(l);
            self.scope_managed_locals.insert(l);
        }
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
        self.scope_stack.push(Vec::new());
        let is_inner_scope = self.scope_stack.len() > 1;
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
                        if matches!(var_ty, Ty::Class(_) | Ty::List(_) | Ty::Map(_, _) | Ty::Set(_) | Ty::StringBuilder) {
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

                    // Release inner-scope locals before early return
                    let to_release: Vec<Local> = self
                        .scope_stack
                        .iter()
                        .skip(1)
                        .rev()
                        .flat_map(|s| s.iter().rev().copied())
                        .filter(|&l| l != local)
                        .collect();
                    for l in to_release {
                        self.push_stmt(Statement::Release(Lvalue::Local(l)));
                    }

                    let current_bb = self.current_block.0 as usize;
                    self.blocks[current_bb].terminator = Some(Terminator::Return(local));
                    break;
                }
                pace_hir::Stmt::Break(_) => {
                    if let Some(&(_, merge_bb)) = self.loop_stack.last() {
                        let current_bb = self.current_block.0 as usize;
                        self.blocks[current_bb].terminator = Some(Terminator::Goto(merge_bb));
                        self.current_block = self.new_block();
                        break;
                    }
                }
                pace_hir::Stmt::Continue(_) => {
                    if let Some(&(cond_bb, _)) = self.loop_stack.last() {
                        let current_bb = self.current_block.0 as usize;
                        self.blocks[current_bb].terminator = Some(Terminator::Goto(cond_bb));
                        self.current_block = self.new_block();
                        break;
                    }
                }
            }
        }

        let scope_locals = self.scope_stack.pop().unwrap();
        if is_inner_scope
            && self.blocks[self.current_block.0 as usize]
                .terminator
                .is_none()
        {
            for l in scope_locals.into_iter().rev() {
                self.push_stmt(Statement::Release(Lvalue::Local(l)));
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
                self.current_self_local
                    .expect("Super used outside of a method")
            }
            Expr::Closure { params, return_type: _, body, captured_vars, span: _ } => {
                static CLOSURE_COUNTER: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let cid = CLOSURE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                let closure_name = format!("__closure_{}", cid);
                
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
                
                let mut captured_mir_locals = Vec::new();
                let mut env_layout = Vec::new();
                for cap_id in captured_vars {
                    if let Some(local) = self.hir_to_local.get(cap_id) {
                        captured_mir_locals.push(*local);
                        let ty = self.locals[local.0 as usize].clone();
                        let fn_local = fn_builder.new_local(ty.clone());
                        fn_builder.hir_to_local.insert(*cap_id, fn_local);
                        env_layout.push((fn_local, ty));
                    }
                }
                
                let ret_ty = match body {
                    pace_hir::ClosureBody::Expr(e) => {
                        let body_local = fn_builder.build_expr(e);
                        let ret_ty = fn_builder.locals[body_local.0 as usize].clone();
                        let ret_local = fn_builder.new_local(ret_ty.clone());
                        fn_builder.push_stmt(Statement::Assign(Lvalue::Local(ret_local), Rvalue::Use(body_local)));
                        if fn_builder.blocks[fn_builder.current_block.0 as usize].terminator.is_none() {
                            fn_builder.blocks[fn_builder.current_block.0 as usize].terminator = Some(Terminator::Return(ret_local));
                        }
                        ret_ty
                    }
                    pace_hir::ClosureBody::Block(b) => {
                        fn_builder.build_block(b);
                        let mut r_ty = Ty::Void;
                        for block in &fn_builder.blocks {
                            if let Some(Terminator::Return(local)) = &block.terminator {
                                r_ty = fn_builder.locals[local.0 as usize].clone();
                                break;
                            }
                        }
                        r_ty
                    }
                };
                
                let mut non_released = mir_params.clone();
                non_released.extend(env_layout.iter().map(|(l, _)| *l));
                let nested_closures = std::mem::take(&mut fn_builder.closure_functions);
                let fn_body = fn_builder.finish(&non_released, &ret_ty);

                self.closure_functions.extend(nested_closures);
                self.closure_functions.push(MirFunction {
                    name: closure_name.clone(),
                    params: mir_params,
                    return_type: ret_ty.clone(),
                    is_async: false,
                    env_layout: Some(env_layout),
                    body: fn_body,
                });
                
                let closure_ty = Ty::Closure(param_tys, Box::new(ret_ty));
                let temp = self.new_local(closure_ty);
                
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(temp),
                    Rvalue::MakeClosure(closure_name, captured_mir_locals),
                ));
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

                if let Some(l) = self.hir_to_local.get(id) {
                    *l
                } else if let Some(ty) = self.global_env.get(id) {
                    let g_name = self
                        .resolved_global_names
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| name.to_string());
                    let temp = self.new_local(ty.clone());
                    self.push_stmt(Statement::Assign(
                        Lvalue::Local(temp),
                        Rvalue::GlobalRead(g_name),
                    ));
                    temp
                } else {
                    eprintln!("ICE: Local not found for id {:?} name {}", id, name);
                    Local(0)
                }
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
            Expr::Unary { op, expr, .. } => {
                let inner = self.build_expr(expr);
                let ret_ty = match op {
                    pace_ast::UnaryOp::Neg => self.locals[inner.0 as usize].clone(),
                    pace_ast::UnaryOp::Not => Ty::Bool,
                };
                let temp = self.new_local(ret_ty);
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(temp),
                    Rvalue::UnaryOp(*op, inner),
                ));
                temp
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                if *op == pace_ast::BinaryOp::And || *op == pace_ast::BinaryOp::Or {
                    let is_and = *op == pace_ast::BinaryOp::And;
                    let lhs = self.build_expr(left);
                    let res_local = self.new_local(Ty::Bool);
                    let rhs_bb = self.new_block();
                    let short_bb = self.new_block();
                    let merge_bb = self.new_block();

                    let (true_bb, false_bb) = if is_and {
                        (rhs_bb, short_bb)
                    } else {
                        (short_bb, rhs_bb)
                    };
                    self.blocks[self.current_block.0 as usize].terminator =
                        Some(Terminator::Branch {
                            cond: lhs,
                            then_block: true_bb,
                            else_block: false_bb,
                        });

                    self.current_block = short_bb;
                    self.push_stmt(Statement::Assign(
                        Lvalue::Local(res_local),
                        Rvalue::Constant(Constant::Bool(!is_and)),
                    ));
                    self.blocks[self.current_block.0 as usize].terminator =
                        Some(Terminator::Goto(merge_bb));

                    self.current_block = rhs_bb;
                    let rhs = self.build_expr(right);
                    self.push_stmt(Statement::Assign(
                        Lvalue::Local(res_local),
                        Rvalue::Use(rhs),
                    ));
                    if self.blocks[self.current_block.0 as usize]
                        .terminator
                        .is_none()
                    {
                        self.blocks[self.current_block.0 as usize].terminator =
                            Some(Terminator::Goto(merge_bb));
                    }

                    self.current_block = merge_bb;
                    return res_local;
                }

                let lhs = self.build_expr(left);
                let rhs = self.build_expr(right);

                let ret_ty = if *op == pace_ast::BinaryOp::NullCoalesce {
                    self.locals[rhs.0 as usize].clone()
                } else if matches!(
                    op,
                    pace_ast::BinaryOp::Add
                        | pace_ast::BinaryOp::Sub
                        | pace_ast::BinaryOp::Mul
                        | pace_ast::BinaryOp::Div
                        | pace_ast::BinaryOp::Mod
                ) {
                    self.locals[lhs.0 as usize].clone()
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

                let inner_ty = if let Ty::Optional(inner) = obj_ty {
                    *inner
                } else {
                    obj_ty
                };

                let mut field_ty = Ty::Optional(Box::new(Ty::Void));
                match inner_ty {
                    Ty::Struct(id) => {
                        if let Some(fields) = self.struct_defs.get(&id) {
                            for pace_ty::ResolvedField { name: n, ty: t, .. } in fields {
                                if n == member {
                                    field_ty = Ty::Optional(Box::new(t.clone()));
                                    break;
                                }
                            }
                        }
                    }
                    Ty::Class(id) | Ty::Actor(id) => {
                        if let Some(fields) = self.class_defs.get(&id) {
                            for pace_ty::ResolvedField { name: n, ty: t, .. } in fields {
                                if n == member {
                                    field_ty = Ty::Optional(Box::new(t.clone()));
                                    break;
                                }
                            }
                        }
                    }
                    _ => {}
                }

                let temp = self.new_local(field_ty);
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
                let len_fn = match &obj_ty {
                    Ty::List(_) => Some("pace_list_len"),
                    Ty::String => Some("pace_string_len"),
                    Ty::StringBuilder => Some("pace_sb_len"),
                    Ty::Map(_, _) => Some("pace_map_len"),
                    Ty::Set(_) => Some("pace_set_len"),
                    _ => None,
                };
                if let Some(fn_name) = len_fn {
                    if member == "len" {
                        let len_local = self.new_local(Ty::Int);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(len_local),
                            Rvalue::BuiltinCall(fn_name.to_string(), vec![obj_local]),
                        ));
                        return len_local;
                    }
                    if member == "isEmpty" || member == "isNotEmpty" {
                        let len_local = self.new_local(Ty::Int);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(len_local),
                            Rvalue::BuiltinCall(fn_name.to_string(), vec![obj_local]),
                        ));
                        let zero = self.new_local(Ty::Int);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(zero),
                            Rvalue::Constant(Constant::Int("0".to_string())),
                        ));
                        let temp = self.new_local(Ty::Bool);
                        let op = if member == "isEmpty" {
                            pace_ast::BinaryOp::EqEq
                        } else {
                            pace_ast::BinaryOp::NotEq
                        };
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(temp),
                            Rvalue::BinaryOp(op, len_local, zero),
                        ));
                        return temp;
                    }
                }
                if let Ty::List(elem_ty) = &obj_ty {
                    if member == "first" || member == "last" {
                            let len_local = self.new_local(Ty::Int);
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(len_local),
                                Rvalue::BuiltinCall("pace_list_len".to_string(), vec![obj_local]),
                            ));
                            let zero = self.new_local(Ty::Int);
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(zero),
                                Rvalue::Constant(Constant::Int("0".to_string())),
                            ));
                            let cond = self.new_local(Ty::Bool);
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(cond),
                                Rvalue::BinaryOp(pace_ast::BinaryOp::Gt, len_local, zero),
                            ));
                            let res = self.new_local(Ty::Optional(elem_ty.clone()));
                            let then_bb = self.new_block();
                            let else_bb = self.new_block();
                            let merge_bb = self.new_block();
                            self.blocks[self.current_block.0 as usize].terminator =
                                Some(Terminator::Branch {
                                    cond,
                                    then_block: then_bb,
                                    else_block: else_bb,
                                });
                            self.current_block = then_bb;
                            let idx = if member == "first" {
                                zero
                            } else {
                                let one = self.new_local(Ty::Int);
                                self.push_stmt(Statement::Assign(
                                    Lvalue::Local(one),
                                    Rvalue::Constant(Constant::Int("1".to_string())),
                                ));
                                let last_idx = self.new_local(Ty::Int);
                                self.push_stmt(Statement::Assign(
                                    Lvalue::Local(last_idx),
                                    Rvalue::BinaryOp(pace_ast::BinaryOp::Sub, len_local, one),
                                ));
                                last_idx
                            };
                            let val = self.new_local(*elem_ty.clone());
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(val),
                                Rvalue::ListGet(obj_local, idx),
                            ));
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(res),
                                Rvalue::Use(val),
                            ));
                            self.blocks[self.current_block.0 as usize].terminator =
                                Some(Terminator::Goto(merge_bb));
                            self.current_block = else_bb;
                            let null_local = self.new_local(Ty::Optional(Box::new(Ty::Void)));
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(null_local),
                                Rvalue::Constant(Constant::Int("0".to_string())),
                            ));
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(res),
                                Rvalue::Use(null_local),
                            ));
                            self.blocks[self.current_block.0 as usize].terminator =
                                Some(Terminator::Goto(merge_bb));
                            self.current_block = merge_bb;
                            return res;
                    }
                }
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
                    Ty::Class(id) | Ty::Actor(id) => {
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
                    if name == "List" {
                        let elem_ty = if let Some(ty) = self.local_types.get(id) {
                            if let Ty::List(inner) = ty {
                                *inner.clone()
                            } else {
                                Ty::Int
                            }
                        } else if let Some(Ty::List(inner)) = &self.current_expected_ty {
                            *inner.clone()
                        } else {
                            Ty::Int
                        };
                        let cap_local = if !args.is_empty() {
                            self.build_expr(&args[0].expr)
                        } else {
                            let cap = self.new_local(Ty::Int);
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(cap),
                                Rvalue::Constant(Constant::Int("8".to_string())),
                            ));
                            cap
                        };
                        let list_ty = Ty::List(Box::new(elem_ty.clone()));
                        let temp = self.new_local(list_ty);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(temp),
                            Rvalue::ListNew(elem_ty, cap_local),
                        ));
                        return temp;
                    } else if name == "Map" {
                        let (k_ty, v_ty) = if let Some(ty) = self.local_types.get(id) {
                            if let Ty::Map(k, v) = ty {
                                (*k.clone(), *v.clone())
                            } else {
                                (Ty::String, Ty::Int)
                            }
                        } else if let Some(Ty::Map(k, v)) = &self.current_expected_ty {
                            (*k.clone(), *v.clone())
                        } else {
                            (Ty::String, Ty::Int)
                        };
                        let map_ty = Ty::Map(Box::new(k_ty.clone()), Box::new(v_ty.clone()));
                        let temp = self.new_local(map_ty);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(temp),
                            Rvalue::MapNew(k_ty, v_ty),
                        ));
                        return temp;
                    } else if name == "Set" {
                        let elem_ty = if let Some(ty) = self.local_types.get(id) {
                            if let Ty::Set(inner) = ty {
                                *inner.clone()
                            } else {
                                Ty::Int
                            }
                        } else if let Some(Ty::Set(inner)) = &self.current_expected_ty {
                            *inner.clone()
                        } else {
                            Ty::Int
                        };
                        let set_ty = Ty::Set(Box::new(elem_ty.clone()));
                        let temp = self.new_local(set_ty);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(temp),
                            Rvalue::SetNew(elem_ty),
                        ));
                        return temp;
                    } else if name == "StringBuilder" {
                        let temp = self.new_local(Ty::StringBuilder);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(temp),
                            Rvalue::BuiltinCall("pace_sb_new".to_string(), vec![]),
                        ));
                        return temp;
                    }

                    if let Some(mangled) = self.resolved_global_names.get(id) {
                        if let Some(&(nid, kind)) = self.named_types.get(mangled) {
                            if kind == 0 || kind == 1 || kind == 3 {
                                is_instantiation = true;
                                inst_ty = Some(if kind == 0 {
                                    Ty::Struct(nid)
                                } else if kind == 1 {
                                    Ty::Class(nid)
                                } else {
                                    Ty::Actor(nid)
                                });
                                struct_name = mangled.clone();
                            } else {
                                is_global = true;
                                global_name = mangled.clone();
                            }
                        } else if let Some(ty) = self.local_types.get(id) {
                            if let Ty::Struct(nid) | Ty::Class(nid) | Ty::Actor(nid) = ty {
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
                        if matches!(ty, Ty::Struct(_) | Ty::Class(_) | Ty::Actor(_)) {
                            is_instantiation = true;
                            inst_ty = Some(ty.clone());
                            let nid = match ty {
                                Ty::Struct(i) => i,
                                Ty::Class(i) => i,
                                Ty::Actor(i) => i,
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
                        if kind == 0 || kind == 1 || kind == 3 {
                            is_instantiation = true;
                            inst_ty = Some(if kind == 0 {
                                Ty::Struct(nid)
                            } else if kind == 1 {
                                Ty::Class(nid)
                            } else {
                                Ty::Actor(nid)
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
                            if kind == 0 || kind == 1 || kind == 3 {
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
                        if let Ty::List(elem_ty) = &obj_ty {
                            match member.as_str() {
                                "__iter" => {
                                    return obj_local;
                                }
                                "len" => {
                                    let temp = self.new_local(Ty::Int);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_list_len".to_string(), vec![obj_local]),
                                    ));
                                    return temp;
                                }
                                "push" | "add" => {
                                    let prev = self.current_expected_ty.take();
                                    self.current_expected_ty = Some(*elem_ty.clone());
                                    let arg_local = self.build_expr(&args[0].expr);
                                    self.current_expected_ty = prev;
                                    let temp = self.new_local(Ty::Void);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_list_push".to_string(), vec![obj_local, arg_local]),
                                    ));
                                    return temp;
                                }
                                "insert" => {
                                    let idx_local = self.build_expr(&args[0].expr);
                                    let prev = self.current_expected_ty.take();
                                    self.current_expected_ty = Some(*elem_ty.clone());
                                    let val_local = self.build_expr(&args[1].expr);
                                    self.current_expected_ty = prev;
                                    let temp = self.new_local(Ty::Void);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_list_insert".to_string(), vec![obj_local, idx_local, val_local]),
                                    ));
                                    return temp;
                                }
                                "removeAt" => {
                                    let idx_local = self.build_expr(&args[0].expr);
                                    let temp = self.new_local(*elem_ty.clone());
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_list_remove_at".to_string(), vec![obj_local, idx_local]),
                                    ));
                                    return temp;
                                }
                                "contains" | "indexOf" => {
                                    let prev = self.current_expected_ty.take();
                                    self.current_expected_ty = Some(*elem_ty.clone());
                                    let val_local = self.build_expr(&args[0].expr);
                                    self.current_expected_ty = prev;
                                    let suffix = if matches!(**elem_ty, Ty::String) {
                                        "str"
                                    } else if matches!(**elem_ty, Ty::Class(_) | Ty::List(_) | Ty::Map(_, _) | Ty::Set(_)) {
                                        "ptr"
                                    } else {
                                        "int"
                                    };
                                    let prefix = if member == "contains" { "pace_list_contains_" } else { "pace_list_index_of_" };
                                    let ret_ty = if member == "contains" { Ty::Bool } else { Ty::Int };
                                    let temp = self.new_local(ret_ty);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall(format!("{}{}", prefix, suffix), vec![obj_local, val_local]),
                                    ));
                                    return temp;
                                }
                                "slice" => {
                                    let start_local = self.build_expr(&args[0].expr);
                                    let end_local = self.build_expr(&args[1].expr);
                                    let temp = self.new_local(Ty::List(elem_ty.clone()));
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_list_slice".to_string(), vec![obj_local, start_local, end_local]),
                                    ));
                                    return temp;
                                }
                                "reversed" => {
                                    let temp = self.new_local(Ty::List(elem_ty.clone()));
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_list_reversed".to_string(), vec![obj_local]),
                                    ));
                                    return temp;
                                }
                                "join" => {
                                    let sep_local = self.build_expr(&args[0].expr);
                                    let temp = self.new_local(Ty::String);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_list_join".to_string(), vec![obj_local, sep_local]),
                                    ));
                                    return temp;
                                }
                                "forEach" | "map" | "filter" | "find" | "any" | "every" | "reduce" => {
                                    let elem_t = *elem_ty.clone();
                                    let (init_local, fn_local) = if member == "reduce" {
                                        (Some(self.build_expr(&args[0].expr)), self.build_expr(&args[1].expr))
                                    } else {
                                        (None, self.build_expr(&args[0].expr))
                                    };
                                    let fn_ret_ty = match &self.locals[fn_local.0 as usize] {
                                        Ty::Closure(_, r) | Ty::Function(_, r) => *r.clone(),
                                        _ => Ty::Int,
                                    };

                                    let len_local = self.new_local(Ty::Int);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(len_local),
                                        Rvalue::BuiltinCall("pace_list_len".to_string(), vec![obj_local]),
                                    ));
                                    let idx_local = self.new_local(Ty::Int);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(idx_local),
                                        Rvalue::Constant(Constant::Int("0".to_string())),
                                    ));
                                    let one_local = self.new_local(Ty::Int);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(one_local),
                                        Rvalue::Constant(Constant::Int("1".to_string())),
                                    ));

                                    // Initialize accumulator / result
                                    let res_local = match member.as_str() {
                                        "forEach" => self.new_local(Ty::Void),
                                        "map" => {
                                            let l = self.new_local(Ty::List(Box::new(fn_ret_ty.clone())));
                                            self.push_stmt(Statement::Assign(
                                                Lvalue::Local(l),
                                                Rvalue::ListNew(fn_ret_ty.clone(), len_local),
                                            ));
                                            l
                                        }
                                        "filter" => {
                                            let l = self.new_local(Ty::List(Box::new(elem_t.clone())));
                                            let cap = self.new_local(Ty::Int);
                                            self.push_stmt(Statement::Assign(
                                                Lvalue::Local(cap),
                                                Rvalue::Constant(Constant::Int("4".to_string())),
                                            ));
                                            self.push_stmt(Statement::Assign(
                                                Lvalue::Local(l),
                                                Rvalue::ListNew(elem_t.clone(), cap),
                                            ));
                                            l
                                        }
                                        "find" => {
                                            let null_local = self.new_local(Ty::Optional(Box::new(Ty::Void)));
                                            self.push_stmt(Statement::Assign(
                                                Lvalue::Local(null_local),
                                                Rvalue::Constant(Constant::Int("0".to_string())),
                                            ));
                                            let l = self.new_local(Ty::Optional(Box::new(elem_t.clone())));
                                            self.push_stmt(Statement::Assign(
                                                Lvalue::Local(l),
                                                Rvalue::Use(null_local),
                                            ));
                                            l
                                        }
                                        "any" => {
                                            let l = self.new_local(Ty::Bool);
                                            self.push_stmt(Statement::Assign(
                                                Lvalue::Local(l),
                                                Rvalue::Constant(Constant::Bool(false)),
                                            ));
                                            l
                                        }
                                        "every" => {
                                            let l = self.new_local(Ty::Bool);
                                            self.push_stmt(Statement::Assign(
                                                Lvalue::Local(l),
                                                Rvalue::Constant(Constant::Bool(true)),
                                            ));
                                            l
                                        }
                                        "reduce" => {
                                            let init_l = init_local.unwrap();
                                            let init_ty = self.locals[init_l.0 as usize].clone();
                                            let l = self.new_local(init_ty);
                                            self.push_stmt(Statement::Assign(
                                                Lvalue::Local(l),
                                                Rvalue::Use(init_l),
                                            ));
                                            l
                                        }
                                        _ => unreachable!(),
                                    };

                                    let cond_bb = self.new_block();
                                    let body_bb = self.new_block();
                                    let step_bb = self.new_block();
                                    let done_bb = self.new_block();

                                    self.blocks[self.current_block.0 as usize].terminator =
                                        Some(Terminator::Goto(cond_bb));

                                    self.current_block = cond_bb;
                                    let loop_cond = self.new_local(Ty::Bool);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(loop_cond),
                                        Rvalue::BinaryOp(pace_ast::BinaryOp::Lt, idx_local, len_local),
                                    ));
                                    self.blocks[self.current_block.0 as usize].terminator =
                                        Some(Terminator::Branch {
                                            cond: loop_cond,
                                            then_block: body_bb,
                                            else_block: done_bb,
                                        });

                                    self.current_block = body_bb;
                                    let elem_local = self.new_local(elem_t.clone());
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(elem_local),
                                        Rvalue::ListGet(obj_local, idx_local),
                                    ));
                                    let call_args = if member == "reduce" {
                                        vec![res_local, elem_local]
                                    } else {
                                        vec![elem_local]
                                    };
                                    let call_res = self.new_local(fn_ret_ty.clone());
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(call_res),
                                        Rvalue::Call(fn_local, call_args),
                                    ));

                                    match member.as_str() {
                                        "forEach" => {
                                            self.blocks[self.current_block.0 as usize].terminator =
                                                Some(Terminator::Goto(step_bb));
                                        }
                                        "map" => {
                                            let dummy = self.new_local(Ty::Void);
                                            self.push_stmt(Statement::Assign(
                                                Lvalue::Local(dummy),
                                                Rvalue::BuiltinCall("pace_list_push".to_string(), vec![res_local, call_res]),
                                            ));
                                            self.blocks[self.current_block.0 as usize].terminator =
                                                Some(Terminator::Goto(step_bb));
                                        }
                                        "filter" => {
                                            let push_bb = self.new_block();
                                            self.blocks[self.current_block.0 as usize].terminator =
                                                Some(Terminator::Branch {
                                                    cond: call_res,
                                                    then_block: push_bb,
                                                    else_block: step_bb,
                                                });
                                            self.current_block = push_bb;
                                            let dummy = self.new_local(Ty::Void);
                                            self.push_stmt(Statement::Assign(
                                                Lvalue::Local(dummy),
                                                Rvalue::BuiltinCall("pace_list_push".to_string(), vec![res_local, elem_local]),
                                            ));
                                            self.blocks[self.current_block.0 as usize].terminator =
                                                Some(Terminator::Goto(step_bb));
                                        }
                                        "find" => {
                                            let hit_bb = self.new_block();
                                            self.blocks[self.current_block.0 as usize].terminator =
                                                Some(Terminator::Branch {
                                                    cond: call_res,
                                                    then_block: hit_bb,
                                                    else_block: step_bb,
                                                });
                                            self.current_block = hit_bb;
                                            self.push_stmt(Statement::Assign(
                                                Lvalue::Local(res_local),
                                                Rvalue::Use(elem_local),
                                            ));
                                            self.blocks[self.current_block.0 as usize].terminator =
                                                Some(Terminator::Goto(done_bb));
                                        }
                                        "any" => {
                                            let hit_bb = self.new_block();
                                            self.blocks[self.current_block.0 as usize].terminator =
                                                Some(Terminator::Branch {
                                                    cond: call_res,
                                                    then_block: hit_bb,
                                                    else_block: step_bb,
                                                });
                                            self.current_block = hit_bb;
                                            self.push_stmt(Statement::Assign(
                                                Lvalue::Local(res_local),
                                                Rvalue::Constant(Constant::Bool(true)),
                                            ));
                                            self.blocks[self.current_block.0 as usize].terminator =
                                                Some(Terminator::Goto(done_bb));
                                        }
                                        "every" => {
                                            let miss_bb = self.new_block();
                                            self.blocks[self.current_block.0 as usize].terminator =
                                                Some(Terminator::Branch {
                                                    cond: call_res,
                                                    then_block: step_bb,
                                                    else_block: miss_bb,
                                                });
                                            self.current_block = miss_bb;
                                            self.push_stmt(Statement::Assign(
                                                Lvalue::Local(res_local),
                                                Rvalue::Constant(Constant::Bool(false)),
                                            ));
                                            self.blocks[self.current_block.0 as usize].terminator =
                                                Some(Terminator::Goto(done_bb));
                                        }
                                        "reduce" => {
                                            self.push_stmt(Statement::Assign(
                                                Lvalue::Local(res_local),
                                                Rvalue::Use(call_res),
                                            ));
                                            self.blocks[self.current_block.0 as usize].terminator =
                                                Some(Terminator::Goto(step_bb));
                                        }
                                        _ => unreachable!(),
                                    }

                                    self.current_block = step_bb;
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(idx_local),
                                        Rvalue::BinaryOp(pace_ast::BinaryOp::Add, idx_local, one_local),
                                    ));
                                    self.blocks[self.current_block.0 as usize].terminator =
                                        Some(Terminator::Goto(cond_bb));

                                    self.current_block = done_bb;
                                    return res_local;
                                }
                                "pop" => {
                                    let temp = self.new_local(Ty::Optional(elem_ty.clone()));
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_list_pop".to_string(), vec![obj_local]),
                                    ));
                                    return temp;
                                }
                                "clear" => {
                                    let temp = self.new_local(Ty::Void);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_list_clear".to_string(), vec![obj_local]),
                                    ));
                                    return temp;
                                }
                                "get" => {
                                    let idx_local = self.build_expr(&args[0].expr);
                                    let temp = self.new_local(*elem_ty.clone());
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::ListGet(obj_local, idx_local),
                                    ));
                                    return temp;
                                }
                                "set" => {
                                    let idx_local = self.build_expr(&args[0].expr);
                                    let prev = self.current_expected_ty.take();
                                    self.current_expected_ty = Some(*elem_ty.clone());
                                    let val_local = self.build_expr(&args[1].expr);
                                    self.current_expected_ty = prev;
                                    let temp = self.new_local(Ty::Void);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::ListSet(obj_local, idx_local, val_local),
                                    ));
                                    return temp;
                                }
                                _ => {}
                            }
                        } else if let Ty::Map(k_ty, v_ty) = &obj_ty {
                            match member.as_str() {
                                "__iter" | "keys" => {
                                    let temp = self.new_local(Ty::List(k_ty.clone()));
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_map_keys".to_string(), vec![obj_local]),
                                    ));
                                    return temp;
                                }
                                "len" => {
                                    let temp = self.new_local(Ty::Int);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_map_len".to_string(), vec![obj_local]),
                                    ));
                                    return temp;
                                }
                                "has" | "containsKey" => {
                                    let prev = self.current_expected_ty.take();
                                    self.current_expected_ty = Some(*k_ty.clone());
                                    let key_local = self.build_expr(&args[0].expr);
                                    self.current_expected_ty = prev;
                                    let is_str = matches!(**k_ty, Ty::String);
                                    let fn_name = if is_str { "pace_map_has_str" } else { "pace_map_has_int" };
                                    let temp = self.new_local(Ty::Bool);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall(fn_name.to_string(), vec![obj_local, key_local]),
                                    ));
                                    return temp;
                                }
                                "getOrDefault" => {
                                    let prev = self.current_expected_ty.take();
                                    self.current_expected_ty = Some(*k_ty.clone());
                                    let key_local = self.build_expr(&args[0].expr);
                                    self.current_expected_ty = Some(*v_ty.clone());
                                    let def_local = self.build_expr(&args[1].expr);
                                    self.current_expected_ty = prev;

                                    let is_str = matches!(**k_ty, Ty::String);
                                    let has_fn = if is_str { "pace_map_has_str" } else { "pace_map_has_int" };
                                    let has_local = self.new_local(Ty::Bool);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(has_local),
                                        Rvalue::BuiltinCall(has_fn.to_string(), vec![obj_local, key_local]),
                                    ));
                                    let res_local = self.new_local(*v_ty.clone());
                                    let then_bb = self.new_block();
                                    let else_bb = self.new_block();
                                    let merge_bb = self.new_block();
                                    self.blocks[self.current_block.0 as usize].terminator =
                                        Some(Terminator::Branch {
                                            cond: has_local,
                                            then_block: then_bb,
                                            else_block: else_bb,
                                        });
                                    self.current_block = then_bb;
                                    let val_local = self.new_local(*v_ty.clone());
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(val_local),
                                        Rvalue::MapGet(obj_local, key_local),
                                    ));
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(res_local),
                                        Rvalue::Use(val_local),
                                    ));
                                    self.blocks[self.current_block.0 as usize].terminator =
                                        Some(Terminator::Goto(merge_bb));
                                    self.current_block = else_bb;
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(res_local),
                                        Rvalue::Use(def_local),
                                    ));
                                    self.blocks[self.current_block.0 as usize].terminator =
                                        Some(Terminator::Goto(merge_bb));
                                    self.current_block = merge_bb;
                                    return res_local;
                                }
                                "values" => {
                                    let temp = self.new_local(Ty::List(v_ty.clone()));
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_map_values".to_string(), vec![obj_local]),
                                    ));
                                    return temp;
                                }
                                "forEach" => {
                                    let fn_local = self.build_expr(&args[0].expr);
                                    let keys_list = self.new_local(Ty::List(k_ty.clone()));
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(keys_list),
                                        Rvalue::BuiltinCall("pace_map_keys".to_string(), vec![obj_local]),
                                    ));
                                    let len_local = self.new_local(Ty::Int);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(len_local),
                                        Rvalue::BuiltinCall("pace_list_len".to_string(), vec![keys_list]),
                                    ));
                                    let idx_local = self.new_local(Ty::Int);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(idx_local),
                                        Rvalue::Constant(Constant::Int("0".to_string())),
                                    ));
                                    let one_local = self.new_local(Ty::Int);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(one_local),
                                        Rvalue::Constant(Constant::Int("1".to_string())),
                                    ));
                                    let cond_bb = self.new_block();
                                    let body_bb = self.new_block();
                                    let done_bb = self.new_block();
                                    self.blocks[self.current_block.0 as usize].terminator =
                                        Some(Terminator::Goto(cond_bb));
                                    self.current_block = cond_bb;
                                    let cond = self.new_local(Ty::Bool);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(cond),
                                        Rvalue::BinaryOp(pace_ast::BinaryOp::Lt, idx_local, len_local),
                                    ));
                                    self.blocks[self.current_block.0 as usize].terminator =
                                        Some(Terminator::Branch {
                                            cond,
                                            then_block: body_bb,
                                            else_block: done_bb,
                                        });
                                    self.current_block = body_bb;
                                    let k_local = self.new_local(*k_ty.clone());
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(k_local),
                                        Rvalue::ListGet(keys_list, idx_local),
                                    ));
                                    let v_local = self.new_local(*v_ty.clone());
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(v_local),
                                        Rvalue::MapGet(obj_local, k_local),
                                    ));
                                    let dummy = self.new_local(Ty::Void);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(dummy),
                                        Rvalue::Call(fn_local, vec![k_local, v_local]),
                                    ));
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(idx_local),
                                        Rvalue::BinaryOp(pace_ast::BinaryOp::Add, idx_local, one_local),
                                    ));
                                    self.blocks[self.current_block.0 as usize].terminator =
                                        Some(Terminator::Goto(cond_bb));
                                    self.current_block = done_bb;
                                    let ret = self.new_local(Ty::Void);
                                    return ret;
                                }
                                "remove" => {
                                    let prev = self.current_expected_ty.take();
                                    self.current_expected_ty = Some(*k_ty.clone());
                                    let key_local = self.build_expr(&args[0].expr);
                                    self.current_expected_ty = prev;
                                    let is_str = matches!(**k_ty, Ty::String);
                                    let fn_name = if is_str { "pace_map_remove_str" } else { "pace_map_remove_int" };
                                    let temp = self.new_local(Ty::Bool);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall(fn_name.to_string(), vec![obj_local, key_local]),
                                    ));
                                    return temp;
                                }
                                "clear" => {
                                    let temp = self.new_local(Ty::Void);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_map_clear".to_string(), vec![obj_local]),
                                    ));
                                    return temp;
                                }
                                "get" => {
                                    let prev = self.current_expected_ty.take();
                                    self.current_expected_ty = Some(*k_ty.clone());
                                    let key_local = self.build_expr(&args[0].expr);
                                    self.current_expected_ty = prev;
                                    let temp = self.new_local(Ty::Optional(v_ty.clone()));
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::MapGet(obj_local, key_local),
                                    ));
                                    return temp;
                                }
                                "set" => {
                                    let prev = self.current_expected_ty.take();
                                    self.current_expected_ty = Some(*k_ty.clone());
                                    let key_local = self.build_expr(&args[0].expr);
                                    self.current_expected_ty = Some(*v_ty.clone());
                                    let val_local = self.build_expr(&args[1].expr);
                                    self.current_expected_ty = prev;
                                    let temp = self.new_local(Ty::Void);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::MapSet(obj_local, key_local, val_local),
                                    ));
                                    return temp;
                                }
                                _ => {}
                            }
                        } else if let Ty::Set(elem_ty) = &obj_ty {
                            let is_str = matches!(**elem_ty, Ty::String);
                            match member.as_str() {
                                "len" => {
                                    let temp = self.new_local(Ty::Int);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_set_len".to_string(), vec![obj_local]),
                                    ));
                                    return temp;
                                }
                                "add" | "has" | "contains" | "remove" => {
                                    let prev = self.current_expected_ty.take();
                                    self.current_expected_ty = Some(*elem_ty.clone());
                                    let key_local = self.build_expr(&args[0].expr);
                                    self.current_expected_ty = prev;
                                    let base = match member.as_str() {
                                        "add" => "pace_set_add_",
                                        "remove" => "pace_set_remove_",
                                        _ => "pace_set_has_",
                                    };
                                    let fn_name = format!("{}{}", base, if is_str { "str" } else { "int" });
                                    let temp = self.new_local(Ty::Bool);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall(fn_name, vec![obj_local, key_local]),
                                    ));
                                    return temp;
                                }
                                "clear" => {
                                    let temp = self.new_local(Ty::Void);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_set_clear".to_string(), vec![obj_local]),
                                    ));
                                    return temp;
                                }
                                "toList" | "__iter" => {
                                    let temp = self.new_local(Ty::List(elem_ty.clone()));
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall("pace_set_to_list".to_string(), vec![obj_local]),
                                    ));
                                    return temp;
                                }
                                "union" | "intersection" | "difference" => {
                                    let other_local = self.build_expr(&args[0].expr);
                                    let fn_name = format!("pace_set_{}", member);
                                    let temp = self.new_local(Ty::Set(elem_ty.clone()));
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::BuiltinCall(fn_name, vec![obj_local, other_local]),
                                    ));
                                    return temp;
                                }
                                _ => {}
                            }
                        } else if let Ty::String = &obj_ty {
                            let (fn_name, ret_ty) = match member.as_str() {
                                "len" => ("pace_string_len", Ty::Int),
                                "contains" => ("pace_string_contains", Ty::Bool),
                                "startsWith" => ("pace_string_starts_with", Ty::Bool),
                                "endsWith" => ("pace_string_ends_with", Ty::Bool),
                                "indexOf" => ("pace_string_index_of", Ty::Int),
                                "trim" => ("pace_string_trim", Ty::String),
                                "slice" => ("pace_string_slice", Ty::String),
                                "replace" => ("pace_string_replace", Ty::String),
                                "toLower" => ("pace_string_to_lower", Ty::String),
                                "toUpper" => ("pace_string_to_upper", Ty::String),
                                "split" => ("pace_string_split", Ty::List(Box::new(Ty::String))),
                                _ => ("", Ty::Void),
                            };
                            if !fn_name.is_empty() {
                                let mut call_args = vec![obj_local];
                                for arg in args {
                                    call_args.push(self.build_expr(&arg.expr));
                                }
                                let temp = self.new_local(ret_ty);
                                self.push_stmt(Statement::Assign(
                                    Lvalue::Local(temp),
                                    Rvalue::BuiltinCall(fn_name.to_string(), call_args),
                                ));
                                return temp;
                            }
                        } else if let Ty::StringBuilder = &obj_ty {
                            let (fn_name, ret_ty) = match member.as_str() {
                                "len" => ("pace_sb_len", Ty::Int),
                                "append" => ("pace_sb_append", Ty::Void),
                                "toString" => ("pace_sb_to_string", Ty::String),
                                "clear" => ("pace_sb_clear", Ty::Void),
                                _ => ("", Ty::Void),
                            };
                            if !fn_name.is_empty() {
                                let mut call_args = vec![obj_local];
                                for arg in args {
                                    call_args.push(self.build_expr(&arg.expr));
                                }
                                let temp = self.new_local(ret_ty);
                                self.push_stmt(Statement::Assign(
                                    Lvalue::Local(temp),
                                    Rvalue::BuiltinCall(fn_name.to_string(), call_args),
                                ));
                                return temp;
                            }
                        } else if let Ty::Result(_, _) = &obj_ty {
                            match member.as_str() {
                                "isOk" => {
                                    let temp = self.new_local(Ty::Bool);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::ResultIsOk(obj_local),
                                    ));
                                    return temp;
                                }
                                "isError" => {
                                    let ok_local = self.new_local(Ty::Bool);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(ok_local),
                                        Rvalue::ResultIsOk(obj_local),
                                    ));
                                    let temp = self.new_local(Ty::Bool);
                                    self.push_stmt(Statement::Assign(
                                        Lvalue::Local(temp),
                                        Rvalue::UnaryOp(pace_ast::UnaryOp::Not, ok_local),
                                    ));
                                    return temp;
                                }
                                _ => {}
                            }
                        } else if let Ty::Struct(id) | Ty::Class(id) | Ty::Actor(id) = obj_ty {
                            let mut target_id = id;
                            if is_super && let Some(&parent_id) = self.class_parents.get(&id) {
                                target_id = parent_id;
                            }
                            let mut curr_id = Some(target_id);
                            let mut resolved_method_name = None;
                            while let Some(cid) = curr_id {
                                let mut tname = "";
                                for (name, &(tid, _)) in self.named_types {
                                    if cid == tid {
                                        tname = name;
                                        break;
                                    }
                                }
                                let mname = format!("{}_{}", tname, member);
                                if self.methods_env.contains_key(&mname) {
                                    resolved_method_name = Some(mname);
                                    break;
                                }
                                curr_id = self.class_parents.get(&cid).copied();
                            }

                            if let Some(method_name) = resolved_method_name {
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
                    if !has_init && self.methods_env.contains_key(&init_name) {
                        has_init = true;
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

                    let mut param_tys_opt = None;
                    if let Some(Ty::Function(ptys, ret)) = self.global_functions_env.get(&global_name)
                    {
                        param_tys_opt = Some(ptys.clone());
                        if let Ty::Enum(eid) = **ret {
                            is_enum_variant = true;
                            enum_id = Some(eid);
                        } else {
                            ret_ty = *ret.clone();
                        }
                    } else if let Some(Ty::Function(ptys, ret)) = self.methods_env.get(&global_name) {
                        param_tys_opt = Some(ptys.clone());
                        if let Ty::Enum(eid) = **ret {
                            is_enum_variant = true;
                            enum_id = Some(eid);
                        } else {
                            ret_ty = *ret.clone();
                        }
                    }
                    if let Some(ptys) = param_tys_opt {
                        for (i, pty) in ptys.iter().enumerate() {
                            if i < arg_locals.len()
                                && matches!(pty, Ty::Optional(_))
                                && self.locals[arg_locals[i].0 as usize] != *pty
                            {
                                let coerced = self.new_local(pty.clone());
                                self.push_stmt(Statement::Assign(
                                    Lvalue::Local(coerced),
                                    Rvalue::Use(arg_locals[i]),
                                ));
                                arg_locals[i] = coerced;
                            }
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
                let ret_ty = match &self.locals[callee_local.0 as usize] {
                    Ty::Closure(_, ret) | Ty::Function(_, ret) => *ret.clone(),
                    _ => Ty::Int,
                };
                let temp = self.new_local(ret_ty);
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
                let ret_ty = if name == "sleep" {
                    Ty::Future(Box::new(Ty::Void))
                } else if name.as_str().starts_with("pace_") {
                    match name.as_str() {
                        "pace_eprintln" | "pace_env_set" | "pace_time_sleep_ms" => Ty::Void,
                        "pace_process_exit" => Ty::Never,
                        "pace_file_exists" | "pace_dir_exists" | "pace_env_has" => Ty::Bool,
                        "pace_time_now_ms" => Ty::Int,
                        "pace_os_name" | "pace_process_cwd" => Ty::String,
                        "pace_env_get" => Ty::Optional(Box::new(Ty::String)),
                        "pace_process_args" => Ty::List(Box::new(Ty::String)),
                        "pace_read_line" | "pace_file_read_text" => {
                            Ty::Result(Box::new(Ty::String), Box::new(Ty::String))
                        }
                        "pace_file_write_text" | "pace_dir_make" => {
                            Ty::Result(Box::new(Ty::Void), Box::new(Ty::String))
                        }
                        "pace_math_sqrt" | "pace_math_pow" | "pace_math_abs"
                        | "pace_math_round" | "pace_math_floor" | "pace_math_ceil"
                        | "pace_math_sin" | "pace_math_cos" | "pace_math_tan" => Ty::Float,
                        _ => Ty::Int,
                    }
                } else if name == "ok" {
                    if let Some(Ty::Result(o, e)) = &self.current_expected_ty {
                        Ty::Result(o.clone(), e.clone())
                    } else {
                        let ok_ty = arg_locals
                            .first()
                            .map(|l| self.locals[l.0 as usize].clone())
                            .unwrap_or(Ty::Void);
                        Ty::Result(Box::new(ok_ty), Box::new(Ty::Void))
                    }
                } else if name == "error" {
                    if let Some(Ty::Result(o, e)) = &self.current_expected_ty {
                        Ty::Result(o.clone(), e.clone())
                    } else {
                        let err_ty = arg_locals
                            .first()
                            .map(|l| self.locals[l.0 as usize].clone())
                            .unwrap_or(Ty::Void);
                        Ty::Result(Box::new(Ty::Void), Box::new(err_ty))
                    }
                } else {
                    Ty::Int
                };
                let temp = self.new_local(ret_ty);
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
                self.loop_stack.push((cond_bb, merge_bb));
                self.build_block(body);
                self.loop_stack.pop();
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
                    } else if let pace_hir::Pattern::Ident(id, name, _) = &arm.pattern {
                        let subject_ty = self.locals[subject_local.0 as usize].clone();
                        let matched_variant_tag = if let Ty::Enum(enum_id) = subject_ty
                            && let Some(variants) = self.enum_defs.get(&enum_id)
                        {
                            variants.iter().position(|v| v.name == *name).map(|i| i as i32)
                        } else {
                            None
                        };

                        if let Some(tag) = matched_variant_tag {
                            arm_tag = tag;
                        } else {
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
                        }
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
                if let Expr::Index { target: idx_target, index: idx_expr, span: _ } = &**target {
                    let obj_local = self.build_expr(idx_target);
                    let obj_ty = self.locals[obj_local.0 as usize].clone();
                    let idx_local = self.build_expr(idx_expr);
                    let prev = self.current_expected_ty.take();
                    match &obj_ty {
                        Ty::List(elem) => {
                            self.current_expected_ty = Some(*elem.clone());
                        }
                        Ty::Map(_, val) => {
                            self.current_expected_ty = Some(*val.clone());
                        }
                        _ => {}
                    }
                    let val_local = self.build_expr(value);
                    self.current_expected_ty = prev;

                    match obj_ty {
                        Ty::List(_) => {
                            let dummy = self.new_local(Ty::Void);
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(dummy),
                                Rvalue::ListSet(obj_local, idx_local, val_local),
                            ));
                        }
                        Ty::Map(_, _) => {
                            let dummy = self.new_local(Ty::Void);
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(dummy),
                                Rvalue::MapSet(obj_local, idx_local, val_local),
                            ));
                        }
                        other => {
                            eprintln!("ICE: Index assignment on non-collection type {:?}", other);
                        }
                    }
                    return val_local;
                }

                let mut is_static_assign = false;
                let mut static_name = String::new();
                let lvalue = match &**target {
                    Expr::Ident(id, name, _, _) => {
                        if let Some(l) = self.hir_to_local.get(id) {
                            Lvalue::Local(*l)
                        } else {
                            is_static_assign = true;
                            static_name = self
                                .resolved_global_names
                                .get(id)
                                .cloned()
                                .unwrap_or_else(|| name.to_string());
                            Lvalue::Local(Local(0))
                        }
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
                    _ => {
                        eprintln!("ICE: Invalid assignment target");
                        Lvalue::Local(Local(0))
                    }
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
                        if let Ty::Struct(id) | Ty::Class(id) | Ty::Actor(id) = obj_ty {
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
                            eprintln!("ICE: Field not found: member={}, obj_ty={:?}", member, obj_ty);
                            field_ty = Ty::Int;
                        }
                        field_ty
                    }
                    Lvalue::EnumFieldAccess(_, _, _) => {
                        eprintln!("ICE: Cannot assign to enum field");
                        Ty::Int
                    }
                };
                let prev = self.current_expected_ty.take();
                self.current_expected_ty = Some(lval_ty.clone());
                let rval = self.build_expr(value);
                self.current_expected_ty = prev;
                if matches!(lval_ty, Ty::Class(_) | Ty::List(_) | Ty::Map(_, _) | Ty::Set(_) | Ty::StringBuilder) {
                    self.push_stmt(Statement::Retain(Lvalue::Local(rval)));
                    self.push_stmt(Statement::Release(lvalue.clone()));
                }
                self.push_stmt(Statement::Assign(lvalue.clone(), Rvalue::Use(rval)));
                rval
            }
            Expr::Await(inner, _) => {
                let inner_local = self.build_expr(inner);
                let inner_ty = self.locals[inner_local.0 as usize].clone();
                let ret_ty = if let Ty::Future(inner_t) = inner_ty {
                    *inner_t
                } else {
                    inner_ty
                };
                let temp = self.new_local(ret_ty);
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(temp),
                    Rvalue::BuiltinCall("pace_await_fiber".to_string(), vec![inner_local]),
                ));
                temp
            }
            Expr::Try(inner, _) => {
                let inner_local = self.build_expr(inner);
                let inner_ty = self.locals[inner_local.0 as usize].clone();
                match inner_ty {
                    Ty::Result(ok_ty, err_ty) => {
                        let cond_local = self.new_local(Ty::Bool);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(cond_local),
                            Rvalue::ResultIsOk(inner_local),
                        ));
                        let ok_bb = self.new_block();
                        let err_bb = self.new_block();
                        let merge_bb = self.new_block();
                        let cur_bb = self.current_block;
                        self.blocks[cur_bb.0 as usize].terminator = Some(Terminator::Branch {
                            cond: cond_local,
                            then_block: ok_bb,
                            else_block: err_bb,
                        });

                        self.current_block = err_bb;
                        let err_ret = self.new_local(Ty::Result(Box::new(Ty::Void), err_ty));
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(err_ret),
                            Rvalue::ResultPropagateErr(inner_local),
                        ));
                        self.blocks[err_bb.0 as usize].terminator =
                            Some(Terminator::Return(err_ret));

                        self.current_block = ok_bb;
                        let out_local = self.new_local(*ok_ty);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(out_local),
                            Rvalue::ResultOk(inner_local),
                        ));
                        self.blocks[ok_bb.0 as usize].terminator = Some(Terminator::Goto(merge_bb));

                        self.current_block = merge_bb;
                        out_local
                    }
                    Ty::Optional(val_ty) => {
                        let cond_local = self.new_local(Ty::Bool);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(cond_local),
                            Rvalue::OptionIsSome(inner_local),
                        ));
                        let some_bb = self.new_block();
                        let none_bb = self.new_block();
                        let merge_bb = self.new_block();
                        let cur_bb = self.current_block;
                        self.blocks[cur_bb.0 as usize].terminator = Some(Terminator::Branch {
                            cond: cond_local,
                            then_block: some_bb,
                            else_block: none_bb,
                        });

                        self.current_block = none_bb;
                        let null_local = self.new_local(Ty::Optional(Box::new(Ty::Void)));
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(null_local),
                            Rvalue::Constant(Constant::Int("0".to_string())),
                        ));
                        self.blocks[none_bb.0 as usize].terminator =
                            Some(Terminator::Return(null_local));

                        self.current_block = some_bb;
                        let out_local = self.new_local(*val_ty);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(out_local),
                            Rvalue::Use(inner_local),
                        ));
                        self.blocks[some_bb.0 as usize].terminator =
                            Some(Terminator::Goto(merge_bb));

                        self.current_block = merge_bb;
                        out_local
                    }
                    _ => inner_local,
                }
            }
            Expr::ListLiteral(elements, _) => {
                let elem_ty = if let Some(Ty::List(inner)) = &self.current_expected_ty {
                    *inner.clone()
                } else {
                    Ty::Int
                };
                let cap_val = elements.len().max(4);
                let cap_local = self.new_local(Ty::Int);
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(cap_local),
                    Rvalue::Constant(Constant::Int(cap_val.to_string())),
                ));
                let mut elem_locals = Vec::new();
                for elem in elements {
                    let prev = self.current_expected_ty.take();
                    self.current_expected_ty = Some(elem_ty.clone());
                    let el = self.build_expr(elem);
                    self.current_expected_ty = prev;
                    elem_locals.push(el);
                }
                let actual_elem_ty = if !elem_locals.is_empty() {
                    self.locals[elem_locals[0].0 as usize].clone()
                } else {
                    elem_ty
                };
                let list_ty = Ty::List(Box::new(actual_elem_ty.clone()));
                let list_local = self.new_local(list_ty);
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(list_local),
                    Rvalue::ListNew(actual_elem_ty, cap_local),
                ));
                for el in elem_locals {
                    let dummy = self.new_local(Ty::Void);
                    self.push_stmt(Statement::Assign(
                        Lvalue::Local(dummy),
                        Rvalue::BuiltinCall("pace_list_push".to_string(), vec![list_local, el]),
                    ));
                }
                list_local
            }
            Expr::MapLiteral(entries, _) => {
                let (mut k_ty, mut v_ty) = if let Some(Ty::Map(k, v)) = &self.current_expected_ty {
                    (*k.clone(), *v.clone())
                } else {
                    (Ty::String, Ty::Int)
                };
                let mut evaluated_entries = Vec::new();
                for (k, v) in entries {
                    let prev = self.current_expected_ty.take();
                    self.current_expected_ty = Some(k_ty.clone());
                    let k_local = self.build_expr(k);
                    self.current_expected_ty = Some(v_ty.clone());
                    let v_local = self.build_expr(v);
                    self.current_expected_ty = prev;
                    evaluated_entries.push((k_local, v_local));
                }
                if !evaluated_entries.is_empty() {
                    k_ty = self.locals[evaluated_entries[0].0.0 as usize].clone();
                    v_ty = self.locals[evaluated_entries[0].1.0 as usize].clone();
                }
                let map_ty = Ty::Map(Box::new(k_ty.clone()), Box::new(v_ty.clone()));
                let map_local = self.new_local(map_ty);
                self.push_stmt(Statement::Assign(
                    Lvalue::Local(map_local),
                    Rvalue::MapNew(k_ty, v_ty),
                ));
                for (k_local, v_local) in evaluated_entries {
                    let dummy = self.new_local(Ty::Void);
                    self.push_stmt(Statement::Assign(
                        Lvalue::Local(dummy),
                        Rvalue::MapSet(map_local, k_local, v_local),
                    ));
                }
                map_local
            }
            Expr::Index { target, index, span: _ } => {
                let obj_local = self.build_expr(target);
                let obj_ty = self.locals[obj_local.0 as usize].clone();
                let idx_local = self.build_expr(index);
                match obj_ty {
                    Ty::List(elem_ty) => {
                        let temp = self.new_local(*elem_ty);
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(temp),
                            Rvalue::ListGet(obj_local, idx_local),
                        ));
                        temp
                    }
                    Ty::Map(_, val_ty) => {
                        let temp = self.new_local(Ty::Optional(val_ty));
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(temp),
                            Rvalue::MapGet(obj_local, idx_local),
                        ));
                        temp
                    }
                    other => {
                        eprintln!("ICE: Indexing non-collection type {:?}", other);
                        self.new_local(Ty::Int)
                    }
                }
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
            } else if let pace_hir::Decl::Actor { methods, .. } = decl {
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
                pace_hir::Decl::Let { id, name, value, .. }
                | pace_hir::Decl::Var { id, name, value, .. } => {
                    let global_name = tc
                        .resolved_global_names
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| name.to_string());
                    if let Some(val) = value {
                        let rval_local = main_builder.build_expr(val);
                        let var_ty = main_builder.locals[rval_local.0 as usize].clone();
                        global_vars.push((global_name.clone(), var_ty.clone()));
                        main_builder.push_stmt(Statement::GlobalWrite(global_name, rval_local));
                        _main_last_local = rval_local;
                    } else {
                        // Uninitialized global variable
                        let var_ty = main_builder
                            .local_types
                            .get(id)
                            .expect("Type not found in local_types")
                            .clone();
                        global_vars.push((global_name, var_ty));
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
                        let ty = tc_get_type(tc, &sf_ty).unwrap_or(Ty::Int);
                        global_vars.push((global_name.clone(), ty.clone()));

                        let rval_local = main_builder.build_expr(&sf_expr);
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
                        let ty = tc_get_type(tc, &cf_ty).unwrap_or(Ty::Int);
                        global_vars.push((global_name.clone(), ty.clone()));

                        let rval_local = main_builder.build_expr(&cf_expr);
                        main_builder.push_stmt(Statement::GlobalWrite(global_name, rval_local));
                    }
                    for method in methods {
                        if let pace_hir::Decl::Function {
                            name,
                            id: func_id,
                            params,
                            return_type,
                            body,
                            is_async,
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
                            for (param_id, param_name, _pty) in params {
                                let ty = tc.local_types.get(param_id).cloned().unwrap_or(Ty::Int);
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
                            let fn_body = fn_builder.finish(&mir_params, &ret_ty);

                            functions.push(MirFunction {
                                name: tc.resolved_global_names.get(func_id).unwrap_or(&name.to_string()).clone(),
                                params: mir_params,
                                return_type: ret_ty,
                                is_async: *is_async,
                                env_layout: None,
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
                    is_async,
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
                    for (param_id, param_name, _pty) in params {
                        let ty = tc.local_types.get(param_id).cloned().unwrap_or(Ty::Int);
                        let local = fn_builder.new_local(ty);
                        if param_name == "self" {
                            fn_builder.current_self_local = Some(local);
                        }
                        fn_builder.hir_to_local.insert(*param_id, local);
                        mir_params.push(local);
                    }

                    let ret_ty = if let Some(Ty::Function(_, ret)) = tc.env.get(func_id) {
                        *ret.clone()
                    } else if let Some(rty) = return_type {
                        tc_get_type(tc, rty).unwrap_or(Ty::Void)
                    } else {
                        Ty::Void
                    };
                    fn_builder.build_block(body);
                    let extracted_closures = std::mem::take(&mut fn_builder.closure_functions);
                    let fn_body = fn_builder.finish(&mir_params, &ret_ty);
                    functions.push(MirFunction {
                        name: tc
                            .resolved_global_names
                            .get(func_id)
                            .cloned()
                            .unwrap_or_else(|| name.to_string()),
                        params: mir_params,
                        return_type: ret_ty,
                        is_async: *is_async,
                        env_layout: None,
                        body: fn_body,
                    });
                    functions.extend(extracted_closures);
                }
                pace_hir::Decl::Enum { .. } => {}

                pace_hir::Decl::Actor {
                    id: _,
                    name: _,
                    methods,
                    generic_params,
                    ..
                } => {
                    if generic_params.is_some() {
                        continue;
                    }

                    for method in methods {
                        if let pace_hir::Decl::Function {
                            name,
                            id: func_id,
                            params,
                            return_type: _,
                            body,
                            is_async,
                            ..
                        } = method
                        {
                            let mut builder = MirBuilder::new(
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
                            for (p_id, param_name, _pty) in params {
                                let param_ty = tc.local_types.get(p_id).cloned().unwrap_or(Ty::Int);
                                let param_local = builder.new_local(param_ty);
                                if param_name == "self" {
                                    builder.current_self_local = Some(param_local);
                                }
                                builder.hir_to_local.insert(*p_id, param_local);
                                mir_params.push(param_local);
                            }
                            
                            let ret_ty = match tc.env.get(func_id) {
                                Some(Ty::Function(_, r)) => *r.clone(),
                                _ => Ty::Void,
                            };
                            builder.build_block(body);
                            let extracted_closures = std::mem::take(&mut builder.closure_functions);
                            let fn_body = builder.finish(&mir_params, &ret_ty);
                            
                            functions.push(MirFunction {
                                name: tc.resolved_global_names.get(func_id).unwrap_or(&name.to_string()).clone(),
                                params: mir_params,
                                return_type: ret_ty,
                                env_layout: None,
                                body: MirBody {
                                    locals: fn_body.locals,
                                    blocks: fn_body.blocks,
                                },
                                is_async: *is_async,
                            });
                            functions.extend(extracted_closures);
                        }
                    }
                }

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

        let mut actor_defs = std::collections::HashSet::new();
        for (_, &(tid, kind)) in &tc.named_types {
            if kind == 3 {
                actor_defs.insert(tid);
            }
        }

        MirProgram {
            functions,
            main_body: main_builder.finish(&[], &Ty::Void),
            struct_defs: tc.struct_defs.clone(),
            class_defs: tc.class_defs.clone(),
            actor_defs,
            class_vtables: tc.class_vtables.clone(),
            enum_defs: resolved_enum_defs,
            global_vars,
        }
    }
}

impl<'a> MirBuilder<'a> {
    pub fn finish(mut self, params: &[Local], ret_ty: &Ty) -> MirBody {
        let current_bb = self.current_block.0 as usize;

        if self.blocks[current_bb].terminator.is_none() {
            let temp = self.new_local(ret_ty.clone());
            self.blocks[current_bb].statements.push(Statement::Assign(
                Lvalue::Local(temp),
                Rvalue::Constant(Constant::Int("0".to_string())),
            ));
            self.blocks[current_bb].terminator = Some(Terminator::Return(temp));
        }

        // ARC: Release all Class, List, and Map locals at return points, except the returned local and params
        for block in &mut self.blocks {
            if let Some(Terminator::Return(ret_local)) = block.terminator {
                for (i, ty) in self.locals.iter().enumerate() {
                    let local = Local(i as u32);
                    if matches!(ty, Ty::Class(_) | Ty::List(_) | Ty::Map(_, _) | Ty::Set(_) | Ty::StringBuilder)
                        && local != ret_local
                        && !params.contains(&local)
                        && !self.scope_managed_locals.contains(&local)
                    {
                        block.statements.push(Statement::Release(Lvalue::Local(local)));
                    }
                }
            }
        }

        MirBody {
            blocks: self.blocks,
            locals: self.locals,
        }
    }
}
