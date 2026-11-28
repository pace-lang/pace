use pace_ast::{Decl, GenericParam, Type};

use super::Formatter;

impl Formatter {
    pub(super) fn format_decl(&mut self, decl: &Decl) {
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
                is_async,
                is_private,
                annotations,
                ..
            } => {
                for ann in annotations {
                    self.write("@");
                    self.write(&ann.name.name);
                    if let Some(args) = &ann.args {
                        self.write("(");
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                self.write(", ");
                            }
                            if let Some(lbl) = &arg.label {
                                self.write(&lbl.name);
                                self.write(": ");
                            }
                            self.format_expr(&arg.expr);
                        }
                        self.write(")");
                    }
                    self.newline();
                }
                if *is_private {
                    self.write("private ");
                }
                if *is_static {
                    self.write("static ");
                }
                if *is_async {
                    self.write("async ");
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
                    annotations: f_annots,
                } in fields
                {
                    for ann in f_annots {
                        self.write_indent();
                        self.write("@");
                        self.write(&ann.name.name);
                        self.newline();
                    }
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
                    annotations: f_annots,
                } in fields
                {
                    for ann in f_annots {
                        self.write_indent();
                        self.write("@");
                        self.write(&ann.name.name);
                        self.newline();
                    }
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

            Decl::Actor {
                name,
                generic_params,
                with,
                fields,
                methods,
                is_private,
                ..
            } => {
                if *is_private {
                    self.write("private ");
                }
                self.write("actor ");
                self.write(&name.name);
                if let Some(gp) = generic_params {
                    self.format_generic_params(gp);
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

                for pace_ast::FieldDef {
                    name: f_name,
                    ty: f_ty,
                    default_value: f_val,
                    is_mut,
                    is_private,
                    annotations: f_annots,
                } in fields
                {
                    for ann in f_annots {
                        self.write_indent();
                        self.write("@");
                        self.write(&ann.name.name);
                        self.newline();
                    }
                    self.print_pending_comments_before(f_name.span);
                    self.write_indent();
                    if *is_private {
                        self.write("private ");
                    }
                    if *is_mut {
                        self.write("var ");
                    } else {
                        self.write("let ");
                    }
                    self.write(&f_name.name);
                    self.write(": ");
                    self.format_type(f_ty);
                    if let Some(val) = f_val {
                        self.write(" = ");
                        self.format_expr(val);
                    }
                    self.newline();
                }

                if !fields.is_empty() && !methods.is_empty() {
                    self.newline();
                }

                for (i, m) in methods.iter().enumerate() {
                    self.format_decl(m);
                    if i < methods.len() - 1 {
                        self.newline();
                    }
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

    pub(super) fn format_type(&mut self, ty: &Type) {
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
            Type::Closure(params, ret, _) => {
                self.write("(");
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_type(p);
                }
                self.write(") -> ");
                self.format_type(ret);
            }
        }
    }

}
