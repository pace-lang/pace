use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::diagnostics::offset_to_position;
use crate::Backend;

impl Backend {
    pub(crate) async fn document_symbol_impl(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        if let Some((ast, source_map, _diags, text)) = self.analyze_uri(uri.as_str()).await {
            let file_path = match uri.to_file_path() {
                Ok(p) => p.to_string_lossy().to_string(),
                Err(_) => return Ok(None),
            };
            let mut symbols = Vec::new();
            if let Some(fid) = source_map.get_file_id(&file_path) {
                for module in ast.modules.values() {
                    for decl in &module.declarations {
                        if let Some(sym) = decl_to_document_symbol(decl, &text, fid) {
                            symbols.push(sym);
                        }
                    }
                }
            }
            if symbols.is_empty() {
                Ok(None)
            } else {
                Ok(Some(DocumentSymbolResponse::Nested(symbols)))
            }
        } else {
            Ok(None)
        }
    }


    pub(crate) async fn inlay_hint_impl(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri.to_string();

        if let Some((ast, source_map, diags, _text)) = self.analyze_uri(&uri).await
            && diags.is_empty()
        {
            let mut lowerer = pace_hir::LoweringContext::new();
            if let Ok(hir) = lowerer.lower_program(ast) {
                let mut tc = pace_ty::TypeChecker::new();
                let _ = tc.check_program(&hir);

                let mut hints = Vec::new();
                for (span, hint_str) in tc.inlay_hints {
                    // Inlay hints apply to the current file
                    if let Some(file_path) = source_map.get_path(span.file_id)
                        && let Ok(file_uri) = Url::from_file_path(file_path)
                        && file_uri.to_string() == uri
                    {
                        let span_text = source_map.get_source(span.file_id).unwrap_or_default();
                        // The span covers the whole `let x = ...` stmt.
                        // Let's just place the hint at the end of the `let x` part.
                        // Actually, placing it at the `span.start + 4 + name.len()` is tricky without the name.
                        // Let's just place it at `span.start` for simplicity and let the user see it there,
                        // or we could search for "=" and place it before it.
                        let stmt_text = &span_text[span.start as usize..span.end as usize];
                        let offset = if let Some(eq_idx) = stmt_text.find('=') {
                            span.start as usize + eq_idx
                        } else {
                            span.start as usize
                        };

                        let pos = offset_to_position(span_text, offset);
                        hints.push(InlayHint {
                            position: pos,
                            label: InlayHintLabel::String(hint_str.clone()),
                            kind: Some(InlayHintKind::TYPE),
                            text_edits: None,
                            tooltip: None,
                            padding_left: Some(true),
                            padding_right: Some(true),
                            data: None,
                        });
                    }
                }
                return Ok(Some(hints));
            }
        }
        Ok(None)
    }

}

