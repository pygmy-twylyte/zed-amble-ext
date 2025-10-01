use dashmap::DashMap;
use std::sync::Arc;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};


#[derive(Debug, Clone, PartialEq)]
enum SymbolType {
    Room,
    Item,
    Npc,
}
#[derive(Debug, Clone)]
struct RoomDefinition {
    uri: Url,
    range: Range,
}

#[derive(Debug, Clone)]
struct RoomReference {
    uri: Url,
    range: Range,
}


#[derive(Debug, Clone)]
struct ItemDefinition {
    uri: Url,
    range: Range,
}

#[derive(Debug, Clone)]
struct ItemReference {
    uri: Url,
    range: Range,
}

#[derive(Debug, Clone)]
struct NpcDefinition {
    uri: Url,
    range: Range,
}

#[derive(Debug, Clone)]
struct NpcReference {
    uri: Url,
    range: Range,
}

struct Backend {
    client: Client,
    // Map from room_id -> definition location
    room_definitions: Arc<DashMap<String, RoomDefinition>>,
    // Map from room_id -> list of references
    room_references: Arc<DashMap<String, Vec<RoomReference>>>,
    // Map from item_id -> definition location
    item_definitions: Arc<DashMap<String, ItemDefinition>>,
    // Map from item_id -> list of references
    item_references: Arc<DashMap<String, Vec<ItemReference>>>,
    // Map from npc_id -> definition location
    npc_definitions: Arc<DashMap<String, NpcDefinition>>,
    // Map from npc_id -> list of references
    npc_references: Arc<DashMap<String, Vec<NpcReference>>>,
    // Map from URI -> document content
    document_map: Arc<DashMap<String, String>>,
    // Tree-sitter parser
    parser: Arc<parking_lot::Mutex<Parser>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_amble::language())
            .expect("Error loading Amble grammar");
        
        Self {
            client,
            room_definitions: Arc::new(DashMap::new()),
            room_references: Arc::new(DashMap::new()),
            item_definitions: Arc::new(DashMap::new()),
            item_references: Arc::new(DashMap::new()),
            npc_definitions: Arc::new(DashMap::new()),
            npc_references: Arc::new(DashMap::new()),
            document_map: Arc::new(DashMap::new()),
            parser: Arc::new(parking_lot::Mutex::new(parser)),
        }
    }

    async fn scan_directory(&self, uri: &Url) {
        if let Ok(path) = uri.to_file_path() {
            if let Some(dir) = path.parent() {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("amble") {
                            if let Ok(uri) = Url::from_file_path(&path) {
                                if let Ok(content) = std::fs::read_to_string(&path) {
                                    self.analyze_document(&uri, &content);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn analyze_document(&self, uri: &Url, text: &str) {
        
        // Parse the document with tree-sitter
        let tree = {
            let mut parser = self.parser.lock();
            match parser.parse(text, None) {
                Some(tree) => tree,
                None => {
                    return;
                }
            }
        };
        
        let root_node = tree.root_node();
        let uri_str = uri.to_string();
        
        self.room_definitions.retain(|_, def| def.uri != *uri);
        self.item_definitions.retain(|_, def| def.uri != *uri);
        self.npc_definitions.retain(|_, def| def.uri != *uri);
        // Clear old data for this document
        
        // Remove old references from this file
        for mut entry in self.room_references.iter_mut() {
            entry.value_mut().retain(|r| r.uri != *uri);
        }
        for mut entry in self.item_references.iter_mut() {
        for mut entry in self.npc_references.iter_mut() {
            entry.value_mut().retain(|r| r.uri != *uri);
        }
            entry.value_mut().retain(|r| r.uri != *uri);
        }
        
        let language = tree_sitter_amble::language();
        
        // Query for room definitions
        let def_query_source = r#"
(room_def
  room_id: (room_id) @room.definition)
"#;
        
        let def_query = Query::new(&language, def_query_source).expect("Bad definition query");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&def_query, root_node, text.as_bytes());
        
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let room_id = &text[node.byte_range()];
                
                // Convert tree-sitter position to LSP position
                let start_point = node.start_position();
                let end_point = node.end_position();
                
                let range = Range {
                    start: Position {
                        line: start_point.row as u32,
                        character: start_point.column as u32,
                    },
                    end: Position {
                        line: end_point.row as u32,
                        character: end_point.column as u32,
                    },
                };
                
                self.room_definitions.insert(
                    room_id.to_string(),
                    RoomDefinition {
                        uri: uri.clone(),
                        range,
                    },
                );
            }
        }
        
        // Query for room references
        let ref_query_source = r#"
(_room_ref
  (room_id) @room.reference)
"#;
        
        let ref_query = Query::new(&language, ref_query_source).expect("Bad reference query");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&ref_query, root_node, text.as_bytes());
        
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let room_id = &text[node.byte_range()];
                
                // Skip if this is the definition itself (room_id in room_def)
                // We can check if the parent is a room_def
                if let Some(parent) = node.parent() {
                    if parent.kind() == "room_def" {
                        continue;
                    }
                }
                
                // Convert tree-sitter position to LSP position
                let start_point = node.start_position();
                let end_point = node.end_position();
                
                let range = Range {
                    start: Position {
                        line: start_point.row as u32,
                        character: start_point.column as u32,
                    },
                    end: Position {
                        line: end_point.row as u32,
                        character: end_point.column as u32,
                    },
                };
                
                self.room_references
                    .entry(room_id.to_string())
                    .or_insert_with(Vec::new)
                    .push(RoomReference {
                        uri: uri.clone(),
                        range,
                    });
            }
        }
        

        // Query for item definitions
        let item_def_query_source = r#"
(item_def
  item_id: (item_id) @item.definition)
"#;
        
        let item_def_query = Query::new(&language, item_def_query_source).expect("Bad item definition query");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&item_def_query, root_node, text.as_bytes());
        
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let item_id = &text[node.byte_range()];
                
                let start_point = node.start_position();
                let end_point = node.end_position();
                
                let range = Range {
                    start: Position {
                        line: start_point.row as u32,
                        character: start_point.column as u32,
                    },
                    end: Position {
                        line: end_point.row as u32,
                        character: end_point.column as u32,
                    },
                };
                
                self.item_definitions.insert(
                    item_id.to_string(),
                    ItemDefinition {
                        uri: uri.clone(),
                        range,
                    },
                );
            }
        }
        
        // Query for item references
        let item_ref_query_source = r#"
