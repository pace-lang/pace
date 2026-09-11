use std::fmt::Write;
use pace_mir::{BasicBlock, MirProgram, MirFunction, Rvalue, Statement, Terminator, Lvalue};
use pace_ast::BinaryOp;
use pace_ty::Ty;

pub struct CGenerator {
    output: String,
}

impl CGenerator {
    pub fn new() -> Self {
        Self {
            output: String::new(),
        }
    }

    pub fn generate(&mut self, program: &MirProgram) -> String {
        self.output.push_str("#include <stdio.h>\n");
        self.output.push_str("#include <stdlib.h>\n");
        self.output.push_str("#include <string.h>\n");
        self.output.push_str("#include \"pace_runtime.h\"\n\n");

        for (id, fields) in &program.struct_defs {
            self.output.push_str(&format!("struct pace_{} {{\n", id.0));
            for (fname, fty) in fields {
                self.output.push_str(&format!("    {} {};\n", self.emit_c_type(fty), fname));
            }
            self.output.push_str("};\n\n");
        }
        
        for (id, fields) in &program.class_defs {
            self.output.push_str(&format!("struct pace_{} {{\n", id.0));
            for (fname, fty) in fields {
                self.output.push_str(&format!("    {} {};\n", self.emit_c_type(fty), fname));
            }
            self.output.push_str("};\n\n");

            self.output.push_str(&format!("void pace_{}_deinit(void* ptr) {{\n", id.0));
            self.output.push_str(&format!("    struct pace_{}* self = (struct pace_{}*)ptr;\n", id.0, id.0));
            for (fname, fty) in fields {
                if matches!(fty, Ty::Class(_)) {
                    self.output.push_str(&format!("    pace_release(self->{});\n", fname));
                }
            }
            self.output.push_str("}\n\n");
        }

        for func in &program.functions {
            self.generate_function(func);
            self.output.push_str("\n");
        }

        self.output.push_str("int main() {\n");
        // Always call the user's main function if it exists.
        // We know it exists if the program has a function named "main".
        let has_main = program.functions.iter().any(|f| f.name == "main");
        if has_main {
            self.output.push_str("    pace_main();\n");
        }
        self.output.push_str("    return 0;\n");
        self.output.push_str("}\n");
        self.output.clone()
    }

    fn emit_c_type(&self, ty: &Ty) -> String {
        match ty {
            Ty::Int | Ty::Bool => "long long".to_string(),
            Ty::Float => "double".to_string(),
            Ty::String => "char*".to_string(),
            Ty::Struct(id) => format!("struct pace_{}", id.0),
            Ty::Class(id) => format!("struct pace_{}*", id.0),
            Ty::Function(_, _) => "void*".to_string(),
            Ty::Optional(inner) => self.emit_c_type(inner),
        }
    }

    fn emit_c_default_val(&self, ty: &Ty) -> String {
        match ty {
            Ty::Int | Ty::Bool | Ty::Float => "0".to_string(),
            Ty::String | Ty::Class(_) | Ty::Function(_, _) => "NULL".to_string(),
            Ty::Struct(_) => "{0}".to_string(),
            Ty::Optional(inner) => self.emit_c_default_val(inner),
        }
    }

    fn generate_function(&mut self, func: &MirFunction) {
        let ret_ty_str = self.emit_c_type(&func.return_type);
        let c_name = if func.name == "main" { "pace_main" } else { &func.name };
        self.output.push_str(&format!("{} {}(", ret_ty_str, c_name));
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 { self.output.push_str(", "); }
            let ty = &func.body.locals[param.0 as usize];
            let ty_str = self.emit_c_type(ty);
            write!(&mut self.output, "{} _{}", ty_str, param.0).unwrap();
        }
        self.output.push_str(") {\n");
        
        let mut locals = Vec::new();
        for i in 0..func.body.locals.len() {
            if !func.params.iter().any(|p| p.0 == i as u32) {
                let ty = &func.body.locals[i];
                locals.push(format!("    {} _{} = {};", self.emit_c_type(ty), i, self.emit_c_default_val(ty)));
            }
        }
        self.output.push_str(&locals.join("\n"));
        self.output.push_str("\n\n");

