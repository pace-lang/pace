use crate::mir::*;
use pace_hir::HirId;
use pace_ty::Ty;
use std::collections::HashMap;

use super::{tc_get_type, MirBuilder};

impl<'a> MirBuilder<'a> {
    pub fn build_program(program: &pace_hir::Program, tc: &mut pace_ty::TypeChecker) -> MirProgram {
        let mut declarations: Vec<pace_hir::Decl> = Vec::new();
        for module in program.modules.values() {
            for decl in &module.declarations {
                declarations.push(decl.clone());
            }
        }

        let mut global_fns = HashMap::new();
        for decl in declarations.iter().chain(tc.instantiated_generics.iter()) {
            if let pace_hir::Decl::Function { name, .. } = decl {
                global_fns.insert(name.to_string(), name.to_string());
            } else if let pace_hir::Decl::Struct { methods, .. } = decl {
                for method in methods {
                    if let pace_hir::Decl::Function { name, .. } = method {
                        global_fns.insert(name.to_string(), name.to_string());
                    }
                }
            } else if let pace_hir::Decl::Class { methods, .. } = decl {
                for method in methods {
                    if let pace_hir::Decl::Function { name, .. } = method {
                        global_fns.insert(name.to_string(), name.to_string());
                    }
                }
            } else if let pace_hir::Decl::Actor { methods, .. } = decl {
                for method in methods {
                    if let pace_hir::Decl::Function { name, .. } = method {
                        global_fns.insert(name.to_string(), name.to_string());
                    }
                }
            }
        }

        let mut functions = Vec::new();
        let mut global_vars = Vec::new();
        let mut main_builder = MirBuilder::new(
            global_fns.clone(),
            &tc.struct_defs,
            &tc.class_defs,
            &tc.class_vtables,
            &tc.enum_defs,
            &tc.env,
            &tc.local_types,
            &tc.named_types,
            &tc.static_fields_env,
            &tc.methods_env,
            &tc.class_parents,
            &tc.global_functions,
            &tc.global_vars,
            &tc.resolved_global_names,
        );
        let mut _main_last_local = Local(0);

        for decl in declarations.iter().chain(tc.instantiated_generics.iter()) {
            match decl {
                pace_hir::Decl::Let { id, name, value, .. }
                | pace_hir::Decl::Var { id, name, value, .. } => {
                    let global_name = tc
                        .resolved_global_names
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| name.to_string());
                    if let Some(val) = value {
                        let rval_local = main_builder.build_expr(val);
                        let var_ty = main_builder.locals[rval_local.0 as usize].clone();
                        global_vars.push((global_name.clone(), var_ty.clone()));
                        main_builder.push_stmt(Statement::GlobalWrite(global_name, rval_local));
                        _main_last_local = rval_local;
                    } else {
                        // Uninitialized global variable
                        let var_ty = main_builder
                            .local_types
                            .get(id)
                            .expect("Type not found in local_types")
                            .clone();
                        global_vars.push((global_name, var_ty));
                    }
                }
                pace_hir::Decl::Const {
                    id, name, value, ..
                } => {
                    let global_name = format!("{}", name);
                    let ty = tc.env.get(id).cloned().unwrap_or(Ty::Int);
                    global_vars.push((global_name.clone(), ty.clone()));
                    let rval_local = main_builder.build_expr(value);
                    main_builder.push_stmt(Statement::GlobalWrite(global_name, rval_local));
                }
                pace_hir::Decl::Struct {
                    id,
                    name,
                    static_fields,
                    const_fields,
                    methods,
                    generic_params,
                    ..
                }
                | pace_hir::Decl::Class {
                    id,
                    name,
                    methods,
                    static_fields,
                    const_fields,
                    generic_params,
                    ..
                } => {
                    let mut mangled_name = name.to_string();
                    for (k, &(tid, _)) in &tc.named_types {
                        if tid == *id {
                            mangled_name = k.clone();
                            break;
                        }
                    }
                    if generic_params.is_some() {
                        continue;
                    }

                    for pace_hir::HirStaticFieldDef {
                        name: sf_name,
                        ty: sf_ty,
                        value: sf_expr,
                        ..
                    } in static_fields
                    {
                        let global_name = format!("{}_{}", mangled_name, sf_name);
                        let ty = tc_get_type(tc, &sf_ty).unwrap_or(Ty::Int);
                        global_vars.push((global_name.clone(), ty.clone()));

                        let rval_local = main_builder.build_expr(&sf_expr);
                        main_builder.push_stmt(Statement::GlobalWrite(global_name, rval_local));
                    }
                    for pace_hir::HirConstFieldDef {
                        name: cf_name,
                        ty: cf_ty,
                        value: cf_expr,
                        ..
                    } in const_fields
                    {
                        let global_name = format!("{}_{}", mangled_name, cf_name);
                        let ty = tc_get_type(tc, &cf_ty).unwrap_or(Ty::Int);
                        global_vars.push((global_name.clone(), ty.clone()));

                        let rval_local = main_builder.build_expr(&cf_expr);
                        main_builder.push_stmt(Statement::GlobalWrite(global_name, rval_local));
                    }
                    for method in methods {
                        if let pace_hir::Decl::Function {
                            name,
                            id: func_id,
                            params,
                            return_type,
                            body,
                            is_async,
                            ..
                        } = method
                        {
                            let mut fn_builder = MirBuilder::new(
                                global_fns.clone(),
                                &tc.struct_defs,
                                &tc.class_defs,
                                &tc.class_vtables,
                                &tc.enum_defs,
                                &tc.env,
                                &tc.local_types,
                                &tc.named_types,
                                &tc.static_fields_env,
                                &tc.methods_env,
                                &tc.class_parents,
                                &tc.global_functions,
                                &tc.global_vars,
                                &tc.resolved_global_names,
                            );
                            let mut mir_params = Vec::new();
                            for (param_id, param_name, _pty) in params {
                                let ty = tc.local_types.get(param_id).cloned().unwrap_or(Ty::Int);
                                let local = fn_builder.new_local(ty);
                                if param_name == "self" {
                                    fn_builder.current_self_local = Some(local);
                                }
                                fn_builder.hir_to_local.insert(*param_id, local);
                                mir_params.push(local);
                            }

                            let ret_ty = if let Some(rty) = return_type {
                                tc_get_type(tc, rty).unwrap_or(Ty::Void)
                            } else {
                                Ty::Void
                            };
                            collect_captured_vars_in_block(body, &mut fn_builder.captured_vars_in_scope);
                            collect_captured_vars_in_block(body, &mut fn_builder.captured_vars_in_scope);
                    fn_builder.build_block(body);
                            let extracted_closures = std::mem::take(&mut fn_builder.closure_functions);
                            let fn_body = fn_builder.finish(&mir_params, &ret_ty);

                            functions.push(MirFunction {
                                name: tc.resolved_global_names.get(func_id).unwrap_or(&name.to_string()).clone(),
                                params: mir_params,
                                return_type: ret_ty,
                                is_async: *is_async,
                                env_layout: None,
                                body: fn_body,
                            });
                            functions.extend(extracted_closures);
                        }
                    }
                }
                pace_hir::Decl::Expr(expr, _) => {
                    let rval_local = main_builder.build_expr(expr);
                    _main_last_local = rval_local;
                }
                pace_hir::Decl::Function {
                    name,
                    id: func_id,
                    params,
                    return_type,
                    body,
                    generic_params,
                    is_async,
                    ..
                } => {
                    if generic_params.is_some() {
                        continue;
                    }
                    let mut fn_builder = MirBuilder::new(
                        global_fns.clone(),
                        &tc.struct_defs,
                        &tc.class_defs,
                        &tc.class_vtables,
                        &tc.enum_defs,
                        &tc.env,
                        &tc.local_types,
                        &tc.named_types,
                        &tc.static_fields_env,
                        &tc.methods_env,
                        &tc.class_parents,
                        &tc.global_functions,
                        &tc.global_vars,
                        &tc.resolved_global_names,
                    );
                    let mut mir_params = Vec::new();
                    for (param_id, param_name, _pty) in params {
                        let ty = tc.local_types.get(param_id).cloned().unwrap_or(Ty::Int);
                        let local = fn_builder.new_local(ty);
                        if param_name == "self" {
                            fn_builder.current_self_local = Some(local);
                        }
                        fn_builder.hir_to_local.insert(*param_id, local);
                        mir_params.push(local);
                    }

                    let ret_ty = if let Some(Ty::Function(_, ret)) = tc
                        .env
                        .get(func_id)
                        .or_else(|| tc.methods_env.get(&name.to_string()))
                    {
                        *ret.clone()
                    } else if let Some(rty) = return_type {
                        tc_get_type(tc, rty).unwrap_or(Ty::Void)
                    } else {
                        Ty::Void
                    };
                    collect_captured_vars_in_block(body, &mut fn_builder.captured_vars_in_scope);
                    fn_builder.build_block(body);
                    let extracted_closures = std::mem::take(&mut fn_builder.closure_functions);
                    let fn_body = fn_builder.finish(&mir_params, &ret_ty);
                    functions.push(MirFunction {
                        name: tc
                            .resolved_global_names
                            .get(func_id)
                            .cloned()
                            .unwrap_or_else(|| name.to_string()),
                        params: mir_params,
                        return_type: ret_ty,
                        is_async: *is_async,
                        env_layout: None,
                        body: fn_body,
                    });
                    functions.extend(extracted_closures);
                }
                pace_hir::Decl::Enum { .. } => {}

                pace_hir::Decl::Actor {
                    id: _,
                    name: _,
                    methods,
                    generic_params,
                    ..
                } => {
                    if generic_params.is_some() {
                        continue;
                    }

                    for method in methods {
                        if let pace_hir::Decl::Function {
                            name,
                            id: func_id,
                            params,
                            return_type: _,
                            body,
                            is_async,
                            ..
                        } = method
                        {
                            let mut builder = MirBuilder::new(
                                global_fns.clone(),
                                &tc.struct_defs,
                                &tc.class_defs,
                                &tc.class_vtables,
                                &tc.enum_defs,
                                &tc.env,
                                &tc.local_types,
                                &tc.named_types,
                                &tc.static_fields_env,
                                &tc.methods_env,
                                &tc.class_parents,
                                &tc.global_functions,
                                &tc.global_vars,
                                &tc.resolved_global_names,
                            );
                            
                            let mut mir_params = Vec::new();
                            for (p_id, param_name, _pty) in params {
                                let param_ty = tc.local_types.get(p_id).cloned().unwrap_or(Ty::Int);
                                let param_local = builder.new_local(param_ty);
                                if param_name == "self" {
                                    builder.current_self_local = Some(param_local);
                                }
                                builder.hir_to_local.insert(*p_id, param_local);
                                mir_params.push(param_local);
                            }
                            
                            let ret_ty = match tc.env.get(func_id) {
                                Some(Ty::Function(_, r)) => *r.clone(),
                                _ => Ty::Void,
                            };
                            builder.build_block(body);
                            let extracted_closures = std::mem::take(&mut builder.closure_functions);
                            let fn_body = builder.finish(&mir_params, &ret_ty);
                            
                            functions.push(MirFunction {
                                name: tc.resolved_global_names.get(func_id).unwrap_or(&name.to_string()).clone(),
                                params: mir_params,
                                return_type: ret_ty,
                                env_layout: None,
                                body: MirBody {
                                    locals: fn_body.locals,
                                    blocks: fn_body.blocks,
                                },
                                is_async: *is_async,
                            });
                            functions.extend(extracted_closures);
                        }
                    }
                }

                pace_hir::Decl::Trait { .. } => {}
                pace_hir::Decl::Import { .. } => {}
            }
        }
        
