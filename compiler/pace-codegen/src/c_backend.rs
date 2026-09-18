use pace_ast::BinaryOp;
use pace_mir::{BasicBlock, Lvalue, MirFunction, MirProgram, Rvalue, Statement, Terminator};
use pace_ty::Ty;
use std::fmt::Write;

pub struct CGenerator {
    output: String,
    #[allow(clippy::type_complexity)]
    enum_defs: std::collections::HashMap<
        pace_hir::HirId,
        Vec<(String, Option<Vec<(String, pace_ty::Ty)>>)>,
    >,
}

impl Default for CGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CGenerator {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            enum_defs: std::collections::HashMap::new(),
        }
    }

    pub fn generate(&mut self, program: &MirProgram) -> String {
        self.output.push_str("#include <stdio.h>\n");
        self.output.push_str("#include <stdlib.h>\n");
        self.output.push_str("#include <string.h>\n");
        self.output.push_str("#include \"pace_runtime.h\"\n\n");

        self.enum_defs = program.enum_defs.clone();

        for (id, fields) in &program.struct_defs {
            self.output.push_str(&format!("struct pace_{} {{\n", id.0));
            for pace_ty::ResolvedField {
                name: fname,
                ty: fty,
                ..
            } in fields
            {
                self.output
                    .push_str(&format!("    {} {};\n", self.emit_c_type(fty), fname));
            }
            self.output.push_str("};\n\n");
        }

        for (id, variants) in &program.enum_defs {
            self.output.push_str(&format!("struct pace_{} {{\n", id.0));
            self.output.push_str("    long long tag;\n");
            self.output.push_str("    union {\n");
            for (v_name, v_fields) in variants {
                if let Some(fields) = v_fields {
                    self.output.push_str("        struct {\n");
                    for (fname, fty) in fields {
                        self.output.push_str(&format!(
                            "            {} {};\n",
                            self.emit_c_type(fty),
                            fname
                        ));
                    }
                    self.output.push_str(&format!("        }} {};\n", v_name));
                }
            }
            self.output.push_str("    } payload;\n");
            self.output.push_str("};\n\n");
        }

        // Forward declarations of class structs
        for id in program.class_defs.keys() {
            self.output.push_str(&format!("struct pace_{};\n", id.0));
        }
        self.output.push('\n');

        for (id, methods) in &program.class_vtables {
            self.output
                .push_str(&format!("struct pace_{}_vtable {{\n", id.0));
            for (i, pace_ty::VTableEntry { ty, .. }) in methods.iter().enumerate() {
                if let Ty::Function(params, ret) = ty {
                    self.output
                        .push_str(&format!("    {} (*m{})(", self.emit_c_type(ret), i));
                    if params.is_empty() {
                        self.output.push_str("void");
                    } else {
                        for (j, p) in params.iter().enumerate() {
                            if j > 0 {
                                self.output.push_str(", ");
                            }
                            self.output.push_str(&self.emit_c_type(p));
                        }
                    }
                    self.output.push_str(");\n");
                }
            }
            self.output.push_str("};\n\n");
        }

        for (id, fields) in &program.class_defs {
            self.output.push_str(&format!("struct pace_{} {{\n", id.0));
            self.output
                .push_str(&format!("    struct pace_{}_vtable* vtable;\n", id.0));
            for pace_ty::ResolvedField {
                name: fname,
                ty: fty,
                ..
            } in fields
            {
                self.output
                    .push_str(&format!("    {} {};\n", self.emit_c_type(fty), fname));
            }
            self.output.push_str("};\n\n");

            self.output
                .push_str(&format!("void pace_{}_deinit(void* ptr) {{\n", id.0));
            self.output.push_str(&format!(
                "    struct pace_{}* self = (struct pace_{}*)ptr;\n",
                id.0, id.0
            ));
            for pace_ty::ResolvedField {
                name: fname,
                ty: fty,
                ..
            } in fields
            {
                if matches!(fty, Ty::Class(_)) {
                    self.output
                        .push_str(&format!("    pace_release(self->{});\n", fname));
                }
            }
            self.output.push_str("}\n\n");
        }

        for (name, ty) in &program.global_vars {
            let c_ty = self.emit_c_type(ty);
            self.output.push_str(&format!("{} {};\n", c_ty, name));
        }
        self.output.push('\n');

        // Forward declare functions
        for func in &program.functions {
            let func_name = if func.name == "main" {
                "pace_main"
            } else {
                &func.name
            };
            self.output.push_str(&format!(
                "{} {}(",
                self.emit_c_type(&func.return_type),
                func_name
            ));
            if func.params.is_empty() {
                self.output.push_str("void");
            } else {
                for (i, p) in func.params.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    let c_ty = self.emit_c_type(&func.body.locals[p.0 as usize]);
                    self.output.push_str(&format!("{} _{}", c_ty, i));
                }
            }
            self.output.push_str(");\n");
        }
        self.output.push('\n');

        // V-Table globals
        for (id, methods) in &program.class_vtables {
            self.output.push_str(&format!(
                "struct pace_{}_vtable pace_{}_vtable_inst = {{\n",
                id.0, id.0
            ));
            for pace_ty::VTableEntry {
                mangled_name: func_name,
                ..
            } in methods
            {
                self.output.push_str(&format!("    {},\n", func_name));
            }
            self.output.push_str("};\n\n");
        }

        for func in &program.functions {
            self.generate_function(func);
            self.output.push('\n');
        }

        // Generate pace_init for top-level code (like static initializers)
        self.output.push_str("void pace_init() {\n");
        for (i, ty) in program.main_body.locals.iter().enumerate() {
            let c_ty = self.emit_c_type(ty);
            let def_val = self.emit_c_default_val(ty);
            self.output
                .push_str(&format!("    {} _{} = {};\n", c_ty, i, def_val));
        }
        self.output.push('\n');
        for (i, block) in program.main_body.blocks.iter().enumerate() {
            self.output.push_str(&format!("init_bb_{}:\n", i));
            self.generate_block(block, &program.main_body.locals);
        }
        self.output.push_str("}\n\n");

        self.output.push_str("int main() {\n");
        self.output.push_str("    pace_init();\n");
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
            Ty::Enum(id) => format!("struct pace_{}", id.0),
            Ty::Function(_, _) => "void*".to_string(),
            Ty::Optional(inner) => {
                let inner_c = self.emit_c_type(inner);
                if inner_c == "void" {
                    "void*".to_string()
                } else {
                    inner_c
                }
            }
            Ty::Void => "void".to_string(),
        }
    }

    fn emit_c_default_val(&self, ty: &Ty) -> String {
        match ty {
            Ty::Int | Ty::Bool | Ty::Float => "0".to_string(),
            Ty::String | Ty::Class(_) | Ty::Function(_, _) => "NULL".to_string(),
            Ty::Struct(_) | Ty::Enum(_) => "{0}".to_string(),
            Ty::Optional(inner) => {
                let default_val = self.emit_c_default_val(inner);
                if default_val.is_empty() {
                    "NULL".to_string()
                } else {
                    default_val
                }
            }
            Ty::Void => "".to_string(),
        }
    }

    fn generate_function(&mut self, func: &MirFunction) {
        let ret_ty_str = self.emit_c_type(&func.return_type);
        let c_name = if func.name == "main" {
            "pace_main"
        } else {
            &func.name
        };
        self.output.push_str(&format!("{} {}(", ret_ty_str, c_name));
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            let ty = &func.body.locals[param.0 as usize];
            let ty_str = self.emit_c_type(ty);
            write!(&mut self.output, "{} _{}", ty_str, param.0).unwrap();
        }
        self.output.push_str(") {\n");

        let mut locals = Vec::new();
        for i in 0..func.body.locals.len() {
            if !func.params.iter().any(|p| p.0 == i as u32) {
                let ty = &func.body.locals[i];
                let c_type = self.emit_c_type(ty);
                if c_type != "void" {
                    locals.push(format!(
                        "    {} _{} = {};",
                        c_type,
                        i,
                        self.emit_c_default_val(ty)
                    ));
                }
            }
        }
        self.output.push_str(&locals.join("\n"));
        self.output.push_str("\n\n");

        for (i, block) in func.body.blocks.iter().enumerate() {
            writeln!(&mut self.output, "{}_bb_{}:", func.name, i).unwrap();
            self.generate_block(block, &func.body.locals);
            match &block.terminator {
                Some(Terminator::Return(local)) => {
                    if self.emit_c_type(&func.return_type) == "void" {
                        writeln!(&mut self.output, "    return;").unwrap();
                    } else {
                        writeln!(&mut self.output, "    return _{};", local.0).unwrap();
                    }
                }
                Some(Terminator::Goto(bb)) => {
                    writeln!(&mut self.output, "    goto {}_bb_{};", func.name, bb.0).unwrap();
                }
                Some(Terminator::Branch {
                    cond,
                    then_block,
                    else_block,
                }) => {
                    writeln!(
                        &mut self.output,
                        "    if (_{}) goto {}_bb_{}; else goto {}_bb_{};",
                        cond.0, func.name, then_block.0, func.name, else_block.0
                    )
                    .unwrap();
                }
                None => {
                    if ret_ty_str != "void" {
                        self.output.push_str("    return 0;\n");
                    }
                }
            }
        }
        self.output.push_str("}\n");
    }

    fn generate_block(&mut self, block: &BasicBlock, locals: &[Ty]) {
        for stmt in &block.statements {
            match stmt {
                Statement::Assign(lval, rval) => {
                    self.output.push_str("    ");

                    let mut is_void = false;
                    if let Rvalue::BuiltinCall(name, _) = rval {
                        if name == "print" || name == "println" {
                            is_void = true;
                        }
                    } else if let Lvalue::Local(local) = lval
                        && self.emit_c_type(&locals[local.0 as usize]) == "void"
                    {
                        is_void = true;
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
                Statement::GlobalWrite(name, local) => {
                    writeln!(&mut self.output, "    {} = _{};", name, local.0).unwrap();
                }
            }
        }
    }

    fn generate_lvalue(&mut self, lval: &Lvalue, locals: &[Ty]) {
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
            Lvalue::EnumFieldAccess(obj, variant_name, field) => {
                write!(
                    &mut self.output,
                    "_{}.payload.{}.{}",
                    obj.0, variant_name, field
                )
                .unwrap();
            }
        }
    }

    fn generate_rvalue(&mut self, rvalue: &Rvalue, locals: &[Ty]) {
        match rvalue {
            Rvalue::Use(local) => write!(&mut self.output, "_{}", local.0).unwrap(),
            Rvalue::IntConstant(val) => write!(&mut self.output, "{}", val).unwrap(),
            Rvalue::FloatConstant(val) => write!(&mut self.output, "{}", val).unwrap(),
            Rvalue::BoolConstant(val) => {
                write!(&mut self.output, "{}", if *val { "1" } else { "0" }).unwrap()
            }
            Rvalue::StringConstant(val) => {
                write!(&mut self.output, "\"{}\"", val.trim_matches('"')).unwrap()
            }
            Rvalue::BinaryOp(op, lhs, rhs) => {
                if matches!(op, BinaryOp::NullCoalesce) {
                    write!(
                        &mut self.output,
                        "_{} != 0 ? _{} : _{}",
                        lhs.0, lhs.0, rhs.0
                    )
                    .unwrap();
                } else if matches!(op, BinaryOp::And) {
                    write!(&mut self.output, "_{} && _{}", lhs.0, rhs.0).unwrap();
                } else if matches!(op, BinaryOp::Or) {
                    write!(&mut self.output, "_{} || _{}", lhs.0, rhs.0).unwrap();
                } else {
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
                        _ => unreachable!(),
                    };
                    write!(&mut self.output, "_{} {} _{}", lhs.0, op_str, rhs.0).unwrap();
                }
            }
            Rvalue::OptionalFieldAccess(obj, field) => {
                let is_ptr = matches!(locals[obj.0 as usize], Ty::Class(_) | Ty::Optional(_));
                if is_ptr {
                    write!(
                        &mut self.output,
                        "_{} != 0 ? _{}->{} : 0",
                        obj.0, obj.0, field
                    )
                    .unwrap();
                } else {
                    write!(
                        &mut self.output,
                        "_{} != 0 ? _{}.{} : 0",
                        obj.0, obj.0, field
                    )
                    .unwrap(); // assuming structs might be checked for 0? Not really safe in C, but works for pointer MVP
                }
            }
            Rvalue::Call(callee, args) => {
                write!(&mut self.output, "_{}(", callee.0).unwrap();
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    write!(&mut self.output, "_{}", arg.0).unwrap();
                }
                self.output.push(')');
            }
            Rvalue::BuiltinCall(name, args) => {
                if name == "interpolate_string" {
                    let mut format_str = String::new();
                    let mut type_args = Vec::new();
                    for arg in args {
                        let mut ty = &locals[arg.0 as usize];
                        if let Ty::Optional(inner) = ty {
                            ty = inner;
                        }
                        if matches!(ty, Ty::Int) {
                            format_str.push_str("%lld");
                            type_args.push(format!("_{}", arg.0));
                        } else if matches!(ty, Ty::Float) {
                            format_str.push_str("%f");
                            type_args.push(format!("_{}", arg.0));
                        } else if matches!(ty, Ty::String) {
                            format_str.push_str("%s");
                            type_args.push(format!("_{}", arg.0));
                        } else if matches!(ty, Ty::Bool) {
                            format_str.push_str("%s");
                            type_args.push(format!("_{} ? \"true\" : \"false\"", arg.0));
                        } else {
                            format_str.push_str("%p");
                            type_args.push(format!("(void*)_{}", arg.0));
                        }
                    }
                    write!(&mut self.output, "pace_format_string(\"{}\"", format_str).unwrap();
                    for t_arg in type_args {
                        write!(&mut self.output, ", {}", t_arg).unwrap();
                    }
                    self.output.push(')');
                    return;
                }

                let c_name = if name == "print" || name == "println" {
                    if let Some(arg) = args.first() {
                        let mut ty = &locals[arg.0 as usize];
                        if let Ty::Optional(inner) = ty {
                            ty = inner;
                        }
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
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    if self.emit_c_type(&locals[arg.0 as usize]).contains("*") {
                        write!(&mut self.output, "(void*)_{}", arg.0).unwrap();
                    } else {
                        write!(&mut self.output, "_{}", arg.0).unwrap();
                    }
                }
                self.output.push(')');
            }
            Rvalue::GlobalCall(name, args) => {
                write!(&mut self.output, "{}(", name).unwrap();
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(&mut self.output, ", ").unwrap();
                    }
                    if self.emit_c_type(&locals[arg.0 as usize]).contains("*") {
                        write!(&mut self.output, "(void*)_{}", arg.0).unwrap();
                    } else {
                        write!(&mut self.output, "_{}", arg.0).unwrap();
                    }
                }
                write!(&mut self.output, ")").unwrap();
            }
            Rvalue::VirtualCall(idx, obj_local, args) => {
                write!(
                    &mut self.output,
                    "_{}->vtable->m{}((void*)_{}",
                    obj_local.0, idx, obj_local.0
                )
                .unwrap();
                for arg in args {
                    if self.emit_c_type(&locals[arg.0 as usize]).contains("*") {
                        write!(&mut self.output, ", (void*)_{}", arg.0).unwrap();
                    } else {
                        write!(&mut self.output, ", _{}", arg.0).unwrap();
                    }
                }
                write!(&mut self.output, ")").unwrap();
            }
            Rvalue::GlobalRead(name) => {
                write!(&mut self.output, "{}", name).unwrap();
            }
            Rvalue::FieldAccess(obj, field) => {
                let is_ptr = matches!(locals[obj.0 as usize], Ty::Class(_));
                if is_ptr {
                    write!(&mut self.output, "_{}->{}", obj.0, field).unwrap();
                } else {
                    write!(&mut self.output, "_{}.{}", obj.0, field).unwrap();
                }
            }
            Rvalue::Instantiate(ty, fields) => match ty {
                Ty::Struct(id) => {
                    write!(&mut self.output, "(struct pace_{}){{ ", id.0).unwrap();
                    if fields.is_empty() {
                        self.output.push('0');
                    } else {
                        for (i, arg) in fields.iter().enumerate() {
                            if i > 0 {
                                self.output.push_str(", ");
                            }
                            write!(&mut self.output, "_{}", arg.0).unwrap();
                        }
                    }
                    self.output.push_str(" }");
                }
                Ty::Class(id) => {
                    write!(&mut self.output, "memcpy(pace_alloc(sizeof(struct pace_{}), pace_{}_deinit), &(struct pace_{}){{ &pace_{}_vtable_inst", id.0, id.0, id.0, id.0).unwrap();
                    if !fields.is_empty() {
                        for arg in fields {
                            write!(&mut self.output, ", _{}", arg.0).unwrap();
                        }
                    }
                    write!(&mut self.output, " }}, sizeof(struct pace_{}))", id.0).unwrap();
                }
                _ => panic!("Instantiating non-struct/class"),
            },
            Rvalue::EnumTag(local) => write!(&mut self.output, "_{}.tag", local.0).unwrap(),
            Rvalue::EnumFieldAccess(obj, variant_name, field_name) => write!(
                &mut self.output,
                "_{}.payload.{}.{}",
                obj.0, variant_name, field_name
            )
            .unwrap(),
            Rvalue::InstantiateEnum(id, variant_name, fields) => {
                let mut tag = 0;
                let variants = self.enum_defs.get(id).unwrap();
                for (i, (v_name, _)) in variants.iter().enumerate() {
                    if v_name == variant_name {
                        tag = i;
                        break;
                    }
                }

                let mut has_fields = false;
                for (v_name, v_fields) in variants {
                    if v_name == variant_name && v_fields.is_some() {
                        has_fields = true;
                        break;
                    }
                }

                if has_fields {
                    write!(
                        &mut self.output,
                        "(struct pace_{}){{ .tag = {}, .payload = {{ .{} = {{ ",
                        id.0, tag, variant_name
                    )
                    .unwrap();
                    if fields.is_empty() {
                        self.output.push('0');
                    } else {
                        for (i, arg) in fields.iter().enumerate() {
                            if i > 0 {
                                self.output.push_str(", ");
                            }
                            write!(&mut self.output, "_{}", arg.0).unwrap();
                        }
                    }
                    self.output.push_str(" } } }");
                } else {
                    write!(
                        &mut self.output,
                        "(struct pace_{}){{ .tag = {} }}",
                        id.0, tag
                    )
                    .unwrap();
                }
            }
        }
    }
}
