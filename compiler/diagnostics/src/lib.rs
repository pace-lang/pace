pub mod span;
pub mod source_map;
pub mod formatter;

pub use span::{Location, Span};
pub use source_map::SourceMap;
pub use formatter::print_diagnostics;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticCode {
    // Lexer & Syntax (P1xxx)
    InvalidToken, // P1001
    UnexpectedToken, // P1002
    
    // Name Resolution (P2xxx)
    UnknownIdentifier, // P2001
    DuplicateDeclaration, // P2002
    
    // Type System (P3xxx)
    TypeMismatch, // P3001
    UnknownType, // P3002
    UninitializedVariable, // P3003
    
    // Ownership & ARC (P4xxx)
    InvalidWeakReference, // P4001
    OwnershipViolation, // P4002
    
    // General
    Custom(String),
}

impl DiagnosticCode {
    pub fn as_str(&self) -> &str {
        match self {
            DiagnosticCode::InvalidToken => "P1001",
            DiagnosticCode::UnexpectedToken => "P1002",
            DiagnosticCode::UnknownIdentifier => "P2001",
            DiagnosticCode::DuplicateDeclaration => "P2002",
            DiagnosticCode::TypeMismatch => "P3001",
            DiagnosticCode::UnknownType => "P3002",
            DiagnosticCode::UninitializedVariable => "P3003",
            DiagnosticCode::InvalidWeakReference => "P4001",
            DiagnosticCode::OwnershipViolation => "P4002",
            DiagnosticCode::Custom(code) => code,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub primary_span: Span,
    pub labels: Vec<Label>,
    pub help: Option<String>,
    pub notes: Vec<String>,
}

pub struct DiagnosticBuilder {
    diagnostic: Diagnostic,
}

impl DiagnosticBuilder {
    pub fn new(severity: Severity, code: DiagnosticCode, message: String, primary_span: Span) -> Self {
        Self {
            diagnostic: Diagnostic {
                severity,
                code,
                message,
                primary_span,
                labels: Vec::new(),
                help: None,
                notes: Vec::new(),
            },
        }
    }

    pub fn error(code: DiagnosticCode, message: impl Into<String>, primary_span: Span) -> Self {
        Self::new(Severity::Error, code, message.into(), primary_span)
    }
    
    pub fn global_error(code: DiagnosticCode, message: impl Into<String>) -> Self {
        let span = Span::new(u32::MAX, 0, 0, Location::new(0, 0), Location::new(0, 0));
        Self::new(Severity::Error, code, message.into(), span)
    }
    
    pub fn warning(code: DiagnosticCode, message: impl Into<String>, primary_span: Span) -> Self {
        Self::new(Severity::Warning, code, message.into(), primary_span)
    }

    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.diagnostic.labels.push(Label {
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.diagnostic.help = Some(help.into());
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.diagnostic.notes.push(note.into());
        self
    }

    pub fn build(self) -> Diagnostic {
        self.diagnostic
    }
}
