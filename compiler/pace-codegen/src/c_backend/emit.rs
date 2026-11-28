use pace_ast::BinaryOp;
use pace_mir::{BasicBlock, Constant, Lvalue, MirFunction, Rvalue, Statement, Terminator};
use pace_ty::Ty;
use std::fmt::Write;

use super::CGenerator;

impl CGenerator {
    pub(super) fn generate_function(&mut self, func: &MirFunction) {
        let ret_ty_str = self.emit_c_type(&func.return_type);
        let c_name = if func.name == "main" {
            "pace_main"
        } else {
            &func.name
        };

        let direct_actor_self = func.is_async
            && func.params.len() == 1
            && func.env_layout.is_none()
            && matches!(func.body.locals[func.params[0].0 as usize], Ty::Actor(_));

        if func.is_async {
            if !direct_actor_self {
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
            }

            self.output.push_str(&format!("void __async_body_{}(void* __env_ptr) {{\n", c_name));
            if direct_actor_self {
                let p0 = func.params[0].0;
                let ty_str = self.emit_c_type(&func.body.locals[p0 as usize]);
                self.output.push_str(&format!("    {} _{} = ({})__env_ptr;\n", ty_str, p0, ty_str));
            } else {
                self.output.push_str(&format!("    struct __AsyncEnv_{}* __async_env = (struct __AsyncEnv_{}*)__env_ptr;\n", c_name, c_name));

                for param in func.params.iter() {
                    let ty = &func.body.locals[param.0 as usize];
                    let ty_str = self.emit_c_type(ty);
                    self.output.push_str(&format!("    {} _{} = __async_env->_{};\n", ty_str, param.0, param.0));
                }
                if func.env_layout.is_some() {
                    self.output.push_str("    void* __env = __async_env->__env;\n");
                }
                self.output.push_str("    free(__async_env);\n");
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
                        } else if let Some(opt_name) = Self::is_primitive_opt(&func.return_type) {
                            let loc_ty = &func.body.locals[local.0 as usize];
                            if matches!(loc_ty, Ty::Optional(inner) if **inner == Ty::Void) {
                                self.output.push_str(&format!("    return ({}){{0, 0}};\n", opt_name));
                            } else if Self::is_primitive_opt(loc_ty).is_none() {
                                self.output.push_str(&format!("    return ({}){{1, _{}}};\n", opt_name, local.0));
                            } else {
                                self.output.push_str(&format!("    return _{};\n", local.0));
                            }
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

            let env_arg = if direct_actor_self {
                format!("(void*)_{}", actor_param.unwrap())
            } else {
                self.output.push_str(&format!("    struct __AsyncEnv_{}* __async_env = (struct __AsyncEnv_{}*)malloc(sizeof(struct __AsyncEnv_{}));\n", c_name, c_name, c_name));
                for param in func.params.iter() {
                    self.output.push_str(&format!("    __async_env->_{} = _{};\n", param.0, param.0));
                }
                if func.env_layout.is_some() {
                    self.output.push_str("    __async_env->__env = __env;\n");
                }
                "__async_env".to_string()
            };
            if is_actor_method {
                if func.return_type == Ty::Void {
                    self.output.push_str(&format!("    pace_send_actor_message_detached((void*)_{}, __async_body_{}, {});\n    return NULL;\n", actor_param.unwrap(), c_name, env_arg));
                } else {
                    self.output.push_str(&format!("    return pace_send_actor_message((void*)_{}, __async_body_{}, {});\n", actor_param.unwrap(), c_name, env_arg));
                }
            } else {
                self.output.push_str(&format!("    return pace_spawn_fiber(__async_body_{}, {});\n", c_name, env_arg));
            }
            self.output.push_str("}\n");
        }
    }
    pub(super) fn generate_block(&mut self, block: &BasicBlock, locals: &[Ty]) {
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
                        if name == "print" || name == "println" || name == "assert" {
                            is_void = true;
                        }
                    }
                    if matches!(rval, Rvalue::ListSet(_, _, _) | Rvalue::MapSet(_, _, _)) {
                        is_void = true;
                    }

                    if is_void
                        && matches!(
                            rval,
                            Rvalue::Use(_) | Rvalue::ResultOk(_) | Rvalue::Constant(_)
                        )
                    {
                        self.output.push_str("/* void */;\n");
                        continue;
                    }

                    let mut has_opt_buf = false;
                    if let Rvalue::BuiltinCall(name, args) = rval {
                        if name == "interpolate_string" {
                            let mut seen = std::collections::HashSet::new();
                            for arg in args {
                                if !seen.insert(arg.0) {
                                    continue;
                                }
                                let raw_ty = &locals[arg.0 as usize];
                                if Self::is_primitive_opt(raw_ty).is_some() {
                                    if let Ty::Optional(inner) = raw_ty {
                                        if matches!(inner.as_ref(), Ty::Int | Ty::Float) {
                                            if !has_opt_buf {
                                                self.output.push_str("    {\n");
                                                has_opt_buf = true;
                                            }
                                            if matches!(inner.as_ref(), Ty::Int) {
                                                writeln!(
                                                    &mut self.output,
                                                    "    char __opt_buf_{}[64]; if (_{}.has_value) snprintf(__opt_buf_{}, sizeof(__opt_buf_{}), \"%lld\", (long long)_{}.value); else snprintf(__opt_buf_{}, sizeof(__opt_buf_{}), \"null\");",
                                                    arg.0, arg.0, arg.0, arg.0, arg.0, arg.0, arg.0
                                                ).unwrap();
                                            } else {
                                                writeln!(
                                                    &mut self.output,
                                                    "    char __opt_buf_{}[64]; if (_{}.has_value) snprintf(__opt_buf_{}, sizeof(__opt_buf_{}), \"%f\", _{}.value); else snprintf(__opt_buf_{}, sizeof(__opt_buf_{}), \"null\");",
                                                    arg.0, arg.0, arg.0, arg.0, arg.0, arg.0, arg.0
                                                ).unwrap();
                                            }
                                        }
                                    }
                                }
                            }
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
                        if let Lvalue::Local(local) = lval {
                            let dest_ty = &locals[local.0 as usize];
                            if let Some(opt_name) = Self::is_primitive_opt(dest_ty) {
                                if matches!(rval, Rvalue::ListGet(_, _) | Rvalue::MapGet(_, _))
                                    || matches!(rval, Rvalue::BuiltinCall(name, _) if name == "pace_list_pop" || name == "pace_list_remove_at")
                                {
                                    let helper = match opt_name {
                                        "PaceOptFloat" => "pace_opt_float_from_ptr",
                                        "PaceOptBool" => "pace_opt_bool_from_ptr",
                                        _ => "pace_opt_int_from_ptr",
                                    };
                                    write!(&mut self.output, "{}(", helper).unwrap();
                                    self.generate_rvalue(rval, locals, None);
                                    self.output.push_str(");\n");
                                    continue;
                                } else if let Rvalue::Use(src) = rval {
                                    let src_ty = &locals[src.0 as usize];
                                    if matches!(src_ty, Ty::Optional(inner) if **inner == Ty::Void) {
                                        writeln!(&mut self.output, "({}){{0, 0}};", opt_name).unwrap();
                                        continue;
                                    } else if Self::is_primitive_opt(src_ty).is_none() {
                                        writeln!(&mut self.output, "({}){{1, _{}}};", opt_name, src.0).unwrap();
                                        continue;
                                    }
                                } else if matches!(rval, Rvalue::Constant(_)) {
                                    write!(&mut self.output, "({}){{1, ", opt_name).unwrap();
                                    self.generate_rvalue(rval, locals, None);
                                    self.output.push_str("};\n");
                                    continue;
                                }
                            } else if matches!(dest_ty, Ty::Optional(inner) if Self::is_boxed_collection_ty(inner)) {
                                if let Rvalue::Use(src) = rval {
                                    let src_ty = &locals[src.0 as usize];
                                    if matches!(src_ty, Ty::Optional(inner) if **inner == Ty::Void) {
                                        writeln!(&mut self.output, "NULL;").unwrap();
                                        continue;
                                    } else if Self::is_boxed_collection_ty(src_ty) {
                                        let boxed = self.emit_box_for_collection(*src, src_ty);
                                        writeln!(&mut self.output, "{};", boxed).unwrap();
                                        continue;
                                    }
                                }
                            } else if matches!(dest_ty, Ty::Int | Ty::Float | Ty::Bool) {
                                if let Rvalue::Use(src) = rval {
                                    if Self::is_primitive_opt(&locals[src.0 as usize]).is_some() {
                                        writeln!(&mut self.output, "_{}.value;", src.0).unwrap();
                                        continue;
                                    }
                                }
                            } else if Self::is_boxed_collection_ty(dest_ty) {
                                if let Rvalue::Use(src) = rval {
                                    if matches!(&locals[src.0 as usize], Ty::Optional(inner) if Self::is_boxed_collection_ty(inner)) {
                                        writeln!(&mut self.output, "*_{};", src.0).unwrap();
                                        continue;
                                    }
                                }
                            } else if let Ty::Class(dest_id) = dest_ty {
                                if let Rvalue::Use(src) = rval
                                    && let Ty::Class(src_id) = &locals[src.0 as usize]
                                    && src_id != dest_id
                                {
                                    let c_dest_ty = self.emit_c_type(dest_ty);
                                    writeln!(&mut self.output, "({}) _{};", c_dest_ty, src.0).unwrap();
                                    continue;
                                }
                            }
                            if matches!(rval, Rvalue::ResultOk(_)) {
                                let c_dest_ty = self.emit_c_type(dest_ty);
                                if c_dest_ty.contains('*') {
                                    write!(&mut self.output, "({})", c_dest_ty).unwrap();
                                } else if Self::is_boxed_collection_ty(dest_ty)
                                    || Self::is_primitive_opt(dest_ty).is_some()
                                    || matches!(dest_ty, Ty::Float | Ty::Result(_, _))
                                {
                                    write!(&mut self.output, "*({}*)", c_dest_ty).unwrap();
                                } else {
                                    write!(&mut self.output, "({})(intptr_t)", c_dest_ty).unwrap();
                                }
                            } else if matches!(rval, Rvalue::ListGet(_, _) | Rvalue::MapGet(_, _))
                                || matches!(rval, Rvalue::BuiltinCall(name, _) if name == "pace_list_pop" || name == "pace_list_remove_at")
                            {
                                let c_dest_ty = self.emit_c_type(dest_ty);
                                if Self::is_boxed_collection_ty(dest_ty) {
                                    write!(&mut self.output, "*({}*)", c_dest_ty).unwrap();
                                } else if !c_dest_ty.contains('*') {
                                    write!(&mut self.output, "({})(intptr_t)", c_dest_ty).unwrap();
                                } else {
                                    write!(&mut self.output, "({})", c_dest_ty).unwrap();
                                }
                            }
                        }
                    }

                    let dest_ty = if let Lvalue::Local(l) = lval {
                        Some(&locals[l.0 as usize])
                    } else {
                        None
                    };
                    self.generate_rvalue(rval, locals, dest_ty);
                    self.output.push_str(";\n");
                    if has_opt_buf {
                        self.output.push_str("    }\n");
                    }
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
                Statement::BoxSet(cell, val) => {
                    let c_ty = self.emit_c_type(&locals[val.0 as usize]);
                    writeln!(&mut self.output, "    *({}*)_{} = _{};", c_ty, cell.0, val.0).unwrap();
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

    fn generate_rvalue(&mut self, rvalue: &Rvalue, locals: &[Ty], dest_ty: Option<&Ty>) {
        match rvalue {
            Rvalue::Use(local) => write!(&mut self.output, "_{}", local.0).unwrap(),
            Rvalue::UnaryOp(op, local) => match op {
                pace_ast::UnaryOp::Neg => write!(&mut self.output, "-_{}", local.0).unwrap(),
                pace_ast::UnaryOp::Not => write!(&mut self.output, "!_{}", local.0).unwrap(),
            },
            Rvalue::Constant(Constant::Int(val)) => write!(&mut self.output, "{}", val).unwrap(),
            Rvalue::Constant(Constant::Float(val)) => write!(&mut self.output, "{}", val).unwrap(),
            Rvalue::Constant(Constant::Bool(val)) => {
                write!(&mut self.output, "{}", if *val { "1" } else { "0" }).unwrap()
            }
            Rvalue::Constant(Constant::String(val)) => {
                let unquoted = val
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .unwrap_or(val);
                write!(&mut self.output, "pace_string_new(\"{}\")", unquoted).unwrap();
            }
            Rvalue::BinaryOp(op, lhs, rhs) => {
                if matches!(op, BinaryOp::NullCoalesce) {
                    if Self::is_primitive_opt(&locals[lhs.0 as usize]).is_some() {
                        if Self::is_primitive_opt(&locals[rhs.0 as usize]).is_some() {
                            write!(
                                &mut self.output,
                                "(_{}.has_value ? _{} : _{})",
                                lhs.0, lhs.0, rhs.0
                            )
                            .unwrap();
                        } else {
                            write!(
                                &mut self.output,
                                "(_{}.has_value ? _{}.value : _{})",
                                lhs.0, lhs.0, rhs.0
                            )
                            .unwrap();
                        }
                    } else if matches!(&locals[lhs.0 as usize], Ty::Optional(inner) if Self::is_boxed_collection_ty(inner))
                        && Self::is_boxed_collection_ty(&locals[rhs.0 as usize])
                    {
                        write!(
                            &mut self.output,
                            "(_{} != NULL ? *_{} : _{})",
                            lhs.0, lhs.0, rhs.0
                        )
                        .unwrap();
                    } else {
                        write!(
                            &mut self.output,
                            "_{} != 0 ? _{} : _{}",
                            lhs.0, lhs.0, rhs.0
                        )
                        .unwrap();
                    }
                } else if matches!(op, BinaryOp::And) {
                    write!(&mut self.output, "_{} && _{}", lhs.0, rhs.0).unwrap();
                } else if matches!(op, BinaryOp::Or) {
                    write!(&mut self.output, "_{} || _{}", lhs.0, rhs.0).unwrap();
                } else {
                    let is_string = matches!(locals[lhs.0 as usize], Ty::String)
                        || matches!(locals[rhs.0 as usize], Ty::String)
                        || matches!(&locals[lhs.0 as usize], Ty::Optional(inner) if matches!(inner.as_ref(), Ty::String))
                        || matches!(&locals[rhs.0 as usize], Ty::Optional(inner) if matches!(inner.as_ref(), Ty::String));
                    let is_float = matches!(locals[lhs.0 as usize], Ty::Float)
                        || matches!(locals[rhs.0 as usize], Ty::Float);
                    let lhs_opt = Self::is_primitive_opt(&locals[lhs.0 as usize]).is_some();
                    let rhs_opt = Self::is_primitive_opt(&locals[rhs.0 as usize]).is_some();
                    let lhs_null = matches!(&locals[lhs.0 as usize], Ty::Optional(inner) if **inner == Ty::Void);
                    let rhs_null = matches!(&locals[rhs.0 as usize], Ty::Optional(inner) if **inner == Ty::Void);

                    if matches!(op, BinaryOp::EqEq | BinaryOp::NotEq) && (lhs_opt || rhs_opt) {
                        let is_eq = matches!(op, BinaryOp::EqEq);
                        if lhs_opt && rhs_null {
                            write!(&mut self.output, "(_{}.has_value {} 0)", lhs.0, if is_eq { "==" } else { "!=" }).unwrap();
                        } else if rhs_opt && lhs_null {
                            write!(&mut self.output, "(_{}.has_value {} 0)", rhs.0, if is_eq { "==" } else { "!=" }).unwrap();
                        } else if lhs_opt && rhs_opt {
                            if is_eq {
                                write!(&mut self.output, "(_{}.has_value == _{}.has_value && (!_{}.has_value || _{}.value == _{}.value))", lhs.0, rhs.0, lhs.0, lhs.0, rhs.0).unwrap();
                            } else {
                                write!(&mut self.output, "(_{}.has_value != _{}.has_value || (_{}.has_value && _{}.value != _{}.value))", lhs.0, rhs.0, lhs.0, lhs.0, rhs.0).unwrap();
                            }
                        } else if lhs_opt {
                            if is_eq {
                                write!(&mut self.output, "(_{}.has_value && _{}.value == _{})", lhs.0, lhs.0, rhs.0).unwrap();
                            } else {
                                write!(&mut self.output, "(!_{}.has_value || _{}.value != _{})", lhs.0, lhs.0, rhs.0).unwrap();
                            }
                        } else if is_eq {
                            write!(&mut self.output, "(_{}.has_value && _{} == _{}.value)", rhs.0, lhs.0, rhs.0).unwrap();
                        } else {
                            write!(&mut self.output, "(!_{}.has_value || _{} != _{}.value)", rhs.0, lhs.0, rhs.0).unwrap();
                        }
                    } else if is_string && matches!(op, BinaryOp::EqEq) {
                        write!(&mut self.output, "pace_string_equals(_{}, _{})", lhs.0, rhs.0).unwrap();
                    } else if is_string && matches!(op, BinaryOp::NotEq) {
                        write!(&mut self.output, "!pace_string_equals(_{}, _{})", lhs.0, rhs.0).unwrap();
                    } else if is_float && matches!(op, BinaryOp::Mod) {
                        write!(&mut self.output, "fmod(_{}, _{})", lhs.0, rhs.0).unwrap();
                    } else {
                        let op_str = match op {
                            BinaryOp::Add => "+",
                            BinaryOp::Sub => "-",
                            BinaryOp::Mul => "*",
                            BinaryOp::Div => "/",
                            BinaryOp::Mod => "%",
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
            }
            Rvalue::OptionalFieldAccess(obj, field) => {
                let obj_ty = &locals[obj.0 as usize];
                let access = if matches!(obj_ty, Ty::Class(_) | Ty::Optional(_)) {
                    format!("_{}->{}", obj.0, field)
                } else {
                    format!("_{}.{}", obj.0, field)
                };
                if let Some(dty) = dest_ty {
                    if let Some(opt_name) = Self::is_primitive_opt(dty) {
                        write!(
                            &mut self.output,
                            "(_{} != NULL ? ({}){{1, {}}} : ({}){{0, 0}})",
                            obj.0, opt_name, access, opt_name
                        )
                        .unwrap();
                        return;
                    }
                    if let Ty::Optional(inner) = dty {
                        if Self::is_boxed_collection_ty(inner) {
                            let c_fty = self.emit_c_type(inner);
                            write!(
                                &mut self.output,
                                "(_{} != NULL ? ({}*)memcpy(pace_alloc(sizeof({}), NULL), &({}), sizeof({})) : NULL)",
                                obj.0, c_fty, c_fty, access, c_fty
                            )
                            .unwrap();
                            return;
                        }
                    }
                }
                write!(&mut self.output, "(_{} != NULL ? {} : NULL)", obj.0, access).unwrap();
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
                        let raw_ty = &locals[arg.0 as usize];
                        let is_prim_opt = Self::is_primitive_opt(raw_ty).is_some();
                        let val_expr = if is_prim_opt {
                            format!("_{}.value", arg.0)
                        } else {
                            format!("_{}", arg.0)
                        };
                        let mut ty = raw_ty;
                        if let Ty::Optional(inner) = ty {
                            ty = inner;
                        }
                        if matches!(ty, Ty::Int) {
                            if is_prim_opt {
                                format_str.push_str("%s");
                                type_args.push(format!("__opt_buf_{}", arg.0));
                            } else {
                                format_str.push_str("%lld");
                                type_args.push(val_expr);
                            }
                        } else if matches!(ty, Ty::Float) {
                            if is_prim_opt {
                                format_str.push_str("%s");
                                type_args.push(format!("__opt_buf_{}", arg.0));
                            } else {
                                format_str.push_str("%f");
                                type_args.push(val_expr);
                            }
                        } else if matches!(ty, Ty::String) {
                            format_str.push_str("%s");
                            type_args.push(format!("(_{} ? _{}->data : \"null\")", arg.0, arg.0));
                        } else if matches!(ty, Ty::Bool) {
                            format_str.push_str("%s");
                            if is_prim_opt {
                                type_args.push(format!("(_{}.has_value ? (_{}.value ? \"true\" : \"false\") : \"null\")", arg.0, arg.0));
                            } else {
                                type_args.push(format!("_{} ? \"true\" : \"false\"", arg.0));
                            }
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
                } else if name == "pace_list_push" {
                    let val_ty = &locals[args[1].0 as usize];
                    let val_cast = self.emit_box_for_collection(args[1], val_ty);
                    write!(&mut self.output, "pace_list_push(_{}, {})", args[0].0, val_cast).unwrap();
                    return;
                } else if name == "pace_list_insert" {
                    let val_ty = &locals[args[2].0 as usize];
                    let val_cast = self.emit_box_for_collection(args[2], val_ty);
                    write!(
                        &mut self.output,
                        "pace_list_insert(_{}, _{}, {})",
                        args[0].0, args[1].0, val_cast
                    )
                    .unwrap();
                    return;
                } else if name == "pace_list_pop" {
                    write!(&mut self.output, "pace_list_pop(_{})", args[0].0).unwrap();
                    return;
                } else if name == "ok" {
                    if args.is_empty() {
                        self.output.push_str("(PaceResult){1, NULL, NULL}");
                    } else {
                        let boxed = self.emit_box_for_result(args[0], &locals[args[0].0 as usize]);
                        write!(&mut self.output, "(PaceResult){{1, {}, NULL}}", boxed).unwrap();
                    }
                    return;
                } else if name == "error" {
                    let boxed = self.emit_box_for_result(args[0], &locals[args[0].0 as usize]);
                    write!(&mut self.output, "(PaceResult){{0, NULL, {}}}", boxed).unwrap();
                    return;
                } else if name == "assert" {
                    if args.len() == 1 {
                        write!(&mut self.output, "pace_assert(_{}, NULL)", args[0].0).unwrap();
                    } else {
                        write!(&mut self.output, "pace_assert(_{}, _{})", args[0].0, args[1].0).unwrap();
                    }
                    return;
                }

                if (name == "print" || name == "println") && args.len() == 1 {
                    if Self::is_primitive_opt(&locals[args[0].0 as usize]).is_some() {
                        let inner_fn = match &locals[args[0].0 as usize] {
                            Ty::Optional(inner) if matches!(inner.as_ref(), Ty::Float) => "pace_print_float",
                            Ty::Optional(inner) if matches!(inner.as_ref(), Ty::Bool) => "pace_print_bool",
                            _ => "pace_print_int",
                        };
                        write!(
                            &mut self.output,
                            "if (_{}.has_value) {{ {}(_{}.value); }} else {{ struct PaceString* __tmp_null = pace_string_new(\"null\"); pace_print_string(__tmp_null); pace_release(__tmp_null); }}",
                            args[0].0, inner_fn, args[0].0
                        )
                        .unwrap();
                        return;
                    }
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
                let is_ptr = matches!(
                    locals[obj.0 as usize],
                    Ty::Class(_) | Ty::Actor(_) | Ty::Optional(_)
                );
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
                        write!(&mut self.output, ", PTHREAD_MUTEX_INITIALIZER, NULL, NULL, 0").unwrap();
                    }
                    if !fields.is_empty() {
                        for arg in fields {
                            write!(&mut self.output, ", _{}", arg.0).unwrap();
                        }
                    }
                    write!(&mut self.output, " }}, sizeof(struct pace_{}))", id.0).unwrap();
                }
                _ => {
                    eprintln!("ICE: Instantiating non-struct/class");
                }
            },
            Rvalue::StackInstantiate(ty, fields) => {
                if let Ty::Class(id) = ty {
                    write!(&mut self.output, "&(struct pace_{}){{ &pace_{}_vtable_inst", id.0, id.0).unwrap();
                    for arg in fields {
                        write!(&mut self.output, ", _{}", arg.0).unwrap();
                    }
                    self.output.push_str(" }");
                } else {
                    self.output.push_str("NULL");
                }
            }
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
            Rvalue::ListNew(elem_ty, cap_local) => {
                let is_ref = matches!(elem_ty, Ty::Class(_) | Ty::String | Ty::List(_) | Ty::Map(_, _) | Ty::Set(_))
                    || Self::is_boxed_collection_ty(elem_ty);
                let dtor = if is_ref { "(void (*)(void*))pace_release" } else { "NULL" };
                write!(&mut self.output, "pace_list_new(_{}, {})", cap_local.0, dtor).unwrap();
            }
            Rvalue::ListGet(list_local, idx_local) => {
                write!(&mut self.output, "pace_list_get(_{}, _{})", list_local.0, idx_local.0).unwrap();
            }
            Rvalue::ListSet(list_local, idx_local, val_local) => {
                let val_ty = &locals[val_local.0 as usize];
                let val_cast = self.emit_box_for_collection(*val_local, val_ty);
                write!(&mut self.output, "pace_list_set(_{}, _{}, {})", list_local.0, idx_local.0, val_cast).unwrap();
            }
            Rvalue::MapNew(k_ty, v_ty) => {
                let key_is_str = if matches!(k_ty, Ty::String) { "1" } else { "0" };
                let is_ref = matches!(v_ty, Ty::Class(_) | Ty::String | Ty::List(_) | Ty::Map(_, _) | Ty::Set(_))
                    || Self::is_boxed_collection_ty(v_ty);
                let dtor = if is_ref { "(void (*)(void*))pace_release" } else { "NULL" };
                write!(&mut self.output, "pace_map_new(16, {}, {})", key_is_str, dtor).unwrap();
            }
            Rvalue::SetNew(elem_ty) => {
                let key_kind = match elem_ty {
                    Ty::String => "1",
                    Ty::Float => "2",
                    _ => "0",
                };
                write!(&mut self.output, "pace_set_new(16, {})", key_kind).unwrap();
            }
            Rvalue::MapGet(map_local, key_local) => {
                let key_ty = &locals[key_local.0 as usize];
                if matches!(key_ty, Ty::String) {
                    write!(&mut self.output, "pace_map_get_str(_{}, _{}, NULL)", map_local.0, key_local.0).unwrap();
                } else {
                    write!(&mut self.output, "pace_map_get_int(_{}, _{}, NULL)", map_local.0, key_local.0).unwrap();
                }
            }
            Rvalue::MapSet(map_local, key_local, val_local) => {
                let key_ty = &locals[key_local.0 as usize];
                let val_ty = &locals[val_local.0 as usize];
                let val_cast = self.emit_box_for_collection(*val_local, val_ty);
                if matches!(key_ty, Ty::String) {
                    write!(&mut self.output, "pace_map_set_str(_{}, _{}, {})", map_local.0, key_local.0, val_cast).unwrap();
                } else {
                    write!(&mut self.output, "pace_map_set_int(_{}, _{}, {})", map_local.0, key_local.0, val_cast).unwrap();
                }
            }
            Rvalue::ResultIsOk(res) => {
                write!(&mut self.output, "_{}.is_ok", res.0).unwrap();
            }
            Rvalue::ResultOk(res) => {
                write!(&mut self.output, "_{}.value", res.0).unwrap();
            }
            Rvalue::ResultPropagateErr(res) => {
                write!(&mut self.output, "(PaceResult){{0, NULL, _{}.error}}", res.0).unwrap();
            }
            Rvalue::OptionIsSome(opt) => {
                if Self::is_primitive_opt(&locals[opt.0 as usize]).is_some() {
                    write!(&mut self.output, "_{}.has_value", opt.0).unwrap();
                } else {
                    write!(&mut self.output, "(_{} != NULL)", opt.0).unwrap();
                }
            }
            Rvalue::BoxNew(src) => {
                let c_ty = self.emit_c_type(&locals[src.0 as usize]);
                write!(
                    &mut self.output,
                    "memcpy(pace_alloc(sizeof({}), NULL), &_{}, sizeof({}))",
                    c_ty, src.0, c_ty
                )
                .unwrap();
            }
            Rvalue::BoxGet(cell, ty) => {
                let c_ty = self.emit_c_type(ty);
                write!(&mut self.output, "*({}*)_{}", c_ty, cell.0).unwrap();
            }
        }
    }
}
