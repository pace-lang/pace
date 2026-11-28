mod decl;
mod expr;
mod stmt;

use pace_ast::{Decl, Module};

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

    pub(super) fn indent(&mut self) {
        self.indent_level += 1;
    }

    pub(super) fn dedent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }

    pub(super) fn write_indent(&mut self) {
        for _ in 0..self.indent_level {
            self.output.push_str("    ");
        }
    }

    pub(super) fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    pub(super) fn newline(&mut self) {
        self.output.push('\n');
    }

    pub(super) fn print_pending_comments_before(&mut self, span: pace_span::Span) {
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

}
