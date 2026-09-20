use crate::TypeChecker;
use pace_hir::{Block, Decl, Expr, Pattern, Stmt};

pub fn reassign_decl_ids(tc: &mut TypeChecker, decl: &mut Decl) {
    match decl {
        Decl::Let { id, value, .. } | Decl::Var { id, value, .. } => {
            *id = tc.generate_id();
            if let Some(v) = value {
                reassign_expr_ids(tc, v);
            }
        }
        Decl::Const { id, value, .. } => {
            *id = tc.generate_id();
            reassign_expr_ids(tc, value);
        }
        Decl::Struct { id, static_fields, const_fields, methods, .. } => {
            *id = tc.generate_id();
            for f in static_fields { reassign_expr_ids(tc, &mut f.value); }
            for f in const_fields { reassign_expr_ids(tc, &mut f.value); }
            for m in methods { reassign_decl_ids(tc, m); }
        }
        Decl::Class { id, static_fields, const_fields, methods, .. } => {
            *id = tc.generate_id();
            for f in static_fields { reassign_expr_ids(tc, &mut f.value); }
            for f in const_fields { reassign_expr_ids(tc, &mut f.value); }
            for m in methods { reassign_decl_ids(tc, m); }
        }
        Decl::Trait { id, methods, .. } => {
            *id = tc.generate_id();
            for m in methods { reassign_decl_ids(tc, m); }
        }
        Decl::Enum { id, variants, .. } => {
            *id = tc.generate_id();
            for v in variants {
                v.id = tc.generate_id();
            }
        }
        Decl::Function { id, params, body, .. } => {
            *id = tc.generate_id();
            for (p_id, _, _) in params {
                *p_id = tc.generate_id();
            }
            reassign_block_ids(tc, body);
        }
        Decl::Expr(expr, _) => {
            reassign_expr_ids(tc, expr);
        }
        Decl::Import { .. } => {}
    }
}

pub fn reassign_block_ids(tc: &mut TypeChecker, block: &mut Block) {
    for stmt in &mut block.statements {
        match stmt {
            Stmt::Let { id, value, .. } | Stmt::Var { id, value, .. } => {
                *id = tc.generate_id();
                if let Some(v) = value {
                    reassign_expr_ids(tc, v);
                }
            }
            Stmt::ExprStmt(expr, _) => {
                reassign_expr_ids(tc, expr);
            }
            Stmt::Return(expr_opt, _) => {
                if let Some(expr) = expr_opt {
                    reassign_expr_ids(tc, expr);
                }
            }
        }
    }
}

pub fn reassign_expr_ids(tc: &mut TypeChecker, expr: &mut Expr) {
    match expr {
        Expr::Ident(id, _, _, _) => {
            *id = tc.generate_id();
        }
        Expr::InterpolatedString(exprs, _) => {
            for e in exprs { reassign_expr_ids(tc, e); }
        }
        Expr::Binary { left, right, .. } => {
            reassign_expr_ids(tc, left);
            reassign_expr_ids(tc, right);
        }
        Expr::MemberAccess { object, .. } | Expr::OptionalMemberAccess { object, .. } => {
            reassign_expr_ids(tc, object);
        }
        Expr::Call { callee, args, .. } => {
            reassign_expr_ids(tc, callee);
            for arg in args {
                reassign_expr_ids(tc, &mut arg.expr);
            }
        }
        Expr::BuiltinCall(_, args, _) => {
            for arg in args {
                reassign_expr_ids(tc, arg);
            }
        }
        Expr::If { cond, then_block, else_block, .. } => {
            reassign_expr_ids(tc, cond);
            reassign_block_ids(tc, then_block);
            if let Some(eb) = else_block {
                reassign_block_ids(tc, eb);
            }
        }
        Expr::While { cond, body, .. } => {
            reassign_expr_ids(tc, cond);
            reassign_block_ids(tc, body);
        }
        Expr::Assign { target, value, .. } => {
            reassign_expr_ids(tc, target);
            reassign_expr_ids(tc, value);
        }
        Expr::Match { subject, arms, .. } => {
            reassign_expr_ids(tc, subject);
            for arm in arms {
                reassign_pattern_ids(tc, &mut arm.pattern);
                reassign_expr_ids(tc, &mut arm.body);
            }
        }
        Expr::IntLiteral(..) | Expr::FloatLiteral(..) | Expr::BoolLiteral(..)
        | Expr::StringLiteral(..) | Expr::Null(_) | Expr::Super(_) => {}
    }
}

pub fn reassign_pattern_ids(tc: &mut TypeChecker, pattern: &mut Pattern) {
    match pattern {
        Pattern::Ident(id, _, _) => {
            *id = tc.generate_id();
        }
        Pattern::Variant { fields, .. } => {
            if let Some(fs) = fields {
                for (f_id, _, _) in fs {
                    *f_id = tc.generate_id();
                }
            }
        }
        Pattern::CatchAll(_) => {}
    }
}
