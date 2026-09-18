use pace_ast::{BinaryOp, Block, Decl, Expr, GenericParam, MatchArm, Module, Pattern, Stmt, Type};

pub struct Formatter {
    indent_level: usize,
    output: String,
    comments: Vec<(pace_span::Span, String)>,
    comment_index: usize,
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter {
    pub fn new() -> Self {
        Self {
            indent_level: 0,
            output: String::new(),
            comments: Vec::new(),
            comment_index: 0,
        }
    }

    fn indent(&mut self) {
        self.indent_level += 1;
    }

    fn dedent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent_level {
            self.output.push_str("    ");
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }

    fn print_pending_comments_before(&mut self, span: pace_span::Span) {
        while self.comment_index < self.comments.len() {
            let comment_span = self.comments[self.comment_index].0;
            if comment_span.start < span.start {
                let text = self.comments[self.comment_index].1.clone();
                self.write_indent();
                self.write(&text);
                self.newline();
                self.comment_index += 1;
            } else {
                break;
            }
        }
    }

    fn print_remaining_comments(&mut self) {
        while self.comment_index < self.comments.len() {
            let text = &self.comments[self.comment_index].1.clone();
            self.write_indent();
            self.write(text);
            self.newline();
            self.comment_index += 1;
        }
    }

    pub fn format_module(&mut self, module: &Module) -> String {
        self.comments = module.comments.clone();
        self.comment_index = 0;

        let mut imports = Vec::new();
        let mut others = Vec::new();

        for decl in &module.declarations {
            if let Decl::Import { .. } = decl {
                imports.push(decl);
            } else {
                others.push(decl);
            }
        }

        imports.sort_by(|a, b| {
            let path_a = match a {
                Decl::Import { path, .. } => path
                    .iter()
                    .map(|i| i.name.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
                _ => String::new(),
            };
            let path_b = match b {
                Decl::Import { path, .. } => path
                    .iter()
                    .map(|i| i.name.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
                _ => String::new(),
            };
            path_a.cmp(&path_b)
        });

        for import in imports {
            self.format_decl(import);
        }

        if !self.output.is_empty() && !others.is_empty() {
            self.newline();
        }

        for decl in others {
            self.format_decl(decl);
            self.newline();
        }

        self.print_remaining_comments();
        self.output.trim().to_string() + "\n"
    }

    fn format_decl(&mut self, decl: &Decl) {
        self.print_pending_comments_before(decl.span());
        self.write_indent();
        match decl {
            Decl::Import { path, alias, .. } => {
                self.write("import ");
                let path_str = path
                    .iter()
                    .map(|i| i.name.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                self.write(&path_str);
                if let Some(a) = alias {
                    self.write(" as ");
                    self.write(&a.name);
                }
                self.newline();
            }
            Decl::Let {
                name,
                ty,
                value,
                is_private,
                ..
            } => {
                if *is_private {
                    self.write("private ");
                }
                self.write("let ");
                self.write(&name.name);
                if let Some(t) = ty {
                    self.write(": ");
                    self.format_type(t);
                }
                if let Some(v) = value {
                    self.write(" = ");
                    self.format_expr(v);
                }
                self.newline();
            }
            Decl::Var {
                name,
                ty,
                value,
                is_private,
                ..
            } => {
                if *is_private {
                    self.write("private ");
                }
                self.write("var ");
                self.write(&name.name);
                if let Some(t) = ty {
                    self.write(": ");
                    self.format_type(t);
                }
                if let Some(v) = value {
                    self.write(" = ");
                    self.format_expr(v);
                }
                self.newline();
            }
            Decl::Const {
                name,
                ty,
                value,
                is_private,
                ..
            } => {
                if *is_private {
                    self.write("private ");
                }
                self.write("const ");
                self.write(&name.name);
                if let Some(t) = ty {
                    self.write(": ");
                    self.format_type(t);
                }
                self.write(" = ");
                self.format_expr(value);
                self.newline();
            }
            Decl::Function {
                name,
                generic_params,
                params,
                return_type,
                body,
                is_static,
                is_override,
                is_private,
                ..
            } => {
                if *is_private {
                    self.write("private ");
                }
                if *is_override {
                    self.write("override ");
                }
                if *is_static {
                    self.write("static ");
                }
                self.write("fn ");
                self.write(&name.name);
                if let Some(gps) = generic_params {
                    self.format_generic_params(gps);
                }
                self.write("(");
                for (i, (p_name, p_ty)) in params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&p_name.name);
                    self.write(": ");
                    self.format_type(p_ty);
                }
                self.write(")");
                if let Some(rt) = return_type {
                    self.write(" -> ");
                    self.format_type(rt);
                }
                self.write(" ");
                self.format_block(body);
                self.newline();
            }
            Decl::Struct {
                name,
                generic_params,
                with,
                fields,
                static_fields,
                const_fields,
                methods,
                is_private,
                ..
            } => {
                if *is_private {
                    self.write("private ");
                }
                self.write("struct ");
                self.write(&name.name);
                if let Some(gps) = generic_params {
                    self.format_generic_params(gps);
                }
                if !with.is_empty() {
                    self.write(" with ");
                    let traits = with
                        .iter()
                        .map(|i| i.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.write(&traits);
                }
                self.write(" {\n");
                self.indent();

                for pace_ast::ConstFieldDef {
                    name: c_name,
                    ty: c_ty,
                    value: c_val,
                    is_private,
                } in const_fields
                {
                    self.print_pending_comments_before(c_name.span);
                    self.write_indent();
                    if *is_private {
                        self.write("private ");
                    }
                    self.write("const ");
                    self.write(&c_name.name);
                    self.write(": ");
                    self.format_type(c_ty);
                    self.write(" = ");
                    self.format_expr(c_val);
                    self.newline();
                }
                for pace_ast::StaticFieldDef {
                    name: s_name,
                    ty: s_ty,
                    value: s_val,
                    is_private,
                } in static_fields
                {
                    self.print_pending_comments_before(s_name.span);
                    self.write_indent();
                    if *is_private {
                        self.write("private ");
                    }
                    self.write("static ");
                    self.write(&s_name.name);
                    self.write(": ");
                    self.format_type(s_ty);
                    self.write(" = ");
                    self.format_expr(s_val);
                    self.newline();
                }
                for pace_ast::FieldDef {
                    name: f_name,
                    ty: f_ty,
                    default_value: f_val,
                    is_mut,
                    is_private,
                } in fields
                {
                    self.print_pending_comments_before(f_name.span);
                    self.write_indent();
                    if *is_private {
                        self.write("private ");
                    }
                    if !*is_mut {
                        self.write("let ");
                    } else {
                        self.write("var ");
                    }
                    self.write(&f_name.name);
                    self.write(": ");
                    self.format_type(f_ty);
                    if let Some(v) = f_val {
                        self.write(" = ");
                        self.format_expr(v);
                    }
                    self.newline();
                }
                for m in methods {
                    self.newline(); // extra space before methods
                    self.format_decl(m);
                }

                self.dedent();
                self.write_indent();
                self.write("}\n");
            }
            Decl::Class {
                name,
                generic_params,
                extends,
                with,
                fields,
                static_fields,
                const_fields,
                methods,
                is_private,
                ..
            } => {
                if *is_private {
                    self.write("private ");
                }
                self.write("class ");
                self.write(&name.name);
                if let Some(gps) = generic_params {
                    self.format_generic_params(gps);
                }
                if let Some(ext) = extends {
                    self.write(" extends ");
                    self.write(&ext.name);
                }
                if !with.is_empty() {
                    self.write(" with ");
                    let traits = with
                        .iter()
                        .map(|i| i.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.write(&traits);
                }
                self.write(" {\n");
                self.indent();

                for pace_ast::ConstFieldDef {
                    name: c_name,
                    ty: c_ty,
                    value: c_val,
                    is_private,
                } in const_fields
                {
                    self.print_pending_comments_before(c_name.span);
                    self.write_indent();
                    if *is_private {
                        self.write("private ");
                    }
                    self.write("const ");
                    self.write(&c_name.name);
                    self.write(": ");
                    self.format_type(c_ty);
                    self.write(" = ");
                    self.format_expr(c_val);
                    self.newline();
                }
                for pace_ast::StaticFieldDef {
                    name: s_name,
                    ty: s_ty,
                    value: s_val,
                    is_private,
                } in static_fields
                {
                    self.print_pending_comments_before(s_name.span);
                    self.write_indent();
                    if *is_private {
                        self.write("private ");
                    }
                    self.write("static ");
                    self.write(&s_name.name);
                    self.write(": ");
                    self.format_type(s_ty);
                    self.write(" = ");
                    self.format_expr(s_val);
                    self.newline();
                }
                for pace_ast::FieldDef {
                    name: f_name,
                    ty: f_ty,
                    default_value: f_val,
                    is_mut,
                    is_private,
                } in fields
                {
                    self.print_pending_comments_before(f_name.span);
                    self.write_indent();
                    if *is_private {
                        self.write("private ");
                    }
                    if !*is_mut {
                        self.write("let ");
                    } else {
                        self.write("var ");
                    }
                    self.write(&f_name.name);
                    self.write(": ");
                    self.format_type(f_ty);
                    if let Some(v) = f_val {
                        self.write(" = ");
                        self.format_expr(v);
                    }
                    self.newline();
                }
                for m in methods {
                    self.newline();
                    self.format_decl(m);
                }

                self.dedent();
                self.write_indent();
                self.write("}\n");
            }
            Decl::Trait {
                name,
                generic_params,
                methods,
                is_private,
                ..
            } => {
                if *is_private {
                    self.write("private ");
                }
                self.write("trait ");
                self.write(&name.name);
                if let Some(gps) = generic_params {
                    self.format_generic_params(gps);
                }
                self.write(" {\n");
                self.indent();
                for m in methods {
                    self.format_decl(m);
                }
                self.dedent();
                self.write_indent();
                self.write("}\n");
            }
            Decl::Enum {
                name,
                generic_params,
                variants,
                is_private,
                ..
            } => {
                if *is_private {
                    self.write("private ");
                }
                self.write("enum ");
                self.write(&name.name);
                if let Some(gps) = generic_params {
                    self.format_generic_params(gps);
                }
                self.write(" {\n");
                self.indent();
                for var in variants.iter() {
                    self.write_indent();
                    self.write(&var.name.name);
                    if let Some(fields) = &var.fields {
                        self.write("(");
                        for (j, (f_name, f_ty)) in fields.iter().enumerate() {
                            if j > 0 {
                                self.write(", ");
                            }
                            self.write(&f_name.name);
                            self.write(": ");
                            self.format_type(f_ty);
                        }
                        self.write(")");
                    }
                    self.newline();
                }
                self.dedent();
                self.write_indent();
                self.write("}\n");
            }
            Decl::Expr(expr, _) => {
                self.format_expr(expr);
                self.newline();
            }
        }
    }

    fn format_generic_params(&mut self, params: &[GenericParam]) {
        self.write("<");
        for (i, p) in params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write(&p.name.name);
            if let Some(def) = &p.default {
                self.write(" = ");
                self.format_type(def);
            }
        }
        self.write(">");
    }

    fn format_type(&mut self, ty: &Type) {
        match ty {
            Type::Named(ident) => self.write(&ident.name),
            Type::Generic(base, args, _) => {
                self.format_type(base);
                self.write("<");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_type(a);
                }
                self.write(">");
            }
            Type::Optional(inner, _) => {
                self.format_type(inner);
                self.write("?");
            }
        }
    }

    fn format_block(&mut self, block: &Block) {
        self.write("{\n");
        self.indent();
        for stmt in &block.statements {
            self.format_stmt(stmt);
        }
        self.dedent();
        self.write_indent();
        self.write("}");
    }

    fn format_stmt(&mut self, stmt: &Stmt) {
        self.print_pending_comments_before(stmt.span());
        self.write_indent();
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
                self.write("let ");
                self.write(&name.name);
                if let Some(t) = ty {
                    self.write(": ");
                    self.format_type(t);
                }
                if let Some(v) = value {
                    self.write(" = ");
                    self.format_expr(v);
                }
                self.newline();
            }
            Stmt::Var {
                name, ty, value, ..
            } => {
                self.write("var ");
                self.write(&name.name);
                if let Some(t) = ty {
                    self.write(": ");
                    self.format_type(t);
                }
                if let Some(v) = value {
                    self.write(" = ");
                    self.format_expr(v);
                }
                self.newline();
            }
            Stmt::ExprStmt(expr, _) => {
                self.format_expr(expr);
                self.newline();
            }
            Stmt::Return(expr, _) => {
                self.write("return");
                if let Some(e) = expr {
                    self.write(" ");
                    self.format_expr(e);
                }
                self.newline();
            }
        }
    }

