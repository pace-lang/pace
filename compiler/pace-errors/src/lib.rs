use pace_span::Span;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    TypeMismatch,
    NonExhaustiveReturn,
    ImmutableAssignment,
    UnusedVariable,
    SnakeCaseName,
    UninitializedVariable,
    MissingFields,
    UnknownField,
    ArityMismatch,
    InvalidArguments,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::TypeMismatch => "E001",
            ErrorCode::NonExhaustiveReturn => "E002",
            ErrorCode::ImmutableAssignment => "E003",
            ErrorCode::UninitializedVariable => "E004",
            ErrorCode::MissingFields => "E005",
            ErrorCode::UnknownField => "E006",
            ErrorCode::ArityMismatch => "E007",
            ErrorCode::InvalidArguments => "E008",
            ErrorCode::UnusedVariable => "W001",
            ErrorCode::SnakeCaseName => "W002",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Note => write!(f, "note"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
    pub hint: Option<String>,
    pub code: Option<ErrorCode>,
}

impl Diagnostic {
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: msg.into(),
            span: None,
            hint: None,
            code: None,
        }
    }
    
    pub fn warning(msg: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: msg.into(),
            span: None,
            hint: None,
            code: None,
        }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
    
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_code(mut self, code: ErrorCode) -> Self {
        self.code = Some(code);
        self
    }
}

#[derive(Debug, Default)]
pub struct Reporter {
    pub diagnostics: Vec<Diagnostic>,
}

impl Reporter {
    pub fn new() -> Self {
        Self { diagnostics: Vec::new() }
    }

    pub fn report(&mut self, diag: Diagnostic) {
        self.diagnostics.push(diag);
    }
    
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }

    pub fn emit_all(&self, source: &str, file_name: &str) {
        for diag in &self.diagnostics {
            let color_code = match diag.severity {
                Severity::Error => "\x1b[31m",   // Red
                Severity::Warning => "\x1b[33m", // Yellow
                Severity::Note => "\x1b[36m",    // Cyan
            };
            let reset_code = "\x1b[0m";
            let bold_code = "\x1b[1m";
            
            let code_str = match &diag.code {
                Some(c) => format!("[{}]", c.as_str()),
                None => "".to_string(),
            };
            
            eprintln!("{}{} {}{}{}: {}", bold_code, color_code, diag.severity, code_str, reset_code, diag.message);
            
            if let Some(span) = diag.span {
                // Find line number and column
                let mut line = 1;
                let mut col = 1;
                let mut line_start = 0;
                let mut line_end = source.len();
                
                for (i, c) in source.char_indices() {
                    if i == span.start {
                        break;
                    }
                    if c == '\n' {
                        line += 1;
                        col = 1;
                        line_start = i + 1;
                    } else {
                        col += 1;
                    }
                }
                
                for (i, c) in source[line_start..].char_indices() {
                    if c == '\n' {
                        line_end = line_start + i;
                        break;
                    }
                }
                
                let line_str = &source[line_start..line_end];
                
                eprintln!("  --> {}:{}:{}", file_name, line, col);
                eprintln!("   |");
                eprintln!("{:<2} | {}", line, line_str);
                eprintln!("   | {}{}{}{}", " ".repeat(col - 1), color_code, "^".repeat(std::cmp::max(1, span.end.saturating_sub(span.start))), reset_code);
            }
            
            if let Some(hint) = &diag.hint {
                eprintln!("   = \x1b[1mhelp\x1b[0m: {}", hint);
            }
            eprintln!();
        }
    }
}
