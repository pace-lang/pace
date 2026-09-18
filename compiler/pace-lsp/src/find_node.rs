use pace_hir::{Block, Decl, Expr, HirId, Program, Stmt};

pub fn position_to_offset(text: &str, position: tower_lsp::lsp_types::Position) -> usize {
    let mut offset = 0;
    let mut line = 0;
    let mut col = 0;

    for (i, c) in text.char_indices() {
        if line == position.line && col == position.character {
            return offset;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
        offset = i + c.len_utf8();
    }
    offset
}

pub fn find_ident_at_offset(
    program: &Program,
    file_id: pace_span::FileId,
    offset: usize,
) -> Option<HirId> {
    for module in program.modules.values() {
        if module.file_id != file_id {
            continue;
        }
        for decl in &module.declarations {
            if let Some(id) = find_ident_in_decl(decl, offset) {
                return Some(id);
            }
        }
    }
    None
}

fn find_ident_in_decl(decl: &Decl, offset: usize) -> Option<HirId> {
    match decl {
        Decl::Let {
            id, value, span, ..
        }
        | Decl::Var {
            id, value, span, ..
        } => {
            if let Some(v) = value
                && let Some(vid) = find_ident_in_expr(v, offset)
            {
                return Some(vid);
            }
            if offset >= span.start as usize && offset <= span.end as usize {
                return Some(*id);
            }
        }
        Decl::Const {
            id, value, span, ..
        } => {
            if let Some(vid) = find_ident_in_expr(value, offset) {
                return Some(vid);
            }
            if offset >= span.start as usize && offset <= span.end as usize {
                return Some(*id);
            }
        }
        Decl::Function { body, .. } => {
            return find_ident_in_block(body, offset);
        }
        Decl::Struct { methods, .. } | Decl::Class { methods, .. } => {
            for m in methods {
                if let Some(id) = find_ident_in_decl(m, offset) {
                    return Some(id);
                }
            }
        }
        Decl::Expr(expr, _) => {
            return find_ident_in_expr(expr, offset);
        }
        _ => {}
    }
    None
}

fn find_ident_in_block(block: &Block, offset: usize) -> Option<HirId> {
    for stmt in &block.statements {
        if let Some(id) = find_ident_in_stmt(stmt, offset) {
            return Some(id);
        }
    }
    None
}

fn find_ident_in_stmt(stmt: &Stmt, offset: usize) -> Option<HirId> {
    match stmt {
        Stmt::Let {
            id, value, span, ..
        }
        | Stmt::Var {
            id, value, span, ..
        } => {
            if let Some(v) = value
                && let Some(vid) = find_ident_in_expr(v, offset)
            {
                return Some(vid);
            }
            if offset >= span.start as usize && offset <= span.end as usize {
                return Some(*id);
            }
        }
        Stmt::ExprStmt(expr, _) => {
            return find_ident_in_expr(expr, offset);
        }
        Stmt::Return(value, _) => {
            if let Some(v) = value {
                return find_ident_in_expr(v, offset);
            }
        }
    }
    None
}

fn find_ident_in_expr(expr: &Expr, offset: usize) -> Option<HirId> {
    match expr {
        Expr::Ident(id, _, _, span) => {
            if offset >= span.start as usize && offset <= span.end as usize {
                return Some(*id);
            }
        }
        Expr::InterpolatedString(exprs, _) => {
            for expr in exprs {
                if let Some(id) = find_ident_in_expr(expr, offset) {
                    return Some(id);
                }
            }
        }
        Expr::Binary { left, right, .. } => {
            if let Some(id) = find_ident_in_expr(left, offset) {
                return Some(id);
            }
            if let Some(id) = find_ident_in_expr(right, offset) {
                return Some(id);
            }
        }
        Expr::Call { callee, args, .. } => {
            if let Some(id) = find_ident_in_expr(callee, offset) {
                return Some(id);
            }
            for pace_hir::HirCallArg { expr: arg, .. } in args {
                if let Some(id) = find_ident_in_expr(arg, offset) {
                    return Some(id);
                }
            }
        }
        Expr::MemberAccess { object, .. } | Expr::OptionalMemberAccess { object, .. } => {
            if let Some(id) = find_ident_in_expr(object, offset) {
                return Some(id);
            }
        }
        Expr::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            if let Some(id) = find_ident_in_expr(cond, offset) {
                return Some(id);
            }
            if let Some(id) = find_ident_in_block(then_block, offset) {
                return Some(id);
            }
            if let Some(b) = else_block
                && let Some(id) = find_ident_in_block(b, offset)
            {
                return Some(id);
            }
        }
        Expr::While { cond, body, .. } => {
            if let Some(id) = find_ident_in_expr(cond, offset) {
                return Some(id);
            }
            if let Some(id) = find_ident_in_block(body, offset) {
                return Some(id);
            }
        }
        Expr::Assign { target, value, .. } => {
            if let Some(id) = find_ident_in_expr(target, offset) {
                return Some(id);
            }
            if let Some(id) = find_ident_in_expr(value, offset) {
                return Some(id);
            }
        }
        Expr::Match { subject, arms, .. } => {
            if let Some(id) = find_ident_in_expr(subject, offset) {
                return Some(id);
            }
            for arm in arms {
                if let Some(id) = find_ident_in_expr(&arm.body, offset) {
                    return Some(id);
                }
            }
        }
        _ => {}
    }
    None
}