fn decl_to_document_symbol(
    decl: &pace_ast::Decl,
    text: &str,
    file_id: pace_span::FileId,
) -> Option<DocumentSymbol> {
    use pace_ast::Decl;

    // Helper closure to check file id and prevent panics
    let is_valid = |span: &pace_span::Span| span.file_id == file_id;

    match decl {
        Decl::Function { name, span, .. } if is_valid(span) =>
        {
            #[allow(deprecated)]
            Some(DocumentSymbol {
                name: name.name.to_string(),
                detail: None,
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                range: Range::new(
                    offset_to_position(text, span.start as usize),
                    offset_to_position(text, span.end as usize),
                ),
                selection_range: Range::new(
                    offset_to_position(text, name.span.start as usize),
                    offset_to_position(text, name.span.end as usize),
                ),
                children: None,
            })
        }
        Decl::Struct {
            name,
            span,
            fields,
            methods,
            ..
        } if is_valid(span) => {
            let mut children = Vec::new();
            for pace_ast::FieldDef { name: fname, .. } in fields {
                if !is_valid(&fname.span) {
                    continue;
                }
                #[allow(deprecated)]
                children.push(DocumentSymbol {
                    name: fname.name.to_string(),
                    detail: None,
                    kind: SymbolKind::FIELD,
                    tags: None,
                    deprecated: None,
                    range: Range::new(
                        offset_to_position(text, fname.span.start as usize),
                        offset_to_position(text, fname.span.end as usize),
                    ),
                    selection_range: Range::new(
                        offset_to_position(text, fname.span.start as usize),
                        offset_to_position(text, fname.span.end as usize),
                    ),
                    children: None,
                });
            }
            for m in methods {
                if let Some(mut m_sym) = decl_to_document_symbol(m, text, file_id) {
                    m_sym.kind = SymbolKind::METHOD;
                    children.push(m_sym);
                }
            }
            #[allow(deprecated)]
            Some(DocumentSymbol {
                name: name.name.to_string(),
                detail: None,
                kind: SymbolKind::STRUCT,
                tags: None,
                deprecated: None,
                range: Range::new(
                    offset_to_position(text, span.start as usize),
                    offset_to_position(text, span.end as usize),
                ),
                selection_range: Range::new(
                    offset_to_position(text, name.span.start as usize),
                    offset_to_position(text, name.span.end as usize),
                ),
                children: Some(children),
            })
        }
        Decl::Class {
            name,
            span,
            fields,
            methods,
            ..
        } if is_valid(span) => {
            let mut children = Vec::new();
            for pace_ast::FieldDef { name: fname, .. } in fields {
                if !is_valid(&fname.span) {
                    continue;
                }
                #[allow(deprecated)]
                children.push(DocumentSymbol {
                    name: fname.name.to_string(),
                    detail: None,
                    kind: SymbolKind::FIELD,
                    tags: None,
                    deprecated: None,
                    range: Range::new(
                        offset_to_position(text, fname.span.start as usize),
                        offset_to_position(text, fname.span.end as usize),
                    ),
                    selection_range: Range::new(
                        offset_to_position(text, fname.span.start as usize),
                        offset_to_position(text, fname.span.end as usize),
                    ),
                    children: None,
                });
            }
            for m in methods {
                if let Some(mut m_sym) = decl_to_document_symbol(m, text, file_id) {
                    m_sym.kind = SymbolKind::METHOD;
                    children.push(m_sym);
                }
            }
            #[allow(deprecated)]
            Some(DocumentSymbol {
                name: name.name.to_string(),
                detail: None,
                kind: SymbolKind::CLASS,
                tags: None,
                deprecated: None,
                range: Range::new(
                    offset_to_position(text, span.start as usize),
                    offset_to_position(text, span.end as usize),
                ),
                selection_range: Range::new(
                    offset_to_position(text, name.span.start as usize),
                    offset_to_position(text, name.span.end as usize),
                ),
                children: Some(children),
            })
        }
        Decl::Enum {
            name,
            span,
            variants,
            ..
        } if is_valid(span) => {
            let mut children = Vec::new();
            for v in variants {
                if !is_valid(&v.name.span) {
                    continue;
                }
                #[allow(deprecated)]
                children.push(DocumentSymbol {
                    name: v.name.name.to_string(),
                    detail: None,
                    kind: SymbolKind::ENUM_MEMBER,
                    tags: None,
                    deprecated: None,
                    range: Range::new(
                        offset_to_position(text, v.name.span.start as usize),
                        offset_to_position(text, v.name.span.end as usize),
                    ),
                    selection_range: Range::new(
                        offset_to_position(text, v.name.span.start as usize),
                        offset_to_position(text, v.name.span.end as usize),
                    ),
                    children: None,
                });
            }
            #[allow(deprecated)]
            Some(DocumentSymbol {
                name: name.name.to_string(),
                detail: None,
                kind: SymbolKind::ENUM,
                tags: None,
                deprecated: None,
                range: Range::new(
                    offset_to_position(text, span.start as usize),
                    offset_to_position(text, span.end as usize),
                ),
                selection_range: Range::new(
                    offset_to_position(text, name.span.start as usize),
                    offset_to_position(text, name.span.end as usize),
                ),
                children: Some(children),
            })
        }
        Decl::Trait {
            name,
            span,
            methods,
            ..
        } if is_valid(span) => {
            let mut children = Vec::new();
            for m in methods {
                if let Some(mut m_sym) = decl_to_document_symbol(m, text, file_id) {
                    m_sym.kind = SymbolKind::METHOD;
                    children.push(m_sym);
                }
            }
            #[allow(deprecated)]
            Some(DocumentSymbol {
                name: name.name.to_string(),
                detail: None,
                kind: SymbolKind::INTERFACE,
                tags: None,
                deprecated: None,
                range: Range::new(
                    offset_to_position(text, span.start as usize),
                    offset_to_position(text, span.end as usize),
                ),
                selection_range: Range::new(
                    offset_to_position(text, name.span.start as usize),
                    offset_to_position(text, name.span.end as usize),
                ),
                children: Some(children),
            })
        }
        Decl::Let { name, span, .. } | Decl::Var { name, span, .. } if is_valid(span) =>
        {
            #[allow(deprecated)]
            Some(DocumentSymbol {
                name: name.name.to_string(),
                detail: None,
                kind: SymbolKind::VARIABLE,
                tags: None,
                deprecated: None,
                range: Range::new(
                    offset_to_position(text, span.start as usize),
                    offset_to_position(text, span.end as usize),
                ),
                selection_range: Range::new(
                    offset_to_position(text, name.span.start as usize),
                    offset_to_position(text, name.span.end as usize),
                ),
                children: None,
            })
        }
        Decl::Const { name, span, .. } if is_valid(span) =>
        {
            #[allow(deprecated)]
            Some(DocumentSymbol {
                name: name.name.to_string(),
                detail: None,
                kind: SymbolKind::CONSTANT,
                tags: None,
                deprecated: None,
                range: Range::new(
                    offset_to_position(text, span.start as usize),
                    offset_to_position(text, span.end as usize),
                ),
                selection_range: Range::new(
                    offset_to_position(text, name.span.start as usize),
                    offset_to_position(text, name.span.end as usize),
                ),
                children: None,
            })
        }
        _ => None,
    }
}