        let extracted_closures = std::mem::take(&mut main_builder.closure_functions);
        functions.extend(extracted_closures);

        let mut resolved_enum_defs = std::collections::HashMap::new();
        for (id, variants) in &tc.enum_defs {
            let mut res_variants = Vec::new();
            for v in variants {
                let res_fields = v.fields.as_ref().map(|fields| {
                    fields
                        .iter()
                        .map(|(name, ty)| {
                            (name.to_string(), tc_get_type(tc, ty).unwrap_or(Ty::Int))
                        })
                        .collect()
                });
                res_variants.push((v.name.to_string(), res_fields));
            }
            resolved_enum_defs.insert(*id, res_variants);
        }

        let mut actor_defs = std::collections::HashSet::new();
        for (_, &(tid, kind)) in &tc.named_types {
            if kind == 3 {
                actor_defs.insert(tid);
            }
        }

        MirProgram {
            functions,
            main_body: main_builder.finish(&[], &Ty::Void),
            struct_defs: tc.struct_defs.clone(),
            class_defs: tc.class_defs.clone(),
            actor_defs,
            class_vtables: tc.class_vtables.clone(),
            enum_defs: resolved_enum_defs,
            global_vars,
        }
    }
}


fn collect_captured_vars_in_block(
    block: &pace_hir::Block,
    out: &mut std::collections::HashSet<HirId>,
) {
    for stmt in &block.statements {
        match stmt {
            pace_hir::Stmt::Let { value, .. } | pace_hir::Stmt::Var { value, .. } => {
                if let Some(v) = value {
                    collect_captured_vars_in_expr(v, out);
                }
            }
            pace_hir::Stmt::ExprStmt(e, _) => collect_captured_vars_in_expr(e, out),
            pace_hir::Stmt::Return(Some(e), _) => collect_captured_vars_in_expr(e, out),
            pace_hir::Stmt::Return(None, _)
            | pace_hir::Stmt::Break(_)
            | pace_hir::Stmt::Continue(_) => {}
        }
    }
}

