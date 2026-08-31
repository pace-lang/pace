use colored::Colorize;
use miette::Result;
use pace_ast::{Expr, Pattern, Stmt};
use pace_common::{TypeAnnotation, Visibility, BinaryOp, UnaryOp};
use pretty::RcDoc;
use std::fs;
use std::path::Path;

pub fn execute() -> Result<()> {
    let current_dir =
        std::env::current_dir().map_err(|e| miette::miette!("Failed to get current dir: {}", e))?;

    let mut formatted_files = 0;
    for entry in walkdir::WalkDir::new(&current_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file()
            && entry.path().extension().and_then(|s| s.to_str()) == Some("pace")
            && format_file(entry.path())?
        {
            formatted_files += 1;
        }
    }

    println!("{} Successfully formatted {} files!", "✨".green(), formatted_files);
    Ok(())
}

fn format_file(path: &Path) -> Result<bool> {
    let src = fs::read_to_string(path)
        .map_err(|e| miette::miette!("Failed to read {}: {}", path.display(), e))?;

    let mut arena = pace_ast::arena::AstArena::new();
    let (ast, comments) = match pace_parser::parse(&mut arena, &src, &path.to_string_lossy()) {
        Ok(res) => res,
        Err(_) => return Ok(false),
    };

    let mut formatter = Formatter::new(&src, comments, &arena);
    let mut docs = Vec::new();
    for &stmt_id in &ast {
        docs.push(formatter.format_stmt(stmt_id));
    }

    let final_doc = RcDoc::intersperse(docs, RcDoc::hardline().append(RcDoc::hardline()));
    let mut w = Vec::new();
    final_doc.render(100, &mut w).unwrap();
    let formatted = String::from_utf8(w).unwrap() + "\n";

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
    comments: Vec<(usize, usize, String)>,
    comment_idx: usize,
}

impl<'a> Formatter<'a> {
    fn new(_src: &'a str, mut comments: Vec<(usize, usize, String)>, arena: &'a pace_ast::arena::AstArena) -> Self {
        comments.sort_by_key(|c| c.0);
        Self { arena, comments, comment_idx: 0 }
    }

