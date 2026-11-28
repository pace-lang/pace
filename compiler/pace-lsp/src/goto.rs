use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::diagnostics::offset_to_position;
use crate::find_node;
use crate::Backend;

impl Backend {
    pub(crate) async fn goto_definition_impl(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

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
                    
                    let mut target_span: Option<pace_span::Span> = None;
                    let mut best_len = usize::MAX;
                    for (usage_span, def_span) in &tc.definition_info {
                        if usage_span.file_id == file_id && offset >= usage_span.start as usize && offset <= usage_span.end as usize {
                            let len = (usage_span.end - usage_span.start) as usize;
                            if len < best_len {
                                best_len = len;
                                target_span = Some(*def_span);
                            }
                        }
                    }

                    if target_span.is_none() {
                        // Fallback to older lookup methods if not found in definition_info
                        if let Some(id) = find_node::find_ident_at_offset(&hir, file_id, offset) {
                            // 1. Check local bindings
                            for (decl_id, _, span) in &tc.declared_bindings {
                                if *decl_id == id {
                                    target_span = Some(*span);
                                    break;
                                }
                            }
    
                            // 2. Check global definitions
                            if target_span.is_none()
                                && let Some(mangled_name) = tc.resolved_global_names.get(&id)
                            {
                                // Find the declaration in HIR
                                for module in hir.modules.values() {
                                    for decl in &module.declarations {
                                        let (decl_name, decl_span) = match decl {
                                            pace_hir::Decl::Function { name, span, .. } => (name, span),
                                            pace_hir::Decl::Class { name, span, .. } => (name, span),
                                            pace_hir::Decl::Struct { name, span, .. } => (name, span),
                                            pace_hir::Decl::Enum { name, span, .. } => (name, span),
                                            pace_hir::Decl::Trait { name, span, .. } => (name, span),
                                            _ => continue,
                                        };
                                        if decl_name == mangled_name {
                                            target_span = Some(*decl_span);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(span) = target_span {
                        let span_text = source_map.get_source(span.file_id).unwrap_or_default();
                        let start = offset_to_position(span_text, span.start as usize);
                        let end = offset_to_position(span_text, span.end as usize);

                        let file_path = source_map.get_path(span.file_id).unwrap_or_default();
                        if let Ok(target_uri) = Url::from_file_path(file_path) {
                            return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                                uri: target_uri,
                                range: Range { start, end },
                            })));
                        }
                    }
                }
            }
        }

        Ok(None)
    }


    pub(crate) async fn document_highlight_impl(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

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
                        let mut highlights = Vec::new();

                        if let Some((_, _, span)) =
                            tc.declared_bindings.iter().find(|(d_id, _, _)| *d_id == id)
                        {
                            let span_text = source_map.get_source(span.file_id).unwrap_or_default();
                            let pos = offset_to_position(span_text, span.start as usize);
                            highlights.push(DocumentHighlight {
                                range: Range {
                                    start: pos,
                                    end: Position {
                                        line: pos.line,
                                        character: pos.character + 5,
                                    }, // approx
                                },
                                kind: Some(DocumentHighlightKind::WRITE),
                            });
                        }

                        if let Some(refs) = tc.symbol_references.get(&id) {
                            for span in refs {
                                let span_text =
                                    source_map.get_source(span.file_id).unwrap_or_default();
                                let pos = offset_to_position(span_text, span.start as usize);
                                highlights.push(DocumentHighlight {
                                    range: Range {
                                        start: pos,
                                        end: Position {
                                            line: pos.line,
                                            character: pos.character + 5,
                                        }, // approx
                                    },
                                    kind: Some(DocumentHighlightKind::READ),
                                });
                            }
                        }

                        return Ok(Some(highlights));
                    }
                }
            }
        }
        Ok(None)
    }

}
