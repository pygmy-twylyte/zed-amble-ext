use crate::analysis::format_hover;
use crate::formatter;
use crate::queries::Queries;
use crate::symbols::{SymbolKind, SymbolStore};
use crate::text::DocumentStore;
use dashmap::DashMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use tree_sitter::Parser;

pub struct Backend {
    pub(crate) client: Client,
    pub(crate) symbols: Arc<SymbolStore>,
    pub(crate) documents: Arc<DocumentStore>,
    pub(crate) document_symbols: Arc<DashMap<String, Vec<crate::symbols::SymbolOccurrence>>>,
    pub(crate) workspace_roots: Arc<parking_lot::RwLock<Vec<PathBuf>>>,
    pub(crate) parser: Arc<parking_lot::Mutex<Parser>>,
    pub(crate) queries: Arc<Queries>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_amble::language())
            .expect("Error loading Amble grammar");

        Self {
            client,
            symbols: Arc::new(SymbolStore::default()),
            documents: Arc::new(DocumentStore::default()),
            document_symbols: Arc::new(DashMap::new()),
            workspace_roots: Arc::new(parking_lot::RwLock::new(Vec::new())),
            parser: Arc::new(parking_lot::Mutex::new(parser)),
            queries: Arc::new(Queries::new()),
        }
    }

    fn collect_rename_edits(
        &self,
        symbol_type: SymbolKind,
        id: &str,
        new_name: &str,
    ) -> HashMap<Url, Vec<TextEdit>> {
        let mut edits: HashMap<Url, Vec<TextEdit>> = HashMap::new();
        let index = self.symbols.index(symbol_type);

        let mut push_edit = |url: &Url, range: &Range| {
            edits.entry(url.clone()).or_default().push(TextEdit {
                range: range.clone(),
                new_text: new_name.to_string(),
            });
        };

        if let Some(def) = index.definition(id) {
            push_edit(&def.location.uri, &def.location.rename_range());
        }
        if let Some(refs) = index.references(id) {
            for reference in refs.iter() {
                push_edit(&reference.location.uri, &reference.location.rename_range());
            }
        }

        edits
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        self.update_workspace_roots(&params);

        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "amble-lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions::default()),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Amble LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        self.analyze_document(&uri, &text);
        self.scan_directory(&uri).await;
        self.check_diagnostics(&uri).await;

        self.client
            .log_message(MessageType::INFO, format!("Opened document: {}", uri))
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;

        if let Some(change) = params.content_changes.into_iter().next() {
            self.analyze_document(&uri, &change.text);
            self.check_diagnostics(&uri).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;

        self.scan_directory(&uri).await;
        self.check_diagnostics(&uri).await;
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let uri_str = uri.to_string();

        if let Some(doc) = self.documents.get(&uri_str) {
            let current = doc.text().to_string();
            let range = doc.range();
            drop(doc);

            let formatted = formatter::format_document(&current);
            if formatted == current {
                return Ok(Some(vec![]));
            }

            return Ok(Some(vec![TextEdit {
                range,
                new_text: formatted,
            }]));
        }

        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some((symbol_type, id)) = self.get_symbol_at_position(&uri, position) {
            let index = self.symbols.index(symbol_type);
            if let Some(def) = index.definition(&id) {
                let value = format_hover(&id, &def);
                return Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value,
                    }),
                    range: None,
                }));
            }
        }

        Ok(None)
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;

        if new_name.is_empty() {
            return Ok(None);
        }

        if let Some((symbol_type, id)) = self.get_symbol_at_position(&uri, position) {
            let edits = self.collect_rename_edits(symbol_type, &id, &new_name);
            if edits.is_empty() {
                return Ok(Some(WorkspaceEdit::default()));
            }
            return Ok(Some(WorkspaceEdit {
                changes: Some(edits),
                ..WorkspaceEdit::default()
            }));
        }

        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some((symbol_type, symbol_id)) = self.get_symbol_at_position(&uri, position) {
            let index = self.symbols.index(symbol_type);
            if let Some(def) = index.definition(&symbol_id) {
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: def.location.uri.clone(),
                    range: def.location.range,
                })));
            }
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        if let Some((symbol_type, symbol_id)) = self.get_symbol_at_position(&uri, position) {
            let mut locations = Vec::new();
            let index = self.symbols.index(symbol_type);

            if params.context.include_declaration {
                if let Some(def) = index.definition(&symbol_id) {
                    locations.push(Location {
                        uri: def.location.uri.clone(),
                        range: def.location.range,
                    });
                }
            }

            if let Some(refs) = index.references(&symbol_id) {
                for reference in refs.value() {
                    locations.push(Location {
                        uri: reference.location.uri.clone(),
                        range: reference.location.range,
                    });
                }
            }

            return Ok(Some(locations));
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        if let Some(symbol_type) = self.get_completion_context(&uri, position) {
            let index = self.symbols.index(symbol_type);
            let mut items = Vec::new();
            let label = symbol_type.label();

            for entry in index.definitions_iter() {
                let id = entry.key();
                items.push(CompletionItem {
                    label: id.clone(),
                    kind: Some(CompletionItemKind::CONSTANT),
                    detail: Some(format!("{}: {}", label, id)),
                    documentation: Some(Documentation::String(format!(
                        "Defined in: {}",
                        entry.value().location.uri
                    ))),
                    ..Default::default()
                });
            }

            if !items.is_empty() {
                return Ok(Some(CompletionResponse::Array(items)));
            }
        }

        Ok(None)
    }
}
