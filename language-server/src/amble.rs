use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

#[derive(Debug, Clone, Copy, PartialEq)]
enum SymbolType {
    Room,
    Item,
    Npc,
    Flag,
    Set,
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

#[derive(Debug, Clone)]
struct FlagDefinition {
    uri: Url,
    range: Range,
}

#[derive(Debug, Clone)]
struct FlagReference {
    uri: Url,
    range: Range,
}

#[derive(Debug, Clone)]
struct SetDefinition {
    uri: Url,
    range: Range,
}

#[derive(Debug, Clone)]
struct SetReference {
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
    // Map from flag_name -> definition location
    flag_definitions: Arc<DashMap<String, FlagDefinition>>,
    // Map from flag_name -> list of references
    flag_references: Arc<DashMap<String, Vec<FlagReference>>>,
    // Map from set_name -> definition location
    set_definitions: Arc<DashMap<String, SetDefinition>>,
    // Map from set_name -> list of references
    set_references: Arc<DashMap<String, Vec<SetReference>>>,
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
            flag_definitions: Arc::new(DashMap::new()),
            flag_references: Arc::new(DashMap::new()),
            set_definitions: Arc::new(DashMap::new()),
            set_references: Arc::new(DashMap::new()),
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
                                let uri_str = uri.to_string();
                                // Skip if already analyzed (e.g., from did_open)
                                if self.document_map.contains_key(&uri_str) {
                                    continue;
                                }
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
        self.flag_definitions.retain(|_, def| def.uri != *uri);
        self.set_definitions.retain(|_, def| def.uri != *uri);
        // Clear old data for this document

        // Remove old references from this file
        for mut entry in self.room_references.iter_mut() {
            entry.value_mut().retain(|r| r.uri != *uri);
        }
        for mut entry in self.item_references.iter_mut() {
            entry.value_mut().retain(|r| r.uri != *uri);
        }
        for mut entry in self.npc_references.iter_mut() {
            entry.value_mut().retain(|r| r.uri != *uri);
        }
        for mut entry in self.flag_references.iter_mut() {
            entry.value_mut().retain(|r| r.uri != *uri);
        }
        for mut entry in self.set_references.iter_mut() {
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

        let item_def_query =
            Query::new(&language, item_def_query_source).expect("Bad item definition query");
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

        let item_ref_query =
            Query::new(&language, item_ref_query_source).expect("Bad item reference query");
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

        let npc_def_query =
            Query::new(&language, npc_def_query_source).expect("Bad npc definition query");
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

        let npc_ref_query =
            Query::new(&language, npc_ref_query_source).expect("Bad npc reference query");
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

        // Query for flag definitions (both regular and sequence flags)
        let flag_def_query_source = r#"
[
  (action_add_flag
    flag: (flag_name) @flag.definition)
  (action_add_seq
    flag_name: (flag_name) @flag.definition)
]
"#;

        let flag_def_query =
            Query::new(&language, flag_def_query_source).expect("Bad flag definition query");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&flag_def_query, root_node, text.as_bytes());

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let flag_name = &text[node.byte_range()];

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

                self.flag_definitions.insert(
                    flag_name.to_string(),
                    FlagDefinition {
                        uri: uri.clone(),
                        range,
                    },
                );
            }
        }

        // Query for flag references
        let flag_ref_query_source = r#"
(_flag_ref) @flag.reference
"#;

        let flag_ref_query =
            Query::new(&language, flag_ref_query_source).expect("Bad flag reference query");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&flag_ref_query, root_node, text.as_bytes());

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let flag_name = &text[node.byte_range()];

                // Skip if this is the definition itself
                if let Some(parent) = node.parent() {
                    if parent.kind() == "action_add_flag" || parent.kind() == "action_add_seq" {
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

                self.flag_references
                    .entry(flag_name.to_string())
                    .or_insert_with(Vec::new)
                    .push(FlagReference {
                        uri: uri.clone(),
                        range,
                    });
            }
        }

        // Query for set definitions
        let set_def_query_source = r#"
(set_decl
  name: (set_name) @set.definition)
"#;

        let set_def_query =
            Query::new(&language, set_def_query_source).expect("Bad set definition query");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&set_def_query, root_node, text.as_bytes());

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let set_name = &text[node.byte_range()];

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

                self.set_definitions.insert(
                    set_name.to_string(),
                    SetDefinition {
                        uri: uri.clone(),
                        range,
                    },
                );
            }
        }

        // Query for set references
        let set_ref_query_source = r#"