(_item_ref
  (item_id) @item.reference)
"#;
        
        let item_ref_query = Query::new(&language, item_ref_query_source).expect("Bad item reference query");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&item_ref_query, root_node, text.as_bytes());
        
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let item_id = &text[node.byte_range()];
                
                // Skip if this is the definition itself
                if let Some(parent) = node.parent() {
                    if parent.kind() == "item_def" {
                        continue;
                    }
                }
                
                let start_point = node.start_position();
                let end_point = node.end_position();
                
                let range = Range {
                    start: Position {
                        line: start_point.row as u32,
                        character: start_point.column as u32,
                    },
                    end: Position {
                        line: end_point.row as u32,
                        character: end_point.column as u32,
                    },
                };
                
                self.item_references
                    .entry(item_id.to_string())
                    .or_insert_with(Vec::new)
                    .push(ItemReference {
                        uri: uri.clone(),
                        range,
                    });
            }
        }

        // Query for NPC definitions
        let npc_def_query_source = r#"
(npc_def
  npc_id: (npc_id) @npc.definition)
"#;
        
        let npc_def_query = Query::new(&language, npc_def_query_source).expect("Bad npc definition query");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&npc_def_query, root_node, text.as_bytes());
        
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let npc_id = &text[node.byte_range()];
                
                let start_point = node.start_position();
                let end_point = node.end_position();
                
                let range = Range {
                    start: Position {
                        line: start_point.row as u32,
                        character: start_point.column as u32,
                    },
                    end: Position {
                        line: end_point.row as u32,
                        character: end_point.column as u32,
                    },
                };
                
                self.npc_definitions.insert(
                    npc_id.to_string(),
                    NpcDefinition {
                        uri: uri.clone(),
                        range,
                    },
                );
            }
        }
        
        // Query for NPC references
        let npc_ref_query_source = r#"
