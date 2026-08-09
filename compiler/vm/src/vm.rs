use std::collections::HashMap;
use mir::{BlockId, Function, Inst, Place, RValue, Terminator, Value};
use ast::{BinaryOp, UnaryOp};

pub struct VirtualMachine {
    memory_vars: HashMap<String, Value>,
    memory_temps: HashMap<usize, Value>,
}

impl VirtualMachine {
    pub fn new() -> Self {
        Self {
            memory_vars: HashMap::new(),
            memory_temps: HashMap::new(),
        }
    }

    pub fn dump_memory(&self) {
        println!("--- VM Memory Dump ---");
        for (k, v) in &self.memory_vars {
            println!("{} = {:?}", k, v);
        }
        println!("----------------------");
    }

    pub fn execute(&mut self, function: &Function) -> Option<Value> {
        let mut current_block_id = BlockId(0);

        loop {
            let block = function.blocks.iter().find(|b| b.id == current_block_id)
                .expect("Block not found");

            for inst in &block.instructions {
                self.execute_inst(inst);
            }

            match &block.terminator {
                Some(Terminator::Jump(next_block)) => {
                    current_block_id = *next_block;
                }
                Some(Terminator::Branch { cond, then_block, else_block }) => {
                    let cond_val = self.resolve_value(cond);
                    if let Value::Boolean(b) = cond_val {
                        if b {
                            current_block_id = *then_block;
                        } else {
                            current_block_id = *else_block;
                        }
                    } else {
                        panic!("Branch condition must be a Boolean.");
                    }
                }
                Some(Terminator::Return(val_opt)) => {
                    return val_opt.as_ref().map(|v| self.resolve_value(v));
                }
                None => {
                    panic!("Block has no terminator!");
                }
            }
        }
    }

    fn execute_inst(&mut self, inst: &Inst) {
        match inst {
            Inst::Assign(place, rvalue) => {
                let val = match rvalue {
                    RValue::Use(v) => self.resolve_value(v),
                    RValue::UnaryOp(op, right) => self.eval_unary(op.clone(), right),
                    RValue::BinaryOp(op, left, right) => self.eval_binary(op.clone(), left, right),
                };
                self.store(place, val);
            }
        }
    }

    fn resolve_value(&self, value: &Value) -> Value {
        match value {
            Value::Place(place) => self.load(place),
            _ => value.clone(),
        }
    }

    fn load(&self, place: &Place) -> Value {
        match place {
            Place::Var(name) => self.memory_vars.get(name).cloned().expect("Variable not found in VM memory"),
            Place::Temp(id) => self.memory_temps.get(id).cloned().expect("Temp not found in VM memory"),
        }
    }

    fn store(&mut self, place: &Place, value: Value) {
        match place {
            Place::Var(name) => { self.memory_vars.insert(name.clone(), value); },
            Place::Temp(id) => { self.memory_temps.insert(*id, value); },
        }
    }

    fn eval_unary(&self, op: UnaryOp, right: &Value) -> Value {
        let r = self.resolve_value(right);
        match op {
            UnaryOp::Negate => match r {
                Value::Int(i) => Value::Int(-i),
                Value::Float(f) => Value::Float(-f),
                _ => panic!("Cannot negate non-numeric value"),
            }
        }
    }

    fn eval_binary(&self, op: BinaryOp, left: &Value, right: &Value) -> Value {
        let l = self.resolve_value(left);
        let r = self.resolve_value(right);

        match op {
            BinaryOp::Add => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Int(a + b),
                (Value::Float(a), Value::Float(b)) => Value::Float(a + b),
                (Value::String(a), Value::String(b)) => Value::String(format!("{}{}", a, b)),
                _ => panic!("Invalid operand types for Add"),
            },
            BinaryOp::Subtract => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Int(a - b),
                (Value::Float(a), Value::Float(b)) => Value::Float(a - b),
                _ => panic!("Invalid operand types for Subtract"),
            },
            BinaryOp::Multiply => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Int(a * b),
                (Value::Float(a), Value::Float(b)) => Value::Float(a * b),
                _ => panic!("Invalid operand types for Multiply"),
            },
            BinaryOp::Divide => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Int(a / b),
                (Value::Float(a), Value::Float(b)) => Value::Float(a / b),
                _ => panic!("Invalid operand types for Divide"),
            },
            BinaryOp::Equal => Value::Boolean(l == r),
            BinaryOp::NotEqual => Value::Boolean(l != r),
            BinaryOp::Less => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Boolean(a < b),
                (Value::Float(a), Value::Float(b)) => Value::Boolean(a < b),
                _ => panic!("Invalid operand types for Less"),
            },
            BinaryOp::LessEqual => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Boolean(a <= b),
                (Value::Float(a), Value::Float(b)) => Value::Boolean(a <= b),
                _ => panic!("Invalid operand types for LessEqual"),
            },
            BinaryOp::Greater => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Boolean(a > b),
                (Value::Float(a), Value::Float(b)) => Value::Boolean(a > b),
                _ => panic!("Invalid operand types for Greater"),
            },
            BinaryOp::GreaterEqual => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Boolean(a >= b),
                (Value::Float(a), Value::Float(b)) => Value::Boolean(a >= b),
                _ => panic!("Invalid operand types for GreaterEqual"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir::{BasicBlock, BlockId, Function, Inst, Place, RValue, Terminator, Value};
    use ast::BinaryOp;

    #[test]
    fn test_execute_math() {
        let mut fun = Function::new("main".into());
        let mut block0 = BasicBlock::new(BlockId(0));
        
        block0.instructions.push(Inst::Assign(Place::Temp(0), RValue::BinaryOp(BinaryOp::Add, Value::Int(10), Value::Int(5))));
        block0.instructions.push(Inst::Assign(Place::Var("x".into()), RValue::Use(Value::Place(Place::Temp(0)))));
        block0.terminator = Some(Terminator::Return(Some(Value::Place(Place::Var("x".into())))));
        
        fun.blocks.push(block0);

        let mut vm = VirtualMachine::new();
        let result = vm.execute(&fun);

        assert_eq!(result, Some(Value::Int(15)));
    }
}
