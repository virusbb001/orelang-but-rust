mod parser;
use parser::{parse, ImCompleteSemanticToken};
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

    rope_map: Mutex<HashMap<String, Rope>>,

    token_types_map: Mutex<HashMap<SemanticTokenType, usize>>,

    semantic_token_map: Mutex<HashMap<String, Vec<ImCompleteSemanticToken>>>,
}

fn create_simple_diagnostics(
    message: String,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
) -> Diagnostic {
    Diagnostic::new_simple(
        Range {
            start: Position {
                line: start_line,
                character: start_column,
            },
            end: Position {
                line: end_line,
                character: end_column,
            },
        },
        message,
    )
}

impl Backend {
    pub fn new(client: Client) -> Backend {
        Backend {
            client,
            publish_diagnostics_capable: Mutex::new(false),
            rope_map: Mutex::new(HashMap::new()),
            token_types_map: Mutex::new(HashMap::new()),
            semantic_token_map: Mutex::new(HashMap::new()),
        }
    }
    pub async fn compile(&self, uri: Url, src: &str) {
        self.rope_map
            .lock()
            .unwrap()
            .insert(uri.to_string(), Rope::from_str(src));

        let semantic_tokens = parse(src).semantic_tokens;

        self.semantic_token_map
            .lock()
            .unwrap()
            .insert(uri.to_string(), semantic_tokens);

        let diagnostics = vec![
            create_simple_diagnostics("diagnostic message 1".into(), 0, 0, 0, 5),
            create_simple_diagnostics("diagnostic message 2".into(), 1, 0, 1, 5),
        ];
        self.send_publish_diagnostics(uri, diagnostics).await;
    }

    pub async fn send_publish_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>) {
        if *(self.publish_diagnostics_capable.lock().unwrap()) {
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let token_types = if let Some(text_document) = params.capabilities.text_document {
            let publish_diagnostics_capable = text_document.publish_diagnostics.is_some();
            *self.publish_diagnostics_capable.lock().unwrap() = publish_diagnostics_capable;
            let token_types =
                || -> Option<_> { Some(text_document.semantic_tokens?.token_types) }()
                    .unwrap_or_default();

            let mut token_types_map = self.token_types_map.lock().unwrap();
            token_types
                .iter()
                .enumerate()
                .for_each(|(index, token_type)| {
                    token_types_map.insert(token_type.clone(), index);
                });

            token_types
        } else {
            vec![]
        };

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types,
                                token_modifiers: vec![],
                            },
                            range: Some(false),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            ..Default::default()
                        },
                    ),
                ),
                ..Default::default()
            },
            server_info: None,
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

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.send_publish_diagnostics(uri, vec![]).await;
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri.to_string();
        let token_types_map = self.token_types_map.lock().unwrap();

        let semantic_tokens = || -> Option<Vec<SemanticToken>> {
            let binding = self.rope_map.lock().unwrap();
            let rope = binding.get(&uri)?;
            let binding = self.semantic_token_map.lock().unwrap();
            let v = binding.get(&uri)?;
            let mut pre_line = 0;
            let mut pre_column = 0;
            let semantic_tokens = v
                .iter()
                .filter_map(|token| {
                    let line = rope.try_byte_to_line(token.start).ok()?;
                    let line_first = rope.try_line_to_char(line).ok()?;
                    let column = rope.try_byte_to_char(token.start).ok()? - line_first;
                    let token_type = token_types_map.get(&token.token_type)?;

                    let delta_line = line - pre_line;
                    let delta_start = if delta_line == 0 {
                        column - pre_column
                    } else {
                        column
                    };

                    let ret = Some(SemanticToken {
                        delta_line: delta_line.try_into().unwrap(),
                        delta_start: delta_start.try_into().unwrap(),
                        length: token.length.try_into().unwrap(),
                        token_type: *token_type as u32,
                        token_modifiers_bitset: 0,
                    });

                    pre_line = line;
                    pre_column = column;

                    ret
                })
                .collect::<Vec<_>>();

            Some(semantic_tokens)
        }();

        let result = semantic_tokens.map(|semantic_tokens| {
            SemanticTokensResult::Tokens(SemanticTokens {
                result_id: None,
                data: semantic_tokens,
            })
        });

        Ok(result)
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
