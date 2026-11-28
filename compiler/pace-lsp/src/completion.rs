use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::find_node;
use crate::Backend;

impl Backend {
    pub(crate) async fn completion_impl(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;
        let mut items = Vec::new();

        if let Some((ast, source_map, _diags, text)) = self.analyze_uri(&uri).await {
            let offset = find_node::position_to_offset(&text, position);
            let before = &text[..offset.min(text.len())];
            let trimmed_ident = before.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
            if trimmed_ident.ends_with('@') {
                return Ok(Some(CompletionResponse::Array(annotation_completions())));
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
                    let is_dot_completion = offset > 0 && text.chars().nth(offset - 1) == Some('.');

                    if is_dot_completion {
                        let mut prev_offset = offset - 1;
                        while prev_offset > 0
                            && text
                                .chars()
                                .nth(prev_offset - 1)
                                .unwrap_or(' ')
                                .is_whitespace()
                        {
                            prev_offset -= 1;
                        }
                        if let Some(id) =
                            find_node::find_ident_at_offset(&hir, file_id, prev_offset - 1)
                            && let Some(ty) = tc.local_types.get(&id)
                        {
                            let mut add_fields = |fields: &Vec<pace_ty::ResolvedField>| {
                                for pace_ty::ResolvedField {
                                    name: fname,
                                    ty: fty,
                                    is_private,
                                    ..
                                } in fields
                                {
                                    let ty_str = tc.display_ty(fty);
                                    items.push(CompletionItem {
                                        label: fname.clone(),
                                        kind: Some(CompletionItemKind::FIELD),
                                        detail: Some(if *is_private {
                                            format!("private {}", ty_str)
                                        } else {
                                            ty_str
                                        }),
                                        ..Default::default()
                                    });
                                }
                            };
                            match ty {
                                pace_ty::Ty::Struct(sid) => {
                                    if let Some(fields) = tc.struct_defs.get(sid) {
                                        add_fields(fields);
                                    }
                                    if let Some(struct_name) =
                                        tc.named_types.iter().find_map(|(k, &(vid, _))| {
                                            if vid == *sid { Some(k) } else { None }
                                        })
                                    {
                                        for mname in tc.methods_env.keys() {
                                            if mname.starts_with(&format!("{}_", struct_name)) {
                                                let method_short_name = mname.trim_start_matches(
                                                    &format!("{}_", struct_name),
                                                );
                                                let detail_str = if let Some(mty) = tc.methods_env.get(mname) {
                                                    tc.display_ty(mty)
                                                } else {
                                                    "method".to_string()
                                                };
                                                items.push(CompletionItem {
                                                    label: method_short_name.to_string(),
                                                    kind: Some(CompletionItemKind::METHOD),
                                                    detail: Some(detail_str),
                                                    ..Default::default()
                                                });
                                            }
                                        }
                                    }
                                }
                                pace_ty::Ty::Class(cid) | pace_ty::Ty::Actor(cid) => {
                                    let is_actor = matches!(ty, pace_ty::Ty::Actor(_));
                                    if let Some(fields) = tc.class_defs.get(cid) {
                                        if !is_actor {
                                            add_fields(fields);
                                        }
                                    }
                                    if let Some(class_name) =
                                        tc.named_types.iter().find_map(|(k, &(vid, _))| {
                                            if vid == *cid { Some(k) } else { None }
                                        })
                                    {
                                        for mname in tc.methods_env.keys() {
                                            if mname.starts_with(&format!("{}_", class_name)) {
                                                let method_short_name = mname.trim_start_matches(
                                                    &format!("{}_", class_name),
                                                );
                                                let detail_str = if let Some(mty) = tc.methods_env.get(mname) {
                                                    tc.display_ty(mty)
                                                } else {
                                                    "method".to_string()
                                                };
                                                items.push(CompletionItem {
                                                    label: method_short_name.to_string(),
                                                    kind: Some(CompletionItemKind::METHOD),
                                                    detail: Some(detail_str),
                                                    ..Default::default()
                                                });
                                            }
                                        }
                                    }
                                }
                                pace_ty::Ty::List(_) => {
                                    for (label, detail, is_prop) in [
                                        ("len", "() -> int", false),
                                        ("isEmpty", "bool", true),
                                        ("isNotEmpty", "bool", true),
                                        ("first", "T?", true),
                                        ("last", "T?", true),
                                        ("push", "(T) -> Unit", false),
                                        ("add", "(T) -> Unit", false),
                                        ("pop", "() -> T?", false),
                                        ("insert", "(int, T) -> Unit", false),
                                        ("removeAt", "(int) -> T", false),
                                        ("contains", "(T) -> bool", false),
                                        ("indexOf", "(T) -> int", false),
                                        ("slice", "(int, int) -> List<T>", false),
                                        ("reversed", "() -> List<T>", false),
                                        ("join", "(string) -> string", false),
                                        ("clear", "() -> Unit", false),
                                        ("forEach", "((T) -> Unit) -> Unit", false),
                                        ("map", "((T) -> U) -> List<U>", false),
                                        ("filter", "((T) -> bool) -> List<T>", false),
                                        ("find", "((T) -> bool) -> T?", false),
                                        ("any", "((T) -> bool) -> bool", false),
                                        ("every", "((T) -> bool) -> bool", false),
                                        ("reduce", "(U, (U, T) -> U) -> U", false),
                                    ] {
                                        items.push(CompletionItem {
                                            label: label.to_string(),
                                            kind: Some(if is_prop { CompletionItemKind::PROPERTY } else { CompletionItemKind::METHOD }),
                                            detail: Some(detail.to_string()),
                                            ..Default::default()
                                        });
                                    }
                                }
                                pace_ty::Ty::Map(_, _) => {
                                    for (label, detail, is_prop) in [
                                        ("len", "() -> int", false),
                                        ("isEmpty", "bool", true),
                                        ("isNotEmpty", "bool", true),
                                        ("get", "(K) -> V?", false),
                                        ("set", "(K, V) -> Unit", false),
                                        ("has", "(K) -> bool", false),
                                        ("containsKey", "(K) -> bool", false),
                                        ("getOrDefault", "(K, V) -> V", false),
                                        ("remove", "(K) -> bool", false),
                                        ("keys", "() -> List<K>", false),
                                        ("values", "() -> List<V>", false),
                                        ("forEach", "((K, V) -> Unit) -> Unit", false),
                                        ("clear", "() -> Unit", false),
                                    ] {
                                        items.push(CompletionItem {
                                            label: label.to_string(),
                                            kind: Some(if is_prop { CompletionItemKind::PROPERTY } else { CompletionItemKind::METHOD }),
                                            detail: Some(detail.to_string()),
                                            ..Default::default()
                                        });
                                    }
                                }
                                pace_ty::Ty::Set(_) => {
                                    for (label, detail, is_prop) in [
                                        ("len", "() -> int", false),
                                        ("isEmpty", "bool", true),
                                        ("isNotEmpty", "bool", true),
                                        ("add", "(T) -> bool", false),
                                        ("has", "(T) -> bool", false),
                                        ("contains", "(T) -> bool", false),
                                        ("remove", "(T) -> bool", false),
                                        ("clear", "() -> Unit", false),
                                        ("toList", "() -> List<T>", false),
                                        ("union", "(Set<T>) -> Set<T>", false),
                                        ("intersection", "(Set<T>) -> Set<T>", false),
                                        ("difference", "(Set<T>) -> Set<T>", false),
                                    ] {
                                        items.push(CompletionItem {
                                            label: label.to_string(),
                                            kind: Some(if is_prop { CompletionItemKind::PROPERTY } else { CompletionItemKind::METHOD }),
                                            detail: Some(detail.to_string()),
                                            ..Default::default()
                                        });
                                    }
                                }
                                pace_ty::Ty::String => {
                                    for (label, detail, is_prop) in [
                                        ("len", "() -> int", false),
                                        ("isEmpty", "bool", true),
                                        ("isNotEmpty", "bool", true),
                                        ("contains", "(string) -> bool", false),
                                        ("startsWith", "(string) -> bool", false),
                                        ("endsWith", "(string) -> bool", false),
                                        ("indexOf", "(string) -> int", false),
                                        ("trim", "() -> string", false),
                                        ("slice", "(int, int) -> string", false),
                                        ("replace", "(string, string) -> string", false),
                                        ("toLower", "() -> string", false),
                                        ("toUpper", "() -> string", false),
                                        ("split", "(string) -> List<string>", false),
                                    ] {
                                        items.push(CompletionItem {
                                            label: label.to_string(),
                                            kind: Some(if is_prop { CompletionItemKind::PROPERTY } else { CompletionItemKind::METHOD }),
                                            detail: Some(detail.to_string()),
                                            ..Default::default()
                                        });
                                    }
                                }
                                pace_ty::Ty::StringBuilder => {
                                    for (label, detail, is_prop) in [
                                        ("len", "() -> int", false),
                                        ("isEmpty", "bool", true),
                                        ("isNotEmpty", "bool", true),
                                        ("append", "(string) -> Unit", false),
                                        ("toString", "() -> string", false),
                                        ("clear", "() -> Unit", false),
                                    ] {
                                        items.push(CompletionItem {
                                            label: label.to_string(),
                                            kind: Some(if is_prop { CompletionItemKind::PROPERTY } else { CompletionItemKind::METHOD }),
                                            detail: Some(detail.to_string()),
                                            ..Default::default()
                                        });
                                    }
                                }
                                _ => {}
                            }
                        }
                    } else {
                        for (id, name, _) in &tc.declared_bindings {
                            let detail = if let Some(ty) = tc.env.get(id) {
                                tc.display_ty(ty)
                            } else {
                                "Local Variable".to_string()
                            };
                            items.push(CompletionItem {
                                label: name.clone(),
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some(detail),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

}

pub(crate) fn annotation_completions() -> Vec<CompletionItem> {
    [
        ("override", "Override a method from a base class"),
        ("deprecated", "Mark a symbol as deprecated"),
        ("inline", "Hint to inline this function"),
    ]
    .into_iter()
    .map(|(label, doc)| CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::KEYWORD),
        detail: Some(format!("@{}", label)),
        documentation: Some(Documentation::String(doc.to_string())),
        ..Default::default()
    })
    .collect()
}
