use crate::{Diagnostic, Severity, SourceMap};
use ariadne::{Color, Label, Report, ReportKind};

pub fn print_diagnostics(diagnostics: &[Diagnostic], source_map: &SourceMap) {
    for diag in diagnostics {
        let kind = match diag.severity {
            Severity::Error => ReportKind::Error,
            Severity::Warning => ReportKind::Warning,
            Severity::Note => ReportKind::Advice,
        };

        let file_id = diag.primary_span.file_id;
        let file_info = source_map.get_file(file_id);
        let is_global = diag.primary_span.file_id == u32::MAX;

        let path_str = if is_global {
            "project".to_string()
        } else {
            match file_info {
                Some((path, _)) => path.to_string_lossy().into_owned(),
                None => "<unknown>".to_string(),
            }
        };

        let mut report = Report::build(
            kind,
            (
                path_str.clone(),
                diag.primary_span.start..diag.primary_span.end,
            ),
        )
        .with_code(diag.code.as_str())
        .with_message(&diag.message);

        // Primary label (only if not global)
        if !is_global {
            report = report.with_label(
                Label::new((
                    path_str.clone(),
                    diag.primary_span.start..diag.primary_span.end,
                ))
                .with_message(&diag.message)
                .with_color(match diag.severity {
                    Severity::Error => Color::Red,
                    Severity::Warning => Color::Yellow,
                    Severity::Note => Color::Cyan,
                }),
            );
        }

        // Additional labels
        for label in &diag.labels {
            let l_file_id = label.span.file_id;
            let l_path_str = match source_map.get_file(l_file_id) {
                Some((p, _)) => p.to_string_lossy().into_owned(),
                None => "<unknown>".to_string(),
            };

            report = report.with_label(
                Label::new((l_path_str, label.span.start..label.span.end))
                    .with_message(&label.message)
                    .with_color(Color::Blue),
            );
        }

        if let Some(help) = &diag.help {
            report = report.with_help(help);
        }

        for note in &diag.notes {
            report = report.with_note(note);
        }

        let mut sources = Vec::new();
        for (path, source) in source_map.get_all_files().values() {
            sources.push((path.to_string_lossy().into_owned(), source.as_str()));
        }

        if file_info.is_none() {
            // Add a dummy entry if missing so ariadne doesn't crash on <unknown>
            sources.push(("<unknown>".to_string(), ""));
        }
        sources.push(("project".to_string(), ""));

        report.finish().print(ariadne::sources(sources)).unwrap();
    }
}