    fn check_comments(&mut self, pos: usize) -> RcDoc<'a, ()> {
        let mut docs = Vec::new();
        while self.comment_idx < self.comments.len() {
            let start = self.comments[self.comment_idx].0;
            if start <= pos {
                let text = self.comments[self.comment_idx].2.clone();
                docs.push(RcDoc::text(text));
                self.comment_idx += 1;
            } else {
                break;
            }
        }
        if docs.is_empty() {
            RcDoc::nil()
        } else {
            RcDoc::intersperse(docs, RcDoc::hardline()).append(RcDoc::hardline())
        }
    }

    fn check_inline_comments(&mut self, end_pos: usize) -> RcDoc<'a, ()> {
        let mut docs = Vec::new();
        while self.comment_idx < self.comments.len() {
            let start = self.comments[self.comment_idx].0;
            if start <= end_pos + 10 {
                let text = self.comments[self.comment_idx].2.clone();
                docs.push(RcDoc::text(" ").append(RcDoc::text(text)));
                self.comment_idx += 1;
            } else {
                break;
            }
        }
        RcDoc::concat(docs)
    }

    fn format_type(&self, ty: &TypeAnnotation) -> RcDoc<'a, ()> {
        let mut doc = RcDoc::nil();
        if let Some(prefix) = &ty.module_prefix {
            doc = doc.append(RcDoc::text(prefix.as_str().to_string())).append(RcDoc::text("::"));
        }
        doc = doc.append(RcDoc::text(ty.name.as_str().to_string()));
        if !ty.args.is_empty() {
            let args: Vec<_> = ty.args.iter().map(|a| self.format_type(a)).collect();
            doc = doc.append(RcDoc::text("<"))
                .append(RcDoc::intersperse(args, RcDoc::text(", ")))
                .append(RcDoc::text(">"));
        }
        if ty.is_nullable {
            doc = doc.append(RcDoc::text("?"));
        }
        doc
    }

    fn format_stmt(&mut self, stmt_id: pace_ast::arena::StmtId) -> RcDoc<'a, ()> {
        let stmt = self.arena.get_stmt(stmt_id);
        match stmt {
            Stmt::Expr(e) => self.format_expr(*e).append(RcDoc::text(";")),
            Stmt::VarDecl { name, is_mutable, type_annotation, is_static, visibility, initializer, span } => {
                let c = self.check_comments(span.start);
                let mut doc = RcDoc::nil();
                if *visibility == Visibility::Private { doc = doc.append(RcDoc::text("private ")); }
                if *is_static { doc = doc.append(RcDoc::text("static ")); }
                doc = doc.append(RcDoc::text(if *is_mutable { "var " } else { "let " })).append(RcDoc::text(name.as_str().to_string()));
                if let Some(ty) = type_annotation {
                    doc = doc.append(RcDoc::text(": ")).append(self.format_type(ty));
                }
                if let Some(init) = initializer {
                    doc = doc.append(RcDoc::text(" = ")).append(self.format_expr(*init));
                }
                c.append(doc).append(RcDoc::text(";")).append(self.check_inline_comments(span.start + span.len))
            }
            Stmt::Block(stmts) => {
                let mut docs = Vec::new();
                for &s in stmts { docs.push(self.format_stmt(s)); }
                RcDoc::text("{")
                    .append(RcDoc::hardline().append(RcDoc::intersperse(docs, RcDoc::hardline())).nest(4))
                    .append(RcDoc::hardline())
                    .append(RcDoc::text("}"))
            }
            Stmt::Return(e) => {
                let mut doc = RcDoc::text("return");
                if let Some(expr) = e {
                    doc = doc.append(RcDoc::space()).append(self.format_expr(*expr));
                }
                doc.append(RcDoc::text(";"))
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let cond = self.format_expr(*condition);
                let then = self.format_stmt(*then_branch);
                let mut doc = RcDoc::text("if ").append(cond).append(RcDoc::space()).append(then);
                if let Some(eb) = else_branch {
                    doc = doc.append(RcDoc::text(" else ")).append(self.format_stmt(*eb));
                }
                doc
            }
            Stmt::While { condition, body } => {
                RcDoc::text("while ").append(self.format_expr(*condition)).append(RcDoc::space()).append(self.format_stmt(*body))
            }
            Stmt::Loop { body } => {
                RcDoc::text("loop ").append(self.format_stmt(*body))
            }
            Stmt::ForIn { item, iterable, body } => {
                RcDoc::text("for ").append(RcDoc::text(item.as_str().to_string())).append(RcDoc::text(" in "))
                    .append(self.format_expr(*iterable)).append(RcDoc::space()).append(self.format_stmt(*body))
            }
            Stmt::Match { expr, arms } => {
                let expr_doc = self.format_expr(*expr);
                let mut arm_docs = Vec::new();
                for (pat, stmt_id) in arms {
                    arm_docs.push(self.format_pattern(pat).append(RcDoc::text(" => ")).append(self.format_stmt(*stmt_id)));
                }
                RcDoc::text("match ").append(expr_doc).append(RcDoc::text(" {"))
                    .append(RcDoc::hardline().append(RcDoc::intersperse(arm_docs, RcDoc::hardline())).nest(4))
                    .append(RcDoc::hardline())
                    .append(RcDoc::text("}"))
            }
            Stmt::FuncDecl { name, generic_params, params, return_type, body, is_async, is_static, is_extern, visibility, span } => {
                let c = self.check_comments(span.start);
                let mut doc = RcDoc::nil();
                if *visibility == Visibility::Private { doc = doc.append(RcDoc::text("private ")); }
                if *is_static { doc = doc.append(RcDoc::text("static ")); }
                if *is_extern { doc = doc.append(RcDoc::text("extern ")); }
                if *is_async { doc = doc.append(RcDoc::text("async ")); }
                doc = doc.append(RcDoc::text("func ")).append(RcDoc::text(name.as_str().to_string()));
                if let Some(gps) = generic_params {
                    let gps_docs: Vec<_> = gps.iter().map(|g| RcDoc::text(g.as_str().to_string())).collect();
                    doc = doc.append(RcDoc::text("<")).append(RcDoc::intersperse(gps_docs, RcDoc::text(", "))).append(RcDoc::text(">"));
                }
                let param_docs: Vec<_> = params.iter().map(|p| RcDoc::text(p.name.as_str().to_string()).append(RcDoc::text(": ")).append(self.format_type(&p.type_annotation))).collect();
                doc = doc.append(RcDoc::text("(")).append(RcDoc::intersperse(param_docs, RcDoc::text(", "))).append(RcDoc::text(")"));
                if let Some(rt) = return_type {
                    doc = doc.append(RcDoc::text(" -> ")).append(self.format_type(rt));
                }
                let mut body_docs = Vec::new();
                for &s in body { body_docs.push(self.format_stmt(s)); }
                let body_doc = RcDoc::text(" {")
                    .append(RcDoc::hardline().append(RcDoc::intersperse(body_docs, RcDoc::hardline())).nest(4))
                    .append(RcDoc::hardline())
                    .append(RcDoc::text("}"));
                
                c.append(doc).append(body_doc)
            }
            Stmt::ClassDecl { name, generic_params, fields, methods, implements } => {
                let mut doc = RcDoc::text("class ").append(RcDoc::text(name.as_str().to_string()));
                if let Some(gps) = generic_params {
                    let gps_docs: Vec<_> = gps.iter().map(|g| RcDoc::text(g.as_str().to_string())).collect();
                    doc = doc.append(RcDoc::text("<")).append(RcDoc::intersperse(gps_docs, RcDoc::text(", "))).append(RcDoc::text(">"));
                }
                if let Some(impls) = implements {
                    doc = doc.append(RcDoc::text(" implement ")).append(self.format_type(impls));
                }
                let mut inner_docs = Vec::new();
                for &f in fields { inner_docs.push(self.format_stmt(f)); }
                if !fields.is_empty() && !methods.is_empty() { inner_docs.push(RcDoc::hardline()); }
                for &m in methods { inner_docs.push(self.format_stmt(m)); }
                
                doc.append(RcDoc::text(" {"))
                   .append(RcDoc::hardline().append(RcDoc::intersperse(inner_docs, RcDoc::hardline())).nest(4))
                   .append(RcDoc::hardline())
                   .append(RcDoc::text("}"))
            }
            Stmt::ActorDecl { name, generic_params, fields, methods, implements } => {
                let mut doc = RcDoc::text("actor ").append(RcDoc::text(name.as_str().to_string()));
                if let Some(gps) = generic_params {
                    let gps_docs: Vec<_> = gps.iter().map(|g| RcDoc::text(g.as_str().to_string())).collect();
                    doc = doc.append(RcDoc::text("<")).append(RcDoc::intersperse(gps_docs, RcDoc::text(", "))).append(RcDoc::text(">"));
                }
                if let Some(impls) = implements {
                    doc = doc.append(RcDoc::text(" implement ")).append(self.format_type(impls));
                }
                let mut inner_docs = Vec::new();
                for &f in fields { inner_docs.push(self.format_stmt(f)); }
                if !fields.is_empty() && !methods.is_empty() { inner_docs.push(RcDoc::hardline()); }
                for &m in methods { inner_docs.push(self.format_stmt(m)); }
                
                doc.append(RcDoc::text(" {"))
                   .append(RcDoc::hardline().append(RcDoc::intersperse(inner_docs, RcDoc::hardline())).nest(4))
                   .append(RcDoc::hardline())
                   .append(RcDoc::text("}"))
            }
            Stmt::InterfaceDecl { name, generic_params, methods } => {
                let mut doc = RcDoc::text("interface ").append(RcDoc::text(name.as_str().to_string()));
                if let Some(gps) = generic_params {
                    let gps_docs: Vec<_> = gps.iter().map(|g| RcDoc::text(g.as_str().to_string())).collect();
                    doc = doc.append(RcDoc::text("<")).append(RcDoc::intersperse(gps_docs, RcDoc::text(", "))).append(RcDoc::text(">"));
                }
                let mut inner_docs = Vec::new();
                for &m in methods { inner_docs.push(self.format_stmt(m)); }
                doc.append(RcDoc::text(" {"))
                   .append(RcDoc::hardline().append(RcDoc::intersperse(inner_docs, RcDoc::hardline())).nest(4))
                   .append(RcDoc::hardline())
                   .append(RcDoc::text("}"))
            }
            Stmt::StructDecl { name, generic_params, fields } => {
                let mut doc = RcDoc::text("struct ").append(RcDoc::text(name.as_str().to_string()));
                if let Some(gps) = generic_params {
                    let gps_docs: Vec<_> = gps.iter().map(|g| RcDoc::text(g.as_str().to_string())).collect();
                    doc = doc.append(RcDoc::text("<")).append(RcDoc::intersperse(gps_docs, RcDoc::text(", "))).append(RcDoc::text(">"));
                }
                let mut inner_docs = Vec::new();
                for &f in fields { inner_docs.push(self.format_stmt(f)); }
                doc.append(RcDoc::text(" {"))
                   .append(RcDoc::hardline().append(RcDoc::intersperse(inner_docs, RcDoc::hardline())).nest(4))
                   .append(RcDoc::hardline())
                   .append(RcDoc::text("}"))
            }
            Stmt::EnumDecl { name, generic_params, variants } => {
                let mut doc = RcDoc::text("enum ").append(RcDoc::text(name.as_str().to_string()));
                if let Some(gps) = generic_params {
                    let gps_docs: Vec<_> = gps.iter().map(|g| RcDoc::text(g.as_str().to_string())).collect();
                    doc = doc.append(RcDoc::text("<")).append(RcDoc::intersperse(gps_docs, RcDoc::text(", "))).append(RcDoc::text(">"));
                }
                let mut inner_docs = Vec::new();
                for v in variants {
                    let mut vdoc = RcDoc::text(v.name.as_str().to_string());
                    if let Some(fs) = &v.fields {
                        let fdocs: Vec<_> = fs.iter().map(|f| self.format_type(f)).collect();
                        vdoc = vdoc.append(RcDoc::text("(")).append(RcDoc::intersperse(fdocs, RcDoc::text(", "))).append(RcDoc::text(")"));
                    }
                    inner_docs.push(vdoc.append(RcDoc::text(",")));
                }
                doc.append(RcDoc::text(" {"))
                   .append(RcDoc::hardline().append(RcDoc::intersperse(inner_docs, RcDoc::hardline())).nest(4))
                   .append(RcDoc::hardline())
                   .append(RcDoc::text("}"))
            }
            Stmt::Import { path, alias, show, hide } => {
                let mut doc = RcDoc::text("import \"").append(RcDoc::text(path.as_str().to_string())).append(RcDoc::text("\""));
                if let Some(a) = alias { doc = doc.append(RcDoc::text(" as ")).append(RcDoc::text(a.as_str().to_string())); }
                if let Some(s) = show {
                    let sdocs: Vec<_> = s.iter().map(|x| RcDoc::text(x.as_str().to_string())).collect();
                    doc = doc.append(RcDoc::text(" show ")).append(RcDoc::intersperse(sdocs, RcDoc::text(", ")));
                }
                if let Some(h) = hide {
                    let hdocs: Vec<_> = h.iter().map(|x| RcDoc::text(x.as_str().to_string())).collect();
                    doc = doc.append(RcDoc::text(" hide ")).append(RcDoc::intersperse(hdocs, RcDoc::text(", ")));
                }
                doc.append(RcDoc::text(";"))
            }
            Stmt::Export { path } => {
                RcDoc::text("export \"").append(RcDoc::text(path.as_str().to_string())).append(RcDoc::text("\";"))
            }
            Stmt::Module { name, body } => {
                let mut inner_docs = Vec::new();
                for &s in body { inner_docs.push(self.format_stmt(s)); }
                RcDoc::text("module ").append(RcDoc::text(name.as_str().to_string())).append(RcDoc::text(" {"))
                    .append(RcDoc::hardline().append(RcDoc::intersperse(inner_docs, RcDoc::hardline())).nest(4))
                    .append(RcDoc::hardline())
                    .append(RcDoc::text("}"))
            }
        }
    }

    fn format_expr(&mut self, expr_id: pace_ast::arena::ExprId) -> RcDoc<'a, ()> {
        let expr = self.arena.get_expr(expr_id);
        match expr {
            Expr::IntLiteral(n) => RcDoc::text(n.to_string()),
            Expr::FloatLiteral(f) => RcDoc::text(f.to_string()),
            Expr::StringLiteral(s) => RcDoc::text(format!("\"{}\"", s.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t"))),
            Expr::InterpolatedString(_parts) => {
                // Formatting interpolated string is skipped for simplicity.
                RcDoc::text("\"(interpolated)\"")
            }
            Expr::BoolLiteral(b) => RcDoc::text(if *b { "true" } else { "false" }),
            Expr::Null => RcDoc::text("null"),
            Expr::Identifier(name, _) => RcDoc::text(name.as_str().to_string()),
            Expr::GenericInstantiation { callee, generic_args } => {
                let c = self.format_expr(*callee);
                let args: Vec<_> = generic_args.iter().map(|a| self.format_type(a)).collect();
                c.append(RcDoc::text("<")).append(RcDoc::intersperse(args, RcDoc::text(", "))).append(RcDoc::text(">"))
            }
            Expr::Unary { op, expr } => {
                let o = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                    UnaryOp::BitNot => "~",
                };
                RcDoc::text(o).append(self.format_expr(*expr))
            }
            Expr::Binary { left, op, right } => {
                let l = self.format_expr(*left);
                let r = self.format_expr(*right);
                let o = match op {
                    BinaryOp::Add => " + ",
                    BinaryOp::Sub => " - ",
                    BinaryOp::Mul => " * ",
                    BinaryOp::Div => " / ",
                    BinaryOp::Mod => " % ",
                    BinaryOp::Eq => " == ",
                    BinaryOp::NotEq => " != ",
                    BinaryOp::Less => " < ",
                    BinaryOp::LessEq => " <= ",
                    BinaryOp::Greater => " > ",
                    BinaryOp::GreaterEq => " >= ",
                    BinaryOp::And => " && ",
                    BinaryOp::Or => " || ",
                };
                l.append(RcDoc::text(o)).append(r)
            }
            Expr::Call { callee, args } => {
                let c = self.format_expr(*callee);
                let mut adocs = Vec::new();
                for &a in args { adocs.push(self.format_expr(a)); }
                c.append(RcDoc::text("(")).append(RcDoc::intersperse(adocs, RcDoc::text(", "))).append(RcDoc::text(")"))
            }
            Expr::Assign { target, value } => {
                self.format_expr(*target).append(RcDoc::text(" = ")).append(self.format_expr(*value))
            }
            Expr::MemberAccess { object, property, is_static_operator, .. } => {
                let o = self.format_expr(*object);
                let op = if *is_static_operator { "::" } else { "." };
                o.append(RcDoc::text(op)).append(RcDoc::text(property.as_str().to_string()))
            }
            Expr::OptionalMemberAccess { object, property } => {
                self.format_expr(*object).append(RcDoc::text("?.")).append(RcDoc::text(property.as_str().to_string()))
            }
            Expr::Unwrap(e) => self.format_expr(*e).append(RcDoc::text("!")),
            Expr::Try(e) => self.format_expr(*e).append(RcDoc::text("?")),
            Expr::Await(e) => RcDoc::text("await ").append(self.format_expr(*e)),
            Expr::NullCoalesce { left, right } => {
                self.format_expr(*left).append(RcDoc::text(" ?? ")).append(self.format_expr(*right))
            }
            Expr::Closure { params, return_type, body } => {
                let mut doc = RcDoc::text("|");
                let mut pdocs = Vec::new();
                for (n, t) in params {
                    pdocs.push(RcDoc::text(n.as_str().to_string()).append(RcDoc::text(": ")).append(self.format_type(t)));
                }
                doc = doc.append(RcDoc::intersperse(pdocs, RcDoc::text(", "))).append(RcDoc::text("|"));
                if let Some(rt) = return_type {
                    doc = doc.append(RcDoc::text(" -> ")).append(self.format_type(rt));
                }
                doc.append(RcDoc::space()).append(self.format_expr(*body))
            }
            Expr::Block(stmts) => {
                let mut docs = Vec::new();
                for &s in stmts { docs.push(self.format_stmt(s)); }
                RcDoc::text("{")
                    .append(RcDoc::hardline().append(RcDoc::intersperse(docs, RcDoc::hardline())).nest(4))
                    .append(RcDoc::hardline())
                    .append(RcDoc::text("}"))
            }
        }
    }

    fn format_pattern(&mut self, pat: &Pattern) -> RcDoc<'a, ()> {
        match pat {
            Pattern::Wildcard => RcDoc::text("_"),
            Pattern::Literal(e) => self.format_expr(*e),
            Pattern::Variable(n, _) => RcDoc::text(n.as_str().to_string()),
            Pattern::Variant { enum_name, variant_name, fields, generic_args } => {
                let mut doc = RcDoc::nil();
                if let Some(en) = enum_name {
                    doc = doc.append(RcDoc::text(en.as_str().to_string())).append(RcDoc::text("::"));
                }
                doc = doc.append(RcDoc::text(variant_name.as_str().to_string()));
                if let Some(ga) = generic_args {
                    let gadocs: Vec<_> = ga.iter().map(|a| self.format_type(a)).collect();
                    doc = doc.append(RcDoc::text("<")).append(RcDoc::intersperse(gadocs, RcDoc::text(", "))).append(RcDoc::text(">"));
                }
                if let Some(fs) = fields {
                    let fdocs: Vec<_> = fs.iter().map(|f| self.format_pattern(f)).collect();
                    doc = doc.append(RcDoc::text("(")).append(RcDoc::intersperse(fdocs, RcDoc::text(", "))).append(RcDoc::text(")"));
                }
                doc
            }
        }
    }
}
