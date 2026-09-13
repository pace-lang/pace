use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use pace_lexer::Lexer;
use pace_parser::parser::Parser;
use pace_span::SourceMap;


mod find_node;

#[derive(Debug)]
struct Backend {
    client: Client,
    document_map: Arc<Mutex<HashMap<String, String>>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    ..Default::default()
                }),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Pace language server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        if let Some((ast, source_map, _diags, text)) = self.analyze_uri(&uri.as_str()).await {
            let file_path = uri.to_file_path().unwrap().to_string_lossy().to_string();
            let mut symbols = Vec::new();
            if let Some(fid) = source_map.get_file_id(&file_path) {
                for module in ast.modules.values() {
                    for decl in &module.declarations {
                        if let Some(sym) = decl_to_document_symbol(&decl, &text, fid) {
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


    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        let text = params.text_document.text.clone();
        
        self.document_map.lock().await.insert(uri.clone(), text.clone());
        self.publish_diagnostics(uri, text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        if let Some(change) = params.content_changes.into_iter().next() {
            self.document_map.lock().await.insert(uri.clone(), change.text.clone());
            self.publish_diagnostics(uri, change.text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        self.document_map.lock().await.remove(&uri);
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri.to_string();
        let text = {
            let map = self.document_map.lock().await;
            map.get(&uri).cloned()
        };

        if let Some(source) = text {
            let mut source_map = SourceMap::new();
            let file_id = source_map.add_file(uri.clone(), source.clone());
            let lexer = Lexer::new(&source, file_id);
            let mut parser = Parser::new(lexer);

            let (ast, diags, comments) = parser.parse_program();

            if diags.is_empty() {
                let module = pace_ast::Module {
                    name: "module".to_string(),
                    file_id,
                    declarations: ast,
                    comments,
                };
                let formatted = pace_fmt::format_module(&module);

                // For simplicity, we replace the entire document
                let line_count = source.lines().count() as u32;
                let range = Range {
                    start: Position { line: 0, character: 0 },
                    end: Position { line: line_count, character: 0 },
                };

                return Ok(Some(vec![TextEdit {
                    range,
                    new_text: formatted,
                }]));
            }
        }
        
        Ok(None)
    }
    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri.to_string();
        let position = params.text_document_position_params.position;

        if let Some((ast, source_map, diags, text)) = self.analyze_uri(&uri).await {
            if diags.is_empty() {
                let mut lowerer = pace_hir::LoweringContext::new();
                if let Ok(hir) = lowerer.lower_program(ast) {
                    let mut tc = pace_ty::TypeChecker::new();
                    let _ = tc.check_program(&hir);

                    let file_path = std::path::PathBuf::from(uri.replace("file://", ""));
                    if let Some(file_id) = source_map.get_file_id(file_path.to_string_lossy().as_ref()) {
                        let offset = find_node::position_to_offset(&text, position);
                        if let Some(id) = find_node::find_ident_at_offset(&hir, file_id, offset) {
                            let mut target_span = None;
                            
                            // 1. Check local bindings
                            for (decl_id, _, span) in &tc.declared_bindings {
                                if *decl_id == id {
                                    target_span = Some(*span);
                                    break;
                                }
                            }

                            // 2. Check global definitions
                            if target_span.is_none() {
                                if let Some(mangled_name) = tc.resolved_global_names.get(&id) {
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
                            if let Some(span) = target_span {
                                let span_text = source_map.get_source(span.file_id).unwrap_or_default();
                                let start = offset_to_position(span_text, span.start as usize);
                                let end = offset_to_position(span_text, span.end as usize);
                                
                                let file_path = source_map.get_path(span.file_id).unwrap_or_default();
                                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                                    uri: Url::from_file_path(file_path).unwrap(),
                                    range: Range { start, end },
                                })));
                            }
                        }
                    }
                }
            }
        }
        
        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri.to_string();
        let position = params.text_document_position_params.position;

        if let Some((ast, source_map, diags, text)) = self.analyze_uri(&uri).await {
            if diags.is_empty() {
                let mut lowerer = pace_hir::LoweringContext::new();
                if let Ok(hir) = lowerer.lower_program(ast) {
                    let mut tc = pace_ty::TypeChecker::new();
                    let _ = tc.check_program(&hir);

                    let file_path = std::path::PathBuf::from(uri.replace("file://", ""));
                    if let Some(file_id) = source_map.get_file_id(file_path.to_string_lossy().as_ref()) {
                        let offset = find_node::position_to_offset(&text, position);
                        if let Some(id) = find_node::find_ident_at_offset(&hir, file_id, offset) {
                            if let Some(ty) = tc.local_types.get(&id).or_else(|| tc.env.get(&id)) {
                                return Ok(Some(Hover {
                                    contents: HoverContents::Scalar(MarkedString::String(tc.display_ty(ty))),
                                    range: None,
                                }));
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;
        let mut items = Vec::new();

        if let Some((ast, source_map, _diags, text)) = self.analyze_uri(&uri).await {
            let mut lowerer = pace_hir::LoweringContext::new();
            if let Ok(hir) = lowerer.lower_program(ast) {
                let mut tc = pace_ty::TypeChecker::new();
                let _ = tc.check_program(&hir);

                let file_path = std::path::PathBuf::from(uri.replace("file://", ""));
                if let Some(file_id) = source_map.get_file_id(file_path.to_string_lossy().as_ref()) {
                    let offset = find_node::position_to_offset(&text, position);
                    let is_dot_completion = offset > 0 && text.chars().nth(offset - 1) == Some('.');
                    
                    if is_dot_completion {
                        let mut prev_offset = offset - 1;
                        while prev_offset > 0 && text.chars().nth(prev_offset - 1).unwrap_or(' ').is_whitespace() {
                            prev_offset -= 1;
                        }
                        if let Some(id) = find_node::find_ident_at_offset(&hir, file_id, prev_offset - 1) {
                            if let Some(ty) = tc.local_types.get(&id) {
                                let mut add_fields = |fields: &Vec<(String, pace_ty::Ty, bool, bool)>| {
                                    for (fname, _, _, is_private) in fields {
                                        items.push(CompletionItem {
                                            label: fname.clone(),
                                            kind: Some(CompletionItemKind::FIELD),
                                            detail: Some(if *is_private { "private field".to_string() } else { "field".to_string() }),
                                            ..Default::default()
                                        });
                                    }
                                };
                                match ty {
                                    pace_ty::Ty::Struct(sid) => {
                                        if let Some(fields) = tc.struct_defs.get(sid) {
                                            add_fields(fields);
                                        }
                                        if let Some(struct_name) = tc.named_types.iter().find_map(|(k, &(vid, _))| if vid == *sid { Some(k) } else { None }) {
                                            for (mname, _) in &tc.methods_env {
                                                if mname.starts_with(&format!("{}_", struct_name)) {
                                                    let method_short_name = mname.trim_start_matches(&format!("{}_", struct_name));
                                                    items.push(CompletionItem {
                                                        label: method_short_name.to_string(),
                                                        kind: Some(CompletionItemKind::METHOD),
                                                        detail: Some("method".to_string()),
                                                        ..Default::default()
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    pace_ty::Ty::Class(cid) => {
                                        if let Some(fields) = tc.class_defs.get(cid) {
                                            add_fields(fields);
                                        }
                                        if let Some(class_name) = tc.named_types.iter().find_map(|(k, &(vid, _))| if vid == *cid { Some(k) } else { None }) {
                                            for (mname, _) in &tc.methods_env {
                                                if mname.starts_with(&format!("{}_", class_name)) {
                                                    let method_short_name = mname.trim_start_matches(&format!("{}_", class_name));
                                                    items.push(CompletionItem {
                                                        label: method_short_name.to_string(),
                                                        kind: Some(CompletionItemKind::METHOD),
                                                        detail: Some("method".to_string()),
                                                        ..Default::default()
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    } else {
                        for (_, name, _) in tc.declared_bindings {
                            items.push(CompletionItem {
                                label: name.clone(),
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some("Local Variable".to_string()),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }
    
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;
        let new_name = params.new_name;

        if let Some((ast, source_map, diags, text)) = self.analyze_uri(&uri).await {
            if diags.is_empty() {
                let mut lowerer = pace_hir::LoweringContext::new();
                if let Ok(hir) = lowerer.lower_program(ast) {
                    let mut tc = pace_ty::TypeChecker::new();
                    let _ = tc.check_program(&hir);

                    let file_path = std::path::PathBuf::from(uri.replace("file://", ""));
                    if let Some(file_id) = source_map.get_file_id(file_path.to_string_lossy().as_ref()) {
                        let offset = find_node::position_to_offset(&text, position);
                        if let Some(id) = find_node::find_ident_at_offset(&hir, file_id, offset) {
                        let mut changes = HashMap::new();
                        let mut edits = Vec::new();
                        
                        // Add the declaration itself
                        if let Some((_, _, span)) = tc.declared_bindings.iter().find(|(d_id, _, _)| *d_id == id) {
                            let span_text = source_map.get_source(span.file_id).unwrap_or_default();
                            let pos = offset_to_position(span_text, span.start as usize);
                            
                            edits.push(TextEdit {
                                range: Range {
                                    start: pos,
                                    end: Position { line: pos.line, character: pos.character + new_name.len() as u32 }, // approx
                                },
                                new_text: new_name.clone(),
                            });
                        }

                        // Add references
                        if let Some(refs) = tc.symbol_references.get(&id) {
                            for span in refs {
                                let span_text = source_map.get_source(span.file_id).unwrap_or_default();
                                let pos = offset_to_position(span_text, span.start as usize);
                                
                                let file_path = source_map.get_path(span.file_id).unwrap_or_default();
                                let file_uri = Url::from_file_path(file_path).unwrap();
                                
                                let edit = TextEdit {
                                    range: Range {
                                        start: pos,
                                        end: Position { line: pos.line, character: pos.character + new_name.len() as u32 }, // approx
                                    },
                                    new_text: new_name.clone(),
                                };
                                changes.entry(file_uri).or_insert_with(Vec::new).push(edit);
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
        }
        Ok(None)
    }

    async fn document_highlight(&self, params: DocumentHighlightParams) -> Result<Option<Vec<DocumentHighlight>>> {
        let uri = params.text_document_position_params.text_document.uri.to_string();
        let position = params.text_document_position_params.position;

        if let Some((ast, source_map, diags, text)) = self.analyze_uri(&uri).await {
            if diags.is_empty() {
                let mut lowerer = pace_hir::LoweringContext::new();
                if let Ok(hir) = lowerer.lower_program(ast) {
                    let mut tc = pace_ty::TypeChecker::new();
                    let _ = tc.check_program(&hir);

                    let file_path = std::path::PathBuf::from(uri.replace("file://", ""));
                    if let Some(file_id) = source_map.get_file_id(file_path.to_string_lossy().as_ref()) {
                        let offset = find_node::position_to_offset(&text, position);
                        if let Some(id) = find_node::find_ident_at_offset(&hir, file_id, offset) {
                        let mut highlights = Vec::new();

                        if let Some((_, _, span)) = tc.declared_bindings.iter().find(|(d_id, _, _)| *d_id == id) {
                            let span_text = source_map.get_source(span.file_id).unwrap_or_default();
                            let pos = offset_to_position(span_text, span.start as usize);
                            highlights.push(DocumentHighlight {
                                range: Range {
                                    start: pos,
                                    end: Position { line: pos.line, character: pos.character + 5 }, // approx
                                },
                                kind: Some(DocumentHighlightKind::WRITE),
                            });
                        }

                        if let Some(refs) = tc.symbol_references.get(&id) {
                            for span in refs {
                                let span_text = source_map.get_source(span.file_id).unwrap_or_default();
                                let pos = offset_to_position(span_text, span.start as usize);
                                highlights.push(DocumentHighlight {
                                    range: Range {
                                        start: pos,
                                        end: Position { line: pos.line, character: pos.character + 5 }, // approx
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
        }
        Ok(None)
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri.to_string();

        if let Some((ast, source_map, diags, _text)) = self.analyze_uri(&uri).await {
            if diags.is_empty() {
                let mut lowerer = pace_hir::LoweringContext::new();
                if let Ok(hir) = lowerer.lower_program(ast) {
                    let mut tc = pace_ty::TypeChecker::new();
                    let _ = tc.check_program(&hir);

                    let mut hints = Vec::new();
                    for (span, hint_str) in tc.inlay_hints {
                        // Inlay hints apply to the current file
                        if let Some(file_path) = source_map.get_path(span.file_id) {
                            if let Ok(file_uri) = Url::from_file_path(file_path) {
                                if file_uri.to_string() == uri {
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
                        }
                    }
                    return Ok(Some(hints));
                }
            }
        }
        Ok(None)
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri.to_string();
        let position = params.text_document_position_params.position;

        if let Some((ast, source_map, diags, text)) = self.analyze_uri(&uri).await {
            if diags.is_empty() {
                let mut lowerer = pace_hir::LoweringContext::new();
                if let Ok(hir) = lowerer.lower_program(ast) {
                    let mut tc = pace_ty::TypeChecker::new();
                    let _ = tc.check_program(&hir);

                    let offset = find_node::position_to_offset(&text, position);
                    
                    // Find the innermost function call whose span contains the offset
                    let mut best_call = None;
                    let mut best_len = usize::MAX;
                    for (span, ty) in &tc.function_calls {
                        let span_len = span.end.saturating_sub(span.start) as usize;
                        if offset >= span.start as usize && offset <= span.end as usize {
                            if span_len < best_len {
                                best_len = span_len;
                                best_call = Some((*span, ty.clone()));
                            }
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
        }
        Ok(None)
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let mut actions = Vec::new();
        
        for diag in params.context.diagnostics {
            if diag.message.contains("must be assigned before it can be used") {
                // Not much we can do automatically here
            }
            if diag.message.contains("Type mismatch") {
                // Type mismatch
            }
            
            // Unused variables often have "unused variable: `x`" (if rustc-like) 
            // but in pace we don't have unused var warnings yet from typechecker.
            
            // As a simple placeholder Code Action for Phase 4:
            // Let's suggest adding an import if we see "Cannot find value 'xyz'"
            if diag.message.starts_with("Cannot find value '") {
                if let Some(start) = diag.message.find('\'') {
                    if let Some(end) = diag.message[start + 1..].find('\'') {
                        let var_name = &diag.message[start + 1..start + 1 + end];
                        
                        let mut changes = HashMap::new();
                        let uri = params.text_document.uri.clone();
                        let edit = TextEdit {
                            range: Range {
                                start: Position { line: 0, character: 0 },
                                end: Position { line: 0, character: 0 },
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
                }
            }

            if diag.message.starts_with("unused variable or function: `") {
                if let Some(start) = diag.message.find('`') {
                    if let Some(end) = diag.message[start + 1..].find('`') {
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
            }
        }

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }
}

impl Backend {
    async fn publish_diagnostics(&self, uri: String, text: String) {
        let mut lsp_diags = Vec::new();
        
        if let Some((ast, source_map, diags, _)) = self.analyze_uri(&uri).await {
            // Find file_id for current uri so we only report its diagnostics
            let mut current_file_id = None;
            if let Ok(url) = Url::parse(&uri) {
                if let Ok(file_path) = url.to_file_path() {
                    current_file_id = source_map.get_file_id(&file_path.display().to_string());
                }
            }

            for diag in diags {
                // If it belongs to another file, skip it
                if let Some(span) = diag.span {
                    if let Some(cf_id) = current_file_id {
                        if span.file_id != cf_id {
                            continue;
                        }
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
                if let Ok(hir) = lowerer.lower_program(ast) {
                    let mut tc = pace_ty::TypeChecker::new();
                    if let Err(msg) = tc.check_program(&hir) {
                    let range = Range {
                        start: Position { line: 0, character: 0 },
                        end: Position { line: 0, character: 1 },
                    };
                    lsp_diags.push(Diagnostic {
                        range,
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

    async fn analyze_uri(
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

        match pace_driver::analyze_workspace(&file_path, &overrides) {
            Ok((ast, source_map, diags)) => Some((ast, source_map, diags, text)),
            Err(_) => None,
        }
    }
}

fn offset_to_position(text: &str, offset: usize) -> Position {
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

pub async fn start_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        document_map: Arc::new(Mutex::new(HashMap::new())),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}


fn decl_to_document_symbol(decl: &pace_ast::Decl, text: &str, file_id: pace_span::FileId) -> Option<DocumentSymbol> {
    use pace_ast::Decl;
    
    // Helper closure to check file id and prevent panics
    let is_valid = |span: &pace_span::Span| span.file_id == file_id;
    
    match decl {
        Decl::Function { name, span, .. } if is_valid(span) => {
            #[allow(deprecated)]
            Some(DocumentSymbol {
                name: name.name.clone(),
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
        Decl::Struct { name, span, fields, methods, .. } if is_valid(span) => {
            let mut children = Vec::new();
            for (fname, _, _, _, _) in fields {
                if !is_valid(&fname.span) { continue; }
                #[allow(deprecated)]
                children.push(DocumentSymbol {
                    name: fname.name.clone(),
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
                name: name.name.clone(),
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
        Decl::Class { name, span, fields, methods, .. } if is_valid(span) => {
            let mut children = Vec::new();
            for (fname, _, _, _, _) in fields {
                if !is_valid(&fname.span) { continue; }
                #[allow(deprecated)]
                children.push(DocumentSymbol {
                    name: fname.name.clone(),
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
                name: name.name.clone(),
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
        Decl::Enum { name, span, variants, .. } if is_valid(span) => {
            let mut children = Vec::new();
            for v in variants {
                if !is_valid(&v.name.span) { continue; }
                #[allow(deprecated)]
                children.push(DocumentSymbol {
                    name: v.name.name.clone(),
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
                name: name.name.clone(),
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
        Decl::Trait { name, span, methods, .. } if is_valid(span) => {
            let mut children = Vec::new();
            for m in methods {
                if let Some(mut m_sym) = decl_to_document_symbol(m, text, file_id) {
                    m_sym.kind = SymbolKind::METHOD;
                    children.push(m_sym);
                }
            }
            #[allow(deprecated)]
            Some(DocumentSymbol {
                name: name.name.clone(),
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
        Decl::Let { name, span, .. } | Decl::Var { name, span, .. } if is_valid(span) => {
            #[allow(deprecated)]
            Some(DocumentSymbol {
                name: name.name.clone(),
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
        Decl::Const { name, span, .. } if is_valid(span) => {
            #[allow(deprecated)]
            Some(DocumentSymbol {
                name: name.name.clone(),
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
