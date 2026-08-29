import re

fmt_file = "crates/pace-cli/src/commands/fmt.rs"
with open(fmt_file, "r") as f:
    content = f.read()

# Replace Expr::Binary in format_expr
old_binary = """            Expr::Binary { left, op, right } => {
                self.format_expr(left)?;
                self.output.push(' ');
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Subtract => "-",
                    BinaryOp::Multiply => "*",
                    BinaryOp::Divide => "/",
                    BinaryOp::Modulo => "%",
                    BinaryOp::EqEq => "==",
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
                self.format_expr(right)?;
            }"""

new_binary = """            Expr::Binary { left, op, right } => {
                let p = Self::binary_precedence(op);
                self.format_sub_expr(left, p, false)?;
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
                self.format_sub_expr(right, p, true)?;
            }"""

content = content.replace(old_binary, new_binary)


# Add helper methods to Formatter
helpers = """
    fn binary_precedence(op: &pace_ast::expr::BinaryOp) -> u8 {
        use pace_ast::expr::BinaryOp::*;
        match op {
            Mul | Div | Mod => 6,
            Add | Sub => 5,
            Less | LessEq | Greater | GreaterEq => 4,
            Eq | NotEq => 3,
            And => 2,
            Or => 1,
        }
    }

    fn format_sub_expr(&mut self, sub: &Expr, parent_prec: u8, is_right: bool) -> Result<(), ()> {
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
        self.format_expr(sub)?;
        if needs_parens {
            self.output.push(')');
        }
        Ok(())
    }
"""

content = content.rstrip()
if content.endswith("}"):
    content = content[:-1] + helpers + "\n}\n"

with open(fmt_file, "w") as f:
    f.write(content)