(set_name) @set.reference
"#;

        let set_ref_query =
            Query::new(&language, set_ref_query_source).expect("Bad set reference query");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&set_ref_query, root_node, text.as_bytes());

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let set_name = &text[node.byte_range()];

                // Skip if this is the definition itself
                if let Some(parent) = node.parent() {
                    if parent.kind() == "set_decl" {
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

                self.set_references
                    .entry(set_name.to_string())
                    .or_insert_with(Vec::new)
                    .push(SetReference {
                        uri: uri.clone(),
                        range,
                    });
            }
        }
        // Store the document
        self.document_map.insert(uri_str, text.to_string());
    }

    fn position_to_offset(text: &str, position: Position) -> Option<usize> {
        let mut current_line = 0u32;
        let mut current_char = 0u32;

        for (byte_idx, ch) in text.char_indices() {
            if current_line == position.line && current_char == position.character {
                return Some(byte_idx);
            }

            if ch == '\n' {
                current_line += 1;
                current_char = 0;
                if current_line > position.line {
                    break;
                }
            } else {
                current_char += 1;
            }
        }

        if current_line == position.line && current_char == position.character {
            return Some(text.len());
        }

        None
    }

    fn get_symbol_at_position(
        &self,
        uri: &Url,
        position: Position,
    ) -> Option<(SymbolType, String)> {
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

        // Check if we're on a flag definition
        for entry in self.flag_definitions.iter() {
            let def = entry.value();
            if def.uri == *uri {
                let start_offset = Self::position_to_offset(&text, def.range.start)?;
                let end_offset = Self::position_to_offset(&text, def.range.end)?;

                if offset >= start_offset && offset <= end_offset {
                    return Some((SymbolType::Flag, entry.key().clone()));
                }
            }
        }

        // Check if we're on a flag reference
        for entry in self.flag_references.iter() {
            let flag_name = entry.key();
            for reference in entry.value() {
                if reference.uri == *uri {
                    let start_offset = Self::position_to_offset(&text, reference.range.start)?;
                    let end_offset = Self::position_to_offset(&text, reference.range.end)?;

                    if offset >= start_offset && offset <= end_offset {
                        return Some((SymbolType::Flag, flag_name.clone()));
                    }
                }
            }
        }

        // Check if we're on a set definition
        for entry in self.set_definitions.iter() {
            let def = entry.value();
            if def.uri == *uri {
                let start_offset = Self::position_to_offset(&text, def.range.start)?;
                let end_offset = Self::position_to_offset(&text, def.range.end)?;

                if offset >= start_offset && offset <= end_offset {
                    return Some((SymbolType::Set, entry.key().clone()));
                }
            }
        }

        // Check if we're on a set reference
        for entry in self.set_references.iter() {
            let set_name = entry.key();
            for reference in entry.value() {
                if reference.uri == *uri {
                    let start_offset = Self::position_to_offset(&text, reference.range.start)?;
                    let end_offset = Self::position_to_offset(&text, reference.range.end)?;

                    if offset >= start_offset && offset <= end_offset {
                        return Some((SymbolType::Set, set_name.clone()));
                    }
                }
            }
        }
        None
    }

    fn node_at_offset<'tree>(root: &Node<'tree>, offset: usize) -> Option<Node<'tree>> {
        if offset > root.end_byte() {
            return None;
        }

        let mut cursor = root.walk();
        if cursor.goto_first_child_for_byte(offset).is_none() {
            return Some(root.clone());
        }

        loop {
            let node = cursor.node();
            if cursor.goto_first_child_for_byte(offset).is_none() {
                return Some(node);
            }
        }
    }

    fn field_name_for_child<'tree>(
        parent: &Node<'tree>,
        child: &Node<'tree>,
    ) -> Option<&'static str> {
        for i in 0..parent.child_count() {
            if let Some(candidate) = parent.child(i) {
                if candidate.id() == child.id() {
                    return parent.field_name_for_child(i as u32);
                }
            }
        }
        None
    }

    fn symbol_type_from_kind(kind: &str) -> Option<SymbolType> {
        match kind {
            "room_id" | "_room_ref" => Some(SymbolType::Room),
            "item_id" | "_item_ref" => Some(SymbolType::Item),
            "npc_id" | "_npc_ref" => Some(SymbolType::Npc),
            "flag_name" | "_flag_ref" => Some(SymbolType::Flag),
            "set_name" | "_set_ref" => Some(SymbolType::Set),
            _ => None,
        }
    }

    fn symbol_type_from_field(field_name: &str) -> Option<SymbolType> {
        match field_name {
            "room_id" | "dest" | "room" | "from_room" | "to_room" => Some(SymbolType::Room),
            "item_id" | "tool_id" | "target_id" | "container_id" | "chest_id" => {
                Some(SymbolType::Item)
            }
            "npc_id" => Some(SymbolType::Npc),
            "flag_name" | "flag" => Some(SymbolType::Flag),
            "set_name" => Some(SymbolType::Set),
            _ => None,
        }
    }

    fn is_definition_node<'tree>(node: &Node<'tree>, symbol_type: SymbolType) -> bool {
        let mut current = node.parent();
        while let Some(parent) = current {
            let kind = parent.kind();
            let is_definition = match symbol_type {
                SymbolType::Room => kind == "room_def",
                SymbolType::Item => kind == "item_def",
                SymbolType::Npc => kind == "npc_def",
                SymbolType::Flag => kind == "action_add_flag" || kind == "action_add_seq",
                SymbolType::Set => kind == "set_decl",
            };

            if is_definition {
                return true;
            }

            current = parent.parent();
        }

        false
    }

    fn is_definition_field(parent_kind: &str, field_name: &str, symbol_type: SymbolType) -> bool {
        match symbol_type {
            SymbolType::Room => parent_kind == "room_def" && field_name == "room_id",
            SymbolType::Item => parent_kind == "item_def" && field_name == "item_id",
            SymbolType::Npc => parent_kind == "npc_def" && field_name == "npc_id",
            SymbolType::Flag => {
                (parent_kind == "action_add_flag" && field_name == "flag")
                    || (parent_kind == "action_add_seq" && field_name == "flag_name")
            }
            SymbolType::Set => parent_kind == "set_decl" && field_name == "name",
        }
    }

    fn symbol_type_from_children<'tree>(
        node: &Node<'tree>,
        offset: usize,
        stack: &mut Vec<Node<'tree>>,
    ) -> Option<SymbolType> {
        for i in 0..node.child_count() {
            let child = match node.child(i) {
                Some(child) => child,
                None => continue,
            };

            let start = child.start_byte();
            let end = child.end_byte();
            let field_name = node.field_name_for_child(i as u32);

            let in_range = if child.is_missing() {
                offset >= node.start_byte() && offset <= node.end_byte()
            } else if start == end {
                offset == start
            } else {
                offset >= start && offset <= end
            };

            if !in_range {
                continue;
            }

            if let Some(field_name) = field_name {
                if let Some(symbol_type) = Self::symbol_type_from_field(field_name) {
                    if Self::is_definition_field(node.kind(), field_name, symbol_type) {
                        continue;
                    }
                    return Some(symbol_type);
                }
            } else if in_range && child.is_named() {
                stack.push(child);
            }
        }

        None
    }

    fn symbol_type_from_syntax<'tree>(node: Node<'tree>, offset: usize) -> Option<SymbolType> {
        let mut stack = vec![node];
        let mut visited = HashSet::new();

        while let Some(n) = stack.pop() {
            if !visited.insert(n.id()) {
                continue;
            }

            let parent = n.parent();

            if let Some(symbol_type) = Self::symbol_type_from_kind(n.kind()) {
                if Self::is_definition_node(&n, symbol_type) {
                    if let Some(parent) = parent {
                        stack.push(parent);
                    }
                    continue;
                }

                let blocked = parent.as_ref().and_then(|p| {
                    Self::field_name_for_child(p, &n)
                        .map(|field| Self::is_definition_field(p.kind(), field, symbol_type))
                });

                if !blocked.unwrap_or(false) {
                    return Some(symbol_type);
                }
            }

            if let Some(symbol_type) = Self::symbol_type_from_children(&n, offset, &mut stack) {
                return Some(symbol_type);
            }

            if let Some(parent) = parent {
                stack.push(parent);
            }
        }

        None
    }

    fn get_completion_context(&self, uri: &Url, position: Position) -> Option<SymbolType> {
        let uri_str = uri.to_string();
        let text = self.document_map.get(&uri_str)?;

        // Parse the document
        let tree = {
            let mut parser = self.parser.lock();
            parser.parse(text.as_str(), None)?
        };

        let root_node = tree.root_node();

        // Convert position to byte offset
        let offset = Self::position_to_offset(&text, position)?;

        let mut candidate_offsets = vec![offset];
        if offset > 0 {
            candidate_offsets.push(offset - 1);
        }

        for candidate in candidate_offsets {
            if let Some(node) = Self::node_at_offset(&root_node, candidate) {
                if let Some(symbol_type) = Self::symbol_type_from_syntax(node, candidate) {
                    return Some(symbol_type);
                }
            }
        }

        None
    }

    async fn check_diagnostics(&self, uri: &Url) {
        let uri_str = uri.to_string();
        // Only check diagnostics if document is loaded
        if !self.document_map.contains_key(&uri_str) {
            return;
        }

        let mut diagnostics = Vec::new();

        // Check room references
        for entry in self.room_references.iter() {
            let room_id = entry.key();
            // Allow set names in place of room references (for "in rooms <set>" context)
            if !self.room_definitions.contains_key(room_id)
                && !self.set_definitions.contains_key(room_id)
            {
                // Undefined room reference (and not a valid set either)
                for reference in entry.value() {
                    if reference.uri == *uri {
                        diagnostics.push(Diagnostic {
                            range: reference.range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: None,
                            code_description: None,
                            source: Some("amble-lsp".to_string()),
                            message: format!("Undefined room: '{}'", room_id),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                    }
                }
            }
        }

        // Check item references
        for entry in self.item_references.iter() {
            let item_id = entry.key();
            if !self.item_definitions.contains_key(item_id) {
                // Undefined item reference
                for reference in entry.value() {
                    if reference.uri == *uri {
                        diagnostics.push(Diagnostic {
                            range: reference.range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: None,
                            code_description: None,
                            source: Some("amble-lsp".to_string()),
                            message: format!("Undefined item: '{}'", item_id),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                    }
                }
            }
        }

        // Check NPC references
        for entry in self.npc_references.iter() {
            let npc_id = entry.key();
            if !self.npc_definitions.contains_key(npc_id) {
                // Undefined NPC reference
                for reference in entry.value() {
                    if reference.uri == *uri {
                        diagnostics.push(Diagnostic {
                            range: reference.range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: None,
                            code_description: None,
                            source: Some("amble-lsp".to_string()),
                            message: format!("Undefined NPC: '{}'", npc_id),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                    }
                }
            }
        }

        // Check flag references
        for entry in self.flag_references.iter() {
            let flag_name = entry.key();

            // Strip step number from sequence flags (e.g., "hal-reboot#3" -> "hal-reboot")
            let base_flag_name = flag_name.split('#').next().unwrap_or(flag_name);

            if !self.flag_definitions.contains_key(base_flag_name) {
                // Undefined flag reference
                for reference in entry.value() {
                    if reference.uri == *uri {
                        diagnostics.push(Diagnostic {
                            range: reference.range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: None,
                            code_description: None,
                            source: Some("amble-lsp".to_string()),
                            message: format!("Undefined flag: '{}'", flag_name),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                    }
                }
            }
        }

        // Check set references
        for entry in self.set_references.iter() {
            let set_name = entry.key();
            if !self.set_definitions.contains_key(set_name) {
                // Undefined set reference
                for reference in entry.value() {
                    if reference.uri == *uri {
                        diagnostics.push(Diagnostic {
                            range: reference.range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: None,
                            code_description: None,
                            source: Some("amble-lsp".to_string()),
                            message: format!("Undefined set: '{}'", set_name),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                    }
                }
            }
        }

        // Publish diagnostics
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    fn parse_source(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_amble::language())
            .expect("load amble grammar");
        parser.parse(source, None).expect("parse source")
    }

    fn completion_at(source: &str, position: Position) -> Option<SymbolType> {
        let tree = parse_source(source);
        let root = tree.root_node();
        let offset = Backend::position_to_offset(source, position).unwrap();

        let mut candidates = vec![offset];
        if offset > 0 {
            candidates.push(offset - 1);
        }

        for candidate in candidates {
            if let Some(node) = Backend::node_at_offset(&root, candidate) {
                if let Some(symbol) = Backend::symbol_type_from_syntax(node, candidate) {
                    return Some(symbol);
                }
            }
        }

        None
    }

    #[test]
    fn detects_room_reference_context_in_exits() {
        let source = "room a {\n    exit north -> \n}\n";
        let symbol = completion_at(
            source,
            Position {
                line: 1,
                character: 17,
            },
        );
        assert_eq!(symbol, Some(SymbolType::Room));
    }

    #[test]
    fn skips_completion_inside_definitions() {
        let source = "room test-room {\n}\n";
        let position = Position {
            line: 0,
            character: 9,
        };
        let symbol = completion_at(source, position);
        assert_eq!(symbol, None);
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
                completion_provider: Some(CompletionOptions::default()),
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

        // Check for diagnostics
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

        // Re-scan the directory on save
        self.scan_directory(&uri).await;

        // Check for diagnostics after re-scanning
        self.check_diagnostics(&uri).await;
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
                SymbolType::Flag => {
                    // Strip step number from sequence flags (e.g., "hal-reboot#2" -> "hal-reboot")
                    let base_flag_name = symbol_id.split('#').next().unwrap_or(&symbol_id);
                    if let Some(def) = self.flag_definitions.get(base_flag_name) {
                        return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                            uri: def.uri.clone(),
                            range: def.range,
                        })));
                    }
                }
                SymbolType::Set => {
                    if let Some(def) = self.set_definitions.get(&symbol_id) {
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
                SymbolType::Flag => {
                    // Strip step number from sequence flags (e.g., "hal-reboot#2" -> "hal-reboot")
                    let base_flag_name = symbol_id.split('#').next().unwrap_or(&symbol_id);

                    // Add the definition if requested
                    if params.context.include_declaration {
                        if let Some(def) = self.flag_definitions.get(base_flag_name) {
                            locations.push(Location {
                                uri: def.uri.clone(),
                                range: def.range,
                            });
                        }
                    }

                    // Add all references
                    if let Some(refs) = self.flag_references.get(base_flag_name) {
                        for reference in refs.value() {
                            locations.push(Location {
                                uri: reference.uri.clone(),
                                range: reference.range,
                            });
                        }
                    }
                }
                SymbolType::Set => {
                    // Add the definition if requested
                    if params.context.include_declaration {
                        if let Some(def) = self.set_definitions.get(&symbol_id) {
                            locations.push(Location {
                                uri: def.uri.clone(),
                                range: def.range,
                            });
                        }
                    }

                    // Add all references
                    if let Some(refs) = self.set_references.get(&symbol_id) {
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

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        // Determine what type of symbol is expected at this position
        if let Some(symbol_type) = self.get_completion_context(&uri, position) {
            let mut items = Vec::new();

            match symbol_type {
                SymbolType::Room => {
                    // Add all room definitions as completion items
                    for entry in self.room_definitions.iter() {
                        let room_id = entry.key();
                        items.push(CompletionItem {
                            label: room_id.clone(),
                            kind: Some(CompletionItemKind::CONSTANT),
                            detail: Some(format!("Room: {}", room_id)),
                            documentation: Some(Documentation::String(format!(
                                "Defined in: {}",
                                entry.value().uri
                            ))),
                            ..Default::default()
                        });
                    }
                }
                SymbolType::Item => {
                    // Add all item definitions as completion items
                    for entry in self.item_definitions.iter() {
                        let item_id = entry.key();
                        items.push(CompletionItem {
                            label: item_id.clone(),
                            kind: Some(CompletionItemKind::CONSTANT),
                            detail: Some(format!("Item: {}", item_id)),
                            documentation: Some(Documentation::String(format!(
                                "Defined in: {}",
                                entry.value().uri
                            ))),
                            ..Default::default()
                        });
                    }
                }
                SymbolType::Npc => {
                    // Add all NPC definitions as completion items
                    for entry in self.npc_definitions.iter() {
                        let npc_id = entry.key();
                        items.push(CompletionItem {
                            label: npc_id.clone(),
                            kind: Some(CompletionItemKind::CONSTANT),
                            detail: Some(format!("NPC: {}", npc_id)),
                            documentation: Some(Documentation::String(format!(
                                "Defined in: {}",
                                entry.value().uri
                            ))),
                            ..Default::default()
                        });
                    }
                }
                SymbolType::Flag => {
                    // Add all flag definitions as completion items
                    for entry in self.flag_definitions.iter() {
                        let flag_name = entry.key();
                        items.push(CompletionItem {
                            label: flag_name.clone(),
                            kind: Some(CompletionItemKind::CONSTANT),
                            detail: Some(format!("Flag: {}", flag_name)),
                            documentation: Some(Documentation::String(format!(
                                "Defined in: {}",
                                entry.value().uri
                            ))),
                            ..Default::default()
                        });
                    }
                }
                SymbolType::Set => {
                    // Add all set definitions as completion items
                    for entry in self.set_definitions.iter() {
                        let set_name = entry.key();
                        items.push(CompletionItem {
                            label: set_name.clone(),
                            kind: Some(CompletionItemKind::CONSTANT),
                            detail: Some(format!("Set: {}", set_name)),
                            documentation: Some(Documentation::String(format!(
                                "Defined in: {}",
                                entry.value().uri
                            ))),
                            ..Default::default()
                        });
                    }
                }
            }

            if !items.is_empty() {
                return Ok(Some(CompletionResponse::Array(items)));
            }
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