(_npc_ref
  (npc_id) @npc.reference)
"#;
        
        let npc_ref_query = Query::new(&language, npc_ref_query_source).expect("Bad npc reference query");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&npc_ref_query, root_node, text.as_bytes());
        
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let npc_id = &text[node.byte_range()];
                
                // Skip if this is the definition itself
                if let Some(parent) = node.parent() {
                    if parent.kind() == "npc_def" {
                        continue;
                    }
                }
                
                let start_point = node.start_position();
                let end_point = node.end_position();
                
                let range = Range {
                    start: Position {
                        line: start_point.row as u32,
                        character: start_point.column as u32,
                    },
                    end: Position {
                        line: end_point.row as u32,
                        character: end_point.column as u32,
                    },
                };
                
                self.npc_references
                    .entry(npc_id.to_string())
                    .or_insert_with(Vec::new)
                    .push(NpcReference {
                        uri: uri.clone(),
                        range,
                    });
            }
        }
        // Store the document
        self.document_map.insert(uri_str, text.to_string());
    }

    }

    fn position_to_offset(text: &str, position: Position) -> Option<usize> {
        let mut current_line = 0;
        let mut current_char = 0;

        for (i, ch) in text.chars().enumerate() {
            if current_line == position.line && current_char == position.character {
                return Some(i);
            }

            if ch == '
' {
                current_line += 1;
                current_char = 0;
            } else {
                current_char += 1;
            }
        }

        if current_line == position.line && current_char == position.character {
            return Some(text.len());
        }

        None
    }

    fn get_symbol_at_position(&self, uri: &Url, position: Position) -> Option<(SymbolType, String)> {
        let uri_str = uri.to_string();
        let text = self.document_map.get(&uri_str)?;

        // Convert position to offset
        let offset = Self::position_to_offset(&text, position)?;

        // Check if we're on a room definition
        for entry in self.room_definitions.iter() {
            let def = entry.value();
            if def.uri == *uri {
                let start_offset = Self::position_to_offset(&text, def.range.start)?;
                let end_offset = Self::position_to_offset(&text, def.range.end)?;

                if offset >= start_offset && offset <= end_offset {
                    return Some((SymbolType::Room, entry.key().clone()));
                }
            }
        }

        // Check if we're on a room reference
        for entry in self.room_references.iter() {
            let room_id = entry.key();
            for reference in entry.value() {
                if reference.uri == *uri {
                    let start_offset = Self::position_to_offset(&text, reference.range.start)?;
                    let end_offset = Self::position_to_offset(&text, reference.range.end)?;

                    if offset >= start_offset && offset <= end_offset {
                        return Some((SymbolType::Room, room_id.clone()));
                    }
                }
            }
        }

        // Check if we're on an item definition
        for entry in self.item_definitions.iter() {
            let def = entry.value();
            if def.uri == *uri {
                let start_offset = Self::position_to_offset(&text, def.range.start)?;
                let end_offset = Self::position_to_offset(&text, def.range.end)?;

                if offset >= start_offset && offset <= end_offset {
                    return Some((SymbolType::Item, entry.key().clone()));
                }
            }
        }

        // Check if we're on an item reference
        for entry in self.item_references.iter() {
            let item_id = entry.key();
            for reference in entry.value() {
                if reference.uri == *uri {
                    let start_offset = Self::position_to_offset(&text, reference.range.start)?;
                    let end_offset = Self::position_to_offset(&text, reference.range.end)?;

                    if offset >= start_offset && offset <= end_offset {
                        return Some((SymbolType::Item, item_id.clone()));
                    }
                }
            }
        }


        // Check if we're on an NPC definition
        for entry in self.npc_definitions.iter() {
            let def = entry.value();
            if def.uri == *uri {
                let start_offset = Self::position_to_offset(&text, def.range.start)?;
                let end_offset = Self::position_to_offset(&text, def.range.end)?;

                if offset >= start_offset && offset <= end_offset {
                    return Some((SymbolType::Npc, entry.key().clone()));
                }
            }
        }

        // Check if we're on an NPC reference
        for entry in self.npc_references.iter() {
            let npc_id = entry.key();
            for reference in entry.value() {
                if reference.uri == *uri {
                    let start_offset = Self::position_to_offset(&text, reference.range.start)?;
                    let end_offset = Self::position_to_offset(&text, reference.range.end)?;

                    if offset >= start_offset && offset <= end_offset {
                        return Some((SymbolType::Npc, npc_id.clone()));
                    }
                }
            }
        }
        None
    }

    fn position_to_offset(text: &str, position: Position) -> Option<usize> {
        let mut current_line = 0;
        let mut current_char = 0;

        for (i, ch) in text.chars().enumerate() {
            if current_line == position.line && current_char == position.character {
                return Some(i);
            }

            if ch == '\n' {
                current_line += 1;
                current_char = 0;
            } else {
                current_char += 1;
            }
        }

        if current_line == position.line && current_char == position.character {
            return Some(text.len());
        }

        None
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
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

        // Analyze this document
        self.analyze_document(&uri, &text);

        // Scan the directory for other .amble files
        self.scan_directory(&uri).await;

        self.client
            .log_message(MessageType::INFO, format!("Opened document: {}", uri))
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;

        if let Some(change) = params.content_changes.into_iter().next() {
            self.analyze_document(&uri, &change.text);
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;

        // Re-scan the directory on save
        self.scan_directory(&uri).await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Get the symbol at the cursor position
        if let Some((symbol_type, symbol_id)) = self.get_symbol_at_position(&uri, position) {
            match symbol_type {
                SymbolType::Room => {
                    if let Some(def) = self.room_definitions.get(&symbol_id) {
                        return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                            uri: def.uri.clone(),
                            range: def.range,
                        })));
                    }
                }
                SymbolType::Item => {
                    if let Some(def) = self.item_definitions.get(&symbol_id) {
                        return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                            uri: def.uri.clone(),
                            range: def.range,
                        })));
                    }
                }
                SymbolType::Npc => {
                    if let Some(def) = self.npc_definitions.get(&symbol_id) {
                        return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                            uri: def.uri.clone(),
                            range: def.range,
                        })));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        // Get the symbol at the cursor position
        if let Some((symbol_type, symbol_id)) = self.get_symbol_at_position(&uri, position) {
            let mut locations = Vec::new();

            match symbol_type {
                SymbolType::Room => {
                    // Add the definition if requested
                    if params.context.include_declaration {
                        if let Some(def) = self.room_definitions.get(&symbol_id) {
                            locations.push(Location {
                                uri: def.uri.clone(),
                                range: def.range,
                            });
                        }
                    }

                    // Add all references
                    if let Some(refs) = self.room_references.get(&symbol_id) {
                        for reference in refs.value() {
                            locations.push(Location {
                                uri: reference.uri.clone(),
                                range: reference.range,
                            });
                        }
                    }
                }
                SymbolType::Item => {
                    // Add the definition if requested
                    if params.context.include_declaration {
                        if let Some(def) = self.item_definitions.get(&symbol_id) {
                            locations.push(Location {
                                uri: def.uri.clone(),
                                range: def.range,
                            });
                        }
                    }

                    // Add all references
                    if let Some(refs) = self.item_references.get(&symbol_id) {
                        for reference in refs.value() {
                            locations.push(Location {
                                uri: reference.uri.clone(),
                                range: reference.range,
                            });
                        }
                    }
                }
                SymbolType::Npc => {
                    // Add the definition if requested
                    if params.context.include_declaration {
                        if let Some(def) = self.npc_definitions.get(&symbol_id) {
                            locations.push(Location {
                                uri: def.uri.clone(),
                                range: def.range,
                            });
                        }
                    }

                    // Add all references
                    if let Some(refs) = self.npc_references.get(&symbol_id) {
                        for reference in refs.value() {
                            locations.push(Location {
                                uri: reference.uri.clone(),
                                range: reference.range,
                            });
                        }
                }
                }
            }

            return Ok(Some(locations));
        }

        Ok(None)
    }

}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend::new(client));
    Server::new(stdin, stdout, socket).serve(service).await;
}