        for (i, block) in func.body.blocks.iter().enumerate() {
            write!(&mut self.output, "{}_bb_{}:\n", func.name, i).unwrap();
            self.generate_block(block, &func.body.locals);
            match &block.terminator {
                Some(Terminator::Return(local)) => {
                    write!(&mut self.output, "    return _{};\n", local.0).unwrap();
                }
                Some(Terminator::Goto(bb)) => {
                    write!(&mut self.output, "    goto {}_bb_{};\n", func.name, bb.0).unwrap();
                }
                Some(Terminator::Branch { cond, then_block, else_block }) => {
                    write!(&mut self.output, "    if (_{}) goto {}_bb_{}; else goto {}_bb_{};\n", cond.0, func.name, then_block.0, func.name, else_block.0).unwrap();
                }
                None => {
                    self.output.push_str("    return 0;\n");
                }
            }
        }
        self.output.push_str("}\n");
    }

    fn generate_block(&mut self, block: &BasicBlock, locals: &Vec<Ty>) {
        for stmt in &block.statements {
            match stmt {
                Statement::Assign(lval, rval) => {
                    self.output.push_str("    ");
                    
                    let mut is_void = false;
                    if let Rvalue::BuiltinCall(name, _) = rval {
                        if name == "print" || name == "println" {
                            is_void = true;
                        }
                    }
                    
                    if !is_void {
                        self.generate_lvalue(lval, locals);
                        self.output.push_str(" = ");
                    }
                    
                    self.generate_rvalue(rval, locals);
                    self.output.push_str(";\n");
                }
                Statement::Retain(lval) => {
                    self.output.push_str("    pace_retain(");
                    self.generate_lvalue(lval, locals);
                    self.output.push_str(");\n");
                }
                Statement::Release(lval) => {
                    self.output.push_str("    pace_release(");
                    self.generate_lvalue(lval, locals);
                    self.output.push_str(");\n");
                }
            }
        }
    }

    fn generate_lvalue(&mut self, lval: &Lvalue, locals: &Vec<Ty>) {
        match lval {
            Lvalue::Local(local) => write!(&mut self.output, "_{}", local.0).unwrap(),
            Lvalue::FieldAccess(obj, field) => {
                let is_ptr = matches!(locals[obj.0 as usize], Ty::Class(_));
                if is_ptr {
                    write!(&mut self.output, "_{}->{}", obj.0, field).unwrap();
                } else {
                    write!(&mut self.output, "_{}.{}", obj.0, field).unwrap();
                }
            }
        }
    }

    fn generate_rvalue(&mut self, rvalue: &Rvalue, locals: &Vec<Ty>) {
        match rvalue {
            Rvalue::Use(local) => write!(&mut self.output, "_{}", local.0).unwrap(),
            Rvalue::IntConstant(val) => write!(&mut self.output, "{}", val).unwrap(),
            Rvalue::FloatConstant(val) => write!(&mut self.output, "{}", val).unwrap(),
            Rvalue::BoolConstant(val) => write!(&mut self.output, "{}", if *val { "1" } else { "0" }).unwrap(),
            Rvalue::StringConstant(val) => write!(&mut self.output, "\"{}\"", val.trim_matches('"')).unwrap(),
            Rvalue::BinaryOp(op, lhs, rhs) => {
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::EqEq => "==",
                    BinaryOp::NotEq => "!=",
                    BinaryOp::Gt => ">",
                    BinaryOp::Lt => "<",
                    BinaryOp::GtEq => ">=",
                    BinaryOp::LtEq => "<=",
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
                    if let Some(arg) = args.first() {
                        let ty = &locals[arg.0 as usize];
                        if matches!(ty, Ty::String) {
                            "pace_print_string"
                        } else if matches!(ty, Ty::Float) {
                            "pace_print_float"
                        } else if matches!(ty, Ty::Bool) {
                            "pace_print_bool"
                        } else {
                            "pace_print_int"
                        }
                    } else {
                        "pace_println"
                    }
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
            Rvalue::GlobalCall(callee, args) => {
                write!(&mut self.output, "{}(", callee).unwrap();
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { self.output.push_str(", "); }
                    write!(&mut self.output, "_{}", arg.0).unwrap();
                }
                self.output.push_str(")");
            }
            Rvalue::FieldAccess(obj, field) => {
                let is_ptr = matches!(locals[obj.0 as usize], Ty::Class(_));
                if is_ptr {
                    write!(&mut self.output, "_{}->{}", obj.0, field).unwrap();
                } else {
                    write!(&mut self.output, "_{}.{}", obj.0, field).unwrap();
                }
            }
            Rvalue::Instantiate(ty, fields) => {
                match ty {
                    Ty::Struct(id) => {
                        write!(&mut self.output, "(struct pace_{}){{ ", id.0).unwrap();
                        if fields.is_empty() {
                            self.output.push_str("0");
                        } else {
                            for (i, arg) in fields.iter().enumerate() {
                                if i > 0 { self.output.push_str(", "); }
                                write!(&mut self.output, "_{}", arg.0).unwrap();
                            }
                        }
                        self.output.push_str(" }");
                    }
                    Ty::Class(id) => {
                        write!(&mut self.output, "memcpy(pace_alloc(sizeof(struct pace_{}), pace_{}_deinit), &(struct pace_{}){{ ", id.0, id.0, id.0).unwrap();
                        if fields.is_empty() {
                            self.output.push_str("0");
                        } else {
                            for (i, arg) in fields.iter().enumerate() {
                                if i > 0 { self.output.push_str(", "); }
                                write!(&mut self.output, "_{}", arg.0).unwrap();
                            }
                        }
                        write!(&mut self.output, " }}, sizeof(struct pace_{}))", id.0).unwrap();
                    }
                    _ => panic!("Instantiating non-struct/class"),
                }
            }
        }
    }
}
