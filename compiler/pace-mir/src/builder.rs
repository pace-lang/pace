use crate::mir::*;
use pace_hir::{Expr, HirId};
use std::collections::HashMap;

pub struct MirBuilder {
    blocks: Vec<BasicBlock>,
    current_block: BasicBlockId,
    next_local: u32,
    hir_to_local: HashMap<HirId, Local>,
}

impl MirBuilder {
    pub fn new() -> Self {
        let initial_block = BasicBlock {
            statements: Vec::new(),
            terminator: None,
        };
        Self {
            blocks: vec![initial_block],
            current_block: BasicBlockId(0),
            next_local: 0,
            hir_to_local: HashMap::new(),
        }
    }

    fn new_local(&mut self) -> Local {
        let l = Local(self.next_local);
        self.next_local += 1;
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
                    let var_local = self.new_local();
                    self.hir_to_local.insert(*id, var_local);
                    self.push_stmt(Statement::Assign(var_local, Rvalue::Use(rval_local)));
                }
                pace_hir::Stmt::ExprStmt(expr, _) => {
                    self.build_expr(expr);
                }
                pace_hir::Stmt::Return(_, _) => {
                    // Ignored for MVP v0.1 since we only have `main` block
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
            Expr::IntLiteral(val, _) => {
                let temp = self.new_local();
                self.push_stmt(Statement::Assign(temp, Rvalue::IntConstant(val.clone())));
                temp
            }
            Expr::StringLiteral(val, _) => {
                let temp = self.new_local();
                self.push_stmt(Statement::Assign(temp, Rvalue::StringConstant(val.clone())));
                // Because string is a reference type in Pace, ARC injects Retain!
                self.push_stmt(Statement::Retain(temp));
                temp
            }
            Expr::Ident(id, _) => {
                if let Some(local) = self.hir_to_local.get(id) {
                    *local
                } else {
                    let local = self.new_local();
                    self.hir_to_local.insert(*id, local);
                    local
                }
            }
            Expr::Binary { left, op, right, .. } => {
                let lhs = self.build_expr(left);
                let rhs = self.build_expr(right);
                
                let temp = self.new_local();
                self.push_stmt(Statement::Assign(temp, Rvalue::BinaryOp(*op, lhs, rhs)));
                temp
            }
            Expr::MemberAccess { .. } => {
                self.new_local() // Mocked for MVP v0.1
            }
            Expr::Call { callee, args, .. } => {
                let callee_local = self.build_expr(callee);
                let mut arg_locals = Vec::new();
                for arg in args {
                    arg_locals.push(self.build_expr(arg));
                }
                let temp = self.new_local();
                self.push_stmt(Statement::Assign(temp, Rvalue::Call(callee_local, arg_locals)));
                temp
            }
            Expr::BuiltinCall(name, args, _) => {
                let mut arg_locals = Vec::new();
                for arg in args {
                    arg_locals.push(self.build_expr(arg));
                }
                let temp = self.new_local();
                self.push_stmt(Statement::Assign(temp, Rvalue::BuiltinCall(name.clone(), arg_locals)));
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
                self.blocks[self.current_block.0 as usize].terminator = Some(Terminator::Goto(merge_bb));

                self.current_block = else_bb;
                if let Some(eb) = else_block {
                    self.build_block(eb);
                }
                self.blocks[self.current_block.0 as usize].terminator = Some(Terminator::Goto(merge_bb));

                self.current_block = merge_bb;
                self.new_local()
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
                self.blocks[self.current_block.0 as usize].terminator = Some(Terminator::Goto(cond_bb));

                self.current_block = merge_bb;
                self.new_local()
            }
            Expr::Assign { target, value, .. } => {
                let rval = self.build_expr(value);
                let target_local = *self.hir_to_local.get(target).unwrap();
                self.push_stmt(Statement::Assign(target_local, Rvalue::Use(rval)));
                target_local
            }
        }
    }

    pub fn build_program(mut self, program: &pace_hir::Program) -> MirBody {
        let mut last_local = Local(0); 
        for decl in &program.declarations {
            match decl {
                pace_hir::Decl::Let { id, value, .. } => {
                    let rval_local = self.build_expr(value);
                    let var_local = self.new_local();
                    self.hir_to_local.insert(*id, var_local);
                    self.push_stmt(Statement::Assign(var_local, Rvalue::Use(rval_local)));
                    last_local = var_local;
                }
                pace_hir::Decl::Struct { .. } | pace_hir::Decl::Class { .. } => {
                    // Type definitions emit no executable instructions at the top level
                }
                pace_hir::Decl::Expr(expr, _) => {
                    let rval_local = self.build_expr(expr);
                    last_local = rval_local;
                }
            }
        }
        self.finish(last_local)
    }

    pub fn finish(mut self, return_val: Local) -> MirBody {
        let idx = self.current_block.0 as usize;
        self.blocks[idx].terminator = Some(Terminator::Return(return_val));

        MirBody {
            blocks: self.blocks,
            locals: self.next_local,
        }
    }
}
