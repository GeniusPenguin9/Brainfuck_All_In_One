use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use formatter::format_pretty_string;
use tower_lsp::jsonrpc::Result;
// use tower_lsp::lsp_types::*;s
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentFormattingParams, InitializeParams, InitializeResult,
    InitializedParams, InlayHint, InlayHintParams, MessageType, OneOf, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Url,
};

use tower_lsp::{Client, LanguageServer, LspService, Server};

pub mod formatter;
pub mod inlay_hint;

struct Backend {
    client: Client,
    text_documents: Arc<Mutex<HashMap<String, TextDocumentItemValue>>>,
}
pub struct TextDocumentItemValue {
    pub version: i32,
    pub text: String,
}

fn convert_range(input: brainfuck_analyzer::Range) -> tower_lsp::lsp_types::Range {
    tower_lsp::lsp_types::Range {
        start: tower_lsp::lsp_types::Position {
            line: input.start.line,
            character: input.start.character,
        },
        end: tower_lsp::lsp_types::Position {
            line: input.end.line,
            character: input.end.character,
        },
    }
}

fn convert_inlay_hint(input: inlay_hint::InlayHint) -> tower_lsp::lsp_types::InlayHint {
    tower_lsp::lsp_types::InlayHint {
        position: tower_lsp::lsp_types::Position {
            line: input.position.line,
            character: input.position.character,
        },
        label: tower_lsp::lsp_types::InlayHintLabel::String(input.label),
        kind: None,
        text_edits: None,
        tooltip: None,
        padding_left: None,
        padding_right: None,
        data: None,
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "brainfuck-lsp".to_string(),
                version: Some("1.0".to_string()),
            }),
            capabilities: ServerCapabilities {
                document_formatting_provider: Some(OneOf::Left(true)),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                inlay_hint_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        self.client
            .log_message(
                MessageType::INFO,
                format!("{:?}", params.text_document.uri.to_string()),
            )
            .await;

        let url = params.text_document.uri.to_string();
        let res;
        let mut err = None;
        {
            let hash_map = self.text_documents.lock().unwrap();

            res = if let Some(contents) = hash_map.get(&url) {
                let format_res = format_pretty_string(&contents.text);

                match format_res {
                    Ok(f) => Ok(Some(vec![TextEdit {
                        range: convert_range(f.range),
                        new_text: f.format_result,
                    }])),
                    Err(e) => {
                        err = Some(e);
                        Ok(None)
                    }
                }
            } else {
                Ok(None)
            };
        }
        self.client
            .log_message(MessageType::INFO, format!("err = {:?}", err))
            .await;
        res
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file opened!")
            .await;
        self.when_change(
            params.text_document.uri,
            TextDocumentItemValue {
                version: params.text_document.version,
                text: params.text_document.text,
            },
        )
        .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(text) = params.content_changes.first() {
            self.when_change(
                params.text_document.uri,
                TextDocumentItemValue {
                    version: params.text_document.version,
                    text: text.text.clone(),
                },
            )
            .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file closed!")
            .await;
        let mut hash_map = self.text_documents.lock().unwrap();
        hash_map.remove(&params.text_document.uri.to_string());
    }
}

impl Backend {
    async fn when_change(&self, url: Url, value: TextDocumentItemValue) {
        self.client
            .log_message(MessageType::INFO, format!("{:?}", url.to_string()))
            .await;
        {
            let mut hash_map = self.text_documents.lock().unwrap();
            let item = hash_map
                .entry(url.to_string())
                .or_insert(TextDocumentItemValue {
                    version: 0,
                    text: "".to_string(),
                });
            *item = value;
        }
        self.check(url).await;
    }

    async fn check(&self, url: Url) {
        self.client
            .log_message(MessageType::INFO, format!("{:?}", url.to_string()))
            .await;
        let mut err = None;
        let mut version = 0;
        {
            let hash_map = self.text_documents.lock().unwrap();
            if let Some(contents) = hash_map.get(&url.to_string()) {
                let format_res = brainfuck_analyzer::parse(&contents.text);
                if let Err(parse_error) = format_res {
                    err = Some(parse_error);
                    version = contents.version;
                }
            }
        }
        if let Some(err) = err {
            self.client
                .publish_diagnostics(
                    url,
                    vec![Diagnostic {
                        range: convert_range(err.range),
                        severity: Some(DiagnosticSeverity::ERROR),
                        message: err.error_message,
                        ..Default::default()
                    }],
                    Some(version),
                )
                .await;
        } else {
            self.client
                .publish_diagnostics(url, vec![], Some(version))
                .await;
        }
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Vec<InlayHint>> {
        self.client
            .log_message(
                MessageType::INFO,
                format!("{:?}", params.text_document.uri.to_string()),
            )
            .await;

        let url = params.text_document.uri.to_string();
        let res;
        let mut err = None;
        {
            let hash_map = self.text_documents.lock().unwrap();

            res = if let Some(contents) = hash_map.get(&url) {
                let inlay_hint_res = inlay_hint::InlayHint::inlay_hint_string(&contents.text);

                match inlay_hint_res {
                    Ok(f) => Ok(f.into_iter().map(convert_inlay_hint).collect()),
                    Err(e) => {
                        err = Some(e);
                        Ok(Vec::new())
                    }
                }
            } else {
                Ok(Vec::new())
            };
        }
        self.client
            .log_message(MessageType::INFO, format!("err = {:?}", err))
            .await;
        res
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
        text_documents: Default::default(),
    })
    .custom_method("textDocument/inlayHint", Backend::inlay_hint)
    .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}
