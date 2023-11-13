mod parser;
use ropey::Rope;
use std::collections::HashMap;
use std::sync::Mutex;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Backend {
    client: Client,

    publish_diagnostics_capable: Mutex<bool>,

    rope_map: Mutex<HashMap<String, Rope>>
}

fn create_simple_diagnostics(
    message: String,
    start_line: u32, start_column: u32,
    end_line: u32, end_column: u32
) -> Diagnostic {
    Diagnostic::new_simple(Range {
        start: Position {
            line: start_line,
            character: start_column,
        },
        end: Position {
            line: end_line,
            character: end_column,
        },
    }, message)
}

impl Backend {
    pub fn new (client: Client) -> Backend {
        Backend {
            client,
            publish_diagnostics_capable: Mutex::new(false),
            rope_map: Mutex::new(HashMap::new())
        }
    }
    pub async fn compile(&self, uri: Url, src: &str) {
        self.rope_map.lock().unwrap().insert(uri.to_string(), Rope::from_str(src));
        let diagnostics = vec![
            create_simple_diagnostics("diagnostic message 1".into(), 0, 0, 0, 5),
            create_simple_diagnostics("diagnostic message 2".into(), 1, 0, 1, 5),
        ];
        self.send_publish_diagnostics(uri, diagnostics).await;
    }

    pub async fn send_publish_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>) {
        if *(self.publish_diagnostics_capable.lock().unwrap()) {
            self.client.publish_diagnostics(uri, diagnostics, None).await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let publish_diagnostics_capable = params
            .capabilities
            .text_document
            .map_or(false, |v| v.publish_diagnostics.is_some());
        *self.publish_diagnostics_capable.lock().unwrap() = publish_diagnostics_capable;

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                ..Default::default()
            },
            server_info: None,
        })
    }
    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "server initialized!")
        .await;
    }
    async fn shutdown (&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.compile(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(content_change) = params.content_changes.last() {
            let uri = params.text_document.uri;
            let text = &content_change.text;
            self.compile(uri, text).await;
        }
    }

    async fn did_close (&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.send_publish_diagnostics(uri, vec![]).await;
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
