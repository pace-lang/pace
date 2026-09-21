use crate::TypeChecker;
use pace_hir::{Block, Decl, Expr, HirId, Pattern, Stmt};
use std::collections::HashMap;

pub fn reassign_decl_ids(tc: &mut TypeChecker, decl: &mut Decl) {
    let mut id_map = HashMap::new();
    reassign_decl_ids_internal(tc, decl, &mut id_map);
}

fn reassign_decl_ids_internal(
    tc: &mut TypeChecker,
    decl: &mut Decl,
    id_map: &mut HashMap<HirId, HirId>,
) {
    match decl {
        Decl::Let { id, value, .. } | Decl::Var { id, value, .. } => {
            let new_id = tc.generate_id();
            id_map.insert(*id, new_id);
            *id = new_id;
            if let Some(v) = value {
                reassign_expr_ids(tc, v, id_map);
            }
        }
        Decl::Const { id, value, .. } => {
            let new_id = tc.generate_id();
            id_map.insert(*id, new_id);
            *id = new_id;
            reassign_expr_ids(tc, value, id_map);
        }
        Decl::Struct {
            id,
            static_fields,
            const_fields,
            methods,
            ..
        } => {
            let new_id = tc.generate_id();
            id_map.insert(*id, new_id);
            *id = new_id;
            for f in static_fields {
                reassign_expr_ids(tc, &mut f.value, id_map);
            }
            for f in const_fields {
                reassign_expr_ids(tc, &mut f.value, id_map);
            }
            for m in methods {
                reassign_decl_ids_internal(tc, m, id_map);
            }
        }
        Decl::Class {
            id,
            static_fields,
            const_fields,
            methods,
            ..
        } => {
            let new_id = tc.generate_id();
            id_map.insert(*id, new_id);
            *id = new_id;
            for f in static_fields {
                reassign_expr_ids(tc, &mut f.value, id_map);
            }
            for f in const_fields {
                reassign_expr_ids(tc, &mut f.value, id_map);
            }
            for m in methods {
                reassign_decl_ids_internal(tc, m, id_map);
            }
        }
        Decl::Trait { id, methods, .. } => {
            let new_id = tc.generate_id();
            id_map.insert(*id, new_id);
            *id = new_id;
            for m in methods {
                reassign_decl_ids_internal(tc, m, id_map);
            }
        }
        Decl::Enum { id, variants, .. } => {
            let new_id = tc.generate_id();
            id_map.insert(*id, new_id);
            *id = new_id;
            for v in variants {
                let new_v_id = tc.generate_id();
                id_map.insert(v.id, new_v_id);
                v.id = new_v_id;
            }
        }
        Decl::Function {
            id, params, body, ..
        } => {
            let new_id = tc.generate_id();
            id_map.insert(*id, new_id);
            *id = new_id;
            for (p_id, _, _) in params {
                let new_p_id = tc.generate_id();
                id_map.insert(*p_id, new_p_id);
                *p_id = new_p_id;
            }
            reassign_block_ids(tc, body, id_map);
        }
        Decl::Expr(expr, _) => {
            reassign_expr_ids(tc, expr, id_map);
        }
        Decl::Import { .. } => {}
    }
}

pub fn reassign_block_ids(
    tc: &mut TypeChecker,
    block: &mut Block,
    id_map: &mut HashMap<HirId, HirId>,
) {
    for stmt in &mut block.statements {
        match stmt {
            Stmt::Let { id, value, .. } | Stmt::Var { id, value, .. } => {
                let new_id = tc.generate_id();
                id_map.insert(*id, new_id);
                *id = new_id;
                if let Some(v) = value {
                    reassign_expr_ids(tc, v, id_map);
                }
            }
            Stmt::ExprStmt(expr, _) => {
                reassign_expr_ids(tc, expr, id_map);
            }
            Stmt::Return(expr_opt, _) => {
                if let Some(expr) = expr_opt {
                    reassign_expr_ids(tc, expr, id_map);
                }
            }
        }
    }
}

pub fn reassign_expr_ids(
    tc: &mut TypeChecker,
    expr: &mut Expr,
    id_map: &mut HashMap<HirId, HirId>,
) {
    match expr {
        Expr::Ident(id, _, _, _) => {
            if let Some(new_id) = id_map.get(id) {
                *id = *new_id;
            }
        }
        Expr::InterpolatedString(exprs, _) => {
            for e in exprs {
                reassign_expr_ids(tc, e, id_map);
            }
        }
        Expr::Binary { left, right, .. } => {
            reassign_expr_ids(tc, left, id_map);
            reassign_expr_ids(tc, right, id_map);
        }
        Expr::MemberAccess { object, .. } | Expr::OptionalMemberAccess { object, .. } => {
            reassign_expr_ids(tc, object, id_map);
        }
        Expr::Call { callee, args, .. } => {
            reassign_expr_ids(tc, callee, id_map);
            for arg in args {
                reassign_expr_ids(tc, &mut arg.expr, id_map);
            }
        }
        Expr::BuiltinCall(_, args, _) => {
            for arg in args {
                reassign_expr_ids(tc, arg, id_map);
            }
        }
        Expr::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            reassign_expr_ids(tc, cond, id_map);
            reassign_block_ids(tc, then_block, id_map);
            if let Some(eb) = else_block {
                reassign_block_ids(tc, eb, id_map);
            }
        }
        Expr::While { cond, body, .. } => {
            reassign_expr_ids(tc, cond, id_map);
            reassign_block_ids(tc, body, id_map);
        }
        Expr::Assign { target, value, .. } => {
            reassign_expr_ids(tc, target, id_map);
            reassign_expr_ids(tc, value, id_map);
        }
        Expr::Match { subject, arms, .. } => {
            reassign_expr_ids(tc, subject, id_map);
            for arm in arms {
                reassign_pattern_ids(tc, &mut arm.pattern, id_map);
                reassign_expr_ids(tc, &mut arm.body, id_map);
            }
        }
        Expr::IntLiteral(..)
        | Expr::FloatLiteral(..)
        | Expr::BoolLiteral(..)
        | Expr::StringLiteral(..)
        | Expr::Null(_)
        | Expr::Super(_) => {}
    }
}

pub fn reassign_pattern_ids(
    tc: &mut TypeChecker,
    pattern: &mut Pattern,
    id_map: &mut HashMap<HirId, HirId>,
) {
    match pattern {
        Pattern::Ident(id, _, _) => {
            let new_id = tc.generate_id();
            id_map.insert(*id, new_id);
            *id = new_id;
        }
        Pattern::Variant { fields, .. } => {
            if let Some(fs) = fields {
                for (f_id, _, _) in fs {
                    let new_id = tc.generate_id();
                    id_map.insert(*f_id, new_id);
                    *f_id = new_id;
                }
            }
        }
        Pattern::CatchAll(_) => {}
    }
}
