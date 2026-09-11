use std::collections::HashMap;
use pace_hir::{Expr, HirId};
use crate::mir::*;
use pace_ty::Ty;

pub struct MirBuilder<'a> {
    pub blocks: Vec<BasicBlock>,
    pub current_block: BasicBlockId,
    pub locals: Vec<Ty>,
    pub hir_to_local: HashMap<HirId, Local>,
    pub global_fns: HashMap<HirId, String>,
    pub struct_defs: &'a HashMap<HirId, Vec<(String, Ty)>>,
    pub class_defs: &'a HashMap<HirId, Vec<(String, Ty)>>,
    pub enum_defs: &'a HashMap<HirId, Vec<pace_hir::EnumVariant>>,
    pub global_env: &'a HashMap<pace_hir::HirId, Ty>,
    pub local_types: &'a HashMap<pace_hir::HirId, Ty>,
    pub named_types: &'a HashMap<String, (pace_hir::HirId, u8)>,
    pub static_fields_env: &'a HashMap<String, Ty>,
    pub methods_env: &'a HashMap<String, Ty>,
    pub current_expected_ty: Option<Ty>,
}

impl<'a> MirBuilder<'a> {
    pub fn new(
        global_fns: HashMap<HirId, String>,
        struct_defs: &'a HashMap<HirId, Vec<(String, Ty)>>,
        class_defs: &'a HashMap<pace_hir::HirId, Vec<(String, Ty)>>,
        enum_defs: &'a HashMap<pace_hir::HirId, Vec<pace_hir::EnumVariant>>,
        global_env: &'a HashMap<pace_hir::HirId, Ty>,
        local_types: &'a HashMap<pace_hir::HirId, Ty>,
        named_types: &'a HashMap<String, (pace_hir::HirId, u8)>,
        static_fields_env: &'a HashMap<String, Ty>,
        methods_env: &'a HashMap<String, Ty>,
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
            global_fns,
            struct_defs,
            class_defs,
            enum_defs,
            global_env,
            local_types,
            named_types,
            static_fields_env,
            methods_env,
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
                pace_hir::Stmt::Let { id, value, ty: _, .. } | pace_hir::Stmt::Var { id, value, ty: _, .. } => {
                    let var_ty = self.local_types.get(id).expect("Variable type not found in local_types environment").clone();
                    if let Some(val) = value {
                        let prev = self.current_expected_ty.take();
                        self.current_expected_ty = Some(var_ty.clone());
                        let rval_local = self.build_expr(val);
                        self.current_expected_ty = prev;
                        let var_local = self.new_local(var_ty.clone());
                        self.hir_to_local.insert(*id, var_local);
                        self.push_stmt(Statement::Assign(Lvalue::Local(var_local), Rvalue::Use(rval_local)));
                        if matches!(var_ty, Ty::Class(_)) {
                            self.push_stmt(Statement::Retain(Lvalue::Local(var_local)));
                        }
                    } else {
                        // Uninitialized variable
                        let var_ty = self.local_types.get(id).expect("Variable type not found in local_types environment").clone();
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
                            self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::IntConstant("0".to_string())));
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
            Expr::Ident(id, name, _) => {
                if let Some(Ty::Enum(enum_id)) = self.global_env.get(id) {
                    if let Some(variants) = self.enum_defs.get(enum_id) {
                        for v in variants {
                            if v.id == *id {
                                let temp = self.new_local(Ty::Enum(*enum_id));
                                self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::InstantiateEnum(*enum_id, v.name.clone(), vec![])));
                                return temp;
                            }
                        }
                    }
                }
                
