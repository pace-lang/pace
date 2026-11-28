use tower_lsp::lsp_types::*;

use crate::Backend;

impl Backend {
    pub(crate) async fn publish_diagnostics(&self, uri: String, text: String) {
        let mut lsp_diags = Vec::new();

        if let Some((ast, source_map, diags, _)) = self.analyze_uri(&uri).await {
            // Find file_id for current uri so we only report its diagnostics
            let mut current_file_id = None;
            if let Ok(url) = Url::parse(&uri)
                && let Ok(file_path) = url.to_file_path()
            {
                current_file_id = source_map.get_file_id(&file_path.display().to_string());
            }

            for diag in diags {
                // If it belongs to another file, skip it
                if let Some(span) = diag.span {
                    if let Some(cf_id) = current_file_id
                        && span.file_id != cf_id
                    {
                        continue;
                    }

                    let start = offset_to_position(&text, span.start as usize);
                    let end = offset_to_position(&text, span.end as usize);
                    let range = Range { start, end };
                    lsp_diags.push(Diagnostic {
                        range,
                        severity: Some(DiagnosticSeverity::ERROR),
                        message: diag.message,
                        ..Default::default()
                    });
                }
            }

            if lsp_diags.is_empty() {
                let mut lowerer = pace_hir::LoweringContext::new();
                if let Ok(mut hir) = lowerer.lower_program(ast) {
                    let mut tc = pace_ty::TypeChecker::new();
                    hir.resolve_traits(&mut tc.reporter);
                    let res = tc.check_program(&hir);
                    for diag in &tc.reporter.diagnostics {
                        if let Some(span) = diag.span {
                            if let Some(cf_id) = current_file_id
                                && span.file_id != cf_id
                            {
                                continue;
                            }
                            let start = offset_to_position(&text, span.start as usize);
                            let end = offset_to_position(&text, span.end as usize);
                            let severity = match diag.severity {
                                pace_errors::Severity::Warning => DiagnosticSeverity::WARNING,
                                _ => DiagnosticSeverity::ERROR,
                            };
                            lsp_diags.push(Diagnostic {
                                range: Range { start, end },
                                severity: Some(severity),
                                message: diag.message.clone(),
                                ..Default::default()
                            });
                        }
                    }
                    if let Err(msg) = res && !lsp_diags.iter().any(|d| d.severity == Some(DiagnosticSeverity::ERROR)) {
                        lsp_diags.push(Diagnostic {
                            range: Range {
                                start: Position { line: 0, character: 0 },
                                end: Position { line: 0, character: 1 },
                            },
                            severity: Some(DiagnosticSeverity::ERROR),
                            message: msg,
                            ..Default::default()
                        });
                    }
                }
            }
        }

        if let Ok(url) = Url::parse(&uri) {
            self.client.publish_diagnostics(url, lsp_diags, None).await;
        }
    }

    pub(crate) async fn analyze_uri(
        &self,
        uri: &str,
    ) -> Option<(
        pace_ast::Program,
        pace_span::SourceMap,
        Vec<pace_errors::Diagnostic>,
        String,
    )> {
        let text = {
            let map = self.document_map.lock().await;
            map.get(uri).cloned().unwrap_or_default()
        };

        let file_path = Url::parse(uri).ok()?.to_file_path().ok()?;

        let mut overrides = std::collections::HashMap::new();
        overrides.insert(file_path.clone(), text.clone());

        let mut dependencies = std::collections::HashMap::new();
        if let Some(manifest_path) = pace_pkg::find_manifest(&file_path) {
            if let Some(root) = manifest_path.parent() {
                let lockfile_path = root.join("pace.lock");
                if let Ok(lock) = pace_pkg::resolve::DependencyResolver::read_lockfile(&lockfile_path) {
                    let cache = pace_pkg::cache::CacheManager::new();
                    for (pkg_name, pkg_info) in &lock.packages {
                        let mut pkg_path = cache.get_package_path(pkg_name, &pkg_info.version);
                        if let Some(source) = &pkg_info.source
                            && let Some(local_path) = source.strip_prefix("local+")
                        {
                            pkg_path = root.join(local_path);
                        }
                        dependencies.insert(pkg_name.clone(), pkg_path);
                    }
                }
            }
        }

        match pace_driver::analyze_workspace(&file_path, &overrides, &dependencies) {
            Ok((ast, source_map, diags)) => Some((ast, source_map, diags, text)),
            Err(_) => None,
        }
    }
}

pub(crate) fn offset_to_position(text: &str, offset: usize) -> Position {
    let mut line = 0;
    let mut col = 0;
    for (i, c) in text.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    Position {
        line,
        character: col,
    }
}
