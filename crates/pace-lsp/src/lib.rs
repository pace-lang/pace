use pace_driver::CompilerSession;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct PaceLanguageServer {
    pub client: Client,
    pub session: CompilerSession,
    pub ast_cache: Arc<RwLock<HashMap<Url, Vec<pace_ast::arena::StmtId>>>>,
    pub arena: Arc<RwLock<pace_ast::arena::AstArena>>,
    pub src_cache: Arc<RwLock<HashMap<Url, String>>>,
    pub env_cache: Arc<RwLock<HashMap<Url, pace_ty::Environment>>>,
    pub root_uri: Arc<RwLock<Option<Url>>>,
}

impl PaceLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            session: CompilerSession::new(),
            ast_cache: Arc::new(RwLock::new(HashMap::new())),
            arena: Arc::new(RwLock::new(pace_ast::arena::AstArena::new())),
            src_cache: Arc::new(RwLock::new(HashMap::new())),
            env_cache: Arc::new(RwLock::new(HashMap::new())),
            root_uri: Arc::new(RwLock::new(None)),
        }
    }

    async fn check_and_publish_diagnostics(&self, uri: Url, src: &str) {
        let mut diagnostics = Vec::new();

        let path = if let Ok(p) = uri.to_file_path() {
            p
        } else {
            return;
        };
        let mut arena = self.arena.write().await;
        let ast_result = self.session.check_file_with_source(&mut arena, &path, src);

        self.src_cache
            .write()
            .await
            .insert(uri.clone(), src.to_string());

        match ast_result {
            Ok((ast, warnings, type_errors, env)) => {
                self.ast_cache.write().await.insert(uri.clone(), ast);
                self.env_cache.write().await.insert(uri.clone(), env);

                let active_file = path.display().to_string();
                for warn in &warnings {
                    if let Some(diag) = map_warning(warn, src, &active_file) {
                        diagnostics.push(diag);
                    }
                }
                for err in &type_errors {
                    if let Some(diag) = map_type_error(err, src, &active_file) {
                        diagnostics.push(diag);
                    }
                }
            }
            Err(e) => {
                let active_file = path.display().to_string();
                // Syntax or package errors
                if let Some(multiple_syntax_errors) =
                    e.downcast_ref::<pace_errors::MultipleSyntaxErrors>()
                {
                    for err in &multiple_syntax_errors.errors {
                        if let Some(diag) = map_syntax_error(err, src, &active_file) {
                            diagnostics.push(diag);
                        }
                    }
                } else {
                    // Generic error
                    let diag = Diagnostic {
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
                        severity: Some(DiagnosticSeverity::ERROR),
                        message: e.to_string(),
                        ..Default::default()
                    };
                    diagnostics.push(diag);
                }
            }
        }

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

fn get_position(src: &str, offset: usize) -> Position {
    let mut line = 0;
    let mut char_idx = 0;

    for (i, c) in src.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            char_idx = 0;
        } else {
            char_idx += 1;
        }
    }

    Position {
        line,
        character: char_idx,
    }
}

#[allow(dead_code)]
fn position_to_offset(src: &str, pos: Position) -> Option<usize> {
    let mut current_line = 0;
    let mut current_char = 0;

    for (i, c) in src.char_indices() {
        if current_line == pos.line && current_char == pos.character {
            return Some(i);
        }
        if c == '\n' {
            current_line += 1;
            current_char = 0;
        } else {
            current_char += 1;
        }
    }

    if current_line == pos.line && current_char == pos.character {
        Some(src.len())
    } else {
        None
    }
}

fn get_word_at_position(src: &str, pos: Position) -> Option<String> {
    let lines: Vec<&str> = src.lines().collect();
    if pos.line as usize >= lines.len() {
        return None;
    }

    let line = lines[pos.line as usize];
    let char_idx = pos.character as usize;
    if char_idx >= line.len() {
        return None;
    }

    let mut start = char_idx;
    while start > 0 {
        let c = line.chars().nth(start - 1)?;
        if !c.is_alphanumeric() && c != '_' {
            break;
        }
        start -= 1;
    }

    let mut end = char_idx;
    while end < line.len() {
        let c = line.chars().nth(end)?;
        if !c.is_alphanumeric() && c != '_' {
            break;
        }
        end += 1;
    }

    if start < end {
        Some(line[start..end].to_string())
    } else {
        None
    }
}

