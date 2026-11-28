use std::collections::HashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::diagnostics::offset_to_position;
use crate::find_node;
use crate::Backend;

impl Backend {
    pub(crate) async fn rename_impl(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;
        let new_name = params.new_name;

        if let Some((ast, source_map, diags, text)) = self.analyze_uri(&uri).await
            && diags.is_empty()
        {
            let mut lowerer = pace_hir::LoweringContext::new();
            if let Ok(mut hir) = lowerer.lower_program(ast) {
                let mut tc = pace_ty::TypeChecker::new();
                hir.resolve_traits(&mut tc.reporter);
                let _ = tc.check_program(&hir);

                let file_path = std::path::PathBuf::from(uri.replace("file://", ""));
                if let Some(file_id) = source_map.get_file_id(file_path.to_string_lossy().as_ref())
                {
                    let offset = find_node::position_to_offset(&text, position);
                    if let Some(id) = find_node::find_ident_at_offset(&hir, file_id, offset) {
                        let mut changes = HashMap::new();
                        let mut edits = Vec::new();

                        // Add the declaration itself
                        if let Some((_, _, span)) =
                            tc.declared_bindings.iter().find(|(d_id, _, _)| *d_id == id)
                        {
                            let span_text = source_map.get_source(span.file_id).unwrap_or_default();
                            let pos = offset_to_position(span_text, span.start as usize);

                            edits.push(TextEdit {
                                range: Range {
                                    start: pos,
                                    end: Position {
                                        line: pos.line,
                                        character: pos.character + new_name.len() as u32,
                                    }, // approx
                                },
                                new_text: new_name.clone(),
                            });
                        }

                        // Add references
                        if let Some(refs) = tc.symbol_references.get(&id) {
                            for span in refs {
                                let span_text =
                                    source_map.get_source(span.file_id).unwrap_or_default();
                                let pos = offset_to_position(span_text, span.start as usize);

                                let file_path =
                                    source_map.get_path(span.file_id).unwrap_or_default();
                                if let Ok(file_uri) = Url::from_file_path(file_path) {
                                    let edit = TextEdit {
                                        range: Range {
                                            start: pos,
                                            end: Position {
                                                line: pos.line,
                                                character: pos.character + new_name.len() as u32,
                                            }, // approx
                                        },
                                        new_text: new_name.clone(),
                                    };
                                    changes.entry(file_uri).or_insert_with(Vec::new).push(edit);
                                }
                            }
                        }

                        // Also push the edits for current file if any
                        if let Ok(url) = Url::parse(&uri) {
                            changes.entry(url).or_insert_with(Vec::new).extend(edits);
                        }

                        return Ok(Some(WorkspaceEdit {
                            changes: Some(changes),
                            document_changes: None,
                            change_annotations: None,
                        }));
                    }
                }
            }
        }
        Ok(None)
    }


    pub(crate) async fn code_action_impl(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let mut actions = Vec::new();

        for diag in params.context.diagnostics {
            if diag
                .message
                .contains("must be assigned before it can be used")
            {
                // Not much we can do automatically here
            }
            if diag.message.contains("Type mismatch") {
                // Type mismatch
            }

            // Unused variables often have "unused variable: `x`" (if rustc-like)
            // but in pace we don't have unused var warnings yet from typechecker.

            // As a simple placeholder Code Action for Phase 4:
            // Let's suggest adding an import if we see "Cannot find value 'xyz'"
            if diag.message.starts_with("Cannot find value '")
                && let Some(start) = diag.message.find('\'')
                && let Some(end) = diag.message[start + 1..].find('\'')
            {
                let var_name = &diag.message[start + 1..start + 1 + end];

                let mut changes = HashMap::new();
                let uri = params.text_document.uri.clone();
                let edit = TextEdit {
                    range: Range {
                        start: Position {
                            line: 0,
                            character: 0,
                        },
                        end: Position {
                            line: 0,
                            character: 0,
                        },
                    },
                    new_text: format!("import {}\n", var_name),
                };
                changes.insert(uri, vec![edit]);

                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title: format!("Import '{}'", var_name),
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![diag.clone()]),
                    edit: Some(WorkspaceEdit {
                        changes: Some(changes),
                        document_changes: None,
                        change_annotations: None,
                    }),
                    ..Default::default()
                }));
            }

            if diag.message.starts_with("unused variable or function: `")
                && let Some(start) = diag.message.find('`')
                && let Some(end) = diag.message[start + 1..].find('`')
            {
                let var_name = &diag.message[start + 1..start + 1 + end];

                let mut changes = HashMap::new();
                let uri = params.text_document.uri.clone();
                let edit = TextEdit {
                    range: diag.range,
                    new_text: format!("_{}", var_name),
                };
                changes.insert(uri, vec![edit]);

                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title: format!("Rename to '_{}'", var_name),
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![diag.clone()]),
                    edit: Some(WorkspaceEdit {
                        changes: Some(changes),
                        document_changes: None,
                        change_annotations: None,
                    }),
                    ..Default::default()
                }));
            }
        }

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }
}
