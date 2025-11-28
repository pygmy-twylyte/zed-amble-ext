use dashmap::DashMap;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};
use walkdir::WalkDir;

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
    name: Option<String>,
    description: Option<String>,
    exits: Vec<String>,
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
    name: Option<String>,
    description: Option<String>,
    portable: Option<bool>,
    location: Option<String>,
    container_state: Option<String>,
    abilities: Vec<String>,
    requirements: Vec<String>,
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
    name: Option<String>,
    description: Option<String>,
    location: Option<String>,
    state: Option<String>,
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
    defined_in: Option<String>,
    sequence_limit: Option<i64>,
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
    rooms: Vec<String>,
}

#[derive(Debug, Clone)]
struct SetReference {
    uri: Url,
    range: Range,
}

#[derive(Clone, Copy)]
struct BraceEvent {
    line: usize,
    column: usize,
    kind: BraceKind,
}

#[derive(Clone, Copy)]
enum BraceKind {
    Open,
    Close,
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
    // Workspace roots provided by the editor
    workspace_roots: Arc<parking_lot::RwLock<Vec<PathBuf>>>,
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
            workspace_roots: Arc::new(parking_lot::RwLock::new(Vec::new())),
            parser: Arc::new(parking_lot::Mutex::new(parser)),
        }
    }

    fn update_workspace_roots(&self, params: &InitializeParams) {
        let mut roots = self.workspace_roots.write();
        roots.clear();

        if let Some(root_uri) = params.root_uri.as_ref() {
            if let Ok(path) = root_uri.to_file_path() {
                if !roots.iter().any(|existing| existing == &path) {
                    roots.push(path);
                }
            }
        }

        #[allow(deprecated)]
        if let Some(root_path) = params.root_path.as_ref() {
            if !root_path.is_empty() {
                let path = PathBuf::from(root_path);
                if !roots.iter().any(|existing| existing == &path) {
                    roots.push(path);
                }
            }
        }

        if let Some(folders) = params.workspace_folders.as_ref() {
            for folder in folders {
                if let Ok(path) = folder.uri.to_file_path() {
                    if !roots.iter().any(|existing| existing == &path) {
                        roots.push(path);
                    }
                }
            }
        }
    }

    async fn scan_directory(&self, uri: &Url) {
        let file_path = match uri.to_file_path() {
            Ok(path) => path,
            Err(_) => return,
        };

        let directories: Vec<PathBuf> = {
            let roots = self.workspace_roots.read();
            if roots.is_empty() {
                file_path
                    .parent()
                    .map(|dir| vec![dir.to_path_buf()])
                    .unwrap_or_default()
            } else {
                let mut dirs = Vec::new();
                for root in roots.iter() {
                    if file_path.starts_with(root) {
                        dirs.push(root.clone());
                    }
                }

                if dirs.is_empty() {
                    dirs.extend(roots.iter().cloned());
                }

                dirs
            }
        };

        let mut visited_dirs = HashSet::new();

        for dir in directories {
            if !visited_dirs.insert(dir.clone()) {
                continue;
            }

            if !dir.exists() {
                continue;
            }

            for entry in WalkDir::new(&dir)
                .follow_links(false)
                .into_iter()
                .filter_map(|entry| entry.ok())
            {
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.into_path();
                if path.extension().and_then(|s| s.to_str()) != Some("amble") {
                    continue;
                }
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

                let (name, description, exits) = node
                    .parent()
                    .map(|room_node| Self::extract_room_metadata(&room_node, text))
                    .unwrap_or((None, None, Vec::new()));

                self.room_definitions.insert(
                    room_id.to_string(),
                    RoomDefinition {
                        uri: uri.clone(),
                        range,
                        name,
                        description,
                        exits,
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

                let (
                    name,
                    description,
                    portable,
                    location,
                    container_state,
                    abilities,
                    requirements,
                ) = node
                    .parent()
                    .map(|item_node| Self::extract_item_metadata(&item_node, text))
                    .unwrap_or((None, None, None, None, None, Vec::new(), Vec::new()));

                self.item_definitions.insert(
                    item_id.to_string(),
                    ItemDefinition {
                        uri: uri.clone(),
                        range,
                        name,
                        description,
                        portable,
                        location,
                        container_state,
                        abilities,
                        requirements,
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

                let (name, description, location, state) = node
                    .parent()
                    .map(|npc_node| Self::extract_npc_metadata(&npc_node, text))
                    .unwrap_or((None, None, None, None));

                self.npc_definitions.insert(
                    npc_id.to_string(),
                    NpcDefinition {
                        uri: uri.clone(),
                        range,
                        name,
                        description,
                        location,
                        state,
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

                let (defined_in, sequence_limit) = node
                    .parent()
                    .map(|action_node| Self::extract_flag_metadata(&action_node, text))
                    .unwrap_or((None, None));

                self.flag_definitions.insert(
                    flag_name.to_string(),
                    FlagDefinition {
                        uri: uri.clone(),
                        range,
                        defined_in,
                        sequence_limit,
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

                let rooms = node
                    .parent()
                    .map(|set_node| Self::extract_set_rooms(&set_node, text))
                    .unwrap_or_default();

                self.set_definitions.insert(
                    set_name.to_string(),
                    SetDefinition {
                        uri: uri.clone(),
                        range,
                        rooms,
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

    fn format_document(text: &str) -> String {
        let mut parser = Parser::new();
        if parser.set_language(&tree_sitter_amble::language()).is_err() {
            return Self::fallback_format(text);
        }

        if let Some(tree) = parser.parse(text, None) {
            let events = Self::collect_brace_events(tree.root_node());
            let mut formatted = Self::format_with_events(text, events);
            if let Some(tree) = parser.parse(&formatted, None) {
                formatted = ParenthesizedListFormatter::new(&formatted).apply(tree.root_node());
            }
            return formatted;
        }

        Self::fallback_format(text)
    }

    fn format_with_events(text: &str, events: Vec<BraceEvent>) -> String {
        let mut events_by_line: HashMap<usize, Vec<BraceEvent>> = HashMap::new();
        for event in events {
            events_by_line.entry(event.line).or_default().push(event);
        }
        for line_events in events_by_line.values_mut() {
            line_events.sort_by(|a, b| a.column.cmp(&b.column));
        }

        let mut result = String::with_capacity(text.len());
        let mut indent_level: usize = 0;
        let mut in_multiline: Option<&'static str> = None;

        for (line_index, segment) in text.split_inclusive('\n').enumerate() {
            let (line, has_newline) = if let Some(stripped) = segment.strip_suffix('\n') {
                (stripped, true)
            } else {
                (segment, false)
            };

            if in_multiline.is_some() {
                result.push_str(line.trim_end());
                if has_newline {
                    result.push('\n');
                }
                Self::update_multiline_state(line, &mut in_multiline);
                continue;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                if has_newline {
                    result.push('\n');
                }
                Self::update_multiline_state(line, &mut in_multiline);
                continue;
            }

            let trimmed_start = line.trim_start();
            let normalized = trimmed_start.trim_end();
            let leading_ws = line.len() - trimmed_start.len();
            if let Some(line_events) = events_by_line.get(&line_index) {
                for _ in line_events.iter().filter(|event| {
                    matches!(event.kind, BraceKind::Close) && event.column <= leading_ws
                }) {
                    indent_level = indent_level.saturating_sub(1);
                }
            }
            result.push_str(&" ".repeat(indent_level * 4));
            result.push_str(normalized);
            if has_newline {
                result.push('\n');
            }

            if let Some(line_events) = events_by_line.get(&line_index) {
                for event in line_events {
                    match event.kind {
                        BraceKind::Open => {
                            indent_level += 1;
                        }
                        BraceKind::Close => {
                            if event.column > leading_ws {
                                indent_level = indent_level.saturating_sub(1);
                            }
                        }
                    }
                }
            }

            Self::update_multiline_state(trimmed_start, &mut in_multiline);
        }

        if !result.ends_with('\n') {
            result.push('\n');
        }

        result
    }

    fn fallback_format(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut indent_level: usize = 0;
        let mut in_multiline: Option<&'static str> = None;

        for segment in text.split_inclusive('\n') {
            let (line, has_newline) = if let Some(stripped) = segment.strip_suffix('\n') {
                (stripped, true)
            } else {
                (segment, false)
            };

            if in_multiline.is_some() {
                result.push_str(line.trim_end());
                if has_newline {
                    result.push('\n');
                }
                Self::update_multiline_state(line, &mut in_multiline);
                continue;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                if has_newline {
                    result.push('\n');
                }
                Self::update_multiline_state(line, &mut in_multiline);
                continue;
            }

            let trimmed_start = line.trim_start();
            let closes_block = trimmed_start.starts_with('}');
            if closes_block {
                indent_level = indent_level.saturating_sub(1);
            }

            let normalized = trimmed_start.trim_end();
            result.push_str(&" ".repeat(indent_level * 4));
            result.push_str(normalized);
            if has_newline {
                result.push('\n');
            }

            let mut delta = Self::brace_delta(normalized);
            if closes_block {
                delta += 1;
            }
            indent_level = ((indent_level as isize) + delta).max(0) as usize;

            Self::update_multiline_state(trimmed_start, &mut in_multiline);
        }

        if !result.ends_with('\n') {
            result.push('\n');
        }

        result
    }

    fn document_range(text: &str) -> Range {
        let segments: Vec<&str> = text.split('\n').collect();
        let line_index = segments.len().saturating_sub(1) as u32;
        let mut last_len = segments
            .last()
            .map(|segment| segment.chars().count() as u32)
            .unwrap_or(0);

        if text.ends_with('\n') {
            last_len = 0;
        }

        Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: line_index,
                character: last_len,
            },
        }
    }

    fn update_multiline_state(line: &str, state: &mut Option<&'static str>) {
        let bytes = line.as_bytes();
        let mut i = 0;
        while i + 2 < bytes.len() {
            if let Some(delim) = state {
                let target = if *delim == "\"\"\"" {
                    b"\"\"\""
                } else {
                    b"'''"
                };
                if bytes[i..].starts_with(target) {
                    *state = None;
                    i += 3;
                    continue;
                }
            } else {
                if bytes[i..].starts_with(b"\"\"\"") {
                    *state = Some("\"\"\"");
                    i += 3;
                    continue;
                }
                if bytes[i..].starts_with(b"'''") {
                    *state = Some("'''");
                    i += 3;
                    continue;
                }
            }
            i += 1;
        }
    }

    fn brace_delta(line: &str) -> isize {
        let bytes = line.as_bytes();
        let mut i = 0;
        let mut delta: isize = 0;
        let mut string_delim: Option<u8> = None;

        while i < bytes.len() {
            let b = bytes[i];
            if let Some(delim) = string_delim {
                if b == b'\\' {
                    if i + 1 < bytes.len() {
                        i += 2;
                    } else {
                        i = bytes.len();
                    }
                    continue;
                }
                if b == delim {
                    string_delim = None;
                }
                i += 1;
                continue;
            } else {
                if (b == b'"' || b == b'\'')
                    && i + 2 < bytes.len()
                    && bytes[i + 1] == b
                    && bytes[i + 2] == b
                {
                    break;
                }
                match b {
                    b'"' | b'\'' => {
                        string_delim = Some(b);
                    }
                    b'{' => {
                        delta += 1;
                    }
                    b'}' => {
                        delta -= 1;
                    }
                    _ => {}
                }
                i += 1;
            }
        }

        delta
    }

    fn collect_brace_events(root: Node) -> Vec<BraceEvent> {
        let mut events = Vec::new();
        Self::walk_brace_nodes(root, &mut events);
        events
    }

    fn walk_brace_nodes(node: Node, events: &mut Vec<BraceEvent>) {
        if !node.is_named() {
            match node.kind() {
                "{" => events.push(BraceEvent {
                    line: node.start_position().row as usize,
                    column: node.start_position().column as usize,
                    kind: BraceKind::Open,
                }),
                "}" => events.push(BraceEvent {
                    line: node.start_position().row as usize,
                    column: node.start_position().column as usize,
                    kind: BraceKind::Close,
                }),
                _ => {}
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_brace_nodes(child, events);
        }
    }

    fn slice_text<'a>(text: &'a str, node: &Node) -> &'a str {
        &text[node.byte_range()]
    }

    fn named_child_by_kind<'tree>(node: &Node<'tree>, kind: &str) -> Option<Node<'tree>> {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == kind {
                return Some(child);
            }
        }
        None
    }

    fn normalize_string_literal(literal: &str) -> String {
        if literal.starts_with("\"\"\"") && literal.ends_with("\"\"\"") && literal.len() >= 6 {
            literal[3..literal.len() - 3].to_string()
        } else if literal.starts_with('"') && literal.ends_with('"') {
            serde_json::from_str::<String>(literal)
                .unwrap_or_else(|_| literal[1..literal.len() - 1].to_string())
        } else if literal.starts_with('\'') && literal.ends_with('\'') && literal.len() >= 2 {
            literal[1..literal.len() - 1].to_string()
        } else if literal.starts_with("'''") && literal.ends_with("'''") && literal.len() >= 6 {
            literal[3..literal.len() - 3].to_string()
        } else {
            literal.to_string()
        }
    }

    fn extract_room_metadata(
        room_node: &Node,
        text: &str,
    ) -> (Option<String>, Option<String>, Vec<String>) {
        let mut name = None;
        let mut description = None;
        let mut exits = Vec::new();

        if let Some(block) = Self::named_child_by_kind(room_node, "room_block") {
            let mut cursor = block.walk();
            for child in block.named_children(&mut cursor) {
                match child.kind() {
                    "room_name" => {
                        if let Some(name_node) = child.child_by_field_name("name") {
                            let raw = Self::slice_text(text, &name_node);
                            name = Some(Self::normalize_string_literal(raw));
                        }
                    }
                    "room_desc" => {
                        if let Some(desc_node) = child.child_by_field_name("description") {
                            let raw = Self::slice_text(text, &desc_node);
                            description = Some(Self::normalize_string_literal(raw));
                        }
                    }
                    "room_exit" => {
                        if let Some(dest) = child.child_by_field_name("dest") {
                            exits.push(Self::slice_text(text, &dest).trim().to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        (name, description, exits)
    }

    fn format_location_node(location_node: &Node, text: &str) -> String {
        if let Some(room) = Self::named_child_by_kind(location_node, "room_id")
            .or_else(|| Self::named_child_by_kind(location_node, "_room_ref"))
        {
            return format!(
                "room {}",
                Self::sanitize_markdown(Self::slice_text(text, &room).trim())
            );
        }
        if let Some(chest) = Self::named_child_by_kind(location_node, "chest_id") {
            return format!(
                "chest {}",
                Self::sanitize_markdown(Self::slice_text(text, &chest).trim())
            );
        }
        if let Some(npc) = Self::named_child_by_kind(location_node, "npc_id") {
            return format!(
                "npc {}",
                Self::sanitize_markdown(Self::slice_text(text, &npc).trim())
            );
        }
        if let Some(spawn_note) = Self::named_child_by_kind(location_node, "spawn_note") {
            return format!(
                "nowhere {}",
                Self::sanitize_markdown(&Self::normalize_string_literal(Self::slice_text(
                    text,
                    &spawn_note,
                )))
            );
        }

        let raw = Self::slice_text(text, location_node).trim();
        Self::sanitize_markdown(
            raw.strip_prefix("location")
                .map(|rest| rest.trim())
                .unwrap_or(raw),
        )
    }

    fn extract_item_metadata(
        item_node: &Node,
        text: &str,
    ) -> (
        Option<String>,
        Option<String>,
        Option<bool>,
        Option<String>,
        Option<String>,
        Vec<String>,
        Vec<String>,
    ) {
        let mut name = None;
        let mut description = None;
        let mut portable = None;
        let mut location = None;
        let mut container_state = None;
        let mut abilities = Vec::new();
        let mut requirements = Vec::new();

        if let Some(block) = Self::named_child_by_kind(item_node, "item_block") {
            let mut cursor = block.walk();
            for child in block.named_children(&mut cursor) {
                match child.kind() {
                    "item_name_stmt" => {
                        if let Some(name_node) = child.child_by_field_name("item_name") {
                            name = Some(Self::normalize_string_literal(Self::slice_text(
                                text, &name_node,
                            )));
                        }
                    }
                    "item_desc_stmt" => {
                        if let Some(desc_node) = child.child_by_field_name("item_description") {
                            description = Some(Self::normalize_string_literal(Self::slice_text(
                                text, &desc_node,
                            )));
                        }
                    }
                    "item_portable_stmt" => {
                        if let Some(port_node) = child.child_by_field_name("portable") {
                            portable = Some(Self::slice_text(text, &port_node).trim() == "true");
                        }
                    }
                    "item_loc_stmt" => {
                        if let Some(loc_node) = Self::named_child_by_kind(&child, "item_location") {
                            location = Some(Self::format_location_node(&loc_node, text));
                        } else {
                            let raw = Self::slice_text(text, &child).trim();
                            location = Some(Self::sanitize_markdown(
                                raw.strip_prefix("location")
                                    .map(|rest| rest.trim())
                                    .unwrap_or(raw),
                            ));
                        }
                    }
                    "item_container_stmt" => {
                        if let Some(state_node) =
                            Self::named_child_by_kind(&child, "container_state")
                        {
                            container_state =
                                Some(Self::slice_text(text, &state_node).trim().to_string());
                        }
                    }
                    "item_ability_stmt" => {
                        abilities.push(Self::sanitize_markdown(
                            Self::slice_text(text, &child).trim(),
                        ));
                    }
                    "item_requires_stmt" => {
                        requirements.push(Self::sanitize_markdown(
                            Self::slice_text(text, &child).trim(),
                        ));
                    }
                    _ => {}
                }
            }
        }

        (
            name,
            description,
            portable,
            location,
            container_state,
            abilities,
            requirements,
        )
    }

    fn extract_npc_metadata(
        npc_node: &Node,
        text: &str,
    ) -> (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) {
        let mut name = None;
        let mut description = None;
        let mut location = None;
        let mut state = None;

        if let Some(block) = Self::named_child_by_kind(npc_node, "npc_block") {
            let mut cursor = block.walk();
            for child in block.named_children(&mut cursor) {
                match child.kind() {
                    "npc_name_stmt" => {
                        if let Some(name_node) = child.child_by_field_name("npc_name") {
                            name = Some(Self::normalize_string_literal(Self::slice_text(
                                text, &name_node,
                            )));
                        }
                    }
                    "npc_desc_stmt" => {
                        if let Some(desc_node) = child.child_by_field_name("npc_description") {
                            description = Some(Self::normalize_string_literal(Self::slice_text(
                                text, &desc_node,
                            )));
                        }
                    }
                    "npc_loc_stmt" => {
                        if let Some(loc_node) = Self::named_child_by_kind(&child, "npc_location") {
                            location = Some(Self::format_location_node(&loc_node, text));
                        } else {
                            let raw = Self::slice_text(text, &child).trim();
                            location = Some(Self::sanitize_markdown(
                                raw.strip_prefix("location")
                                    .map(|rest| rest.trim())
                                    .unwrap_or(raw),
                            ));
                        }
                    }
                    "npc_state_stmt" => {
                        if let Some(state_node) = Self::named_child_by_kind(&child, "npc_state") {
                            state = Some(Self::slice_text(text, &state_node).trim().to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        (name, description, location, state)
    }

    fn find_trigger_name(node: Node, text: &str) -> Option<String> {
        let mut current = node;
        while let Some(parent) = current.parent() {
            if parent.kind() == "trigger_def" {
                if let Some(name_node) = parent.child_by_field_name("name") {
                    return Some(Self::normalize_string_literal(Self::slice_text(
                        text, &name_node,
                    )));
                }
                break;
            }
            current = parent;
        }
        None
    }

    fn extract_flag_metadata(action_node: &Node, text: &str) -> (Option<String>, Option<i64>) {
        let defined_in = Self::find_trigger_name(*action_node, text);
        let limit = if action_node.kind() == "action_add_seq" {
            let mut cursor = action_node.walk();
            let mut result = None;
            for child in action_node.named_children(&mut cursor) {
                if child.kind() == "number" {
                    if let Ok(value) = Self::slice_text(text, &child).trim().parse::<i64>() {
                        result = Some(value);
                    }
                }
            }
            result
        } else {
            None
        };
        (defined_in, limit)
    }

    fn extract_set_rooms(set_node: &Node, text: &str) -> Vec<String> {
        if let Some(list_node) = Self::named_child_by_kind(set_node, "set_list")
            .or_else(|| Self::named_child_by_kind(set_node, "room_list"))
        {
            let mut cursor = list_node.walk();
            let mut rooms = Vec::new();
            for child in list_node.named_children(&mut cursor) {
                match child.kind() {
                    "room_id" | "_room_ref" => rooms.push(Self::sanitize_markdown(
                        Self::slice_text(text, &child).trim(),
                    )),
                    _ => {}
                }
            }
            rooms
        } else {
            Vec::new()
        }
    }

    fn sanitize_markdown(value: &str) -> String {
        value.trim().replace('|', "\\|").replace('\n', "<br>")
    }

    fn format_room_hover(id: &str, def: &RoomDefinition) -> String {
        let mut lines = vec![format!("**Room:** {}", Self::sanitize_markdown(id))];
        lines.push(format!(
            "- Name: {}",
            def.name
                .as_deref()
                .map(Self::sanitize_markdown)
                .unwrap_or_else(|| "(missing)".to_string())
        ));
        lines.push(format!(
            "- Description: {}",
            def.description
                .as_deref()
                .map(Self::sanitize_markdown)
                .unwrap_or_else(|| "(missing)".to_string())
        ));
        lines.push(format!(
            "- Exits: {}",
            if def.exits.is_empty() {
                "(none)".to_string()
            } else {
                def.exits
                    .iter()
                    .map(|exit| Self::sanitize_markdown(exit))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ));
        lines.join("\n")
    }

    fn format_item_hover(id: &str, def: &ItemDefinition) -> String {
        let mut lines = vec![format!("**Item:** {}", Self::sanitize_markdown(id))];
        lines.push(format!(
            "- Name: {}",
            def.name
                .as_deref()
                .map(Self::sanitize_markdown)
                .unwrap_or_else(|| "(missing)".to_string())
        ));
        lines.push(format!(
            "- Description: {}",
            def.description
                .as_deref()
                .map(Self::sanitize_markdown)
                .unwrap_or_else(|| "(missing)".to_string())
        ));
        lines.push(format!(
            "- Portable: {}",
            def.portable
                .map(|p| if p { "true" } else { "false" }.to_string())
                .unwrap_or_else(|| "(none)".to_string())
        ));
        lines.push(format!(
            "- Location: {}",
            def.location
                .as_deref()
                .map(Self::sanitize_markdown)
                .unwrap_or_else(|| "(missing)".to_string())
        ));
        lines.push(format!(
            "- Container state: {}",
            def.container_state
                .as_deref()
                .map(Self::sanitize_markdown)
                .unwrap_or_else(|| "(none)".to_string())
        ));
        let format_list = |values: &[String]| -> String {
            if values.is_empty() {
                "(none)".to_string()
            } else {
                values
                    .iter()
                    .map(|value| Self::sanitize_markdown(value))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };
        lines.push(format!("- Abilities: {}", format_list(&def.abilities)));
        lines.push(format!(
            "- Requirements: {}",
            format_list(&def.requirements)
        ));
        lines.join("\n")
    }

    fn format_npc_hover(id: &str, def: &NpcDefinition) -> String {
        let mut lines = vec![format!("**NPC:** {}", Self::sanitize_markdown(id))];
        lines.push(format!(
            "- Name: {}",
            def.name
                .as_deref()
                .map(Self::sanitize_markdown)
                .unwrap_or_else(|| "(missing)".to_string())
        ));
        lines.push(format!(
            "- Description: {}",
            def.description
                .as_deref()
                .map(Self::sanitize_markdown)
                .unwrap_or_else(|| "(missing)".to_string())
        ));
        lines.push(format!(
            "- Location: {}",
            def.location
                .as_deref()
                .map(Self::sanitize_markdown)
                .unwrap_or_else(|| "(missing)".to_string())
        ));
        lines.push(format!(
            "- State: {}",
            def.state
                .as_deref()
                .map(Self::sanitize_markdown)
                .unwrap_or_else(|| "(none)".to_string())
        ));
        lines.join("\n")
    }

    fn format_flag_hover(id: &str, def: &FlagDefinition) -> String {
        let mut lines = vec![format!("**Flag:** {}", Self::sanitize_markdown(id))];
        if let Some(trigger) = &def.defined_in {
            lines.push(format!(
                "- Defined in trigger: {}",
                Self::sanitize_markdown(trigger)
            ));
        }
        if let Some(limit) = def.sequence_limit {
            lines.push(format!("- Sequence limit: {}", limit));
        }
        if lines.len() == 1 {
            lines.push("- Defined in trigger: (unknown)".to_string());
        }
        lines.join("\n")
    }

    fn format_set_hover(id: &str, def: &SetDefinition) -> String {
        let mut lines = vec![format!("**Set:** {}", Self::sanitize_markdown(id))];
        lines.push(format!(
            "- Rooms: {}",
            if def.rooms.is_empty() {
                "(none)".to_string()
            } else {
                def.rooms.join(", ")
            }
        ));
        lines.join("\n")
    }

    fn collect_rename_edits(
        &self,
        symbol_type: SymbolType,
        id: &str,
        new_name: &str,
    ) -> HashMap<Url, Vec<TextEdit>> {
        let mut edits: HashMap<Url, Vec<TextEdit>> = HashMap::new();

        let mut push_edit = |url: &Url, range: &Range| {
            edits.entry(url.clone()).or_default().push(TextEdit {
                range: range.clone(),
                new_text: new_name.to_string(),
            });
        };

        match symbol_type {
            SymbolType::Room => {
                if let Some(def) = self.room_definitions.get(id) {
                    push_edit(&def.uri, &def.range);
                }
                if let Some(refs) = self.room_references.get(id) {
                    for reference in refs.iter() {
                        push_edit(&reference.uri, &reference.range);
                    }
                }
            }
            SymbolType::Item => {
                if let Some(def) = self.item_definitions.get(id) {
                    push_edit(&def.uri, &def.range);
                }
                if let Some(refs) = self.item_references.get(id) {
                    for reference in refs.iter() {
                        push_edit(&reference.uri, &reference.range);
                    }
                }
            }
            SymbolType::Npc => {
                if let Some(def) = self.npc_definitions.get(id) {
                    push_edit(&def.uri, &def.range);
                }
                if let Some(refs) = self.npc_references.get(id) {
                    for reference in refs.iter() {
                        push_edit(&reference.uri, &reference.range);
                    }
                }
            }
            SymbolType::Flag => {
                if let Some(def) = self.flag_definitions.get(id) {
                    push_edit(&def.uri, &def.range);
                }
                if let Some(refs) = self.flag_references.get(id) {
                    for reference in refs.iter() {
                        push_edit(&reference.uri, &reference.range);
                    }
                }
            }
            SymbolType::Set => {
                if let Some(def) = self.set_definitions.get(id) {
                    push_edit(&def.uri, &def.range);
                }
                if let Some(refs) = self.set_references.get(id) {
                    for reference in refs.iter() {
                        push_edit(&reference.uri, &reference.range);
                    }
                }
            }
        }

        edits
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

    fn position_for_token(source: &str, line: usize, token: &str, offset: usize) -> Position {
        let line_str = source.lines().nth(line).expect("line missing");
        let start = line_str.find(token).expect("token missing on line");
        Position {
            line: line as u32,
            character: (start + offset) as u32,
        }
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
    fn detects_item_reference_context_in_conditions() {
        let source = "trigger \"test-trigger\" when always {\n    if has item test_item {\n        do show \"\"\n    }\n}\n\nitem test_item {\n    name \"Item\"\n}\n";
        let position = position_for_token(source, 1, "test_item", 2);
        let symbol = completion_at(source, position);
        assert_eq!(symbol, Some(SymbolType::Item));
    }

    #[test]
    fn detects_npc_reference_context_in_events() {
        let source = "trigger \"npc-trigger\" when talk to npc test_npc {\n    do show \"\"\n}\n\nnpc test_npc {\n    name \"Npc\"\n}\n";
        let position = position_for_token(source, 0, "test_npc", 2);
        let symbol = completion_at(source, position);
        assert_eq!(symbol, Some(SymbolType::Npc));
    }

    #[test]
    fn detects_flag_reference_context_in_actions() {
        let source = "trigger \"flag-trigger\" when always {\n    if has flag quest_flag {\n        do show \"\"\n    }\n}\n";
        let position = position_for_token(source, 1, "quest_flag", 2);
        let symbol = completion_at(source, position);
        assert_eq!(symbol, Some(SymbolType::Flag));
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

    #[test]
    fn formats_item_block() {
        let source = "item sample {\n  name \"Sample\"\n  portable true\n}\n";
        let expected = "item sample {\n    name \"Sample\"\n    portable true\n}\n";
        assert_eq!(Backend::format_document(source), expected);
    }

    #[test]
    fn preserves_multiline_text_blocks() {
        let source = "item example {\n  text \"\"\"line1\nline2\"\"\"\n}\n";
        let expected = "item example {\n    text \"\"\"line1\nline2\"\"\"\n}\n";
        assert_eq!(Backend::format_document(source), expected);
    }

    #[test]
    fn ignores_braces_inside_raw_strings() {
        let source = "item raw {\n  name r#\"{curly}\"#\n}\n";
        let expected = "item raw {\n    name r#\"{curly}\"#\n}\n";
        assert_eq!(Backend::format_document(source), expected);
    }

    #[test]
    fn formats_any_group_single_line_with_spacing() {
        let source = "trigger \"example\" when always {\n    if any(missing item quest_scroll, has flag quest_started) {\n        do show \"\"\n    }\n}\n";
        let expected = "trigger \"example\" when always {\n    if any( missing item quest_scroll, has flag quest_started ) {\n        do show \"\"\n    }\n}\n";
        assert_eq!(Backend::format_document(source), expected);
    }

    #[test]
    fn formats_any_group_multiline_with_nested_all() {
        let source = "trigger \"example\" when always {\n    if any(missing item some_item, has flag some_flag, all(with npc guide_bot, flag in progress guide_bot_intro, missing item guide_token)) {\n        do show \"\"\n    }\n}\n";
        let expected = "trigger \"example\" when always {\n    if any(\n        missing item some_item,\n        has flag some_flag,\n        all(\n            with npc guide_bot,\n            flag in progress guide_bot_intro,\n            missing item guide_token,\n        ),\n    ) {\n        do show \"\"\n    }\n}\n";
        assert_eq!(Backend::format_document(source), expected);
    }

    #[test]
    fn formats_any_group_trailing_commas_without_duplicates() {
        let source = "trigger \"example\" when always {\n    if any(has flag flag_1, has flag flag_2, has flag flag_3,) {\n        do show \"\"\n    }\n}\n";
        let expected = "trigger \"example\" when always {\n    if any(\n        has flag flag_1,\n        has flag flag_2,\n        has flag flag_3,\n    ) {\n        do show \"\"\n    }\n}\n";
        assert_eq!(Backend::format_document(source), expected);
    }

    #[test]
    fn formats_set_lists_into_multiline_blocks() {
        let source = "let set hallway = (room_a, room_b, room_c)\n";
        let expected = "let set hallway = (\n    room_a,\n    room_b,\n    room_c,\n)\n";
        assert_eq!(Backend::format_document(source), expected);
    }

    #[test]
    fn formats_required_items_with_parenthesis_spacing() {
        let source = "room foyer {\n    exit north -> hall {\n        required_items(item_key, item_badge)\n    }\n}\n";
        let expected = "room foyer {\n    exit north -> hall {\n        required_items( item_key, item_badge )\n    }\n}\n";
        assert_eq!(Backend::format_document(source), expected);
    }

    #[test]
    fn formats_overlay_conditions_with_two_items_single_line() {
        let source = "room entry {\n    overlay if (flag set foo, item present bar) {\n        text \"\"\n    }\n}\n";
        let expected = "room entry {\n    overlay if ( flag set foo, item present bar ) {\n        text \"\"\n    }\n}\n";
        assert_eq!(Backend::format_document(source), expected);
    }

    #[test]
    fn formats_overlay_conditions_multiline_when_three_items() {
        let source = "room entry {\n    overlay if (flag set foo, item present bar, player has item baz) {\n        text \"\"\n    }\n}\n";
        let expected = "room entry {\n    overlay if (\n        flag set foo,\n        item present bar,\n        player has item baz,\n    ) {\n        text \"\"\n    }\n}\n";
        assert_eq!(Backend::format_document(source), expected);
    }

    #[test]
    fn formats_room_hover_markdown() {
        let def = RoomDefinition {
            uri: Url::parse("file:///tmp/test.amble").unwrap(),
            range: Range::default(),
            name: Some("Test Room".into()),
            description: Some("A description".into()),
            exits: vec!["north-hall".into(), "south-porch".into()],
        };

        let hover = Backend::format_room_hover("test-room", &def);
        assert!(hover.contains("**Room:** test-room"));
        assert!(hover.contains("Test Room"));
        assert!(hover.contains("north-hall"));
    }

    #[test]
    fn formats_item_hover_lists_abilities() {
        let def = ItemDefinition {
            uri: Url::parse("file:///tmp/test.amble").unwrap(),
            range: Range::default(),
            name: Some("Widget".into()),
            description: Some("Useful widget".into()),
            portable: Some(true),
            location: Some("room lab".into()),
            container_state: Some("closed".into()),
            abilities: vec!["ability Unlock".into()],
            requirements: vec!["requires ability Use to interact".into()],
        };

        let hover = Backend::format_item_hover("widget", &def);
        assert!(hover.contains("Abilities: ability Unlock"));
        assert!(hover.contains("Requirements: requires ability Use to interact"));
    }
}

struct ParenthesizedListFormatter<'a> {
    text: &'a str,
    output: String,
    cursor: usize,
}

impl<'a> ParenthesizedListFormatter<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            output: String::with_capacity(text.len()),
            cursor: 0,
        }
    }

    fn apply(mut self, root: Node) -> String {
        self.visit(root);
        self.output.push_str(&self.text[self.cursor..]);
        self.output
    }

    fn visit(&mut self, node: Node) {
        if node.start_byte() < self.cursor {
            return;
        }
        if self.format_node(node) {
            return;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit(child);
        }
    }

    fn format_node(&mut self, node: Node) -> bool {
        let indent = Self::line_indent(self.text, node.start_byte());
        if let Some(replacement) = self.render_nested(&node, &indent) {
            self.replace(node.start_byte(), node.end_byte(), &replacement);
            return true;
        }

        if node.kind() == "(" {
            if let Some((replacement, end_byte)) = self.render_overlay_cond_list(&node, &indent) {
                self.replace(node.start_byte(), end_byte, &replacement);
                return true;
            }
        }

        false
    }

    fn replace(&mut self, start: usize, end: usize, replacement: &str) {
        self.output.push_str(&self.text[self.cursor..start]);
        self.output.push_str(replacement);
        self.cursor = end;
    }

    fn render_condition_group(&self, node: Node, keyword: &str, base_indent: &str) -> String {
        let items = self.collect_items(node);
        Self::format_parenthesized(keyword, &items, base_indent)
    }

    fn render_prefixed_paren(&self, node: Node, keyword: &str, base_indent: &str) -> String {
        let items = self.collect_items(node);
        Self::format_parenthesized(keyword, &items, base_indent)
    }

    fn render_paren_only(&self, node: Node, base_indent: &str) -> String {
        let items = self.collect_items(node);
        Self::format_parenthesized("", &items, base_indent)
    }

    fn render_overlay_cond_list(
        &self,
        open_paren: &Node,
        base_indent: &str,
    ) -> Option<(String, usize)> {
        let parent = open_paren.parent()?;
        if parent.kind() != "overlay_stmt" {
            return None;
        }

        let mut items = Vec::new();
        let mut cursor = open_paren.next_sibling();
        let mut end_byte = None;
        while let Some(node) = cursor {
            if node.kind() == ")" {
                end_byte = Some(node.end_byte());
                break;
            }
            if node.is_named() && Self::is_overlay_condition(node.kind()) {
                items.push(self.render_child(&node));
            }
            cursor = node.next_sibling();
        }

        let end = end_byte?;
        let rendered = Self::format_parenthesized("", &items, base_indent);
        Some((rendered, end))
    }

    fn is_overlay_condition(kind: &str) -> bool {
        kind.starts_with("ovl_")
    }

    fn collect_items(&self, node: Node) -> Vec<String> {
        let mut cursor = node.walk();
        node.named_children(&mut cursor)
            .map(|child| self.render_child(&child))
            .filter(|item| !item.is_empty())
            .collect()
    }

    fn render_child(&self, node: &Node) -> String {
        if let Some(rendered) = self.render_nested(node, "") {
            rendered
        } else {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if let Some(nested) = self.render_nested(&child, "") {
                    return nested;
                }
            }
            Backend::slice_text(self.text, node).trim().to_string()
        }
    }

    fn render_nested(&self, node: &Node, base_indent: &str) -> Option<String> {
        match node.kind() {
            "cond_any_group" => Some(self.render_condition_group(*node, "any", base_indent)),
            "cond_all_group" => Some(self.render_condition_group(*node, "all", base_indent)),
            "set_list" => Some(self.render_paren_only(*node, base_indent)),
            "room_list" => Some(self.render_paren_only(*node, base_indent)),
            "npc_patch_route" => Some(self.render_prefixed_paren(*node, "route", base_indent)),
            "npc_patch_random_rooms" => {
                Some(self.render_prefixed_paren(*node, "random rooms", base_indent))
            }
            "required_items_stmt" => {
                Some(self.render_prefixed_paren(*node, "required_items", base_indent))
            }
            "required_flags_stmt" => {
                Some(self.render_prefixed_paren(*node, "required_flags", base_indent))
            }
            _ => None,
        }
    }

    fn format_parenthesized(prefix: &str, items: &[String], base_indent: &str) -> String {
        if items.is_empty() {
            let mut empty = String::new();
            if !prefix.is_empty() {
                empty.push_str(prefix);
            }
            empty.push_str("()");
            return empty;
        }

        let multiline = items.len() >= 3 || items.iter().any(|item| item.contains('\n'));
        if !multiline {
            let mut single = String::new();
            if !prefix.is_empty() {
                single.push_str(prefix);
            }
            single.push('(');
            single.push(' ');
            single.push_str(&items.join(", "));
            single.push(' ');
            single.push(')');
            return single;
        }

        let mut multi = String::new();
        if !prefix.is_empty() {
            multi.push_str(prefix);
        }
        multi.push('(');
        multi.push('\n');
        let item_indent = format!("{}{}", base_indent, "    ");
        for item in items {
            let normalized = item.trim();
            // Skip empty/error nodes that only surface parser-recovered commas.
            if normalized.is_empty() || normalized.chars().all(|ch| ch == ',') {
                continue;
            }
            multi.push_str(&Self::indent_block(normalized, &item_indent));
            multi.push(',');
            multi.push('\n');
        }
        multi.push_str(base_indent);
        multi.push(')');
        multi
    }

    fn indent_block(block: &str, indent: &str) -> String {
        let mut result = String::new();
        let mut lines = block.split('\n').peekable();
        while let Some(line) = lines.next() {
            result.push_str(indent);
            result.push_str(line);
            if lines.peek().is_some() {
                result.push('\n');
            }
        }
        result
    }

    fn line_indent(text: &str, byte_pos: usize) -> String {
        let line_start = text[..byte_pos].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
        text[line_start..byte_pos]
            .chars()
            .take_while(|ch| ch.is_whitespace() && *ch != '\n' && *ch != '\r')
            .collect()
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

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let uri_str = uri.to_string();

        if let Some(doc) = self.document_map.get(&uri_str) {
            let current = doc.clone();
            drop(doc);
            let formatted = Self::format_document(&current);
            if formatted == current {
                return Ok(Some(vec![]));
            }

            let range = Self::document_range(&current);
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
            let markdown = match symbol_type {
                SymbolType::Room => self
                    .room_definitions
                    .get(&id)
                    .map(|def| Self::format_room_hover(&id, &def)),
                SymbolType::Item => self
                    .item_definitions
                    .get(&id)
                    .map(|def| Self::format_item_hover(&id, &def)),
                SymbolType::Npc => self
                    .npc_definitions
                    .get(&id)
                    .map(|def| Self::format_npc_hover(&id, &def)),
                SymbolType::Flag => self
                    .flag_definitions
                    .get(&id)
                    .map(|def| Self::format_flag_hover(&id, &def)),
                SymbolType::Set => self
                    .set_definitions
                    .get(&id)
                    .map(|def| Self::format_set_hover(&id, &def)),
            };

            if let Some(value) = markdown {
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