fn get_string_at_position(src: &str, pos: Position) -> Option<String> {
    let lines: Vec<&str> = src.lines().collect();
    if pos.line as usize >= lines.len() {
        return None;
    }

    let line = lines[pos.line as usize];
    let char_idx = pos.character as usize;
    if char_idx >= line.len() {
        return None;
    }

    let mut start = char_idx;
    let mut found_start_quote = false;
    while start > 0 {
        start -= 1;
        if line.chars().nth(start) == Some('"') {
            found_start_quote = true;
            start += 1; // Move past the quote
            break;
        }
    }

    if !found_start_quote {
        return None;
    }

    let mut end = char_idx;
    let mut found_end_quote = false;
    while end < line.len() {
        if line.chars().nth(end) == Some('"') {
            found_end_quote = true;
            break;
        }
        end += 1;
    }

    if !found_end_quote {
        return None;
    }

    if start <= end {
        Some(line[start..end].to_string())
    } else {
        None
    }
}

fn map_warning(
    warn: &pace_errors::SemanticWarning,
    src: &str,
    active_file: &str,
) -> Option<Diagnostic> {
    use pace_errors::SemanticWarning::*;
    let severity = DiagnosticSeverity::WARNING;

    let (message, start_offset, length, warn_src) = match warn {
        NamingConvention {
            name, span, src: s, ..
        } => (
            format!("Variable or function '{}' should use camelCase", name),
            span.start,
            span.len,
            s.name(),
        ),
        UnusedItem {
            kind,
            name,
            span,
            src: s,
            ..
        } => (
            format!("Unused {} '{}'", kind, name),
            span.start,
            span.len,
            s.name(),
        ),
    };

    if warn_src != active_file {
        return None;
    }

    let start = get_position(src, start_offset);
    let end = get_position(src, start_offset + length);

    Some(Diagnostic {
        range: Range { start, end },
        severity: Some(severity),
        message,
        ..Default::default()
    })
}

fn map_syntax_error(
    err: &pace_errors::SyntaxError,
    src: &str,
    active_file: &str,
) -> Option<Diagnostic> {
    let (msg, err_src, span) = match err {
        pace_errors::SyntaxError::Generic { message, src, span } => {
            (message.clone(), src.name(), span)
        }
    };

    if err_src != active_file {
        return None;
    }

    let offset = span.start;
    let length = span.len;
    let start = get_position(src, offset);
    let end = get_position(src, offset + length);

    Some(Diagnostic {
        range: Range { start, end },
        severity: Some(DiagnosticSeverity::ERROR),
        message: msg,
        ..Default::default()
    })
}

fn map_type_error(err: &pace_ty::TypeError, src: &str, active_file: &str) -> Option<Diagnostic> {
    use pace_ty::TypeError::*;
    let severity = DiagnosticSeverity::ERROR;

    let err_src_name = match err {
        Generic { src: s, .. }
        | TypeMismatch { src: s, .. }
        | UnknownIdentifier { src: s, .. }
        | DuplicateDeclaration { src: s, .. }
        | UnknownType { src: s, .. }
        | InvalidWeakReference { src: s, .. }
        | OwnershipViolation { src: s, .. } => s.name(),
    };

    if err_src_name != active_file {
        return None;
    }

    let (message, start_offset, length) = match err {
        Generic {
            span, message: msg, ..
        } => (msg.clone(), span.start, span.len),
        TypeMismatch {
            message: msg, span, ..
        } => (format!("Type mismatch: {}", msg), span.start, span.len),
        UnknownIdentifier {
            name,
            help_text,
            span,
            ..
        } => (
            format!("Unknown identifier '{}'\nHelp: {}", name, help_text),
            span.start,
            span.len,
        ),
        DuplicateDeclaration { name, span, .. } => (
            format!("Duplicate declaration of '{}'", name),
            span.start,
            span.len,
        ),
        UnknownType { name, span, .. } => {
            (format!("Unknown type '{}'", name), span.start, span.len)
        }
        InvalidWeakReference { span, .. } => {
            ("Invalid weak reference".to_string(), span.start, span.len)
        }
        OwnershipViolation {
            message: msg, span, ..
        } => (
            format!("Ownership violation: {}", msg),
            span.start,
            span.len,
        ),
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
        let orig_start = get_position(src, original_span.start);
        let orig_end = get_position(src, original_span.start + original_span.len);
        diag.related_information = Some(vec![DiagnosticRelatedInformation {
            location: Location {
                uri: Url::parse("file:///dummy").unwrap(), // We need actual URI but this is tricky without keeping track. In a real LSP we'd resolve it.
                range: Range {
                    start: orig_start,
                    end: orig_end,
                },
            },
            message: "Original declaration here".to_string(),
        }]);
    }

    Some(diag)
}

