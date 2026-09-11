use std::collections::HashMap;
use pace_hir::{Expr, Stmt as HirStmt, HirId};
use crate::mir::*;
use pace_ty::{Ty, TypeChecker};

pub struct MirBuilder<'a> {
    pub blocks: Vec<BasicBlock>,
    pub current_block: BasicBlockId,
    pub locals: Vec<Ty>,
    pub hir_to_local: HashMap<HirId, Local>,
    pub global_fns: HashMap<HirId, String>,
    pub struct_defs: &'a HashMap<HirId, Vec<(String, Ty)>>,
    pub class_defs: &'a HashMap<HirId, Vec<(String, Ty)>>,
    pub global_env: &'a HashMap<HirId, Ty>,
    pub named_types: &'a HashMap<String, HirId>,
}

impl<'a> MirBuilder<'a> {
    pub fn new(
        global_fns: HashMap<HirId, String>,
        struct_defs: &'a HashMap<HirId, Vec<(String, Ty)>>,
        class_defs: &'a HashMap<HirId, Vec<(String, Ty)>>,
        global_env: &'a HashMap<HirId, Ty>,
        named_types: &'a HashMap<String, HirId>,
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
            global_fns,
            struct_defs,
            class_defs,
            global_env,
            named_types,
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
                pace_hir::Stmt::Let { id, value, .. } => {
                    let rval_local = self.build_expr(value);
                    let var_ty = self.locals[rval_local.0 as usize].clone();
                    let var_local = self.new_local(var_ty.clone());
                    self.hir_to_local.insert(*id, var_local);
                    self.push_stmt(Statement::Assign(Lvalue::Local(var_local), Rvalue::Use(rval_local)));
                    if matches!(var_ty, Ty::Class(_)) {
                        self.push_stmt(Statement::Retain(Lvalue::Local(var_local)));
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
            Expr::Ident(id, _) => {
                let local = *self.hir_to_local.get(id).expect("Local not found");
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
                let temp = self.new_local(Ty::Int);
                self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::IntConstant(val.clone())));
                temp
            }
            Expr::StringLiteral(val, _) => {
                let temp = self.new_local(Ty::String);
                self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::StringConstant(val.clone())));
                temp
            }
            Expr::MemberAccess { object, member, .. } => {
                let obj_local = self.build_expr(object);
                // In MVP we assume fields are integers, ideally we should lookup struct_defs.
                let temp = self.new_local(Ty::Int);
                self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::FieldAccess(obj_local, member.clone())));
                // If it was a class, we'd retain here.
                temp
            }
            Expr::Call { callee, args, .. } => {
                let mut is_instantiation = false;
                let mut inst_ty = None;
                let mut struct_name = String::new();
                
                if let Expr::Ident(id, _) = &**callee {
                    if let Some(ty) = self.global_env.get(id) {
                        if matches!(ty, Ty::Struct(_) | Ty::Class(_)) {
                            is_instantiation = true;
                            inst_ty = Some(ty.clone());
                            let nid = match ty { Ty::Struct(i) => i, Ty::Class(i) => i, _ => unreachable!() };
                            for (name, tid) in self.named_types {
                                if nid == tid { struct_name = name.clone(); break; }
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
                
                if let Expr::Ident(id, _) = &**callee {
                    if let Some(func_name) = self.global_fns.get(id).cloned() {
                        let ret_ty = if let Some(Ty::Function(_, ret)) = self.global_env.get(id) {
                            *ret.clone()
                        } else {
                            Ty::Int
                        };
                        let temp = self.new_local(ret_ty);
                        self.push_stmt(Statement::Assign(Lvalue::Local(temp), Rvalue::GlobalCall(func_name, arg_locals)));
                        return temp;
                    }
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
            Expr::Assign { target, value, .. } => {
                let rval = self.build_expr(value);
                let lvalue = match &**target {
                    Expr::Ident(id, _) => {
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
                    Lvalue::FieldAccess(obj, _) => Ty::Int, // MVP: assumes integer field
                };
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

    pub fn build_program(program: &pace_hir::Program, tc: &TypeChecker) -> MirProgram {
        let mut global_fns = HashMap::new();
        for decl in &program.declarations {
            if let pace_hir::Decl::Function { name, id, .. } = decl {
                global_fns.insert(*id, name.clone());
            }
        }

        let mut functions = Vec::new();
        let mut main_builder = MirBuilder::new(global_fns.clone(), &tc.struct_defs, &tc.class_defs, &tc.env, &tc.named_types);
        let mut _main_last_local = Local(0);

        for decl in &program.declarations {
            match decl {
                pace_hir::Decl::Let { id, value, .. } => {
                    let rval_local = main_builder.build_expr(value);
                    let var_ty = main_builder.locals[rval_local.0 as usize].clone();
                    let var_local = main_builder.new_local(var_ty.clone());
                    main_builder.hir_to_local.insert(*id, var_local);
                    main_builder.push_stmt(Statement::Assign(Lvalue::Local(var_local), Rvalue::Use(rval_local)));
                    if matches!(var_ty, Ty::Class(_)) {
                        main_builder.push_stmt(Statement::Retain(Lvalue::Local(var_local)));
                    }
                    _main_last_local = rval_local;
                }
                pace_hir::Decl::Struct { methods, .. } | pace_hir::Decl::Class { methods, .. } => {
                    for method in methods {
                        if let pace_hir::Decl::Function { name, params, return_type, body, .. } = method {
                            let mut fn_builder = MirBuilder::new(
                                global_fns.clone(),
                                &tc.struct_defs,
                                &tc.class_defs,
                                &tc.env,
                                &tc.named_types,
                            );
                            let mut mir_params = Vec::new();
                            for (param_id, _, pty) in params {
                                let ty = tc.resolve_type(pty).unwrap_or(Ty::Int);
                                let local = fn_builder.new_local(ty);
                                fn_builder.hir_to_local.insert(*param_id, local);
                                mir_params.push(local);
                            }
                            
                            let ret_ty = if let Some(rty) = return_type {
                                tc.resolve_type(rty).unwrap_or(Ty::Int)
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
                pace_hir::Decl::Function { name, id, params, return_type, body, .. } => {
                    let mut fn_builder = MirBuilder::new(
                        global_fns.clone(),
                        &tc.struct_defs,
                        &tc.class_defs,
                        &tc.env,
                        &tc.named_types,
                    );
                    let mut mir_params = Vec::new();
                    for (param_id, _, pty) in params {
                        let ty = tc.resolve_type(pty).unwrap_or(Ty::Int);
                        let local = fn_builder.new_local(ty);
                        fn_builder.hir_to_local.insert(*param_id, local);
                        mir_params.push(local);
                    }
                    
                    let ret_ty = if let Some(rty) = return_type {
                        tc.resolve_type(rty).unwrap_or(Ty::Int)
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

        MirProgram {
            functions,
            main_body: main_builder.finish(&[]),
            struct_defs: tc.struct_defs.clone(),
            class_defs: tc.class_defs.clone(),
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