    fn format_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::IntLiteral(val, _) => self.write(val),
            Expr::FloatLiteral(val, _) => self.write(val),
            Expr::StringLiteral(val, _) => {
                self.write(val);
            }
            Expr::InterpolatedString(exprs, _) => {
                self.write("\"");
                for expr in exprs {
                    if let Expr::StringLiteral(val, _) = expr {
                        self.write(val.trim_matches('"'));
                    } else {
                        self.write("${");
                        self.format_expr(expr);
                        self.write("}");
                    }
                }
                self.write("\"");
            }
            Expr::Null(_) => self.write("null"),
            Expr::Ident(ident, generic_args) => {
                self.write(&ident.name);
                if let Some(args) = generic_args {
                    self.write("<");
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.format_type(a);
                    }
                    self.write(">");
                }
            }
            Expr::Super(_) => self.write("super"),
            Expr::Binary {
                left, op, right, ..
            } => {
                self.format_expr(left);
                self.write(" ");
                self.write(match op {
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
                    BinaryOp::NullCoalesce => "??",
                    BinaryOp::And => "&&",
                    BinaryOp::Or => "||",
                });
                self.write(" ");
                self.format_expr(right);
            }
            Expr::MemberAccess { object, member, .. } => {
                self.format_expr(object);
                self.write(".");
                self.write(&member.name);
            }
            Expr::OptionalMemberAccess { object, member, .. } => {
                self.format_expr(object);
                self.write("?.");
                self.write(&member.name);
            }
            Expr::Call { callee, args, .. } => {
                self.format_expr(callee);
                self.write("(");
                for (
                    i,
                    pace_ast::CallArg {
                        label: name,
                        expr: arg_expr,
                    },
                ) in args.iter().enumerate()
                {
                    if i > 0 {
                        self.write(", ");
                    }
                    if let Some(n) = name {
                        self.write(&n.name);
                        self.write(": ");
                    }
                    self.format_expr(arg_expr);
                }
                self.write(")");
            }
            Expr::If {
                cond,
                then_block,
                else_block,
                ..
            } => {
                self.write("if ");
                self.format_expr(cond);
                self.write(" ");
                self.format_block(then_block);
                if let Some(el) = else_block {
                    self.write(" else ");
                    self.format_block(el);
                }
            }
            Expr::While { cond, body, .. } => {
                self.write("while ");
                self.format_expr(cond);
                self.write(" ");
                self.format_block(body);
            }
            Expr::Assign { target, value, .. } => {
                self.format_expr(target);
                self.write(" = ");
                self.format_expr(value);
            }
            Expr::Match { subject, arms, .. } => {
                self.write("match ");
                self.format_expr(subject);
                self.write(" {\n");
                self.indent();
                for arm in arms {
                    self.format_match_arm(arm);
                }
                self.dedent();
                self.write_indent();
                self.write("}");
            }
        }
    }

    fn format_match_arm(&mut self, arm: &MatchArm) {
        self.write_indent();
        match &arm.pattern {
            Pattern::Ident(ident) => self.write(&ident.name),
            Pattern::Variant { name, fields, .. } => {
                self.write(&name.name);
                if let Some(fs) = fields {
                    self.write("(");
                    for (i, f) in fs.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.write(&f.name);
                    }
                    self.write(")");
                }
            }
            Pattern::CatchAll(_) => self.write("_"),
        }
        self.write(" => ");
        self.format_expr(&arm.body);
        self.newline();
    }
}
