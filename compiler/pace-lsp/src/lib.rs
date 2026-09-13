use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use pace_lexer::Lexer;
use pace_parser::parser::Parser;
use pace_span::SourceMap;
use std::path::PathBuf;

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
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
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

        if let Some((ast, _source_map, diags, text)) = self.analyze_uri(&uri).await {
            if diags.is_empty() {
                let mut lowerer = pace_hir::LoweringContext::new();
                if let Ok(hir) = lowerer.lower_program(ast) {
                    let mut tc = pace_ty::TypeChecker::new();
                    let _ = tc.check_program(&hir);

                let offset = find_node::position_to_offset(&text, position);
                if let Some(id) = find_node::find_ident_at_offset(&hir, offset) {
                    for (decl_id, _, span) in tc.declared_bindings {
                        if decl_id == id {
                            let start = offset_to_position(&text, span.start as usize);
                            let end = offset_to_position(&text, span.end as usize);
                            return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                                uri: Url::parse(&uri).unwrap(),
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

        if let Some((ast, _source_map, diags, text)) = self.analyze_uri(&uri).await {
            if diags.is_empty() {
                let mut lowerer = pace_hir::LoweringContext::new();
                if let Ok(hir) = lowerer.lower_program(ast) {
                    let mut tc = pace_ty::TypeChecker::new();
                    let _ = tc.check_program(&hir);

                let offset = find_node::position_to_offset(&text, position);
                if let Some(id) = find_node::find_ident_at_offset(&hir, offset) {
                    if let Some(ty) = tc.local_types.get(&id).or_else(|| tc.env.get(&id)) {
                        return Ok(Some(Hover {
                            contents: HoverContents::Scalar(MarkedString::String(format!("{:?}", ty))),
                            range: None,
                        }));
                    }
                }
                }
            }
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let _position = params.text_document_position.position;
        let mut items = Vec::new();

        if let Some((ast, _source_map, diags, _text)) = self.analyze_uri(&uri).await {
            if diags.is_empty() {
                let mut lowerer = pace_hir::LoweringContext::new();
                if let Ok(hir) = lowerer.lower_program(ast) {
                    let mut tc = pace_ty::TypeChecker::new();
                    let _ = tc.check_program(&hir);

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
    
        Ok(Some(CompletionResponse::Array(items)))
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
