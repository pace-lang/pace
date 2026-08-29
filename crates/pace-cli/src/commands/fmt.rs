use miette::Result;
use colored::Colorize;
use std::fs;
use std::path::Path;
use pace_ast::{Stmt, Expr, TypeAnnotation, Visibility, BinaryOp};

pub fn execute() -> Result<()> {
    let current_dir = std::env::current_dir().map_err(|e| miette::miette!("Failed to get current dir: {}", e))?;
    
    // Process all .pace files
    let mut formatted_files = 0;
    
    for entry in walkdir::WalkDir::new(&current_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() && entry.path().extension().and_then(|s| s.to_str()) == Some("pace") {
            if format_file(entry.path())? {
                formatted_files += 1;
            }
        }
    }
    
    println!("{} Successfully formatted {} files!", "✨".green(), formatted_files);
    
    Ok(())
}

fn format_file(path: &Path) -> Result<bool> {
    let src = fs::read_to_string(path).map_err(|e| miette::miette!("Failed to read {}: {}", path.display(), e))?;
    
    let (ast, comments) = match pace_parser::parse(&src, &path.to_string_lossy()) {
        Ok(res) => res,
        Err(_) => {
            // Silently skip files with syntax errors during fmt
            return Ok(false);
        }
    };
    
    let mut formatter = Formatter::new(&src, comments);
    for stmt in &ast {
        if formatter.format_stmt(stmt).is_err() {
            // Unimplemented AST variant, skip formatting this file to avoid dataloss
            return Ok(false);
        }
    }
    
    let formatted = formatter.finish();
    if formatted != src {
        fs::write(path, formatted.clone()).map_err(|e| miette::miette!("Failed to write {}: {}", path.display(), e))?;
        println!("  Formatted {}", path.display());
        Ok(true)
    } else {
        Ok(false)
    }
}

struct Formatter<'a> {
    _src: &'a str,
    comments: Vec<(usize, usize, String)>,
    comment_idx: usize,
    output: String,
    indent: usize,
}

impl<'a> Formatter<'a> {
    fn new(src: &'a str, mut comments: Vec<(usize, usize, String)>) -> Self {
        comments.sort_by_key(|c| c.0);
        Self {
            _src: src,
            comments,
            comment_idx: 0,
            output: String::new(),
            indent: 0,
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
            if start <= end_pos + 10 { // naive threshold for inline comments on same line
                self.output.push(' ');
                let text = self.comments[self.comment_idx].2.clone();
                self.output.push_str(&text);
                self.comment_idx += 1;
            } else {
                break;
            }
        }
    }

    fn format_stmt(&mut self, stmt: &Stmt) -> Result<(), ()> {
        match stmt {
            Stmt::Expr(e) => {
                // span is not available for Expr in Stmt::Expr, so just format it.
                self.write_indent();
                self.format_expr(e)?;
                self.output.push(';');
                self.output.push('\n');
                Ok(())
            }
            Stmt::VarDecl { name, is_mutable, type_annotation, visibility, initializer, is_static, span } => {
                self.check_comments(span.0);
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
                    self.format_expr(init)?;
                }
                self.output.push(';');
                self.check_inline_comments(span.1);
                self.output.push('\n');
                Ok(())
            }
            Stmt::Return(e) => {
                self.write_indent();
                self.output.push_str("return");
                if let Some(e) = e {
                    self.output.push(' ');
                    self.format_expr(e)?;
                }
                self.output.push(';');
                self.output.push('\n');
                Ok(())
            }
            Stmt::FuncDecl { name, generic_params, params, return_type, body, is_async, is_static, visibility, doc_comment, span } => {
                self.check_comments(span.0);
                if let Some(doc) = doc_comment {
                    self.write_indent();
                    self.output.push_str("///");
                    self.output.push_str(doc);
                    self.output.push('\n');
                }
                self.write_indent();
                if *visibility == Visibility::Private { self.output.push_str("private "); }
                if *is_static { self.output.push_str("static "); }
                if *is_async { self.output.push_str("async "); }
                self.output.push_str("func ");
                self.output.push_str(name);
                if let Some(gps) = generic_params {
                    self.output.push('<');
                    self.output.push_str(&gps.join(", "));
                    self.output.push('>');
                }
                self.output.push('(');
                for (i, param) in params.iter().enumerate() {
                    if i > 0 { self.output.push_str(", "); }
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
                    self.format_stmt(s)?;
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("}\n");
                self.output.push('\n');
                Ok(())
            }
            Stmt::Import { path, alias, show, hide } => {
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
                    self.output.push_str(&show.join(", "));
                }
                if let Some(hide) = hide {
                    self.output.push_str(" hide ");
                    self.output.push_str(&hide.join(", "));
                }
                self.output.push_str(";\n");
                Ok(())
            }
            // Fallback for missing stmt types
            _ => {
                Err(())
            }
        }
    }
    
    fn format_expr(&mut self, expr: &Expr) -> Result<(), ()> {
        match expr {
            Expr::IntLiteral(n) => self.output.push_str(&n.to_string()),
            Expr::FloatLiteral(f) => self.output.push_str(&f.to_string()),
            Expr::StringLiteral(s) => {
                self.output.push('"');
                self.output.push_str(s);
                self.output.push('"');
            }
            Expr::BoolLiteral(b) => self.output.push_str(if *b { "true" } else { "false" }),
            Expr::Null => self.output.push_str("null"),
            Expr::Identifier(name) => self.output.push_str(name),
            Expr::Call { callee, args } => {
                self.format_expr(callee)?;
                self.output.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { self.output.push_str(", "); }
                    self.format_expr(arg)?;
                }
                self.output.push(')');
            }
            Expr::Binary { left, op, right } => {
                self.format_expr(left)?;
                self.output.push(' ');
                let op_str = match op {
                    BinaryOp::Add => "+", BinaryOp::Sub => "-", BinaryOp::Mul => "*", BinaryOp::Div => "/",
                    BinaryOp::Mod => "%", BinaryOp::Eq => "==", BinaryOp::NotEq => "!=", BinaryOp::Less => "<",
                    BinaryOp::LessEq => "<=", BinaryOp::Greater => ">", BinaryOp::GreaterEq => ">=",
                    BinaryOp::And => "&&", BinaryOp::Or => "||",
                };
                self.output.push_str(op_str);
                self.output.push(' ');
                self.format_expr(right)?;
            }
            _ => {
                return Err(());
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
                if i > 0 { self.output.push_str(", "); }
                self.format_type(arg);
            }
            self.output.push('>');
        }
        if ty.is_nullable {
            self.output.push('?');
        }
    }
}
