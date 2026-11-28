use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use pace_lexer::Lexer;
use pace_parser::parser::Parser;
use pace_span::SourceMap;

mod completion;
mod diagnostics;
mod find_node;
mod goto;
mod hover;
mod rename;
mod symbols;

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
                    trigger_characters: Some(vec![".".to_string(), ":".to_string(), "@".to_string()]),
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
        self.document_symbol_impl(params).await
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        let text = params.text_document.text.clone();

        self.document_map
            .lock()
            .await
            .insert(uri.clone(), text.clone());
        self.publish_diagnostics(uri, text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        if let Some(change) = params.content_changes.into_iter().next() {
            self.document_map
                .lock()
                .await
                .insert(uri.clone(), change.text.clone());
            self.publish_diagnostics(uri, change.text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        self.document_map.lock().await.remove(&uri);
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
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
                    name: pace_span::intern("module"),
                    file_id,
                    declarations: ast,
                    comments,
                };
                let formatted = pace_fmt::format_module(&module);

                // For simplicity, we replace the entire document
                let line_count = source.lines().count() as u32;
                let range = Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: line_count,
                        character: 0,
                    },
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
        self.goto_definition_impl(params).await
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        self.hover_impl(params).await
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        self.completion_impl(params).await
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        self.rename_impl(params).await
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        self.document_highlight_impl(params).await
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        self.inlay_hint_impl(params).await
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        self.signature_help_impl(params).await
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        self.code_action_impl(params).await
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_to_position() {
        let text = "hello\nworld\nfoo";
        let pos0 = diagnostics::offset_to_position(text, 0);
        assert_eq!(pos0.line, 0);
        assert_eq!(pos0.character, 0);

        let pos6 = diagnostics::offset_to_position(text, 6);
        assert_eq!(pos6.line, 1);
        assert_eq!(pos6.character, 0);

        let pos8 = diagnostics::offset_to_position(text, 8);
        assert_eq!(pos8.line, 1);
        assert_eq!(pos8.character, 2);
    }

    #[test]
    fn test_non_file_uri_handling() {
        let untitled_uri = Url::parse("untitled:Untitled-1").unwrap();
        assert!(untitled_uri.to_file_path().is_err());

        let http_uri = Url::parse("https://example.com/test.pace").unwrap();
        assert!(http_uri.to_file_path().is_err());
    }

    #[test]
    fn test_annotation_hover_and_completions() {
        let src = "@override\nfn speak() {}";
        assert!(hover::annotation_hover_at(src, 3).is_some());
        assert!(hover::annotation_hover_at(src, 12).is_none());
        let comps = completion::annotation_completions();
        assert!(comps.iter().any(|c| c.label == "override"));
        assert!(comps.iter().any(|c| c.label == "deprecated"));
    }
}
