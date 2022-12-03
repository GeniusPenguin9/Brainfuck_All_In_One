use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use formatter::format_string;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFormattingParams, InitializeParams, InitializeResult, InitializedParams, MessageType,
    OneOf, ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextEdit, Url,
};
use tower_lsp::{Client, LanguageServer, LspService, Server};

pub mod formatter;

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
                ..Default::default()
            },
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
                let format_res = format_string(&contents.text);

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
        let mut hash_map = self.text_documents.lock().unwrap();
        let item = hash_map
            .entry(url.to_string())
            .or_insert(TextDocumentItemValue {
                version: 0,
                text: "".to_string(),
            });
        *item = value;
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        text_documents: Default::default(),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
