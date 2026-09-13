pub mod formatter;

pub use formatter::Formatter;

/// Formats a complete AST module and returns the formatted source code as a string.
pub fn format_module(module: &pace_ast::Module) -> String {
    let mut formatter = Formatter::new();
    formatter.format_module(module)
}
