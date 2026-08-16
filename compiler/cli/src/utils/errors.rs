use diagnostics::{DiagnosticBuilder, DiagnosticCode, SourceMap, print_diagnostics};

pub fn print_global_error(message: &str) {
    let diag =
        DiagnosticBuilder::global_error(DiagnosticCode::Custom("E001".into()), message).build();
    print_diagnostics(&[diag], &SourceMap::new());
}