                let keys: Vec<_> = self.named_types.keys().collect();
                let local = *self.hir_to_local.get(id).expect(&format!("Local not found for id {:?} name {}. Named types: {:?}", id, name, keys));
                let temp = self.new_local(self.locals[local.0 as usize].clone());
                self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::Use(local)));
                if matches!(self.locals[temp.0 as usize], Ty::Class(_)) {
                    self.push_stmt(Statement::Retain(Lvalue::Local(temp)));
                }
                temp
            }
            Expr::Binary { left, op, right, .. } => {
                let lhs = self.build_expr(left);
                let rhs = self.build_expr(right);
                let temp = self.new_local(Ty::Int);
                self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::BinaryOp(*op, lhs, rhs)));
                temp
            }
            Expr::IntLiteral(val, _) => {
                let local = self.new_local(Ty::Int);
                self.push_stmt(Statement::Assign(Lvalue::Local(local), Rvalue::IntConstant(val.clone())));
                local
            }
            Expr::FloatLiteral(val, _) => {
                let local = self.new_local(Ty::Float);
                self.push_stmt(Statement::Assign(Lvalue::Local(local), Rvalue::FloatConstant(val.clone())));
                local
            }
            Expr::BoolLiteral(val, _) => {
                let local = self.new_local(Ty::Bool);
                self.push_stmt(Statement::Assign(Lvalue::Local(local), Rvalue::BoolConstant(*val)));
                local
            }
            Expr::StringLiteral(val, _) => {
                let temp = self.new_local(Ty::String);
                self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::StringConstant(val.clone())));
                temp
            }
            Expr::MemberAccess { object, member, .. } => {
                // If the object is a type (class/struct), it's a static access
                if let Expr::Ident(id, name, _) = &**object {
                    if let Some(ty) = self.global_env.get(id) {
                        if matches!(ty, Ty::Struct(_) | Ty::Class(_)) {
                            let mut type_name = "";
                            let nid = match ty { Ty::Struct(i) => i, Ty::Class(i) => i, _ => unreachable!() };
                            for (name, &(tid, _)) in self.named_types {
                                if *nid == tid { type_name = name; break; }
                            }
                            
                            let static_name = format!("{}_{}", type_name, member);
                            
                            let mut field_ty = Ty::Int;
                            if let Some(sfty) = self.static_fields_env.get(&static_name) {
                                field_ty = sfty.clone();
                            } else if let Some(mty) = self.methods_env.get(&static_name) {
                                field_ty = mty.clone();
                            }
                            
                            let temp = self.new_local(field_ty);
                            self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::GlobalRead(static_name)));
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
                            for (n, t) in fields {
                                if n == member { ft = t.clone(); break; }
                            }
                        }
                        ft
                    }
                    Ty::Class(id) => {
                        let mut ft = Ty::Int;
                        if let Some(fields) = self.class_defs.get(&id) {
                            for (n, t) in fields {
                                if n == member { ft = t.clone(); break; }
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
                self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::FieldAccess(obj_local, member.clone())));
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
                
                if let Expr::Ident(id, name, _) = &**callee {
                    if let Some(name) = self.global_fns.get(id) {
                        is_global = true;
                        global_name = name.clone();
                    } else if let Some(ty) = self.global_env.get(id) {
                        if matches!(ty, Ty::Struct(_) | Ty::Class(_)) {
                            is_instantiation = true;
                            inst_ty = Some(ty.clone());
                            let nid = match ty { Ty::Struct(i) => i, Ty::Class(i) => i, _ => unreachable!() };
                            for (name, &(tid, _)) in self.named_types {
                                if *nid == tid { struct_name = name.clone(); break; }
                            }
                        } else if let Ty::Function(_, ret) = ty {
                            if let Ty::Enum(eid) = **ret {
                                is_global = true;
                                if let Some(variants) = self.enum_defs.get(&eid) {
                                    for v in variants {
                                        if v.id == *id {
                                            global_name = v.name.clone();
                                        }
                                    }
                                }
                            }
                        }
                    } else if let Some(&(nid, kind)) = self.named_types.get(name) {
                        if kind == 0 || kind == 1 {
                            is_instantiation = true;
                            inst_ty = Some(if kind == 0 { Ty::Struct(nid) } else { Ty::Class(nid) });
                            struct_name = name.clone();
                        }
                    } else if let Some(expected) = &self.current_expected_ty {
                        if let Ty::Struct(nid) | Ty::Class(nid) = expected {
                            is_instantiation = true;
                            inst_ty = Some(expected.clone());
                            for (t_name, &(tid, _)) in self.named_types {
                                if *nid == tid { struct_name = t_name.clone(); break; }
                            }
                        }
                    }
                } else if let Expr::MemberAccess { object, member, .. } = &**callee {
                    let mut is_static_method = false;
                    if let Expr::Ident(id, name, _) = &**object {
                        if let Some(ty) = self.global_env.get(id) {
                            if matches!(ty, Ty::Struct(_) | Ty::Class(_)) {
                                let mut type_name = "";
                                let nid = match ty { Ty::Struct(i) => i, Ty::Class(i) => i, _ => unreachable!() };
                                for (name, &(tid, _)) in self.named_types {
                                    if *nid == tid { type_name = name; break; }
                                }
                                let static_name = format!("{}_{}", type_name, member);
                                if self.methods_env.contains_key(&static_name) {
                                    is_global = true;
                                    global_name = static_name;
                                    is_static_method = true;
                                }
                            } else if let Ty::Enum(_) = ty {
                                is_global = true;
                                global_name = member.clone();
                                is_static_method = true;
                            }
                        } else if let Some(&(nid, kind)) = self.named_types.get(name) {
                            if kind == 0 || kind == 1 {
                                let mut type_name = "";
                                for (name, &(tid, _)) in self.named_types {
                                    if nid == tid { type_name = name; break; }
                                }
                                let static_name = format!("{}_{}", type_name, member);
                                if self.methods_env.contains_key(&static_name) {
                                    is_global = true;
                                    global_name = static_name;
                                    is_static_method = true;
                                }
                            } else if kind == 2 {
                                is_global = true;
                                global_name = member.clone();
                                is_static_method = true;
                            }
                        } else if let Some(expected) = &self.current_expected_ty {
                            if let Ty::Enum(eid) = expected {
                                is_global = true;
                                global_name = member.clone();
                                is_static_method = true;
                            } else if let Ty::Struct(nid) | Ty::Class(nid) = expected {
                                let mut type_name = "";
                                for (name, &(tid, _)) in self.named_types {
                                    if *nid == tid { type_name = name; break; }
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
                        let obj_local = self.build_expr(object);
                        let obj_ty = self.locals[obj_local.0 as usize].clone();
                        if let Ty::Struct(id) | Ty::Class(id) = obj_ty {
                            let mut type_name = "";
                            for (name, &(tid, _)) in self.named_types {
                                if id == tid { type_name = name; break; }
                            }
                            let method_name = format!("{}_{}", type_name, member);
                            if self.methods_env.contains_key(&method_name) {
                                global_name = method_name;
                                
                                // Insert the object as the first argument (self)
                                let mut new_arg_locals = vec![obj_local];
                                for (_, arg) in args {
                                    new_arg_locals.push(self.build_expr(arg));
                                }
                                
                                let mut ret_ty = Ty::Int;
                                if let Some(Ty::Function(_, ret)) = self.methods_env.get(&global_name) {
                                    ret_ty = *ret.clone();
                                }
                                let temp = self.new_local(ret_ty);
                                self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::GlobalCall(global_name, new_arg_locals)));
                                return temp;
                            }
                        }
                    }
                }
                
                let mut arg_locals = Vec::new();
                for (_, arg) in args {
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
                        
                        let ret_dummy = self.new_local(Ty::Int);
                        self.push_stmt(Statement::Assign(Lvalue::Local(ret_dummy), Rvalue::GlobalCall(init_name, call_args)));
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
                    
                    if let Expr::Ident(id, name, _) = &**callee {
                        if let Some(Ty::Function(_, ret)) = self.global_env.get(id) {
                            if let Ty::Enum(eid) = **ret {
                                is_enum_variant = true;
                                enum_id = Some(eid);
                            } else {
                                ret_ty = *ret.clone();
                            }
                        }
                    } else if let Expr::MemberAccess { object, member, .. } = &**callee {
                        if let Expr::Ident(id, name, _) = &**object {
                            if let Some(Ty::Enum(eid)) = self.global_env.get(id) {
                                is_enum_variant = true;
                                enum_id = Some(*eid);
                            } else if let Some(&(eid, 2)) = self.named_types.get(name) {
                                is_enum_variant = true;
                                enum_id = Some(eid);
                            } else if let Some(expected) = &self.current_expected_ty {
                                if let Ty::Enum(eid) = expected {
                                    is_enum_variant = true;
                                    enum_id = Some(*eid);
                                } else if let Some(Ty::Function(_, ret)) = self.methods_env.get(&global_name) {
                                    ret_ty = *ret.clone();
                                }
                            } else if let Some(Ty::Function(_, ret)) = self.methods_env.get(&global_name) {
                                ret_ty = *ret.clone();
                            }
                        } else if let Some(Ty::Function(_, ret)) = self.methods_env.get(&global_name) {
                            ret_ty = *ret.clone();
                        }
                    } else if let Some(Ty::Function(_, ret)) = self.methods_env.get(&global_name) {
                        ret_ty = *ret.clone();
                    }
                    
                    if is_enum_variant {
                        let temp = self.new_local(Ty::Enum(enum_id.unwrap()));
                        self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::InstantiateEnum(enum_id.unwrap(), global_name.clone(), arg_locals)));
                        return temp;
                    }
                    
                    let temp = self.new_local(ret_ty);
                    self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::GlobalCall(global_name, arg_locals)));
                    return temp;
                }
                
                let callee_local = self.build_expr(callee);
                let temp = self.new_local(Ty::Int); // Placeholder MVP return ty
                self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::Call(callee_local, arg_locals)));
                temp
            }
            Expr::BuiltinCall(name, args, _) => {
                let mut arg_locals = Vec::new();
                for arg in args {
                    arg_locals.push(self.build_expr(arg));
                }
                let temp = self.new_local(Ty::Int);
                self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::BuiltinCall(name.clone(), arg_locals)));
                temp
            }
            Expr::If { cond, then_block, else_block, .. } => {
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
                if self.blocks[self.current_block.0 as usize].terminator.is_none() {
                    self.blocks[self.current_block.0 as usize].terminator = Some(Terminator::Goto(merge_bb));
                }

                self.current_block = else_bb;
                if let Some(eb) = else_block {
                    self.build_block(eb);
                }
                if self.blocks[self.current_block.0 as usize].terminator.is_none() {
                    self.blocks[self.current_block.0 as usize].terminator = Some(Terminator::Goto(merge_bb));
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
                if self.blocks[self.current_block.0 as usize].terminator.is_none() {
                    self.blocks[self.current_block.0 as usize].terminator = Some(Terminator::Goto(cond_bb));
                }

                self.current_block = merge_bb;
                self.new_local(Ty::Int)
            }
            Expr::Match { subject, arms, .. } => {
                let subject_local = self.build_expr(subject);
                let temp = self.new_local(Ty::Int); // MVP return Ty
                
                let tag_local = self.new_local(Ty::Int);
                self.push_stmt(Statement::Assign(Lvalue::Local(tag_local), Rvalue::EnumTag(subject_local)));
                
                let merge_bb = self.new_block();
                let mut current_bb = self.current_block;
                
                for arm in arms {
                    let next_arm_bb = self.new_block();
                    let body_bb = self.new_block();
                    
                    let mut arm_tag = 0;
                    if let pace_hir::Pattern::Variant { name, .. } = &arm.pattern {
                        let subject_ty = self.locals[subject_local.0 as usize].clone();
                        if let Ty::Enum(enum_id) = subject_ty {
                            if let Some(variants) = self.enum_defs.get(&enum_id) {
                                for (i, v) in variants.iter().enumerate() {
                                    if v.name == *name {
                                        arm_tag = i as i32;
                                        break;
                                    }
                                }
                            }
                        }
                    } else if let pace_hir::Pattern::Ident(id, _, _) = &arm.pattern {
                        let subject_ty = self.locals[subject_local.0 as usize].clone();
                        let f_local = self.new_local(subject_ty);
                        self.hir_to_local.insert(*id, f_local);
                        self.push_stmt(Statement::Assign(Lvalue::Local(f_local), Rvalue::Use(subject_local)));
                        
                        self.blocks[current_bb.0 as usize].terminator = Some(Terminator::Goto(body_bb));
                        self.current_block = body_bb;
                        let arm_local = self.build_expr(&arm.body);
                        self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::Use(arm_local)));
                        if self.blocks[self.current_block.0 as usize].terminator.is_none() {
                            self.blocks[self.current_block.0 as usize].terminator = Some(Terminator::Goto(merge_bb));
                        }
                        current_bb = next_arm_bb;
                        self.current_block = next_arm_bb;
                        continue;
                    } else if let pace_hir::Pattern::CatchAll(_) = &arm.pattern {
                        self.blocks[current_bb.0 as usize].terminator = Some(Terminator::Goto(body_bb));
                        self.current_block = body_bb;
                        let arm_local = self.build_expr(&arm.body);
                        self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::Use(arm_local)));
                        if self.blocks[self.current_block.0 as usize].terminator.is_none() {
                            self.blocks[self.current_block.0 as usize].terminator = Some(Terminator::Goto(merge_bb));
                        }
                        current_bb = next_arm_bb;
                        self.current_block = next_arm_bb;
                        continue;
                    }
                    
                    let cond_local = self.new_local(Ty::Int);
                    let tag_const_local = self.new_local(Ty::Int);
                    self.push_stmt(Statement::Assign(Lvalue::Local(tag_const_local), Rvalue::IntConstant(arm_tag.to_string())));
                    self.push_stmt(Statement::Assign(Lvalue::Local(cond_local), Rvalue::BinaryOp(pace_ast::BinaryOp::EqEq, tag_local, tag_const_local)));
                    
                    self.blocks[current_bb.0 as usize].terminator = Some(Terminator::Branch {
                        cond: cond_local,
                        then_block: body_bb,
                        else_block: next_arm_bb,
                    });
                    
                    self.current_block = body_bb;
                    
                    if let pace_hir::Pattern::Variant { name, fields, .. } = &arm.pattern {
                        if let Some(pfields) = fields {
                            let subject_ty = self.locals[subject_local.0 as usize].clone();
                            if let Ty::Enum(enum_id) = subject_ty {
                                if let Some(variants) = self.enum_defs.get(&enum_id) {
                                    if let Some(v) = variants.iter().find(|v| v.name == *name) {
                                        if let Some(vfields) = &v.fields {
                                            for (i, (pf_id, _, _)) in pfields.iter().enumerate() {
                                                if i < vfields.len() {
                                                    let fty = self.local_types.get(pf_id).unwrap_or(&Ty::Int).clone();
                                                    let f_local = self.new_local(fty);
                                                    self.hir_to_local.insert(*pf_id, f_local);
                                                    self.push_stmt(Statement::Assign(Lvalue::Local(f_local), Rvalue::EnumFieldAccess(subject_local, name.clone(), vfields[i].0.clone())));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    let arm_local = self.build_expr(&arm.body);
                    self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::Use(arm_local)));
                    
                    if self.blocks[self.current_block.0 as usize].terminator.is_none() {
                        self.blocks[self.current_block.0 as usize].terminator = Some(Terminator::Goto(merge_bb));
                    }
                    
                    current_bb = next_arm_bb;
                    self.current_block = next_arm_bb;
                }
                
                if self.blocks[current_bb.0 as usize].terminator.is_none() {
                     self.blocks[current_bb.0 as usize].terminator = Some(Terminator::Goto(merge_bb));
                }
                
                self.current_block = merge_bb;
                temp
            }
            Expr::Assign { target, value, .. } => {
                let lvalue = match &**target {
                    Expr::Ident(id, name, _) => {
                        Lvalue::Local(*self.hir_to_local.get(id).unwrap())
                    }
                    Expr::MemberAccess { object, member, .. } => {
                        let obj_local = self.build_expr(object);
                        Lvalue::FieldAccess(obj_local, member.clone())
                    }
                    _ => panic!("Invalid assignment target"),
                };
                let lval_ty = match &lvalue {
                    Lvalue::Local(l) => self.locals[l.0 as usize].clone(),
                    Lvalue::FieldAccess(obj, member) => {
                        let mut field_ty = Ty::Int;
                        let obj_ty = self.locals[obj.0 as usize].clone();
                        let mut found = false;
                        if let Ty::Struct(id) | Ty::Class(id) = obj_ty {
                            let defs = if matches!(obj_ty, Ty::Struct(_)) { self.struct_defs.get(&id) } else { self.class_defs.get(&id) };
                            if let Some(fields) = defs {
                                for (n, t) in fields {
                                    if n == member { field_ty = t.clone(); found = true; break; }
                                }
                            }
                        }
                        if !found { panic!("Field not found"); }
                        field_ty
                    },
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
        let mut global_fns = HashMap::new();
        for decl in &program.declarations {
            if let pace_hir::Decl::Function { id, name, .. } = decl {
                global_fns.insert(*id, name.clone());
            } else if let pace_hir::Decl::Struct { methods, .. } = decl {
                for method in methods {
                    if let pace_hir::Decl::Function { id, name, .. } = method {
                        global_fns.insert(*id, name.clone());
                    }
                }
            } else if let pace_hir::Decl::Class { methods, .. } = decl {
                for method in methods {
                    if let pace_hir::Decl::Function { id, name, .. } = method {
                        global_fns.insert(*id, name.clone());
                    }
                }
            }
        }

        let mut functions = Vec::new();
        let mut global_vars = Vec::new();
        let mut main_builder = MirBuilder::new(global_fns.clone(), &tc.struct_defs, &tc.class_defs, &tc.enum_defs, &tc.env, &tc.local_types, &tc.named_types, &tc.static_fields_env, &tc.methods_env);
        let mut _main_last_local = Local(0);

        for decl in &program.declarations {
            match decl {
                pace_hir::Decl::Let { id, value, .. } | pace_hir::Decl::Var { id, value, .. } => {
                    if let Some(val) = value {
                        let rval_local = main_builder.build_expr(val);
                        let var_ty = main_builder.locals[rval_local.0 as usize].clone();
                        let var_local = main_builder.new_local(var_ty.clone());
                        main_builder.hir_to_local.insert(*id, var_local);
                        main_builder.push_stmt(Statement::Assign(Lvalue::Local(var_local), Rvalue::Use(rval_local)));
                        if matches!(var_ty, Ty::Class(_)) {
                            main_builder.push_stmt(Statement::Retain(Lvalue::Local(var_local)));
                        }
                        _main_last_local = rval_local;
                    } else {
                        // Uninitialized global variable
                        let var_ty = main_builder.local_types.get(id).expect("Type not found in local_types").clone();
                        let var_local = main_builder.new_local(var_ty.clone());
                        main_builder.hir_to_local.insert(*id, var_local);
                        // No assignment, C handles initialization
                    }
                }
                pace_hir::Decl::Const { id, name, value, .. } => {
                    let global_name = format!("{}", name);
                    let ty = tc.env.get(id).cloned().unwrap_or(Ty::Int);
                    global_vars.push((global_name.clone(), ty.clone()));
                    let rval_local = main_builder.build_expr(value);
                    main_builder.push_stmt(Statement::GlobalWrite(global_name, rval_local));
                }
                pace_hir::Decl::Struct { name, static_fields, const_fields, methods, .. } | pace_hir::Decl::Class { name, static_fields, const_fields, methods, .. } => {
                    for (sf_name, sf_ty, sf_expr) in static_fields {
                        let global_name = format!("{}_{}", name, sf_name);
                        let ty = tc.get_type(sf_ty).unwrap_or(Ty::Int);
                        global_vars.push((global_name.clone(), ty.clone()));
                        
                        let rval_local = main_builder.build_expr(sf_expr);
                        main_builder.push_stmt(Statement::GlobalWrite(global_name, rval_local));
                    }
                    for (cf_name, cf_ty, cf_expr) in const_fields {
                        let global_name = format!("{}_{}", name, cf_name);
                        let ty = tc.get_type(cf_ty).unwrap_or(Ty::Int);
                        global_vars.push((global_name.clone(), ty.clone()));
                        
                        let rval_local = main_builder.build_expr(cf_expr);
                        main_builder.push_stmt(Statement::GlobalWrite(global_name, rval_local));
                    }
                    for method in methods {
                        if let pace_hir::Decl::Function { name, params, return_type, body, .. } = method {
                            let mut fn_builder = MirBuilder::new(
                                global_fns.clone(),
                                &tc.struct_defs,
                                &tc.class_defs,
                                &tc.enum_defs,
                                &tc.env,
                                &tc.local_types,
                                &tc.named_types,
                                &tc.static_fields_env,
                                &tc.methods_env,
                            );
                            let mut mir_params = Vec::new();
                            for (param_id, _, pty) in params {
                                let ty = tc.get_type(pty).unwrap_or(Ty::Int);
                                let local = fn_builder.new_local(ty);
                                fn_builder.hir_to_local.insert(*param_id, local);
                                mir_params.push(local);
                            }
                            
                            let ret_ty = if let Some(rty) = return_type {
                                tc.get_type(rty).unwrap_or(Ty::Int)
                            } else {
                                Ty::Int
                            };
                            fn_builder.build_block(body);
                            let fn_body = fn_builder.finish(&mir_params);
                            
                            functions.push(MirFunction {
                                name: name.clone(),
                                params: mir_params,
                                return_type: ret_ty,
                                body: fn_body,
                            });
                        }
                    }
                }
                pace_hir::Decl::Expr(expr, _) => {
                    let rval_local = main_builder.build_expr(expr);
                    _main_last_local = rval_local;
                }
                pace_hir::Decl::Function { name, id: _, params, return_type, body, .. } => {
                    let mut fn_builder = MirBuilder::new(
                        global_fns.clone(),
                        &tc.struct_defs,
                        &tc.class_defs,
                        &tc.enum_defs,
                        &tc.env,
                        &tc.local_types,
                        &tc.named_types,
                        &tc.static_fields_env,
                        &tc.methods_env,
                    );
                    let mut mir_params = Vec::new();
                    for (param_id, _, pty) in params {
                        let ty = tc.get_type(pty).unwrap_or(Ty::Int);
                        let local = fn_builder.new_local(ty);
                        fn_builder.hir_to_local.insert(*param_id, local);
                        mir_params.push(local);
                    }
                    
                    let ret_ty = if let Some(rty) = return_type {
                        tc.get_type(rty).unwrap_or(Ty::Int)
                    } else {
                        Ty::Int
                    };
                    fn_builder.build_block(body);
                    let fn_body = fn_builder.finish(&mir_params);
                    functions.push(MirFunction {
                        name: name.clone(),
                        params: mir_params,
                        return_type: ret_ty,
                        body: fn_body,
                    });
                }
                pace_hir::Decl::Enum { .. } => {}
            }
        }

        let mut resolved_enum_defs = std::collections::HashMap::new();
        for (id, variants) in &tc.enum_defs {
            let mut res_variants = Vec::new();
            for v in variants {
                let res_fields = v.fields.as_ref().map(|fields| {
                    fields.iter().map(|(name, ty)| (name.clone(), tc.get_type(ty).unwrap_or(Ty::Int))).collect()
                });
                res_variants.push((v.name.clone(), res_fields));
            }
            resolved_enum_defs.insert(*id, res_variants);
        }

        MirProgram {
            functions,
            main_body: main_builder.finish(&[]),
            struct_defs: tc.struct_defs.clone(),
            class_defs: tc.class_defs.clone(),
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
            self.blocks[current_bb].statements.push(Statement::Assign(Lvalue::Local(temp), Rvalue::IntConstant("0".to_string())));
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
            if matches!(ty, Ty::Class(_)) && !return_locals.contains(&local) && !params.contains(&local) {
                self.blocks[current_bb].statements.push(Statement::Release(Lvalue::Local(local)));
            }
        }

        MirBody {
            blocks: self.blocks,
            locals: self.locals,
        }
    }
}
