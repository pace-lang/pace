use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use pace_driver::CompilerSession;


pub struct PaceLanguageServer {
    pub client: Client,
    pub session: CompilerSession,
}

impl PaceLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            session: CompilerSession::new(),
        }
    }

    async fn check_and_publish_diagnostics(&self, uri: Url, src: &str) {
        let mut diagnostics = Vec::new();
        
        let ast_result = self.session.check_source(src);
        
        match ast_result {
            Ok(_) => {
                // No errors
            },
            Err(e) => {
                // It's a miette::Report.
                // We map miette errors to LSP Diagnostics.
                if let Some(multiple_ty_errors) = e.downcast_ref::<pace_driver::MultipleTypeErrors>() {
                    for err in &multiple_ty_errors.errors {
                        diagnostics.push(map_type_error(err, src));
                    }
                } else if let Some(multiple_syntax_errors) = e.downcast_ref::<pace_errors::MultipleSyntaxErrors>() {
                    for err in &multiple_syntax_errors.errors {
                        diagnostics.push(map_syntax_error(err, src));
                    }
                } else {
                    // Generic error
                    let diag = Diagnostic {
                        range: Range {
                            start: Position { line: 0, character: 0 },
                            end: Position { line: 0, character: 0 },
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        message: e.to_string(),
                        ..Default::default()
                    };
                    diagnostics.push(diag);
                }
            }
        }
        
        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }
}

fn get_position(src: &str, offset: usize) -> Position {
    let mut line = 0;
    let mut char_idx = 0;
    
    for (i, c) in src.chars().enumerate() {
        if i == offset {
            break;
        }
        if c == '\n' {
            line += 1;
            char_idx = 0;
        } else {
            char_idx += 1;
        }
    }
    
    Position { line, character: char_idx }
}

fn map_syntax_error(err: &pace_errors::SyntaxError, src: &str) -> Diagnostic {
    let (offset, length) = err.span;
    let start = get_position(src, offset);
    let end = get_position(src, offset + length);
    
    Diagnostic {
        range: Range { start, end },
        severity: Some(DiagnosticSeverity::ERROR),
        message: err.message.clone(),
        ..Default::default()
    }
}

fn map_type_error(err: &pace_ty::TypeError, src: &str) -> Diagnostic {
    use pace_ty::TypeError::*;
    let severity = DiagnosticSeverity::ERROR;
    
    let (message, start_offset, length) = match err {
        Generic { span, message: msg, .. } => {
            (msg.clone(), span.0, span.1)
        },
        TypeMismatch { message: msg, span, .. } => {
            (format!("Type mismatch: {}", msg), span.0, span.1)
        },
        UnknownIdentifier { name, help_text, span, .. } => {
            (format!("Unknown identifier '{}'\nHelp: {}", name, help_text), span.0, span.1)
        },
        DuplicateDeclaration { name, span, .. } => {
            (format!("Duplicate declaration of '{}'", name), span.0, span.1)
        },
        UnknownType { name, span, .. } => {
            (format!("Unknown type '{}'", name), span.0, span.1)
        },
        InvalidWeakReference { span, .. } => {
            ("Invalid weak reference".to_string(), span.0, span.1)
        },
        OwnershipViolation { message: msg, span, .. } => {
            (format!("Ownership violation: {}", msg), span.0, span.1)
        }
    };
    
    let start = get_position(src, start_offset);
    let end = get_position(src, start_offset + length);
    
    let mut diag = Diagnostic {
        range: Range { start, end },
        severity: Some(severity),
        message,
        ..Default::default()
    };
    
    if let DuplicateDeclaration { original_span, .. } = err {
        let orig_start = get_position(src, original_span.0);
        let orig_end = get_position(src, original_span.0 + original_span.1);
        diag.related_information = Some(vec![
            DiagnosticRelatedInformation {
                location: Location {
                    uri: Url::parse("file:///dummy").unwrap(), // We need actual URI but this is tricky without keeping track. In a real LSP we'd resolve it.
                    range: Range { start: orig_start, end: orig_end }
                },
                message: "Original declaration here".to_string(),
            }
        ]);
    }
    
    diag
}

#[tower_lsp::async_trait]
impl LanguageServer for PaceLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                ..ServerCapabilities::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Pace language server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.check_and_publish_diagnostics(params.text_document.uri, &params.text_document.text).await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.pop() {
            self.check_and_publish_diagnostics(params.text_document.uri, &change.text).await;
        }
    }
}

pub fn run_server() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();

        let (service, socket) = tower_lsp::LspService::new(|client| PaceLanguageServer::new(client));
        tower_lsp::Server::new(stdin, stdout, socket).serve(service).await;
    });
}
