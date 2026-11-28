use pace_ast::{BinaryOp, Expr, MatchArm, Pattern};

use super::Formatter;

impl Formatter {
    pub(super) fn format_expr(&mut self, expr: &Expr) {
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
                let prec = Self::binary_precedence(*op);
                let left_paren = match left.as_ref() {
                    Expr::Binary { op: lop, .. } => Self::binary_precedence(*lop) < prec,
                    Expr::Assign { .. } | Expr::Closure { .. } => true,
                    _ => false,
                };
                let right_paren = match right.as_ref() {
                    Expr::Binary { op: rop, .. } => {
                        let rp = Self::binary_precedence(*rop);
                        rp < prec
                            || (rp == prec
                                && !matches!(
                                    op,
                                    BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or
                                ))
                    }
                    Expr::Assign { .. } | Expr::Closure { .. } => true,
                    _ => false,
                };
                self.format_expr_paren(left, left_paren);
                self.write(" ");
                self.write(match op {
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
                    BinaryOp::NullCoalesce => "??",
                    BinaryOp::And => "&&",
                    BinaryOp::Or => "||",
                });
                self.write(" ");
                self.format_expr_paren(right, right_paren);
            }
            Expr::Unary { op, expr, .. } => {
                self.write(match op {
                    pace_ast::UnaryOp::Neg => "-",
                    pace_ast::UnaryOp::Not => "!",
                });
                let needs_paren = matches!(
                    expr.as_ref(),
                    Expr::Binary { .. } | Expr::Assign { .. } | Expr::Closure { .. }
                );
                self.format_expr_paren(expr, needs_paren);
            }
            Expr::MemberAccess { object, member, .. } => {
                self.format_expr_paren(object, Self::is_low_prec_expr(object));
                self.write(".");
                self.write(&member.name);
            }
            Expr::OptionalMemberAccess { object, member, .. } => {
                self.format_expr_paren(object, Self::is_low_prec_expr(object));
                self.write("?.");
                self.write(&member.name);
            }
            Expr::Call { callee, args, .. } => {
                self.format_expr_paren(callee, Self::is_low_prec_expr(callee));
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
            Expr::Closure { params, return_type, body, .. } => {
                self.write("(");
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&param.name.name);
                    if let Some(ty) = &param.ty {
                        self.write(": ");
                        self.format_type(ty);
                    }
                }
                self.write(")");
                if let Some(ty) = return_type {
                    self.write(" -> ");
                    self.format_type(ty);
                }
                self.write(" => ");
                match body {
                    pace_ast::ClosureBody::Expr(e) => self.format_expr(e),
                    pace_ast::ClosureBody::Block(b) => {
                        self.write("{\n");
                        self.indent();
                        for stmt in &b.statements {
                            self.format_stmt(stmt);
                        }
                        self.dedent();
                        self.write_indent();
                        self.write("}");
                    }
                }
            }
            Expr::Await(inner, _) => {
                self.write("await ");
                self.format_expr(inner);
            }
            Expr::Try(inner, _) => {
                self.format_expr_paren(inner, Self::is_low_prec_expr(inner));
                self.write("?");
            }
            Expr::ListLiteral(elements, _) => {
                self.write("[");
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(elem);
                }
                self.write("]");
            }
            Expr::MapLiteral(entries, _) => {
                self.write("{");
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(k);
                    self.write(": ");
                    self.format_expr(v);
                }
                self.write("}");
            }
            Expr::Index { target, index, .. } => {
                self.format_expr_paren(target, Self::is_low_prec_expr(target));
                self.write("[");
                self.format_expr(index);
                self.write("]");
            }
        }
    }

    fn binary_precedence(op: BinaryOp) -> u8 {
        match op {
            BinaryOp::Or => 1,
            BinaryOp::And => 2,
            BinaryOp::NullCoalesce => 3,
            BinaryOp::EqEq | BinaryOp::NotEq => 4,
            BinaryOp::Gt | BinaryOp::Lt | BinaryOp::GtEq | BinaryOp::LtEq => 5,
            BinaryOp::Add | BinaryOp::Sub => 6,
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => 7,
        }
    }

    fn is_low_prec_expr(expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::Binary { .. }
                | Expr::Unary { .. }
                | Expr::Assign { .. }
                | Expr::Closure { .. }
                | Expr::Await(_, _)
        )
    }

    fn format_expr_paren(&mut self, expr: &Expr, needs_paren: bool) {
        if needs_paren {
            self.write("(");
            self.format_expr(expr);
            self.write(")");
        } else {
            self.format_expr(expr);
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
