use std::fmt::Write;
use pace_mir::{BasicBlock, MirBody, Rvalue, Statement, Terminator};
use pace_ast::BinaryOp;

pub struct CGenerator {
    output: String,
}

impl CGenerator {
    pub fn new() -> Self {
        Self {
            output: String::new(),
        }
    }

    pub fn generate(&mut self, body: &MirBody) -> String {
        // Emit headers and runtime include
        self.output.push_str("#include <stdio.h>\n");
        self.output.push_str("#include <stdlib.h>\n");
        self.output.push_str("#include \"pace_runtime.h\"\n\n");

        self.output.push_str("int main() {\n");
        
        // Declare all locals up front
        // Note: For v0.1 we type-pun everything to 'int' or 'char*'. 
        // In the future, we will read the 'Ty' from pace-ty to emit correct C types.
        for i in 0..body.locals {
            writeln!(&mut self.output, "    long long _{} = 0;", i).unwrap();
        }
        self.output.push_str("\n");

        // Emit blocks
        for (idx, block) in body.blocks.iter().enumerate() {
            writeln!(&mut self.output, "block_{}:", idx).unwrap();
            self.generate_block(block);
        }

        self.output.push_str("}\n");
        self.output.clone()
    }

    fn generate_block(&mut self, block: &BasicBlock) {
        for stmt in &block.statements {
            match stmt {
                Statement::Assign(local, rval) => {
                    self.output.push_str("    ");
                    
                    let mut is_void = false;
                    if let Rvalue::BuiltinCall(name, _) = rval {
                        if name == "print" || name == "println" {
                            is_void = true;
                        }
                    }
                    
                    if !is_void {
                        write!(&mut self.output, "_{} = ", local.0).unwrap();
                    }
                    
                    self.generate_rvalue(rval);
                    self.output.push_str(";\n");
                }
                Statement::Retain(local) => {
                    writeln!(&mut self.output, "    PACE_RETAIN(_{});", local.0).unwrap();
                }
                Statement::Release(local) => {
                    writeln!(&mut self.output, "    PACE_RELEASE(_{});", local.0).unwrap();
                }
            }
        }

        if let Some(terminator) = &block.terminator {
            match terminator {
                Terminator::Return(local) => {
                    writeln!(&mut self.output, "    return _{};", local.0).unwrap();
                }
                Terminator::Goto(block_id) => {
                    writeln!(&mut self.output, "    goto block_{};", block_id.0).unwrap();
                }
            }
        }
    }

    fn generate_rvalue(&mut self, rvalue: &Rvalue) {
        match rvalue {
            Rvalue::Use(local) => write!(&mut self.output, "_{}", local.0).unwrap(),
            Rvalue::IntConstant(val) => write!(&mut self.output, "{}", val).unwrap(),
            Rvalue::StringConstant(val) => write!(&mut self.output, "(long long)\"{}\"", val).unwrap(), // Cast string pointer to integer type for generic holding
            Rvalue::BinaryOp(op, lhs, rhs) => {
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::EqEq => "==",
                    BinaryOp::NotEq => "!=",
                    _ => "+",
                };
                write!(&mut self.output, "_{} {} _{}", lhs.0, op_str, rhs.0).unwrap();
            }
            Rvalue::Call(callee, args) => {
                write!(&mut self.output, "_{}(", callee.0).unwrap();
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { self.output.push_str(", "); }
                    write!(&mut self.output, "_{}", arg.0).unwrap();
                }
                self.output.push_str(")");
            }
            Rvalue::BuiltinCall(name, args) => {
                let c_name = if name == "print" || name == "println" {
                    "pace_print_int"
                } else {
                    name.as_str()
                };
                write!(&mut self.output, "{}(", c_name).unwrap();
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { self.output.push_str(", "); }
                    write!(&mut self.output, "_{}", arg.0).unwrap();
                }
                self.output.push_str(")");
            }
        }
    }
}
