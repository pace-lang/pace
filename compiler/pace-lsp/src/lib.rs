use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use pace_lexer::Lexer;
use pace_parser::parser::Parser;
use pace_span::SourceMap;

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

            if let Ok(ast) = parser.parse_program() {
                let module = pace_ast::Module {
                    name: "module".to_string(),
                    file_id,
                    declarations: ast,
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
}

impl Backend {
    async fn publish_diagnostics(&self, uri: String, text: String) {
        let mut source_map = SourceMap::new();
        let file_id = source_map.add_file(uri.clone(), text.clone());
        let lexer = Lexer::new(&text, file_id);
        let mut parser = Parser::new(lexer);

        let mut lsp_diags = Vec::new();

        if let Err(diag) = parser.parse_program() {
            // Convert Pace Diagnostic to LSP Diagnostic
            let (start_line, start_col, end_line, end_col) = if let Some(span) = diag.span {
                let start = offset_to_position(&text, span.start as usize);
                let end = offset_to_position(&text, span.end as usize);
                (start.line, start.character, end.line, end.character)
            } else {
                (0, 0, 0, 0)
            };

            let range = Range {
                start: Position {
                    line: start_line,
                    character: start_col,
                },
                end: Position {
                    line: end_line,
                    character: end_col,
                },
            };

            lsp_diags.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                message: diag.message,
                ..Default::default()
            });
        }

        if let Ok(url) = Url::parse(&uri) {
            self.client.publish_diagnostics(url, lsp_diags, None).await;
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
