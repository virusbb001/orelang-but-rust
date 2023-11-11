use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Backend {
    client: Client,
}

impl Backend {
    pub async fn compile(&self, uri: &str, src: &str) {
        self.client.log_message(MessageType::INFO, format!("{}:{}", uri, src)).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
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
        self.compile(uri.as_ref(), &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(content_change) = params.content_changes.last() {
            let uri = params.text_document.uri;
            let text = &content_change.text;
            self.compile(uri.as_ref(), text).await;
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
