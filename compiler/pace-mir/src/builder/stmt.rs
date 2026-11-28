use crate::mir::*;
use pace_ty::Ty;

use super::MirBuilder;

impl<'a> MirBuilder<'a> {
    pub fn build_block(&mut self, block: &pace_hir::Block) {
        self.scope_stack.push(Vec::new());
        let is_inner_scope = self.scope_stack.len() > 1;
        for stmt in &block.statements {
            match stmt {
                pace_hir::Stmt::Let { id, value, .. } => {
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
                        let var_local = self.new_local(var_ty.clone());
                        self.hir_to_local.insert(*id, var_local);
                    }
                }
                pace_hir::Stmt::Var { id, value, .. } => {
                    let var_ty = self
                        .local_types
                        .get(id)
                        .expect("Variable type not found in local_types environment")
                        .clone();
                    let is_boxed = self.captured_vars_in_scope.contains(id);
                    if let Some(val) = value {
                        let prev = self.current_expected_ty.take();
                        self.current_expected_ty = Some(var_ty.clone());
                        let rval_local = self.build_expr(val);
                        self.current_expected_ty = prev;
                        if is_boxed {
                            if matches!(var_ty, Ty::Class(_) | Ty::List(_) | Ty::Map(_, _) | Ty::Set(_) | Ty::StringBuilder) {
                                self.push_stmt(Statement::Retain(Lvalue::Local(rval_local)));
                            }
                            let cell_local = self.new_local(Ty::Optional(Box::new(Ty::Void)));
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(cell_local),
                                Rvalue::BoxNew(rval_local),
                            ));
                            self.boxed_var_cells.insert(*id, (cell_local, var_ty));
                        } else {
                            let var_local = self.new_local(var_ty.clone());
                            self.hir_to_local.insert(*id, var_local);
                            self.push_stmt(Statement::Assign(
                                Lvalue::Local(var_local),
                                Rvalue::Use(rval_local),
                            ));
                            if matches!(var_ty, Ty::Class(_) | Ty::List(_) | Ty::Map(_, _) | Ty::Set(_) | Ty::StringBuilder) {
                                self.push_stmt(Statement::Retain(Lvalue::Local(var_local)));
                            }
                        }
                    } else if is_boxed {
                        let init_local = self.new_local(var_ty.clone());
                        let cell_local = self.new_local(Ty::Optional(Box::new(Ty::Void)));
                        self.push_stmt(Statement::Assign(
                            Lvalue::Local(cell_local),
                            Rvalue::BoxNew(init_local),
                        ));
                        self.boxed_var_cells.insert(*id, (cell_local, var_ty));
                    } else {
                        let var_local = self.new_local(var_ty.clone());
                        self.hir_to_local.insert(*id, var_local);
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

}
