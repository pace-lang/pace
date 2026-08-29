import re

fmt_file = "crates/pace-cli/src/commands/fmt.rs"

with open(fmt_file, "r") as f:
    content = f.read()

# Replace _ => Err(()) in format_stmt
stmt_fallback = """            // Fallback for missing stmt types
            _ => Err(()),"""

stmt_implementations = """            Stmt::Block(stmts) => {
                self.write_indent();
                self.output.push_str("{\\n");
                self.indent += 1;
                for s in stmts {
                    self.format_stmt(s)?;
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\\n");
                Ok(())
            }
            Stmt::If { .. } => {
                self.write_indent();
                self.format_if_inline(stmt)?;
                Ok(())
            }
            Stmt::While { condition, body } => {
                self.write_indent();
                self.output.push_str("while ");
                self.format_expr(condition)?;
                self.output.push_str(" {\\n");
                self.indent += 1;
                if let Stmt::Block(stmts) = &**body {
                    for s in stmts { self.format_stmt(s)?; }
                } else {
                    self.format_stmt(body)?;
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\\n");
                Ok(())
            }
            Stmt::Loop { body } => {
                self.write_indent();
                self.output.push_str("loop {\\n");
                self.indent += 1;
                if let Stmt::Block(stmts) = &**body {
                    for s in stmts { self.format_stmt(s)?; }
                } else {
                    self.format_stmt(body)?;
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\\n");
                Ok(())
            }
            Stmt::ForIn { item, iterable, body } => {
                self.write_indent();
                self.output.push_str("for ");
                self.output.push_str(item);
                self.output.push_str(" in ");
                self.format_expr(iterable)?;
                self.output.push_str(" {\\n");
                self.indent += 1;
                if let Stmt::Block(stmts) = &**body {
                    for s in stmts { self.format_stmt(s)?; }
                } else {
                    self.format_stmt(body)?;
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\\n");
                Ok(())
            }
            Stmt::ClassDecl { name, generic_params, fields, methods, implements, doc_comment } => {
                if let Some(doc) = doc_comment {
                    self.write_indent();
                    self.output.push_str("///");
                    self.output.push_str(doc);
                    self.output.push('\\n');
                }
                self.write_indent();
                self.output.push_str("class ");
                self.output.push_str(name);
                if let Some(gps) = generic_params {
                    self.output.push('<');
                    self.output.push_str(&gps.join(", "));
                    self.output.push('>');
                }
                if let Some(impls) = implements {
                    self.output.push_str(" implement ");
                    self.format_type(impls);
                }
                self.output.push_str(" {\\n");
                self.indent += 1;
                for f in fields { self.format_stmt(f)?; }
                if !fields.is_empty() && !methods.is_empty() { self.output.push('\\n'); }
                for m in methods { self.format_stmt(m)?; }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\\n\\n");
                Ok(())
            }
            Stmt::ActorDecl { name, generic_params, fields, methods, implements, doc_comment } => {
                if let Some(doc) = doc_comment {
                    self.write_indent();
                    self.output.push_str("///");
                    self.output.push_str(doc);
                    self.output.push('\\n');
                }
                self.write_indent();
                self.output.push_str("actor ");
                self.output.push_str(name);
                if let Some(gps) = generic_params {
                    self.output.push('<');
                    self.output.push_str(&gps.join(", "));
                    self.output.push('>');
                }
                if let Some(impls) = implements {
                    self.output.push_str(" implement ");
                    self.format_type(impls);
                }
                self.output.push_str(" {\\n");
                self.indent += 1;
                for f in fields { self.format_stmt(f)?; }
                if !fields.is_empty() && !methods.is_empty() { self.output.push('\\n'); }
                for m in methods { self.format_stmt(m)?; }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\\n\\n");
                Ok(())
            }
            Stmt::InterfaceDecl { name, generic_params, methods, doc_comment } => {
                if let Some(doc) = doc_comment {
                    self.write_indent();
                    self.output.push_str("///");
                    self.output.push_str(doc);
                    self.output.push('\\n');
                }
                self.write_indent();
                self.output.push_str("interface ");
                self.output.push_str(name);
                if let Some(gps) = generic_params {
                    self.output.push('<');
                    self.output.push_str(&gps.join(", "));
                    self.output.push('>');
                }
                self.output.push_str(" {\\n");
                self.indent += 1;
                for m in methods { self.format_stmt(m)?; }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\\n\\n");
                Ok(())
            }
            Stmt::StructDecl { name, generic_params, fields, doc_comment } => {
                if let Some(doc) = doc_comment {
                    self.write_indent();
                    self.output.push_str("///");
                    self.output.push_str(doc);
                    self.output.push('\\n');
                }
                self.write_indent();
                self.output.push_str("struct ");
                self.output.push_str(name);
                if let Some(gps) = generic_params {
                    self.output.push('<');
                    self.output.push_str(&gps.join(", "));
                    self.output.push('>');
                }
                self.output.push_str(" {\\n");
                self.indent += 1;
                for f in fields { self.format_stmt(f)?; }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\\n\\n");
                Ok(())
            }
            Stmt::EnumDecl { name, generic_params, variants, doc_comment } => {
                if let Some(doc) = doc_comment {
                    self.write_indent();
                    self.output.push_str("///");
                    self.output.push_str(doc);
                    self.output.push('\\n');
                }
                self.write_indent();
                self.output.push_str("enum ");
                self.output.push_str(name);
                if let Some(gps) = generic_params {
                    self.output.push('<');
                    self.output.push_str(&gps.join(", "));
                    self.output.push('>');
                }
                self.output.push_str(" {\\n");
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
                        self.output.push_str(",\\n");
                    } else {
                        self.output.push('\\n');
                    }
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\\n\\n");
                Ok(())
            }
            Stmt::Match { expr: match_expr, arms } => {
                self.write_indent();
                self.output.push_str("match ");
                self.format_expr(match_expr)?;
                self.output.push_str(" {\\n");
                self.indent += 1;
                for (pat, arm_body) in arms {
                    self.write_indent();
                    self.format_pattern(pat)?;
                    self.output.push_str(" => ");
                    if let Stmt::Block(stmts) = &**arm_body {
                        self.output.push_str("{\\n");
                        self.indent += 1;
                        for s in stmts { self.format_stmt(s)?; }
                        self.indent -= 1;
                        self.write_indent();
                        self.output.push_str("}\\n");
                    } else {
                        self.output.push_str("{\\n");
                        self.indent += 1;
                        self.format_stmt(arm_body)?;
                        self.indent -= 1;
                        self.write_indent();
                        self.output.push_str("}\\n");
                    }
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\\n");
                Ok(())
            }
            Stmt::Export { path } => {
                self.write_indent();
                self.output.push_str("export \\"");
                self.output.push_str(path);
                self.output.push_str("\\";\\n");
                Ok(())
            }
            Stmt::Module { name, body } => {
                self.write_indent();
                self.output.push_str("module ");
                self.output.push_str(name);
                self.output.push_str(" {\\n");
                self.indent += 1;
                for s in body { self.format_stmt(s)?; }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\\n\\n");
                Ok(())
            }
            _ => Err(()),"""

