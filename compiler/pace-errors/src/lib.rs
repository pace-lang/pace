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
        Self {
            diagnostics: Vec::new(),
        }
    }

    pub fn report(&mut self, diag: Diagnostic) {
        self.diagnostics.push(diag);
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    pub fn emit_all(&self, source_map: &pace_span::SourceMap) {
        use ariadne::{Color, Label, Report, ReportKind, Source};

        for diag in &self.diagnostics {
            let kind = match diag.severity {
                Severity::Error => ReportKind::Error,
                Severity::Warning => ReportKind::Warning,
                Severity::Note => ReportKind::Advice,
            };

            let color = match diag.severity {
                Severity::Error => Color::Red,
                Severity::Warning => Color::Yellow,
                Severity::Note => Color::Cyan,
            };

            let file_id = diag
                .span
                .map(|s| s.file_id)
                .unwrap_or(pace_span::FileId::DUMMY);
            let file_name = source_map.get_path(file_id).unwrap_or("unknown");
            let source_text = source_map.get_source(file_id).unwrap_or("");

            let span_start = diag.span.map(|s| s.start as usize).unwrap_or(0);
            let span_end = diag.span.map(|s| s.end as usize).unwrap_or(span_start + 1);

            let mut builder =
                Report::build(kind, (file_name, span_start..span_end)).with_message(&diag.message);

            if let Some(code) = &diag.code {
                builder = builder.with_code(code.as_str());
            }

            if let Some(span) = diag.span {
                let mut label = Label::new((file_name, (span.start as usize)..(span.end as usize)))
                    .with_color(color);

                if let Some(hint) = &diag.hint {
                    label = label.with_message(hint);
                } else {
                    label = label.with_message("here");
                }

                builder = builder.with_label(label);
            } else if let Some(hint) = &diag.hint {
                builder = builder.with_note(hint);
            }

            builder
                .finish()
                .eprint((file_name, Source::from(source_text)))
                .unwrap();
        }
    }
}
