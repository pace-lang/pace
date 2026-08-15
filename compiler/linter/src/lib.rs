use ast::stmt::{Stmt, StmtKind};
use diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticCode, Span};

pub struct Linter<'a> {
    session: &'a session::CompilerSession,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Linter<'a> {
    pub fn new(session: &'a session::CompilerSession) -> Self {
        Self {
            session,
            diagnostics: Vec::new(),
        }
    }

    pub fn lint(&mut self, program: &[Stmt]) {
        for stmt in program {
            self.check_stmt(stmt);
        }
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Enum { name, variants, .. } => {
                self.check_pascal_case(
                    self.session.interner.borrow().lookup(*name),
                    stmt.span,
                    "Enum",
                );
                for variant in variants {
                    self.check_pascal_case(
                        self.session.interner.borrow().lookup(variant.name),
                        stmt.span,
                        "Enum variant",
                    );
                }
            }
            StmtKind::Class {
                name,
                methods,
                fields,
                ..
            } => {
                self.check_pascal_case(
                    self.session.interner.borrow().lookup(*name),
                    stmt.span,
                    "Class",
                );
                for field in fields {
                    self.check_stmt(field);
                }
                for method in methods {
                    self.check_stmt(method);
                }
            }
            StmtKind::Interface { name, methods, .. } => {
                self.check_pascal_case(
                    self.session.interner.borrow().lookup(*name),
                    stmt.span,
                    "Interface",
                );
                for method in methods {
                    self.check_stmt(method);
                }
            }
            StmtKind::Func { name, body, .. } => {
                self.check_camel_case(
                    self.session.interner.borrow().lookup(*name),
                    stmt.span,
                    "Function",
                );
                self.check_stmt(body);
            }
            StmtKind::ForeignFunc { name, .. } => {
                self.check_camel_case(
                    self.session.interner.borrow().lookup(*name),
                    stmt.span,
                    "Foreign function",
                );
            }
            StmtKind::Let { name, .. } | StmtKind::Var { name, .. } => {
                // If it's a global constant-like value that they wrote in UPPER_SNAKE_CASE,
                // we warn them. For now, Pace prefers camelCase for variables/lets.
                // However, they specifically mentioned they want constants to be PascalCase.
                // Since Pace doesn't have a strict `const` keyword yet, we will just
                // ensure it's not UPPER_SNAKE_CASE. If it is UPPER_SNAKE_CASE, we'll
                // guide them to PascalCase or camelCase.
                let name_str = self.session.interner.borrow().lookup(*name).to_string();
                if is_upper_snake_case(&name_str) {
                    self.report_naming_violation(
                        stmt.span,
                        &format!("Variable `{}` uses UPPER_SNAKE_CASE which is not idiomatic in Pace", name_str),
                        &format!("use camelCase for variables, or PascalCase if this is intended to be a constant (e.g. `{}`)", to_pascal_case(&name_str)),
                    );
                } else if !is_camel_case(&name_str) && !is_pascal_case(&name_str) {
                    self.report_naming_violation(
                        stmt.span,
                        &format!(
                            "Variable `{}` does not follow Pace naming conventions",
                            name_str
                        ),
                        "use camelCase for variables",
                    );
                }
            }
            StmtKind::Block(stmts) => {
                for s in stmts {
                    self.check_stmt(s);
                }
            }
            StmtKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.check_stmt(then_branch);
                if let Some(e) = else_branch {
                    self.check_stmt(e);
                }
            }
            StmtKind::While { body, .. } | StmtKind::For { body, .. } => {
                self.check_stmt(body);
            }
            _ => {}
        }
    }

    fn check_pascal_case(&mut self, name: &str, span: Span, kind: &str) {
        if !is_pascal_case(name) {
            self.report_naming_violation(
                span,
                &format!("{} `{}` does not follow Pace naming convention", kind, name),
                &format!("use PascalCase (e.g. `{}`)", to_pascal_case(name)),
            );
        }
    }

    fn check_camel_case(&mut self, name: &str, span: Span, kind: &str) {
        if !is_camel_case(name) {
            self.report_naming_violation(
                span,
                &format!("{} `{}` does not follow Pace naming convention", kind, name),
                &format!("use camelCase (e.g. `{}`)", to_camel_case(name)),
            );
        }
    }

    fn report_naming_violation(&mut self, span: Span, message: &str, help: &str) {
        self.diagnostics.push(
            DiagnosticBuilder::warning(DiagnosticCode::NamingConventionViolation, message, span)
                .with_help(help)
                .build(),
        );
    }
}

fn is_pascal_case(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    if !chars.next().unwrap().is_ascii_uppercase() {
        return false;
    }
    !s.contains('_')
}

fn is_camel_case(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    if s == "main" {
        return true;
    }

    // We allow init as a special method name
    if s == "init" {
        return true;
    }

    let mut chars = s.chars();
    if !chars.next().unwrap().is_ascii_lowercase() {
        return false;
    }
    !s.contains('_')
}

fn is_upper_snake_case(s: &str) -> bool {
    // Has at least one uppercase letter, no lowercase letters, and contains an underscore
    // OR is just all uppercase
    s.chars().any(|c| c.is_ascii_uppercase()) && !s.chars().any(|c| c.is_ascii_lowercase())
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c.to_ascii_lowercase());
        }
    }
    result
}

fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for (i, c) in s.chars().enumerate() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else if i == 0 {
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}