content = content.replace(stmt_fallback, stmt_implementations)


expr_fallback = """            _ => {
                return Err(());
            }"""

expr_implementations = """            Expr::InterpolatedString(parts) => {
                self.output.push_str("\\"");
                for (i, part) in parts.iter().enumerate() {
                    if i % 2 == 0 {
                        if let Expr::StringLiteral(s) = part {
                            self.output.push_str(s);
                        }
                    } else {
                        self.output.push_str("${");
                        self.format_expr(part)?;
                        self.output.push('}');
                    }
                }
                self.output.push_str("\\"");
            }
            Expr::GenericInstantiation { callee, generic_args } => {
                self.format_expr(callee)?;
                self.output.push('<');
                for (i, arg) in generic_args.iter().enumerate() {
                    if i > 0 { self.output.push_str(", "); }
                    self.format_type(arg);
                }
                self.output.push('>');
            }
            Expr::Assign { target, value } => {
                self.format_expr(target)?;
                self.output.push_str(" = ");
                self.format_expr(value)?;
            }
            Expr::MemberAccess { object, property, .. } => {
                self.format_expr(object)?;
                self.output.push('.');
                self.output.push_str(property);
            }
            Expr::OptionalMemberAccess { object, property } => {
                self.format_expr(object)?;
                self.output.push_str("?.");
                self.output.push_str(property);
            }
            Expr::Unwrap(inner) => {
                self.format_expr(inner)?;
                self.output.push('!');
            }
            Expr::Try(inner) => {
                self.format_expr(inner)?;
                self.output.push('?');
            }
            Expr::NullCoalesce { left, right } => {
                self.format_expr(left)?;
                self.output.push_str(" ?? ");
                self.format_expr(right)?;
            }
            Expr::Await(inner) => {
                self.output.push_str("await ");
                self.format_expr(inner)?;
            }
            Expr::Closure { params, return_type, body } => {
                self.output.push_str("|");
                for (i, (name, ty)) in params.iter().enumerate() {
                    if i > 0 { self.output.push_str(", "); }
                    self.output.push_str(name);
                    self.output.push_str(": ");
                    self.format_type(ty);
                }
                self.output.push_str("| ");
                if let Some(rt) = return_type {
                    self.output.push_str("-> ");
                    self.format_type(rt);
                    self.output.push(' ');
                }
                self.format_expr(body)?;
            }
            Expr::Block(stmts) => {
                self.output.push_str("{\\n");
                self.indent += 1;
                for s in stmts {
                    self.format_stmt(s)?;
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push('}');
            }
            _ => { return Err(()); }"""

content = content.replace(expr_fallback, expr_implementations)


helpers = """
    fn format_if_inline(&mut self, stmt: &Stmt) -> Result<(), ()> {
        if let Stmt::If { condition, then_branch, else_branch } = stmt {
            self.output.push_str("if ");
            self.format_expr(condition)?;
            self.output.push_str(" {\\n");
            self.indent += 1;
            if let Stmt::Block(stmts) = &**then_branch {
                for s in stmts { self.format_stmt(s)?; }
            } else {
                self.format_stmt(then_branch)?;
            }
            self.indent -= 1;
            self.write_indent();
            self.output.push('}');
            if let Some(els) = else_branch {
                if let Stmt::If { .. } = &**els {
                    self.output.push_str(" else ");
                    self.format_if_inline(els)?;
                } else {
                    self.output.push_str(" else {\\n");
                    self.indent += 1;
                    if let Stmt::Block(stmts) = &**els {
                        for s in stmts { self.format_stmt(s)?; }
                    } else {
                        self.format_stmt(els)?;
                    }
                    self.indent -= 1;
                    self.write_indent();
                    self.output.push_str("}\\n");
                }
            } else {
                self.output.push('\\n');
            }
            Ok(())
        } else {
            Err(())
        }
    }

    fn format_pattern(&mut self, pat: &pace_ast::Pattern) -> Result<(), ()> {
        match pat {
            pace_ast::Pattern::Wildcard => self.output.push('_'),
            pace_ast::Pattern::Literal(expr) => self.format_expr(expr)?,
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
"""

content = content.rstrip()
if content.endswith("}"):
    content = content[:-1] + helpers + "\n}\n"

with open(fmt_file, "w") as f:
    f.write(content)
