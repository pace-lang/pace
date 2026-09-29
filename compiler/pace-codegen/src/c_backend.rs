use pace_ast::BinaryOp;
use pace_mir::{
    BasicBlock, Constant, Lvalue, MirFunction, MirProgram, Rvalue, Statement, Terminator,
};
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
            output: String::with_capacity(1024 * 1024),
            enum_defs: std::collections::HashMap::new(),
        }
    }

    pub fn generate(&mut self, program: &MirProgram) -> String {
        self.output.push_str("#include <stdio.h>\n");
        self.output.push_str("#include <stdlib.h>\n");
        self.output.push_str("#include <string.h>\n");
        self.output.push_str("#include <pthread.h>\n");
        self.output.push_str("#include \"pace_runtime.h\"\n\n");

        self.output.push_str("typedef struct PaceClosure {\n");
        self.output.push_str("    void* func;\n");
        self.output.push_str("    void* env;\n");
        self.output.push_str("} PaceClosure;\n\n");

        for func in &program.functions {
            if let Some(env_layout) = &func.env_layout {
                self.output.push_str(&format!("struct __Env_{} {{\n", func.name));
                for (local, ty) in env_layout {
                    self.output.push_str(&format!("    {} _{};\n", self.emit_c_type(ty), local.0));
                }
                self.output.push_str("};\n\n");
            }
        }

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
            if program.actor_defs.contains(id) {
                self.output.push_str("    pthread_mutex_t __mailbox_mutex;\n");
                self.output.push_str("    void* __mailbox_head;\n");
                self.output.push_str("    void* __mailbox_tail;\n");
                self.output.push_str("    int __is_running;\n");
            }
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
                if matches!(fty, Ty::Class(_)) || matches!(fty, Ty::String) {
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
            if func.env_layout.is_some() {
                self.output.push_str("void* __env");
                if !func.params.is_empty() {
                    self.output.push_str(", ");
                }
            } else if func.params.is_empty() {
                self.output.push_str("void");
            }
            
            for (i, p) in func.params.iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                let c_ty = self.emit_c_type(&func.body.locals[p.0 as usize]);
                self.output.push_str(&format!("{} _{}", c_ty, i));
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
            if c_ty != "void" {
                let def_val = self.emit_c_default_val(ty);
                self.output
                    .push_str(&format!("    {} _{} = {};\n", c_ty, i, def_val));
            }
        }
        self.output.push('\n');
        for (i, block) in program.main_body.blocks.iter().enumerate() {
            self.output.push_str(&format!("init_bb_{}:\n", i));
            self.generate_block(block, &program.main_body.locals);
        }
        self.output.push_str("}\n\n");

        self.output.push_str("int main() {\n");
        self.output.push_str("    pace_thread_pool_init(4);\n");
        self.output.push_str("    pace_event_loop_init();\n");
        self.output.push_str("    pace_init();\n");
        let main_func = program.functions.iter().find(|f| f.name == "main");
        if let Some(m) = main_func {
            self.output.push_str("    pace_main();\n");
            if m.is_async {
                self.output.push_str("    pace_event_loop_run();\n");
            }
        }
        self.output.push_str("    pace_thread_pool_shutdown();\n");
        self.output.push_str("    return 0;\n");
        self.output.push_str("}\n");
        self.output.clone()
    }

    fn emit_c_type(&self, ty: &Ty) -> String {
        match ty {
            Ty::Int | Ty::Bool => "long long".to_string(),
            Ty::Float => "double".to_string(),
            Ty::String => "struct PaceString*".to_string(),
            Ty::Struct(id) => format!("struct pace_{}", id.0),
            Ty::Class(id) | Ty::Actor(id) => format!("struct pace_{}*", id.0),
            Ty::Enum(id) => format!("struct pace_{}", id.0),
            Ty::Closure(_, _) => "PaceClosure*".to_string(),
            Ty::Function(_, _) => "void*".to_string(),
            Ty::Optional(inner) => {
                let inner_c = self.emit_c_type(inner);
                if inner_c == "void" {
                    "void*".to_string()
                } else {
                    inner_c
                }
            }
            Ty::Future(_) => "void*".to_string(),
            Ty::Void => "void".to_string(),
        }
    }

    fn emit_c_default_val(&self, ty: &Ty) -> String {
        match ty {
            Ty::Int | Ty::Bool | Ty::Float => "0".to_string(),
            Ty::String | Ty::Class(_) | Ty::Actor(_) | Ty::Function(_, _) | Ty::Closure(_, _) => "NULL".to_string(),
            Ty::Struct(_) | Ty::Enum(_) => "{0}".to_string(),
            Ty::Optional(inner) => {
                let default_val = self.emit_c_default_val(inner);
                if default_val.is_empty() {
                    "NULL".to_string()
                } else {
                    default_val
                }
            }
            Ty::Future(inner) => {
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

        if func.is_async {
            self.output.push_str(&format!("struct __AsyncEnv_{} {{\n", c_name));
            for param in func.params.iter() {
                let ty = &func.body.locals[param.0 as usize];
                let ty_str = self.emit_c_type(ty);
                self.output.push_str(&format!("    {} _{};\n", ty_str, param.0));
            }
            if func.env_layout.is_some() {
                self.output.push_str("    void* __env;\n");
            }
            self.output.push_str("};\n\n");
            
            self.output.push_str(&format!("void __async_body_{}(void* __env_ptr) {{\n", c_name));
            self.output.push_str(&format!("    struct __AsyncEnv_{}* __async_env = (struct __AsyncEnv_{}*)__env_ptr;\n", c_name, c_name));
            
            for param in func.params.iter() {
                let ty = &func.body.locals[param.0 as usize];
                let ty_str = self.emit_c_type(ty);
                self.output.push_str(&format!("    {} _{} = __async_env->_{};\n", ty_str, param.0, param.0));
            }
            if func.env_layout.is_some() {
                self.output.push_str("    void* __env = __async_env->__env;\n");
            }
        } else {
            self.output.push_str(&format!("{} {}(", ret_ty_str, c_name));
            let mut has_params = false;
            if func.env_layout.is_some() {
                self.output.push_str("void* __env");
                has_params = true;
            }
            for param in func.params.iter() {
                if has_params {
                    self.output.push_str(", ");
                }
                let ty = &func.body.locals[param.0 as usize];
                let ty_str = self.emit_c_type(ty);
                self.output.push_str(&format!("{} _{}", ty_str, param.0));
                has_params = true;
            }
            if !has_params {
                self.output.push_str("void");
            }
            self.output.push_str(") {\n");
        }

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

        if let Some(env_layout) = &func.env_layout {
            self.output.push_str(&format!("    struct __Env_{}* __env_ptr = (struct __Env_{}*)__env;\n", func.name, func.name));
            for (local, _) in env_layout {
                self.output.push_str(&format!("    _{} = __env_ptr->_{};\n", local.0, local.0));
            }
            self.output.push_str("\n");
        }

        for (i, block) in func.body.blocks.iter().enumerate() {
            self.output.push_str(&format!("{}_bb_{}:\n", func.name, i));
            self.generate_block(block, &func.body.locals);
            match &block.terminator {
                Some(Terminator::Return(local)) => {
                    if func.is_async {
                        if self.emit_c_type(&func.return_type) != "void" {
                            self.output.push_str(&format!("    pace_fiber_set_current_result((void*)(long long)_{});\n", local.0));
                        }
                        self.output.push_str("    return;\n");
                    } else {
                        if self.emit_c_type(&func.return_type) == "void" {
                            self.output.push_str("    return;\n");
                        } else {
                            self.output.push_str(&format!("    return _{};\n", local.0));
                        }
                    }
                }
                Some(Terminator::Goto(bb)) => {
                    self.output.push_str(&format!("    goto {}_bb_{};\n", func.name, bb.0));
                }
                Some(Terminator::Branch {
                    cond,
                    then_block,
                    else_block,
                }) => {
                    self.output.push_str(&format!(
                        "    if (_{}) goto {}_bb_{}; else goto {}_bb_{};\n",
                        cond.0, func.name, then_block.0, func.name, else_block.0
                    ));
                }
                None => {
                    if func.is_async {
                        self.output.push_str("    return;\n");
                    } else if ret_ty_str != "void" {
                        self.output.push_str("    return 0;\n");
                    }
                }
            }
        }
        self.output.push_str("}\n\n");

        if func.is_async {
            self.output.push_str(&format!("void* {}(", c_name));
            let mut has_params = false;
            if func.env_layout.is_some() {
                self.output.push_str("void* __env");
                has_params = true;
            }
            for param in func.params.iter() {
                if has_params {
                    self.output.push_str(", ");
                }
                let ty = &func.body.locals[param.0 as usize];
                let ty_str = self.emit_c_type(ty);
                self.output.push_str(&format!("{} _{}", ty_str, param.0));
                has_params = true;
            }
            if !has_params {
                self.output.push_str("void");
            }
            self.output.push_str(") {\n");
            
            let mut is_actor_method = false;
            let mut actor_param = None;
            if let Some(param) = func.params.first() {
                if let Ty::Actor(_) = func.body.locals[param.0 as usize] {
                    is_actor_method = true;
                    actor_param = Some(param.0);
                }
            }

            self.output.push_str(&format!("    struct __AsyncEnv_{}* __async_env = (struct __AsyncEnv_{}*)malloc(sizeof(struct __AsyncEnv_{}));\n", c_name, c_name, c_name));
            for param in func.params.iter() {
                self.output.push_str(&format!("    __async_env->_{} = _{};\n", param.0, param.0));
            }
            if func.env_layout.is_some() {
                self.output.push_str("    __async_env->__env = __env;\n");
            }
            if is_actor_method {
                self.output.push_str(&format!("    return pace_send_actor_message((void*)_{}, __async_body_{}, __async_env);\n", actor_param.unwrap(), c_name));
            } else {
                self.output.push_str(&format!("    return pace_spawn_fiber(__async_body_{}, __async_env);\n", c_name));
            }
            self.output.push_str("}\n");
        }
    }
    fn generate_block(&mut self, block: &BasicBlock, locals: &[Ty]) {
        for stmt in &block.statements {
            match stmt {
                Statement::Assign(lval, rval) => {
                    self.output.push_str("    ");

                    let mut is_void = false;
                    if let Lvalue::Local(local) = lval {
                        if self.emit_c_type(&locals[local.0 as usize]) == "void" {
                            is_void = true;
                        }
                    }
                    if let Rvalue::BuiltinCall(name, _) = rval {
                        if name == "print" || name == "println" {
                            is_void = true;
                        }
                    }

                    if !is_void {
                        self.generate_lvalue(lval, locals);
                        self.output.push_str(" = ");
                        if let Rvalue::BuiltinCall(name, _) = rval {
                            if name == "pace_await_fiber" {
                                if let Lvalue::Local(local) = lval {
                                    self.output.push_str(&format!("({})", self.emit_c_type(&locals[local.0 as usize])));
                                }
                            }
                        }
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
                let is_ptr = matches!(locals[obj.0 as usize], Ty::Class(_) | Ty::Actor(_));
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
            Rvalue::Constant(Constant::Int(val)) => write!(&mut self.output, "{}", val).unwrap(),
            Rvalue::Constant(Constant::Float(val)) => write!(&mut self.output, "{}", val).unwrap(),
            Rvalue::Constant(Constant::Bool(val)) => {
                write!(&mut self.output, "{}", if *val { "1" } else { "0" }).unwrap()
            }
            Rvalue::Constant(Constant::String(val)) => write!(
                &mut self.output,
                "pace_string_new(\"{}\")",
                val.trim_matches('"')
            )
            .unwrap(),
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
                let callee_ty = &locals[callee.0 as usize];
                if let Ty::Closure(params, ret) = callee_ty {
                    let mut fn_ptr = format!("(({} (*)(void*", self.emit_c_type(ret));
                    for p in params {
                        fn_ptr.push_str(", ");
                        fn_ptr.push_str(&self.emit_c_type(p));
                    }
                    fn_ptr.push_str("))");
                    write!(&mut self.output, "{}_{}->func)(_{}->env", fn_ptr, callee.0, callee.0).unwrap();
                    if !args.is_empty() {
                        self.output.push_str(", ");
                    }
                } else if let Ty::Function(params, ret) = callee_ty {
                    let mut fn_ptr = format!("(({} (*)(", self.emit_c_type(ret));
                    if params.is_empty() {
                        fn_ptr.push_str("void");
                    } else {
                        fn_ptr.push_str(&params.iter().map(|p| self.emit_c_type(p)).collect::<Vec<_>>().join(", "));
                    }
                    fn_ptr.push_str("))");
                    write!(&mut self.output, "{}_{})(", fn_ptr, callee.0).unwrap();
                } else {
                    write!(&mut self.output, "_{}(", callee.0).unwrap();
                }
                
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    write!(&mut self.output, "_{}", arg.0).unwrap();
                }
                self.output.push(')');
            }
            Rvalue::BuiltinCall(name, args) => {
                if name == "pace_await_fiber" {
                    write!(&mut self.output, "pace_await_fiber((void*)_{})", args[0].0).unwrap();
                    return;
                } else if name == "sleep" {
                    write!(&mut self.output, "pace_sleep(_{})", args[0].0).unwrap();
                    return;
                } else if name == "interpolate_string" {
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
                            type_args.push(format!("(_{} ? _{}->data : \"null\")", arg.0, arg.0));
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
                let is_ptr = matches!(locals[obj.0 as usize], Ty::Class(_) | Ty::Actor(_));
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
                Ty::Class(id) | Ty::Actor(id) => {
                    let is_actor = matches!(ty, Ty::Actor(_));
                    write!(&mut self.output, "memcpy(pace_alloc(sizeof(struct pace_{}), pace_{}_deinit), &(struct pace_{}){{ &pace_{}_vtable_inst", id.0, id.0, id.0, id.0).unwrap();
                    if is_actor {
                        // pthread_mutex_t PTHREAD_MUTEX_INITIALIZER uses braces on some platforms,
                        // but since it's inside a struct literal, we can just supply it as a field value.
                        // Or we can just use zero-initialization and call pthread_mutex_init dynamically.
                        // Wait, C allows `{ ... }` as an expression, but nested brace init is fine.
                        // Wait, it's safer to just set to 0 and dynamically initialize if needed. Actually PTHREAD_MUTEX_INITIALIZER works as long as it's not nested in another brace if compiler is picky.
                        // Let's output it as the macro:
                        write!(&mut self.output, ", PTHREAD_MUTEX_INITIALIZER, NULL, NULL, 0").unwrap();
                    }
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
            Rvalue::MakeClosure(func_name, captured_locals) => {
                let env_struct_name = format!("__Env_{}", func_name);
                self.output.push_str("(PaceClosure*)memcpy(pace_alloc(sizeof(PaceClosure), NULL), &(PaceClosure){");
                if captured_locals.is_empty() {
                    write!(&mut self.output, " .func = (void*){}, .env = NULL }}, sizeof(PaceClosure))", func_name).unwrap();
                } else {
                    write!(&mut self.output, " .func = (void*){}, .env = memcpy(pace_alloc(sizeof(struct {}), NULL), &(struct {}){{ ", func_name, env_struct_name, env_struct_name).unwrap();
                    for (i, arg) in captured_locals.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        write!(&mut self.output, "_{}", arg.0).unwrap();
                    }
                    write!(&mut self.output, " }}, sizeof(struct {})) }}, sizeof(PaceClosure))", env_struct_name).unwrap();
                }
            }
        }
    }
}