#[tower_lsp::async_trait]
impl LanguageServer for PaceLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(uri) = params.root_uri {
            *self.root_uri.write().await = Some(uri);
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                ..ServerCapabilities::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Pace language server initialized")
            .await;

        let root_uri = self.root_uri.read().await.clone();

        if let Some(uri) = root_uri
            && let Ok(path) = uri.to_file_path()
        {
            self.client
                .log_message(
                    MessageType::INFO,
                    format!("Scanning workspace: {}", path.display()),
                )
                .await;

            for entry in walkdir::WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let p = entry.path();
                if p.is_file()
                    && p.extension().is_some_and(|ext| ext == "pace")
                    && let Ok(src) = std::fs::read_to_string(p)
                    && let Ok(file_uri) = Url::from_file_path(p)
                {
                    self.check_and_publish_diagnostics(file_uri, &src).await;
                }
            }

            self.client
                .log_message(MessageType::INFO, "Workspace scanning complete")
                .await;
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.check_and_publish_diagnostics(params.text_document.uri, &params.text_document.text)
            .await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.pop() {
            self.check_and_publish_diagnostics(params.text_document.uri, &change.text)
                .await;
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let src_cache = self.src_cache.read().await;
        if let Some(src) = src_cache.get(&uri)
            && let Some(word) = get_word_at_position(src, pos)
        {
            let mut hover_text = format!("Symbol: `{}`", word);

            let env_cache = self.env_cache.read().await;
            if let Some(env) = env_cache.get(&uri) {
                if let Some(ty) = env.symbol_types.get(&ustr::Ustr::from(&word)) {
                    hover_text = format!("```pace\nlet {}: {:?}\n```", word, ty);
                } else if let Some(func) = env.functions.get(&ustr::Ustr::from(&word)) {
                    let params_str = func
                        .params
                        .iter()
                        .map(|p| format!("{:?}", p))
                        .collect::<Vec<_>>()
                        .join(", ");
                    hover_text = format!(
                        "```pace\nfunc {}({}) -> {:?}\n```",
                        word, params_str, func.return_type
                    );
                } else if let Some(_cls) = env.classes.get(&ustr::Ustr::from(&word)) {
                    hover_text = format!("```pace\nclass {}\n```", word);
                } else if let Some(_strct) = env.structs.get(&ustr::Ustr::from(&word)) {
                    hover_text = format!("```pace\nstruct {}\n```", word);
                } else if let Some(_enm) = env.enums.get(&ustr::Ustr::from(&word)) {
                    hover_text = format!("```pace\nenum {}\n```", word);
                } else if let Some(_act) = env.actors.get(&ustr::Ustr::from(&word)) {
                    hover_text = format!("```pace\nactor {}\n```", word);
                }
            }

            return Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(hover_text)),
                range: None,
            }));
        }

        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let src_cache = self.src_cache.read().await;
        let ast_cache = self.ast_cache.read().await;

        if let Some(src) = src_cache.get(&uri) {
            // First check if we're hovering over a string (likely an import path)
            if let Some(string_content) = get_string_at_position(src, pos)
                && (string_content.starts_with("pace:")
                    || string_content.starts_with("package:")
                    || string_content.starts_with("self:")
                    || string_content.starts_with("./")
                    || string_content.starts_with("../"))
                && let Ok(path_buf) = uri.to_file_path()
                && let Ok(resolved_path) =
                    pace_driver::CompilerSession::resolve_import_path(&string_content, &path_buf)
                && resolved_path.exists()
                && let Ok(resolved_uri) = Url::from_file_path(resolved_path)
            {
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: resolved_uri,
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
                })));
            }

            if let Some(word) = get_word_at_position(src, pos)
                && let Some(ast) = ast_cache.get(&uri)
            {
                // Try to find the symbol declaration in the AST
                let arena = self.arena.read().await;
                for stmt_id in ast {
                    let stmt = arena.get_stmt(*stmt_id);
                    match stmt {
                        pace_ast::Stmt::FuncDecl { name, .. }
                        | pace_ast::Stmt::VarDecl { name, .. }
                        | pace_ast::Stmt::ClassDecl { name, .. }
                        | pace_ast::Stmt::StructDecl { name, .. }
                        | pace_ast::Stmt::EnumDecl { name, .. }
                        | pace_ast::Stmt::ActorDecl { name, .. }
                        | pace_ast::Stmt::InterfaceDecl { name, .. } => {
                            // Basic matching. `pace_ast::Stmt::ClassDecl` etc don't have span yet,
                            // but we can use the ones that do.
                            if (name == &word || name.ends_with(&format!("__{}", word)))
                                && let pace_ast::Stmt::FuncDecl { span, .. }
                                | pace_ast::Stmt::VarDecl { span, .. } = stmt
                            {
                                let start = get_position(src, span.start);
                                let end = get_position(src, span.start + span.len);
                                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                                    uri: uri.clone(),
                                    range: Range { start, end },
                                })));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(None)
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let mut responses = Vec::new();

        for diag in params.context.diagnostics {
            if diag.message.starts_with("Unknown identifier '") {
                let parts: Vec<&str> = diag.message.split('\'').collect();
                if parts.len() >= 2 {
                    let ident = parts[1];
                    let mut target_uri = None;
                    let env_cache = self.env_cache.read().await;
                    for (uri, env) in env_cache.iter() {
                        if uri != &params.text_document.uri
                            && (env.functions.contains_key(&ustr::Ustr::from(ident))
                                || env.classes.contains_key(&ustr::Ustr::from(ident))
                                || env.symbol_types.contains_key(&ustr::Ustr::from(ident))
                                || env.structs.contains_key(&ustr::Ustr::from(ident))
                                || env.actors.contains_key(&ustr::Ustr::from(ident))
                                || env.enums.contains_key(&ustr::Ustr::from(ident)))
                        {
                            target_uri = Some(uri.clone());
                            break;
                        }
                    }

                    let mut import_path = None;
                    if let Some(target) = target_uri
                        && let (Ok(current_path), Ok(target_path)) = (
                            params.text_document.uri.to_file_path(),
                            target.to_file_path(),
                        )
                        && let Some(parent) = current_path.parent()
                        && let Some(mut rel_path) = pathdiff::diff_paths(&target_path, parent)
                    {
                        rel_path.set_extension(""); // Remove .pace
                        let mut path_str = rel_path.to_string_lossy().into_owned();
                        if !path_str.starts_with(".") && !path_str.starts_with("/") {
                            path_str = format!("./{}", path_str);
                        }
                        import_path = Some(path_str);
                    }

                    if let Some(path) = import_path {
                        let mut changes = std::collections::HashMap::new();
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
                            new_text: format!("import \"{}\";\n", path),
                        };
                        changes.insert(params.text_document.uri.clone(), vec![edit]);

                        let action = CodeAction {
                            title: format!("Import '{}'", path),
                            kind: Some(CodeActionKind::QUICKFIX),
                            diagnostics: Some(vec![diag.clone()]),
                            edit: Some(WorkspaceEdit {
                                changes: Some(changes),
                                ..Default::default()
                            }),
                            ..Default::default()
                        };

                        responses.push(CodeActionOrCommand::CodeAction(action));
                    }
                }
            }
        }

        if responses.is_empty() {
            Ok(None)
        } else {
            Ok(Some(responses))
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let mut items = Vec::new();

        let env_cache = self.env_cache.read().await;
        if let Some(env) = env_cache.get(&uri) {
            // Suggest variables
            for (name, ty) in &env.symbol_types {
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some(format!("{:?}", ty)),
                    ..Default::default()
                });
            }

            // Suggest functions
            for (name, func) in &env.functions {
                let params_str = func
                    .params
                    .iter()
                    .map(|p| format!("{:?}", p))
                    .collect::<Vec<_>>()
                    .join(", ");
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(format!(
                        "func {}({}) -> {:?}",
                        name, params_str, func.return_type
                    )),
                    ..Default::default()
                });
            }

            // Suggest classes
            for name in env.classes.keys() {
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::CLASS),
                    ..Default::default()
                });
            }

            // Standard library modules
            let std_modules = vec![
                "std:math",
                "std:io",
                "std:http",
                "std:datetime",
                "std:os",
                "std:process",
                "std:collections",
            ];
            for mod_name in std_modules {
                items.push(CompletionItem {
                    label: mod_name.to_string(),
                    kind: Some(CompletionItemKind::MODULE),
                    ..Default::default()
                });
            }
        }

        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(CompletionResponse::Array(items)))
        }
    }
}

pub fn run_server() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();

        let (service, socket) = tower_lsp::LspService::new(PaceLanguageServer::new);
        tower_lsp::Server::new(stdin, stdout, socket)
            .serve(service)
            .await;
    });
}
