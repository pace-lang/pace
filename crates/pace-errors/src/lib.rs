use miette::{Diagnostic, NamedSource};
use thiserror::Error;

#[derive(Clone, Error, Diagnostic, Debug)]
pub enum SyntaxError {
    #[error("Syntax error: {message}")]
    #[diagnostic(
        code(P1002),
        help("Check the language syntax guidelines for proper formatting.")
    )]
    Generic {
        message: String,
        #[source_code]
        src: miette::NamedSource<String>,
        #[label("Unexpected token")]
        span: pace_common::Span,
    },
}

#[derive(Error, Diagnostic, Debug)]
#[error("Found multiple syntax errors")]
#[diagnostic(code(P1000))]
pub struct MultipleSyntaxErrors {
    #[related]
    pub errors: Vec<SyntaxError>,
}

#[derive(Clone, Error, Diagnostic, Debug)]
pub enum TypeError {
    #[error("Unknown identifier '{name}'")]
    #[diagnostic(code(P2001), help("{help_text}"))]
    UnknownIdentifier {
        name: String,
        help_text: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("Not found")]
        span: pace_common::Span,
    },

    #[error("Duplicate declaration of '{name}'")]
    #[diagnostic(
        code(P2002),
        help("You tried to declare two variables or functions with the exact same name.")
    )]
    DuplicateDeclaration {
        name: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("Cannot redeclare '{name}'")]
        span: pace_common::Span,
        #[label("Originally defined here")]
        original_span: pace_common::Span,
    },

    #[error("Type mismatch: {message}")]
    #[diagnostic(code(P3001), help("Ensure you are passing the correct type."))]
    TypeMismatch {
        message: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("Type mismatch")]
        span: pace_common::Span,
    },

    #[error("Unknown type '{name}'")]
    #[diagnostic(
        code(P3002),
        help("The compiler hasn't seen this type defined anywhere.")
    )]
    UnknownType {
        name: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("Type not found")]
        span: pace_common::Span,
    },

    #[error("Invalid weak reference")]
    #[diagnostic(
        code(P4001),
        help("You tried to use the 'weak' keyword on a value type or a non-optional variable.")
    )]
    InvalidWeakReference {
        #[source_code]
        src: NamedSource<String>,
        #[label("Invalid weak")]
        span: pace_common::Span,
    },

    #[error("Ownership violation: {message}")]
    #[diagnostic(
        code(P4002),
        help(
            "You tried to illegally transfer or consume an object in a way that violates ARC memory rules."
        )
    )]
    OwnershipViolation {
        message: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("Violation here")]
        span: pace_common::Span,
    },

    #[error("Type error: {message}")]
    #[diagnostic(code(P3000))]
    Generic {
        message: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("Here")]
        span: pace_common::Span,
    },
}

#[derive(Clone, Error, Diagnostic, Debug)]
pub enum SemanticWarning {
    #[error("Variable or function '{name}' should use camelCase")]
    #[diagnostic(code(W1001::naming_convention), severity(warning))]
    NamingConvention {
        name: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("Consider renaming to camelCase")]
        span: pace_common::Span,
    },

    #[error("Unused {kind} '{name}'")]
    #[diagnostic(code(W1002::unused_item), severity(warning))]
    UnusedItem {
        kind: String, // "variable" or "function"
        name: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("This is never used")]
        span: pace_common::Span,
    },
}
