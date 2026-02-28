use crate::analysis::{format_hover, PlayerStart};
use crate::formatter;
use crate::queries::Queries;
use crate::symbols::{SymbolDefinition, SymbolIndex, SymbolKind, SymbolMetadata, SymbolStore};
use crate::text::DocumentStore;
use dashmap::DashMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use tree_sitter::Parser;

const COMPLETION_DETAIL_MAX_CHARS: usize = 80;

pub struct Backend {
    pub(crate) client: Client,
    pub(crate) symbols: Arc<SymbolStore>,
    pub(crate) documents: Arc<DocumentStore>,
    pub(crate) document_symbols: Arc<DashMap<String, Vec<crate::symbols::SymbolOccurrence>>>,
    pub(crate) workspace_roots: Arc<parking_lot::RwLock<Vec<PathBuf>>>,
    pub(crate) parser: Arc<parking_lot::Mutex<Parser>>,
    pub(crate) queries: Arc<Queries>,
    pub(crate) scanned_directories: Arc<DashMap<PathBuf, Option<SystemTime>>>,
    /// Cached `player_start` nodes per document; used for workspace-level diagnostics.
    pub(crate) player_starts: Arc<DashMap<String, Vec<PlayerStart>>>,
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
            scanned_directories: Arc::new(DashMap::new()),
            player_starts: Arc::new(DashMap::new()),
        }
    }

    fn completion_item_from_definition(
        &self,
        kind: SymbolKind,
        id: &str,
        definition: &SymbolDefinition,
    ) -> CompletionItem {
        let path_hint = self.definition_display_path(&definition.location.uri);
        let documentation = format_hover(id, definition, path_hint.as_deref());
        CompletionItem {
            label: id.to_string(),
            kind: Some(completion_item_kind(kind)),
            detail: truncate_completion_detail(definition_detail(definition)),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: documentation,
            })),
            sort_text: Some(id.to_string()),
            ..Default::default()
        }
    }

    fn collect_document_symbols(&self, uri: &Url) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();
        self.push_document_symbols_for_index(
            uri,
            SymbolKind::Room,
            &self.symbols.rooms,
            &mut symbols,
        );
        self.push_document_symbols_for_index(
            uri,
            SymbolKind::Item,
            &self.symbols.items,
            &mut symbols,
        );
        self.push_document_symbols_for_index(
            uri,
            SymbolKind::Npc,
            &self.symbols.npcs,
            &mut symbols,
        );
        self.push_document_symbols_for_index(
            uri,
            SymbolKind::Flag,
            &self.symbols.flags,
            &mut symbols,
        );
        self.push_document_symbols_for_index(
            uri,
            SymbolKind::Set,
            &self.symbols.sets,
            &mut symbols,
        );
        symbols
    }

    fn push_document_symbols_for_index(
        &self,
        uri: &Url,
        kind: SymbolKind,
        index: &SymbolIndex,
        output: &mut Vec<DocumentSymbol>,
    ) {
        for entry in index.definitions_iter() {
            if entry.value().location.uri == *uri {
                let name = entry.key().clone();
                let definition = entry.value().clone();
                output.push(document_symbol_from_definition(&name, kind, &definition));
            }
        }
    }

    fn collect_workspace_symbols(&self, query: &str) -> Vec<SymbolInformation> {
        let mut symbols = Vec::new();
        self.push_workspace_symbols_for_index(
            query,
            SymbolKind::Room,
            &self.symbols.rooms,
            &mut symbols,
        );
        self.push_workspace_symbols_for_index(
            query,
            SymbolKind::Item,
            &self.symbols.items,
            &mut symbols,
        );
        self.push_workspace_symbols_for_index(
            query,
            SymbolKind::Npc,
            &self.symbols.npcs,
            &mut symbols,
        );
        self.push_workspace_symbols_for_index(
            query,
            SymbolKind::Flag,
            &self.symbols.flags,
            &mut symbols,
        );
        self.push_workspace_symbols_for_index(
            query,
            SymbolKind::Set,
            &self.symbols.sets,
            &mut symbols,
        );
        symbols
    }

    fn push_workspace_symbols_for_index(
        &self,
        query: &str,
        kind: SymbolKind,
        index: &SymbolIndex,
        output: &mut Vec<SymbolInformation>,
    ) {
        for entry in index.definitions_iter() {
            let name = entry.key().clone();
            let definition = entry.value().clone();
            if query_matches_symbol(&name, definition_detail(&definition).as_deref(), query) {
                output.push(workspace_symbol_from_definition(&name, kind, &definition));
            }
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

    fn rename_range_for_occurrence(
        &self,
        uri: &Url,
        position: Position,
        kind: SymbolKind,
        id: &str,
        fallback: Range,
    ) -> Range {
        let index = self.symbols.index(kind);

        if let Some(def) = index.definition(id) {
            if def.location.uri == *uri && range_contains(&def.location.range, position) {
                return def.location.rename_range();
            }
        }

        if let Some(refs) = index.references(id) {
            for reference in refs.iter() {
                if reference.location.uri == *uri
                    && range_contains(&reference.location.range, position)
                {
                    return reference.location.rename_range();
                }
            }
        }

        fallback
    }

    fn definition_display_path(&self, uri: &Url) -> Option<String> {
        let file_path = uri.to_file_path().ok()?;
        let mut best_match: Option<(usize, PathBuf)> = None;
        {
            let roots = self.workspace_roots.read();
            for root in roots.iter() {
                if file_path.starts_with(root) {
                    let depth = root.components().count();
                    let replace = best_match
                        .as_ref()
                        .map(|(best_depth, _)| depth > *best_depth)
                        .unwrap_or(true);
                    if replace {
                        best_match = Some((depth, root.clone()));
                    }
                }
            }
        }

        if let Some((_, root)) = best_match {
            if let Ok(relative) = file_path.strip_prefix(root) {
                let mut rel = relative.to_string_lossy().replace('\\', "/");
                if rel.starts_with('/') {
                    rel = rel.trim_start_matches('/').to_string();
                }
                if rel.is_empty() {
                    rel = file_path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| file_path.to_string_lossy().into_owned());
                }
                return Some(rel);
            }
        }

        Some(file_path.to_string_lossy().replace('\\', "/"))
    }
}

