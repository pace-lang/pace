use crate::mir::{BasicBlock, Constant, Local, Lvalue, MirFunction, MirProgram, Rvalue, Statement};
use pace_ast::BinaryOp;
use std::collections::HashMap;

pub struct ConstantFolder;

impl Default for ConstantFolder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConstantFolder {
    pub fn new() -> Self {
        Self
    }

    pub fn optimize_program(&mut self, program: &mut MirProgram) {
        for func in &mut program.functions {
            self.optimize_function(func);
        }

        // Also optimize main body
        for block in &mut program.main_body.blocks {
            self.optimize_block(block);
        }
    }

    fn optimize_function(&mut self, func: &mut MirFunction) {
        for block in &mut func.body.blocks {
            self.optimize_block(block);
        }
    }

    fn optimize_block(&mut self, block: &mut BasicBlock) {
        let mut known_constants: HashMap<Local, Constant> = HashMap::new();

        for stmt in &mut block.statements {
            if let Statement::Assign(Lvalue::Local(local), rvalue) = stmt {
                // First, check if we can fold the rvalue based on known constants
                self.fold_rvalue(rvalue, &known_constants);

                // Then, if the rvalue is a constant, record it
                if let Rvalue::Constant(c) = rvalue {
                    known_constants.insert(*local, c.clone());
                } else {
                    // If reassigned to non-constant, remove from known constants
                    known_constants.remove(local);
                }
            }
        }
    }

    fn fold_rvalue(&self, rvalue: &mut Rvalue, known_constants: &HashMap<Local, Constant>) {
        if let Rvalue::BinaryOp(op, l1, l2) = rvalue
            && let (Some(c1), Some(c2)) = (known_constants.get(l1), known_constants.get(l2))
            && let Some(folded) = self.eval_binary_op(op, c1, c2)
        {
            *rvalue = Rvalue::Constant(folded);
        }
    }

    fn eval_binary_op(&self, op: &BinaryOp, c1: &Constant, c2: &Constant) -> Option<Constant> {
        match (c1, c2) {
            (Constant::Int(i1), Constant::Int(i2)) => {
                if let (Ok(v1), Ok(v2)) = (i1.parse::<i64>(), i2.parse::<i64>()) {
                    match op {
                        BinaryOp::Add => Some(Constant::Int((v1 + v2).to_string())),
                        BinaryOp::Sub => Some(Constant::Int((v1 - v2).to_string())),
                        BinaryOp::Mul => Some(Constant::Int((v1 * v2).to_string())),
                        BinaryOp::Div if v2 != 0 => Some(Constant::Int((v1 / v2).to_string())),

                        BinaryOp::EqEq => Some(Constant::Bool(v1 == v2)),
                        BinaryOp::NotEq => Some(Constant::Bool(v1 != v2)),
                        BinaryOp::Lt => Some(Constant::Bool(v1 < v2)),
                        BinaryOp::LtEq => Some(Constant::Bool(v1 <= v2)),
                        BinaryOp::Gt => Some(Constant::Bool(v1 > v2)),
                        BinaryOp::GtEq => Some(Constant::Bool(v1 >= v2)),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            (Constant::Bool(b1), Constant::Bool(b2)) => match op {
                BinaryOp::And => Some(Constant::Bool(*b1 && *b2)),
                BinaryOp::Or => Some(Constant::Bool(*b1 || *b2)),
                BinaryOp::EqEq => Some(Constant::Bool(*b1 == *b2)),
                BinaryOp::NotEq => Some(Constant::Bool(*b1 != *b2)),
                _ => None,
            },
            // Add float folding if needed, but this is a good start
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::Terminator;

    #[test]
    fn test_constant_folding_add() {
        let mut block = BasicBlock {
            statements: vec![
                Statement::Assign(
                    Lvalue::Local(Local(0)),
                    Rvalue::Constant(Constant::Int("5".to_string())),
                ),
                Statement::Assign(
                    Lvalue::Local(Local(1)),
                    Rvalue::Constant(Constant::Int("3".to_string())),
                ),
                Statement::Assign(
                    Lvalue::Local(Local(2)),
                    Rvalue::BinaryOp(BinaryOp::Add, Local(0), Local(1)),
                ),
            ],
            terminator: Some(Terminator::Return(Local(2))),
        };

        let mut folder = ConstantFolder::new();
        folder.optimize_block(&mut block);

        match &block.statements[2] {
            Statement::Assign(_, Rvalue::Constant(Constant::Int(val))) => {
                assert_eq!(val, "8");
            }
            _ => panic!("Expected folded constant!"),
        }
    }

    #[test]
    fn test_constant_folding_bool() {
        let mut block = BasicBlock {
            statements: vec![
                Statement::Assign(
                    Lvalue::Local(Local(0)),
                    Rvalue::Constant(Constant::Bool(true)),
                ),
                Statement::Assign(
                    Lvalue::Local(Local(1)),
                    Rvalue::Constant(Constant::Bool(false)),
                ),
                Statement::Assign(
                    Lvalue::Local(Local(2)),
                    Rvalue::BinaryOp(BinaryOp::And, Local(0), Local(1)),
                ),
            ],
            terminator: Some(Terminator::Return(Local(2))),
        };

        let mut folder = ConstantFolder::new();
        folder.optimize_block(&mut block);

        match &block.statements[2] {
            Statement::Assign(_, Rvalue::Constant(Constant::Bool(val))) => {
                assert_eq!(*val, false);
            }
            _ => panic!("Expected folded constant bool!"),
        }
    }
}
