use std::fmt::format;
use std::fs;

use formatter::format_string;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    DocumentFormattingParams, InitializeParams, InitializeResult, InitializedParams, MessageType,
    OneOf, ServerCapabilities, ServerInfo, TextEdit,
};
use tower_lsp::{Client, LanguageServer, LspService, Server};

pub mod formatter;

#[derive(Debug)]
struct Backend {
    client: Client,
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
            .log_message(MessageType::INFO, format!("{:?}", params))
            .await;

        if let Ok(file) = params.text_document.uri.to_file_path() {
            let contents = fs::read_to_string(file)
                .expect(format!("Something went wrong reading the file: ").as_str());

            let format_res = format_string(&contents);

            match format_res {
                Ok(f) => Ok(Some(vec![TextEdit {
                    range: convert_range(f.range),
                    new_text: f.format_result,
                }])),
                Err(_) => Ok(None),
            }
        } else {
            Ok(None)
        }
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend { client });
    Server::new(stdin, stdout, socket).serve(service).await;
}
