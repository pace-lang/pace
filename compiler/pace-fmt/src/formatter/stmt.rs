use pace_ast::{Block, Stmt};

use super::Formatter;

impl Formatter {
    pub(super) fn format_block(&mut self, block: &Block) {
        self.write("{\n");
        self.indent();
        for stmt in &block.statements {
            self.format_stmt(stmt);
        }
        self.dedent();
        self.write_indent();
        self.write("}");
    }

    pub(super) fn format_stmt(&mut self, stmt: &Stmt) {
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
            Stmt::Break(_) => {
                self.write("break");
                self.newline();
            }
            Stmt::Continue(_) => {
                self.write("continue");
                self.newline();
            }
            Stmt::For { var, iter, body, .. } => {
                self.write("for ");
                self.write(&var.name);
                self.write(" in ");
                self.format_expr(iter);
                self.write(" ");
                self.format_block(body);
                self.newline();
            }
        }
    }

}
