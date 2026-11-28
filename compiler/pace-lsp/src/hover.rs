use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::find_node;
use crate::Backend;

impl Backend {
    pub(crate) async fn hover_impl(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

        if let Some((ast, source_map, diags, text)) = self.analyze_uri(&uri).await
            && diags.is_empty()
        {
            let offset = find_node::position_to_offset(&text, position);
            if let Some(ann_hover) = annotation_hover_at(&text, offset) {
                return Ok(Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(ann_hover.to_string())),
                    range: None,
                }));
            }

            let mut lowerer = pace_hir::LoweringContext::new();
            if let Ok(mut hir) = lowerer.lower_program(ast) {
                let mut tc = pace_ty::TypeChecker::new();
                hir.resolve_traits(&mut tc.reporter);
                let _ = tc.check_program(&hir);

                let file_path = std::path::PathBuf::from(uri.replace("file://", ""));
                if let Some(file_id) = source_map.get_file_id(file_path.to_string_lossy().as_ref())
                {
                    let offset = find_node::position_to_offset(&text, position);
                    
                    let mut best_hover: Option<&String> = None;
                    let mut best_len = usize::MAX;
                    for (span, hover_text) in &tc.hover_info {
                        if span.file_id == file_id && offset >= span.start as usize && offset <= span.end as usize {
                            let len = (span.end - span.start) as usize;
                            if len < best_len {
                                best_len = len;
                                best_hover = Some(hover_text);
                            }
                        }
                    }
                    
                    if let Some(hover_text) = best_hover {
                        return Ok(Some(Hover {
                            contents: HoverContents::Scalar(MarkedString::String(
                                hover_text.clone(),
                            )),
                            range: None,
                        }));
                    }
                    
                    // Fallback to old behavior for anything not yet in hover_info
                    if let Some(id) = find_node::find_ident_at_offset(&hir, file_id, offset)
                        && let Some(ty) = tc.local_types.get(&id).or_else(|| tc.env.get(&id))
                    {
                        return Ok(Some(Hover {
                            contents: HoverContents::Scalar(MarkedString::String(
                                tc.display_ty(ty),
                            )),
                            range: None,
                        }));
                    }
                }
            }
        }

        Ok(None)
    }


    pub(crate) async fn signature_help_impl(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
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

                let offset = find_node::position_to_offset(&text, position);

                // Find the innermost function call whose span contains the offset
                let mut best_call = None;
                let mut best_len = usize::MAX;
                for (span, ty) in &tc.function_calls {
                    let span_len = span.end.saturating_sub(span.start) as usize;
                    if offset >= span.start as usize
                        && offset <= span.end as usize
                        && span_len < best_len
                    {
                        best_len = span_len;
                        best_call = Some((*span, ty.clone()));
                    }
                }

                if let Some((span, pace_ty::Ty::Function(args, ret))) = best_call {
                    let mut param_infos = Vec::new();
                    for arg_ty in args {
                        param_infos.push(ParameterInformation {
                            label: ParameterLabel::Simple(tc.display_ty(&arg_ty)),
                            documentation: None,
                        });
                    }

                    let span_text = source_map.get_source(span.file_id).unwrap_or_default();
                    let call_text = &span_text[span.start as usize..offset];
                    let active_param = call_text.chars().filter(|&c| c == ',').count() as u32;

                    let sig_info = SignatureInformation {
                        label: format!("fn(…) -> {}", tc.display_ty(&ret)),
                        documentation: None,
                        parameters: Some(param_infos),
                        active_parameter: Some(active_param),
                    };

                    return Ok(Some(SignatureHelp {
                        signatures: vec![sig_info],
                        active_signature: Some(0),
                        active_parameter: Some(active_param),
                    }));
                }
            }
        }
        Ok(None)
    }

}

pub(crate) fn annotation_hover_at(text: &str, offset: usize) -> Option<&'static str> {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let mut start = offset.min(bytes.len());
    while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
        start -= 1;
    }
    if start == 0 || bytes[start - 1] != b'@' {
        return None;
    }
    let mut end = offset.min(bytes.len());
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    match &text[start..end] {
        "override" => Some("`@override` — Marks a method as overriding a base class method."),
        "deprecated" => Some("`@deprecated(\"reason\")` — Marks a function, type, or method as deprecated."),
        "inline" => Some("`@inline` — Hints to the compiler to inline this function."),
        _ => None,
    }
}
