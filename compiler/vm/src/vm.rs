use std::collections::HashMap;
use mir::{BlockId, Function, Inst, Place, RValue, Terminator, Value, Program};
use ast::{BinaryOp, UnaryOp};

pub struct Frame {
    pub memory_vars: HashMap<String, Value>,
    pub memory_temps: HashMap<usize, Value>,
}

#[derive(Debug, Clone)]
pub struct Object {
    pub class_name: String,
    pub fields: HashMap<String, Value>,
}

pub struct VirtualMachine<'a> {
    program: &'a Program,
    frames: Vec<Frame>,
    heap: Vec<Object>,
}

impl<'a> VirtualMachine<'a> {
    pub fn new(program: &'a Program) -> Self {
        Self {
            program,
            frames: Vec::new(),
            heap: Vec::new(),
        }
    }

    pub fn dump_memory(&self) {
        println!("--- VM Memory Dump ---");
        if let Some(frame) = self.frames.last() {
            for (k, v) in &frame.memory_vars {
                println!("{} = {:?}", k, v);
            }
        }
        println!("----------------------");
    }

    pub fn execute(&mut self) -> Option<Value> {
        self.call_function("main", &[])
    }

    pub fn call_function(&mut self, name: &str, args: &[Value]) -> Option<Value> {
        if name == "print" {
            let mut out = String::new();
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                let resolved = match arg {
                    Value::Place(_) => panic!("Pass evaluated arguments to print"),
                    _ => arg.clone()
                };
                match resolved {
                    Value::Int(v) => out.push_str(&v.to_string()),
                    Value::Float(v) => out.push_str(&v.to_string()),
                    Value::String(v) => out.push_str(&v),
                    Value::Boolean(v) => out.push_str(&v.to_string()),
                    Value::Object(id) => {
                        let obj = &self.heap[id];
                        out.push_str(&format!("<{} object at {}>", obj.class_name, id));
                    },
                    Value::Void => out.push_str("void"),
                    Value::Place(_) => unreachable!(),
                }
            }
            println!("{}", out);
            return Some(Value::Void);
        }

        let function = self.program.functions.get(name).unwrap_or_else(|| panic!("Function '{}' not found", name));
        
        let mut frame = Frame {
            memory_vars: HashMap::new(),
            memory_temps: HashMap::new(),
        };

        // Map arguments to parameters
        for (i, param_name) in function.parameters.iter().enumerate() {
            frame.memory_vars.insert(param_name.clone(), args[i].clone());
        }

        self.frames.push(frame);

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
                    let ret = val_opt.as_ref().map(|v| self.resolve_value(v));
                    self.frames.pop();
                    return ret;
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
                    RValue::AllocateObject(class_name) => {
                        let id = self.heap.len();
                        self.heap.push(Object {
                            class_name: class_name.clone(),
                            fields: HashMap::new(),
                        });
                        Value::Object(id)
                    }
                    RValue::GetProperty(object_place, name) => {
                        let obj_val = self.resolve_value(object_place);
                        if let Value::Object(id) = obj_val {
                            if let Some(obj) = self.heap.get(id) {
                                obj.fields.get(name).cloned().unwrap_or(Value::Void)
                            } else {
                                panic!("Object ID {} not found in heap", id);
                            }
                        } else {
                            panic!("Cannot get property on non-object");
                        }
                    }
                    RValue::MethodCall(object_place, method_name, args) => {
                        let obj_val = self.resolve_value(object_place);
                        if let Value::Object(id) = obj_val {
                            let class_name = self.heap.get(id).unwrap().class_name.clone();
                            let func_name = format!("{}::{}", class_name, method_name);
                            
                            let mut resolved_args = vec![Value::Object(id)];
                            for arg in args {
                                resolved_args.push(self.resolve_value(arg));
                            }
                            
                            self.call_function(&func_name, &resolved_args).unwrap_or(Value::Void)
                        } else {
                            panic!("Cannot call method on non-object");
                        }
                    }
                    RValue::Call(func_name, args) => {
                        let mut resolved_args = Vec::new();
                        for arg in args {
                            resolved_args.push(self.resolve_value(arg));
                        }
                        
                        if self.program.classes.contains_key(func_name) {
                            let id = self.heap.len();
                            self.heap.push(Object {
                                class_name: func_name.clone(),
                                fields: HashMap::new(),
                            });
                            
                            let class_def = &self.program.classes[func_name];
                            for field in &class_def.fields {
                                self.heap[id].fields.insert(field.clone(), Value::Void);
                            }
                            
                            let init_func = format!("{}::init", func_name);
                            if self.program.functions.contains_key(&init_func) {
                                let mut init_args = vec![Value::Object(id)];
                                init_args.extend(resolved_args);
                                self.call_function(&init_func, &init_args);
                            }
                            
                            Value::Object(id)
                        } else {
                            self.call_function(func_name, &resolved_args).unwrap_or(Value::Void)
                        }
                    }
                };
                self.store(place, val);
            }
            Inst::SetProperty(object_place, name, value) => {
                let obj_val = self.resolve_value(object_place);
                let val = self.resolve_value(value);
                if let Value::Object(id) = obj_val {
                    if let Some(obj) = self.heap.get_mut(id) {
                        obj.fields.insert(name.clone(), val);
                    } else {
                        panic!("Object ID {} not found in heap", id);
                    }
                } else {
                    panic!("Cannot set property on non-object");
                }
            }
            Inst::Retain(_) => {}
            Inst::Release(_) => {}
        }
    }

    fn resolve_value(&self, value: &Value) -> Value {
        match value {
            Value::Place(place) => self.load(place),
            _ => value.clone(),
        }
    }

    fn load(&self, place: &Place) -> Value {
        let frame = self.frames.last().expect("No active frame");
        match place {
            Place::Var(name) => frame.memory_vars.get(name).cloned().expect("Variable not found in VM memory"),
            Place::Temp(id) => frame.memory_temps.get(id).cloned().expect("Temp not found in VM memory"),
        }
    }

    fn store(&mut self, place: &Place, value: Value) {
        let frame = self.frames.last_mut().expect("No active frame");
        match place {
            Place::Var(name) => { frame.memory_vars.insert(name.clone(), value); },
            Place::Temp(id) => { frame.memory_temps.insert(*id, value); },
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
    // No longer need eval_call as it is in call_function

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
        let mut fun = Function::new("main".into(), vec![]);
        let mut block0 = BasicBlock::new(BlockId(0));
        
        block0.instructions.push(Inst::Assign(Place::Temp(0), RValue::BinaryOp(BinaryOp::Add, Value::Int(10), Value::Int(5))));
        block0.instructions.push(Inst::Assign(Place::Var("x".into()), RValue::Use(Value::Place(Place::Temp(0)))));
        block0.terminator = Some(Terminator::Return(Some(Value::Place(Place::Var("x".into())))));
        
        fun.blocks.push(block0);

        let mut program = Program::new();
        program.functions.insert("main".into(), fun);

        let mut vm = VirtualMachine::new(&program);
        let result = vm.execute();

        assert_eq!(result, Some(Value::Int(15)));
    }
}