fn document_symbol_from_definition(
    name: &str,
    kind: SymbolKind,
    definition: &SymbolDefinition,
) -> DocumentSymbol {
    #[allow(deprecated)]
    DocumentSymbol {
        name: name.to_string(),
        detail: definition_detail(definition),
        kind: lsp_symbol_kind(kind),
        tags: None,
        deprecated: None,
        range: definition.location.range,
        selection_range: definition.location.rename_range(),
        children: None,
    }
}

fn workspace_symbol_from_definition(
    name: &str,
    kind: SymbolKind,
    definition: &SymbolDefinition,
) -> SymbolInformation {
    #[allow(deprecated)]
    SymbolInformation {
        name: name.to_string(),
        kind: lsp_symbol_kind(kind),
        tags: None,
        deprecated: None,
        location: Location {
            uri: definition.location.uri.clone(),
            range: definition.location.range,
        },
        container_name: None,
    }
}

fn definition_detail(definition: &SymbolDefinition) -> Option<String> {
    match &definition.metadata {
        SymbolMetadata::Room(meta) => meta.name.clone().or_else(|| meta.description.clone()),
        SymbolMetadata::Item(meta) => meta
            .name
            .clone()
            .or_else(|| meta.location.clone())
            .or_else(|| meta.description.clone()),
        SymbolMetadata::Npc(meta) => meta
            .name
            .clone()
            .or_else(|| meta.location.clone())
            .or_else(|| meta.description.clone()),
        SymbolMetadata::Flag(meta) => meta.defined_in.clone(),
        SymbolMetadata::Set(meta) => {
            if meta.rooms.is_empty() {
                Some("No rooms assigned".to_string())
            } else {
                Some(format!("Rooms: {}", meta.rooms.join(", ")))
            }
        }
    }
}

fn lsp_symbol_kind(kind: SymbolKind) -> tower_lsp::lsp_types::SymbolKind {
    match kind {
        SymbolKind::Room => tower_lsp::lsp_types::SymbolKind::CLASS,
        SymbolKind::Item => tower_lsp::lsp_types::SymbolKind::STRUCT,
        SymbolKind::Npc => tower_lsp::lsp_types::SymbolKind::INTERFACE,
        SymbolKind::Flag => tower_lsp::lsp_types::SymbolKind::ENUM_MEMBER,
        SymbolKind::Set => tower_lsp::lsp_types::SymbolKind::NAMESPACE,
    }
}

fn completion_item_kind(kind: SymbolKind) -> CompletionItemKind {
    match kind {
        SymbolKind::Room => CompletionItemKind::CLASS,
        SymbolKind::Item => CompletionItemKind::STRUCT,
        SymbolKind::Npc => CompletionItemKind::FIELD,
        SymbolKind::Flag => CompletionItemKind::ENUM_MEMBER,
        SymbolKind::Set => CompletionItemKind::MODULE,
    }
}

fn truncate_completion_detail(detail: Option<String>) -> Option<String> {
    detail.map(|value| {
        let chars = value.chars();
        if chars.clone().count() <= COMPLETION_DETAIL_MAX_CHARS {
            return value;
        }
        let truncated: String = chars.take(COMPLETION_DETAIL_MAX_CHARS).collect();
        format!("{}...", truncated)
    })
}

fn query_matches_symbol(name: &str, detail: Option<&str>, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let name_match = name.to_lowercase().contains(query);
    let detail_match = detail
        .map(|value| value.to_lowercase().contains(query))
        .unwrap_or(false);
    name_match || detail_match
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
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions::default()),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                })),
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

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let symbols = self.collect_document_symbols(&uri);
        if symbols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(DocumentSymbolResponse::Nested(symbols)))
        }
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query = params.query.to_lowercase();
        let symbols = self.collect_workspace_symbols(&query);
        if symbols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(symbols))
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some((symbol_type, id)) = self.get_symbol_at_position(&uri, position) {
            let index = self.symbols.index(symbol_type);
            if let Some(def) = index.definition(&id) {
                let path_hint = self.definition_display_path(&def.location.uri);
                let value = format_hover(&id, &def, path_hint.as_deref());
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

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;

        if let Some(occurrence) = self.get_symbol_occurrence_at_position(&uri, position) {
            let range = self.rename_range_for_occurrence(
                &uri,
                position,
                occurrence.kind,
                &occurrence.id,
                occurrence.range,
            );
            return Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
                range,
                placeholder: occurrence.id,
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

            for entry in index.definitions_iter() {
                let id = entry.key().clone();
                let definition = entry.value().clone();
                items.push(self.completion_item_from_definition(symbol_type, &id, &definition));
            }

            if !items.is_empty() {
                return Ok(Some(CompletionResponse::Array(items)));
            }
        }

        Ok(None)
    }
}

fn range_contains(range: &Range, position: Position) -> bool {
    if position.line < range.start.line || position.line > range.end.line {
        return false;
    }
    if position.line == range.start.line && position.character < range.start.character {
        return false;
    }
    if position.line == range.end.line && position.character > range.end.character {
        return false;
    }
    true
}
