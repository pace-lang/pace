use colored::Colorize;
use miette::Result;
use pace_ast::{BinaryOp, Expr, Stmt, TypeAnnotation, Visibility};
use std::fs;
use std::path::Path;

pub fn execute() -> Result<()> {
    let current_dir =
        std::env::current_dir().map_err(|e| miette::miette!("Failed to get current dir: {}", e))?;

    // Process all .pace files
    let mut formatted_files = 0;

    for entry in walkdir::WalkDir::new(&current_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().is_file()
            && entry.path().extension().and_then(|s| s.to_str()) == Some("pace")
            && format_file(entry.path())?
        {
            formatted_files += 1;
        }
    }

    println!(
        "{} Successfully formatted {} files!",
        "✨".green(),
        formatted_files
    );

    Ok(())
}

fn format_file(path: &Path) -> Result<bool> {
    let src = fs::read_to_string(path)
        .map_err(|e| miette::miette!("Failed to read {}: {}", path.display(), e))?;

    let mut arena = pace_ast::arena::AstArena::new();
    let (ast, comments) = match pace_parser::parse(&mut arena, &src, &path.to_string_lossy()) {
        Ok(res) => res,
        Err(_) => {
            // Silently skip files with syntax errors during fmt
            return Ok(false);
        }
    };

    let mut formatter = Formatter::new(&src, comments, &arena);
    for &stmt_id in &ast {
        if formatter.format_stmt(stmt_id).is_err() {
            // Unimplemented AST variant, skip formatting this file to avoid dataloss
            return Ok(false);
        }
    }

    let formatted = formatter.finish();
    if formatted != src {
        fs::write(path, formatted.clone())
            .map_err(|e| miette::miette!("Failed to write {}: {}", path.display(), e))?;
        println!("  Formatted {}", path.display());
        Ok(true)
    } else {
        Ok(false)
    }
}

struct Formatter<'a> {
    arena: &'a pace_ast::arena::AstArena,
    _src: &'a str,
    comments: Vec<(usize, usize, String)>,
    comment_idx: usize,
    output: String,
    indent: usize,
}

impl<'a> Formatter<'a> {
    fn new(src: &'a str, mut comments: Vec<(usize, usize, String)>, arena: &'a pace_ast::arena::AstArena) -> Self {
        comments.sort_by_key(|c| c.0);
        Self {
            _src: src,
            comments,
            comment_idx: 0,
            output: String::new(),
            indent: 0,
            arena,
        }
    }

    fn finish(mut self) -> String {
        // Output remaining comments
        while self.comment_idx < self.comments.len() {
            self.write_indent();
            let text = self.comments[self.comment_idx].2.clone();
            self.output.push_str(&text);
            self.output.push('\n');
            self.comment_idx += 1;
        }
        self.output
    }

    fn write_indent(&mut self) {
        self.output.push_str(&"    ".repeat(self.indent));
    }

    fn check_comments(&mut self, pos: usize) {
        while self.comment_idx < self.comments.len() {
            let start = self.comments[self.comment_idx].0;
            if start <= pos {
                self.write_indent();
                let text = self.comments[self.comment_idx].2.clone();
                self.output.push_str(&text);
                self.output.push('\n');
                self.comment_idx += 1;
            } else {
                break;
            }
        }
    }

    fn check_inline_comments(&mut self, end_pos: usize) {
        while self.comment_idx < self.comments.len() {
            let start = self.comments[self.comment_idx].0;
            if start <= end_pos + 10 {
                // naive threshold for inline comments on same line
                self.output.push(' ');
                let text = self.comments[self.comment_idx].2.clone();
                self.output.push_str(&text);
                self.comment_idx += 1;
            } else {
                break;
            }
        }
    }

