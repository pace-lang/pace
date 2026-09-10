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