fn collect_captured_vars_in_expr(
    expr: &pace_hir::Expr,
    out: &mut std::collections::HashSet<HirId>,
) {
    match expr {
        pace_hir::Expr::Closure {
            captured_vars,
            body,
            ..
        } => {
            out.extend(captured_vars.iter().copied());
            match body {
                pace_hir::ClosureBody::Expr(e) => collect_captured_vars_in_expr(e, out),
                pace_hir::ClosureBody::Block(b) => collect_captured_vars_in_block(b, out),
            }
        }
        pace_hir::Expr::InterpolatedString(parts, _) => {
            for p in parts {
                collect_captured_vars_in_expr(p, out);
            }
        }
        pace_hir::Expr::Binary { left, right, .. } => {
            collect_captured_vars_in_expr(left, out);
            collect_captured_vars_in_expr(right, out);
        }
        pace_hir::Expr::MemberAccess { object, .. }
        | pace_hir::Expr::OptionalMemberAccess { object, .. } => {
            collect_captured_vars_in_expr(object, out);
        }
        pace_hir::Expr::Call { callee, args, .. } => {
            collect_captured_vars_in_expr(callee, out);
            for a in args {
                collect_captured_vars_in_expr(&a.expr, out);
            }
        }
        pace_hir::Expr::BuiltinCall(_, args, _) | pace_hir::Expr::ListLiteral(args, _) => {
            for a in args {
                collect_captured_vars_in_expr(a, out);
            }
        }
        pace_hir::Expr::MapLiteral(entries, _) => {
            for (k, v) in entries {
                collect_captured_vars_in_expr(k, out);
                collect_captured_vars_in_expr(v, out);
            }
        }
        pace_hir::Expr::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            collect_captured_vars_in_expr(cond, out);
            collect_captured_vars_in_block(then_block, out);
            if let Some(eb) = else_block {
                collect_captured_vars_in_block(eb, out);
            }
        }
        pace_hir::Expr::While { cond, body, .. } => {
            collect_captured_vars_in_expr(cond, out);
            collect_captured_vars_in_block(body, out);
        }
        pace_hir::Expr::Assign { target, value, .. }
        | pace_hir::Expr::Index {
            target,
            index: value,
            ..
        } => {
            collect_captured_vars_in_expr(target, out);
            collect_captured_vars_in_expr(value, out);
        }
        pace_hir::Expr::Match { subject, arms, .. } => {
            collect_captured_vars_in_expr(subject, out);
            for arm in arms {
                collect_captured_vars_in_expr(&arm.body, out);
            }
        }
        pace_hir::Expr::Await(inner, _)
        | pace_hir::Expr::Try(inner, _)
        | pace_hir::Expr::Unary { expr: inner, .. } => {
            collect_captured_vars_in_expr(inner, out);
        }
        pace_hir::Expr::IntLiteral(_, _)
        | pace_hir::Expr::FloatLiteral(_, _)
        | pace_hir::Expr::BoolLiteral(_, _)
        | pace_hir::Expr::StringLiteral(_, _)
        | pace_hir::Expr::Null(_)
        | pace_hir::Expr::Ident(_, _, _, _)
        | pace_hir::Expr::Super(_) => {}
    }
}