    fn format_stmt(&mut self, stmt_id: pace_ast::arena::StmtId) -> Result<(), ()> {
        let stmt = self.arena.get_stmt(stmt_id);
        match stmt {
            Stmt::Expr(e) => {
                // span is not available for Expr in Stmt::Expr, so just format it.
                self.write_indent();
                self.format_expr(*e)?;
                self.output.push(';');
                self.output.push('\n');
                Ok(())
            }
            Stmt::VarDecl {
                name,
                is_mutable,
                type_annotation,
                visibility,
                initializer,
                is_static,
                span,
            } => {
                self.check_comments(span.start);
                self.write_indent();
                if *visibility == Visibility::Private {
                    self.output.push_str("private ");
                }
                if *is_static {
                    self.output.push_str("static ");
                }
                if *is_mutable {
                    self.output.push_str("var ");
                } else {
                    self.output.push_str("let ");
                }
                self.output.push_str(name);
                if let Some(ty) = type_annotation {
                    self.output.push_str(": ");
                    self.format_type(ty);
                }
                if let Some(init) = initializer {
                    self.output.push_str(" = ");
                    self.format_expr(*init)?;
                }
                self.output.push(';');
                self.check_inline_comments(span.start + span.len);
                self.output.push('\n');
                Ok(())
            }
            Stmt::Return(e) => {
                self.write_indent();
                self.output.push_str("return");
                if let Some(e) = e {
                    self.output.push(' ');
                    self.format_expr(*e)?;
                }
                self.output.push(';');
                self.output.push('\n');
                Ok(())
            }
            Stmt::FuncDecl {
                name,
                generic_params,
                params,
                return_type,
                body,
                is_async,
                is_static,
                visibility,
                doc_comment,
                span,
            } => {
                self.check_comments(span.start);
                if let Some(doc) = doc_comment {
                    self.write_indent();
                    self.output.push_str("///");
                    self.output.push_str(doc);
                    self.output.push('\n');
                }
                self.write_indent();
                if *visibility == Visibility::Private {
                    self.output.push_str("private ");
                }
                if *is_static {
                    self.output.push_str("static ");
                }
                if *is_async {
                    self.output.push_str("async ");
                }
                self.output.push_str("func ");
                self.output.push_str(name);
                if let Some(gps) = generic_params {
                    self.output.push('<');
                    self.output.push_str(&gps.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
                    self.output.push('>');
                }
                self.output.push('(');
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(&param.name);
                    self.output.push_str(": ");
                    self.format_type(&param.type_annotation);
                }
                self.output.push(')');
                if let Some(rt) = return_type {
                    self.output.push_str(" -> ");
                    self.format_type(rt);
                }
                self.output.push_str(" {\n");
                self.indent += 1;
                for s in body {
                    self.format_stmt(*s)?;
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\n");
                self.output.push('\n');
                Ok(())
            }
            Stmt::Import {
                path,
                alias,
                show,
                hide,
            } => {
                self.write_indent();
                self.output.push_str("import \"");
                self.output.push_str(path);
                self.output.push('"');
                if let Some(alias) = alias {
                    self.output.push_str(" as ");
                    self.output.push_str(alias);
                }
                if let Some(show) = show {
                    self.output.push_str(" show ");
                    self.output.push_str(&show.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
                }
                if let Some(hide) = hide {
                    self.output.push_str(" hide ");
                    self.output.push_str(&hide.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
                }
                self.output.push_str(";\n");
                Ok(())
            }
            Stmt::Block(stmts) => {
                self.write_indent();
                self.output.push_str("{\n");
                self.indent += 1;
                for s in stmts {
                    self.format_stmt(*s)?;
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\n");
                Ok(())
            }
            Stmt::If { .. } => {
                self.write_indent();
                self.format_if_inline(stmt_id)?;
                Ok(())
            }
            Stmt::While { condition, body } => {
                self.write_indent();
                self.output.push_str("while ");
                self.format_expr(*condition)?;
                self.output.push_str(" {\n");
                self.indent += 1;
                if let Stmt::Block(stmts) = self.arena.get_stmt(*body) {
                    for s in stmts { self.format_stmt(*s)?; }
                } else {
                    self.format_stmt(*body)?;
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\n");
                Ok(())
            }
            Stmt::Loop { body } => {
                self.write_indent();
                self.output.push_str("loop {\n");
                self.indent += 1;
                if let Stmt::Block(stmts) = self.arena.get_stmt(*body) {
                    for s in stmts { self.format_stmt(*s)?; }
                } else {
                    self.format_stmt(*body)?;
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\n");
                Ok(())
            }
            Stmt::ForIn { item, iterable, body } => {
                self.write_indent();
                self.output.push_str("for ");
                self.output.push_str(item);
                self.output.push_str(" in ");
                self.format_expr(*iterable)?;
                self.output.push_str(" {\n");
                self.indent += 1;
                if let Stmt::Block(stmts) = self.arena.get_stmt(*body) {
                    for s in stmts { self.format_stmt(*s)?; }
                } else {
                    self.format_stmt(*body)?;
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\n");
                Ok(())
            }
            Stmt::ClassDecl { name, generic_params, fields, methods, implements, doc_comment } => {
                if let Some(doc) = doc_comment {
                    self.write_indent();
                    self.output.push_str("///");
                    self.output.push_str(doc);
                    self.output.push('\n');
                }
                self.write_indent();
                self.output.push_str("class ");
                self.output.push_str(name);
                if let Some(gps) = generic_params {
                    self.output.push('<');
                    self.output.push_str(&gps.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
                    self.output.push('>');
                }
                if let Some(impls) = implements {
                    self.output.push_str(" implement ");
                    self.format_type(impls);
                }
                self.output.push_str(" {\n");
                self.indent += 1;
                for f in fields { self.format_stmt(*f)?; }
                if !fields.is_empty() && !methods.is_empty() { self.output.push('\n'); }
                for m in methods { self.format_stmt(*m)?; }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\n\n");
                Ok(())
            }
            Stmt::ActorDecl { name, generic_params, fields, methods, implements, doc_comment } => {
                if let Some(doc) = doc_comment {
                    self.write_indent();
                    self.output.push_str("///");
                    self.output.push_str(doc);
                    self.output.push('\n');
                }
                self.write_indent();
                self.output.push_str("actor ");
                self.output.push_str(name);
                if let Some(gps) = generic_params {
                    self.output.push('<');
                    self.output.push_str(&gps.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
                    self.output.push('>');
                }
                if let Some(impls) = implements {
                    self.output.push_str(" implement ");
                    self.format_type(impls);
                }
                self.output.push_str(" {\n");
                self.indent += 1;
                for f in fields { self.format_stmt(*f)?; }
                if !fields.is_empty() && !methods.is_empty() { self.output.push('\n'); }
                for m in methods { self.format_stmt(*m)?; }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\n\n");
                Ok(())
            }
            Stmt::InterfaceDecl { name, generic_params, methods, doc_comment } => {
                if let Some(doc) = doc_comment {
                    self.write_indent();
                    self.output.push_str("///");
                    self.output.push_str(doc);
                    self.output.push('\n');
                }
                self.write_indent();
                self.output.push_str("interface ");
                self.output.push_str(name);
                if let Some(gps) = generic_params {
                    self.output.push('<');
                    self.output.push_str(&gps.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
                    self.output.push('>');
                }
                self.output.push_str(" {\n");
                self.indent += 1;
                for m in methods { self.format_stmt(*m)?; }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\n\n");
                Ok(())
            }
            Stmt::StructDecl { name, generic_params, fields, doc_comment } => {
                if let Some(doc) = doc_comment {
                    self.write_indent();
                    self.output.push_str("///");
                    self.output.push_str(doc);
                    self.output.push('\n');
                }
                self.write_indent();
                self.output.push_str("struct ");
                self.output.push_str(name);
                if let Some(gps) = generic_params {
                    self.output.push('<');
                    self.output.push_str(&gps.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
                    self.output.push('>');
                }
                self.output.push_str(" {\n");
                self.indent += 1;
                for f in fields { self.format_stmt(*f)?; }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\n\n");
                Ok(())
            }
            Stmt::EnumDecl { name, generic_params, variants, doc_comment } => {
                if let Some(doc) = doc_comment {
                    self.write_indent();
                    self.output.push_str("///");
                    self.output.push_str(doc);
                    self.output.push('\n');
                }
                self.write_indent();
                self.output.push_str("enum ");
                self.output.push_str(name);
                if let Some(gps) = generic_params {
                    self.output.push('<');
                    self.output.push_str(&gps.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
                    self.output.push('>');
                }
                self.output.push_str(" {\n");
                self.indent += 1;
                for (i, v) in variants.iter().enumerate() {
                    self.write_indent();
                    self.output.push_str(&v.name);
                    if let Some(fields) = &v.fields {
                        self.output.push('(');
                        for (fi, fty) in fields.iter().enumerate() {
                            if fi > 0 { self.output.push_str(", "); }
                            self.format_type(fty);
                        }
                        self.output.push(')');
                    }
                    if i < variants.len() - 1 {
                        self.output.push_str(",\n");
                    } else {
                        self.output.push('\n');
                    }
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\n\n");
                Ok(())
            }
            Stmt::Match { expr: match_expr, arms } => {
                self.write_indent();
                self.output.push_str("match ");
                self.format_expr(*match_expr)?;
                self.output.push_str(" {\n");
                self.indent += 1;
                for (pat, arm_body) in arms {
                    self.write_indent();
                    self.format_pattern(pat)?;
                    self.output.push_str(" => ");
                    if let Stmt::Block(stmts) = self.arena.get_stmt(*arm_body) {
                        self.output.push_str("{\n");
                        self.indent += 1;
                        for s in stmts { self.format_stmt(*s)?; }
                        self.indent -= 1;
                        self.write_indent();
                        self.output.push_str("}\n");
                    } else {
                        self.output.push_str("{\n");
                        self.indent += 1;
                        self.format_stmt(*arm_body)?;
                        self.indent -= 1;
                        self.write_indent();
                        self.output.push_str("}\n");
                    }
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\n");
                Ok(())
            }
            Stmt::Export { path } => {
                self.write_indent();
                self.output.push_str("export \"");
                self.output.push_str(path);
                self.output.push_str("\";\n");
                Ok(())
            }
            Stmt::Module { name, body } => {
                self.write_indent();
                self.output.push_str("module ");
                self.output.push_str(name);
                self.output.push_str(" {\n");
                self.indent += 1;
                for s in body { self.format_stmt(*s)?; }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\n\n");
                Ok(())
            }
        }
    }

    fn format_expr(&mut self, expr_id: pace_ast::arena::ExprId) -> Result<(), ()> {
        let expr = self.arena.get_expr(expr_id);
        match expr {
            Expr::IntLiteral(n) => self.output.push_str(&n.to_string()),
            Expr::FloatLiteral(f) => self.output.push_str(&f.to_string()),
            Expr::StringLiteral(s) => {
                self.output.push('"');
                let escaped = s.replace('\\', "\\\\").replace('\"', "\\\"").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t");
                self.output.push_str(&escaped);
                self.output.push('"');
            }
            Expr::BoolLiteral(b) => self.output.push_str(if *b { "true" } else { "false" }),
            Expr::Null => self.output.push_str("null"),
            Expr::Identifier(name, _) => self.output.push_str(name),
            Expr::Unary { op, expr: inner } => {
                let op_str = match op {
                    pace_ast::UnaryOp::Not => "!",
                    pace_ast::UnaryOp::Neg => "-",
                    pace_ast::UnaryOp::BitNot => "~",
                };
                self.output.push_str(op_str);
                self.format_sub_expr(*inner, 100, false)?;
            }
            Expr::Call { callee, args } => {
                self.format_expr(*callee)?;
                self.output.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.format_expr(*arg)?;
                }
                self.output.push(')');
            }
            Expr::Binary { left, op, right } => {
                let p = Self::binary_precedence(op);
                self.format_sub_expr(*left, p, false)?;
                self.output.push(' ');
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Mod => "%",
                    BinaryOp::Eq => "==",
                    BinaryOp::NotEq => "!=",
                    BinaryOp::Less => "<",
                    BinaryOp::LessEq => "<=",
                    BinaryOp::Greater => ">",
                    BinaryOp::GreaterEq => ">=",
                    BinaryOp::And => "&&",
                    BinaryOp::Or => "||",
                };
                self.output.push_str(op_str);
                self.output.push(' ');
                self.format_sub_expr(*right, p, true)?;
            }
            Expr::InterpolatedString(parts) => {
                self.output.push('"');
                for part in parts {
                    if let Expr::StringLiteral(s) = self.arena.get_expr(*part) {
                        let escaped = s.replace('\\', "\\\\").replace('\"', "\\\"").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t");
                        self.output.push_str(&escaped);
                    } else {
                        self.output.push_str("${");
                        self.format_expr(*part)?;
                        self.output.push('}');
                    }
                }
                self.output.push('"');
            }
            Expr::GenericInstantiation { callee, generic_args } => {
                self.format_expr(*callee)?;
                self.output.push('<');
                for (i, arg) in generic_args.iter().enumerate() {
                    if i > 0 { self.output.push_str(", "); }
                    self.format_type(arg);
                }
                self.output.push('>');
            }
            Expr::Assign { target, value } => {
                self.format_expr(*target)?;
                self.output.push_str(" = ");
                self.format_expr(*value)?;
            }
            Expr::MemberAccess { object, property, is_static_operator, .. } => {
                self.format_expr(*object)?;
                if *is_static_operator {
                    self.output.push_str("::");
                } else {
                    self.output.push('.');
                }
                self.output.push_str(property);
            }
            Expr::OptionalMemberAccess { object, property } => {
                self.format_expr(*object)?;
                self.output.push_str("?.");
                self.output.push_str(property);
            }
            Expr::Unwrap(inner) => {
                self.format_expr(*inner)?;
                self.output.push('!');
            }
            Expr::Try(inner) => {
                self.format_expr(*inner)?;
                self.output.push('?');
            }
            Expr::NullCoalesce { left, right } => {
                self.format_expr(*left)?;
                self.output.push_str(" ?? ");
                self.format_expr(*right)?;
            }
            Expr::Await(inner) => {
                self.output.push_str("await ");
                self.format_expr(*inner)?;
            }
            Expr::Closure { params, return_type, body } => {
                self.output.push('(');
                for (i, (name, ty)) in params.iter().enumerate() {
                    if i > 0 { self.output.push_str(", "); }
                    self.output.push_str(name);
                    self.output.push_str(": ");
                    self.format_type(ty);
                }
                self.output.push(')');
                if let Some(rt) = return_type {
                    self.output.push_str(" -> ");
                    self.format_type(rt);
                }
                self.output.push_str(" => ");
                self.format_expr(*body)?;
            }
            Expr::Block(stmts) => {
                self.output.push_str("{\n");
                self.indent += 1;
                for s in stmts {
                    self.format_stmt(*s)?;
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push('}');
            }
        }
        Ok(())
    }

    fn format_type(&mut self, ty: &TypeAnnotation) {
        if let Some(prefix) = &ty.module_prefix {
            self.output.push_str(prefix);
            self.output.push_str("::");
        }
        self.output.push_str(&ty.name);
        if !ty.args.is_empty() {
            self.output.push('<');
            for (i, arg) in ty.args.iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.format_type(arg);
            }
            self.output.push('>');
        }
        if ty.is_nullable {
            self.output.push('?');
        }
    }

    fn format_if_inline(&mut self, stmt_id: pace_ast::arena::StmtId) -> Result<(), ()> {
        let stmt = self.arena.get_stmt(stmt_id);
        if let Stmt::If { condition, then_branch, else_branch } = stmt {
            self.output.push_str("if ");
            self.format_expr(*condition)?;
            self.output.push_str(" {\n");
            self.indent += 1;
            if let Stmt::Block(stmts) = self.arena.get_stmt(*then_branch) {
                for s in stmts { self.format_stmt(*s)?; }
            } else {
                self.format_stmt(*then_branch)?;
            }
            self.indent -= 1;
            self.write_indent();
            self.output.push('}');
            if let Some(els) = else_branch {
                if let Stmt::If { .. } = self.arena.get_stmt(*els) {
                    self.output.push_str(" else ");
                    self.format_if_inline(*els)?;
                } else {
                    self.output.push_str(" else {\n");
                    self.indent += 1;
                    if let Stmt::Block(stmts) = self.arena.get_stmt(*els) {
                        for s in stmts { self.format_stmt(*s)?; }
                    } else {
                        self.format_stmt(*els)?;
                    }
                    self.indent -= 1;
                    self.write_indent();
                    self.output.push_str("}\n");
                }
            } else {
                self.output.push('\n');
            }
            Ok(())
        } else {
            Err(())
        }
    }

    fn format_pattern(&mut self, pat: &pace_ast::Pattern) -> Result<(), ()> {
        match pat {
            pace_ast::Pattern::Wildcard => self.output.push('_'),
            pace_ast::Pattern::Literal(expr) => self.format_expr(*expr)?,
            pace_ast::Pattern::Variable(name, _) => self.output.push_str(name),
            pace_ast::Pattern::Variant { enum_name, variant_name, fields, generic_args } => {
                if let Some(e) = enum_name {
                    self.output.push_str(e);
                    self.output.push_str("::");
                }
                self.output.push_str(variant_name);
                if let Some(g) = generic_args {
                    self.output.push('<');
                    for (i, arg) in g.iter().enumerate() {
                        if i > 0 { self.output.push_str(", "); }
                        self.format_type(arg);
                    }
                    self.output.push('>');
                }
                if let Some(f) = fields {
                    self.output.push('(');
                    for (i, p) in f.iter().enumerate() {
                        if i > 0 { self.output.push_str(", "); }
                        self.format_pattern(p)?;
                    }
                    self.output.push(')');
                }
            }
        }
        Ok(())
    }


    fn binary_precedence(op: &pace_ast::BinaryOp) -> u8 {
        use pace_ast::BinaryOp::*;
        match op {
            Mul | Div | Mod => 6,
            Add | Sub => 5,
            Less | LessEq | Greater | GreaterEq => 4,
            Eq | NotEq => 3,
            And => 2,
            Or => 1,
        }
    }

    fn format_sub_expr(&mut self, sub_id: pace_ast::arena::ExprId, parent_prec: u8, is_right: bool) -> Result<(), ()> {
        let sub = self.arena.get_expr(sub_id);
        let prec = match sub {
            Expr::Binary { op, .. } => Self::binary_precedence(op),
            Expr::NullCoalesce { .. } => 0,
            Expr::Assign { .. } => 0,
            _ => 100,
        };
        let needs_parens = if is_right {
            prec <= parent_prec
        } else {
            prec < parent_prec
        };
        if needs_parens {
            self.output.push('(');
        }
        self.format_expr(sub_id)?;
        if needs_parens {
            self.output.push(')');
        }
        Ok(())
    }

}
