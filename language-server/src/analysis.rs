use crate::backend::Backend;
use crate::symbols::{
    sanitize_markdown, FlagMetadata, ItemMetadata, NpcMetadata, RoomMetadata, SetMetadata,
    SymbolDefinition, SymbolKind, SymbolLocation, SymbolMetadata, SymbolOccurrence,
    SymbolReference,
};
use crate::text::Document;
use std::collections::HashSet;
use std::path::PathBuf;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, InitializeParams, Position, Range, Url,
};
use tree_sitter::{Node, QueryCursor, StreamingIterator};
use walkdir::WalkDir;

impl Backend {
    pub(crate) fn update_workspace_roots(&self, params: &InitializeParams) {
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

    pub(crate) async fn scan_directory(&self, uri: &Url) {
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
                    if self.documents.contains_key(&uri_str) {
                        continue;
                    }
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        self.analyze_document(&uri, &content);
                    }
                }
            }
        }
    }

    pub(crate) fn analyze_document(&self, uri: &Url, text: &str) {
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
        let document = Document::new(text.to_string());

        self.symbols.clear_document(uri);
        let mut occurrences = Vec::new();

        let mut cursor = QueryCursor::new();
        let mut matches =
            cursor.matches(&self.queries.room_definitions, root_node, text.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let room_id = slice_text(text, &node).trim();
                if room_id.is_empty() {
                    continue;
                }

                let range = range_from_node(&document, &node);
                let (name, description, exits) = node
                    .parent()
                    .map(|room_node| extract_room_metadata(&room_node, text))
                    .unwrap_or((None, None, Vec::new()));

                let location = SymbolLocation {
                    uri: uri.clone(),
                    range: range.clone(),
                    rename_range: None,
                };

                self.symbols.rooms.insert_definition(
                    room_id.to_string(),
                    SymbolDefinition {
                        location,
                        metadata: SymbolMetadata::Room(RoomMetadata {
                            name,
                            description,
                            exits,
                        }),
                    },
                );

                occurrences.push(SymbolOccurrence {
                    kind: SymbolKind::Room,
                    id: room_id.to_string(),
                    range,
                });
            }
        }

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&self.queries.room_references, root_node, text.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let room_id = slice_text(text, &node).trim();
                if room_id.is_empty() {
                    continue;
                }

                if let Some(parent) = node.parent() {
                    if parent.kind() == "room_def" {
                        continue;
                    }
                }

                let range = range_from_node(&document, &node);
                let location = SymbolLocation {
                    uri: uri.clone(),
                    range: range.clone(),
                    rename_range: None,
                };

                self.symbols.rooms.add_reference(
                    room_id.to_string(),
                    SymbolReference {
                        location,
                        raw_id: room_id.to_string(),
                    },
                );

                occurrences.push(SymbolOccurrence {
                    kind: SymbolKind::Room,
                    id: room_id.to_string(),
                    range,
                });
            }
        }

        let mut cursor = QueryCursor::new();
        let mut matches =
            cursor.matches(&self.queries.item_definitions, root_node, text.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let item_id = slice_text(text, &node).trim();
                if item_id.is_empty() {
                    continue;
                }

                let range = range_from_node(&document, &node);
                let (
                    name,
                    description,
                    portable,
                    item_location,
                    container_state,
                    abilities,
                    requirements,
                ) = node
                    .parent()
                    .map(|item_node| extract_item_metadata(&item_node, text))
                    .unwrap_or((None, None, None, None, None, Vec::new(), Vec::new()));

                let location = SymbolLocation {
                    uri: uri.clone(),
                    range: range.clone(),
                    rename_range: None,
                };

                self.symbols.items.insert_definition(
                    item_id.to_string(),
                    SymbolDefinition {
                        location,
                        metadata: SymbolMetadata::Item(ItemMetadata {
                            name,
                            description,
                            portable,
                            location: item_location,
                            container_state,
                            abilities,
                            requirements,
                        }),
                    },
                );

                occurrences.push(SymbolOccurrence {
                    kind: SymbolKind::Item,
                    id: item_id.to_string(),
                    range,
                });
            }
        }

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&self.queries.item_references, root_node, text.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let item_id = slice_text(text, &node).trim();
                if item_id.is_empty() {
                    continue;
                }

                if let Some(parent) = node.parent() {
                    if parent.kind() == "item_def" {
                        continue;
                    }
                }

                let range = range_from_node(&document, &node);
                let location = SymbolLocation {
                    uri: uri.clone(),
                    range: range.clone(),
                    rename_range: None,
                };

                self.symbols.items.add_reference(
                    item_id.to_string(),
                    SymbolReference {
                        location,
                        raw_id: item_id.to_string(),
                    },
                );

                occurrences.push(SymbolOccurrence {
                    kind: SymbolKind::Item,
                    id: item_id.to_string(),
                    range,
                });
            }
        }

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&self.queries.npc_definitions, root_node, text.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let npc_id = slice_text(text, &node).trim();
                if npc_id.is_empty() {
                    continue;
                }

                let range = range_from_node(&document, &node);
                let (name, description, npc_location, state) = node
                    .parent()
                    .map(|npc_node| extract_npc_metadata(&npc_node, text))
                    .unwrap_or((None, None, None, None));

                let location = SymbolLocation {
                    uri: uri.clone(),
                    range: range.clone(),
                    rename_range: None,
                };

                self.symbols.npcs.insert_definition(
                    npc_id.to_string(),
                    SymbolDefinition {
                        location,
                        metadata: SymbolMetadata::Npc(NpcMetadata {
                            name,
                            description,
                            location: npc_location,
                            state,
                        }),
                    },
                );

                occurrences.push(SymbolOccurrence {
                    kind: SymbolKind::Npc,
                    id: npc_id.to_string(),
                    range,
                });
            }
        }

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&self.queries.npc_references, root_node, text.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let npc_id = slice_text(text, &node).trim();
                if npc_id.is_empty() {
                    continue;
                }

                if let Some(parent) = node.parent() {
                    if parent.kind() == "npc_def" {
                        continue;
                    }
                }

                let range = range_from_node(&document, &node);
                let location = SymbolLocation {
                    uri: uri.clone(),
                    range: range.clone(),
                    rename_range: None,
                };

                self.symbols.npcs.add_reference(
                    npc_id.to_string(),
                    SymbolReference {
                        location,
                        raw_id: npc_id.to_string(),
                    },
                );

                occurrences.push(SymbolOccurrence {
                    kind: SymbolKind::Npc,
                    id: npc_id.to_string(),
                    range,
                });
            }
        }

        let mut cursor = QueryCursor::new();
        let mut matches =
            cursor.matches(&self.queries.flag_definitions, root_node, text.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let flag_name = slice_text(text, &node).trim();
                if flag_name.is_empty() {
                    continue;
                }

                let range = range_from_node(&document, &node);
                let (defined_in, sequence_limit) = node
                    .parent()
                    .map(|action_node| extract_flag_metadata(&action_node, text))
                    .unwrap_or((None, None));

                let location = SymbolLocation {
                    uri: uri.clone(),
                    range: range.clone(),
                    rename_range: None,
                };

                self.symbols.flags.insert_definition(
                    flag_name.to_string(),
                    SymbolDefinition {
                        location,
                        metadata: SymbolMetadata::Flag(FlagMetadata {
                            defined_in,
                            sequence_limit,
                        }),
                    },
                );

                occurrences.push(SymbolOccurrence {
                    kind: SymbolKind::Flag,
                    id: flag_name.to_string(),
                    range,
                });
            }
        }

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&self.queries.flag_references, root_node, text.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let flag_name = slice_text(text, &node).trim();
                if flag_name.is_empty() {
                    continue;
                }

                if let Some(parent) = node.parent() {
                    if parent.kind() == "action_add_flag" || parent.kind() == "action_add_seq" {
                        continue;
                    }
                }

                let range = range_from_node(&document, &node);
                let (normalized, rename_range) = normalize_flag_reference(flag_name, &range);

                let location = SymbolLocation {
                    uri: uri.clone(),
                    range: range.clone(),
                    rename_range,
                };

                self.symbols.flags.add_reference(
                    normalized.clone(),
                    SymbolReference {
                        location,
                        raw_id: flag_name.to_string(),
                    },
                );

                occurrences.push(SymbolOccurrence {
                    kind: SymbolKind::Flag,
                    id: normalized,
                    range,
                });
            }
        }

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&self.queries.set_definitions, root_node, text.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let set_name = slice_text(text, &node).trim();
                if set_name.is_empty() {
                    continue;
                }

                let range = range_from_node(&document, &node);
                let rooms = node
                    .parent()
                    .map(|set_node| extract_set_rooms(&set_node, text))
                    .unwrap_or_default();

                let location = SymbolLocation {
                    uri: uri.clone(),
                    range: range.clone(),
                    rename_range: None,
                };

                self.symbols.sets.insert_definition(
                    set_name.to_string(),
                    SymbolDefinition {
                        location,
                        metadata: SymbolMetadata::Set(SetMetadata { rooms }),
                    },
                );

                occurrences.push(SymbolOccurrence {
                    kind: SymbolKind::Set,
                    id: set_name.to_string(),
                    range,
                });
            }
        }

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&self.queries.set_references, root_node, text.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let set_name = slice_text(text, &node).trim();
                if set_name.is_empty() {
                    continue;
                }

                if let Some(parent) = node.parent() {
                    if parent.kind() == "set_decl" {
                        continue;
                    }
                }

                let range = range_from_node(&document, &node);
                let location = SymbolLocation {
                    uri: uri.clone(),
                    range: range.clone(),
                    rename_range: None,
                };

                self.symbols.sets.add_reference(
                    set_name.to_string(),
                    SymbolReference {
                        location,
                        raw_id: set_name.to_string(),
                    },
                );

                occurrences.push(SymbolOccurrence {
                    kind: SymbolKind::Set,
                    id: set_name.to_string(),
                    range,
                });
            }
        }

        self.document_symbols.insert(uri_str.clone(), occurrences);
        self.documents
            .insert(uri_str, Document::new(text.to_string()));
    }

    pub(crate) fn get_symbol_at_position(
        &self,
        uri: &Url,
        position: Position,
    ) -> Option<(SymbolKind, String)> {
        let uri_str = uri.to_string();
        let occurrences = self.document_symbols.get(&uri_str)?;

        for occurrence in occurrences.iter() {
            if range_contains(&occurrence.range, position) {
                return Some((occurrence.kind, occurrence.id.clone()));
            }
        }

        None
    }

    pub(crate) fn get_completion_context(
        &self,
        uri: &Url,
        position: Position,
    ) -> Option<SymbolKind> {
        let uri_str = uri.to_string();
        let doc = self.documents.get(&uri_str)?;
        let offset = doc.offset(position)?;
        let text = doc.text().to_string();
        drop(doc);

        let tree = {
            let mut parser = self.parser.lock();
            parser.parse(text.as_str(), None)?
        };

        let root_node = tree.root_node();
        let mut candidate_offsets = vec![offset];
        if offset > 0 {
            candidate_offsets.push(offset - 1);
        }

        for candidate in candidate_offsets {
            if let Some(node) = node_at_offset(&root_node, candidate) {
                if let Some(symbol_type) = symbol_kind_from_syntax(node, candidate) {
                    return Some(symbol_type);
                }
            }
        }

        None
    }

    pub(crate) async fn check_diagnostics(&self, uri: &Url) {
        let uri_str = uri.to_string();
        if !self.documents.contains_key(&uri_str) {
            return;
        }

        let mut diagnostics = Vec::new();

        for entry in self.symbols.rooms.references_iter() {
            let room_id = entry.key();
            if !self.symbols.rooms.has_definition(room_id)
                && !self.symbols.sets.has_definition(room_id)
            {
                for reference in entry.value() {
                    if reference.location.uri == *uri {
                        diagnostics.push(Diagnostic {
                            range: reference.location.range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: None,
                            code_description: None,
                            source: Some("amble-lsp".to_string()),
                            message: format!("Undefined room: '{}'", reference.raw_id),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                    }
                }
            }
        }

        for entry in self.symbols.items.references_iter() {
            let item_id = entry.key();
            if !self.symbols.items.has_definition(item_id) {
                for reference in entry.value() {
                    if reference.location.uri == *uri {
                        diagnostics.push(Diagnostic {
                            range: reference.location.range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: None,
                            code_description: None,
                            source: Some("amble-lsp".to_string()),
                            message: format!("Undefined item: '{}'", reference.raw_id),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                    }
                }
            }
        }

        for entry in self.symbols.npcs.references_iter() {
            let npc_id = entry.key();
            if !self.symbols.npcs.has_definition(npc_id) {
                for reference in entry.value() {
                    if reference.location.uri == *uri {
                        diagnostics.push(Diagnostic {
                            range: reference.location.range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: None,
                            code_description: None,
                            source: Some("amble-lsp".to_string()),
                            message: format!("Undefined NPC: '{}'", reference.raw_id),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                    }
                }
            }
        }

        for entry in self.symbols.flags.references_iter() {
            let flag_name = entry.key();
            if !self.symbols.flags.has_definition(flag_name) {
                for reference in entry.value() {
                    if reference.location.uri == *uri {
                        diagnostics.push(Diagnostic {
                            range: reference.location.range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: None,
                            code_description: None,
                            source: Some("amble-lsp".to_string()),
                            message: format!("Undefined flag: '{}'", reference.raw_id),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                    }
                }
            }
        }

        for entry in self.symbols.sets.references_iter() {
            let set_name = entry.key();
            if !self.symbols.sets.has_definition(set_name) {
                for reference in entry.value() {
                    if reference.location.uri == *uri {
                        diagnostics.push(Diagnostic {
                            range: reference.location.range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: None,
                            code_description: None,
                            source: Some("amble-lsp".to_string()),
                            message: format!("Undefined set: '{}'", reference.raw_id),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                    }
                }
            }
        }

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }
}

pub(crate) fn format_hover(id: &str, def: &SymbolDefinition) -> String {
    match &def.metadata {
        SymbolMetadata::Room(meta) => format_room_hover(id, meta),
        SymbolMetadata::Item(meta) => format_item_hover(id, meta),
        SymbolMetadata::Npc(meta) => format_npc_hover(id, meta),
        SymbolMetadata::Flag(meta) => format_flag_hover(id, meta),
        SymbolMetadata::Set(meta) => format_set_hover(id, meta),
    }
}

fn format_room_hover(id: &str, meta: &RoomMetadata) -> String {
    let mut lines = vec![format!("**Room:** {}", sanitize_markdown(id))];
    lines.push(format!(
        "- Name: {}",
        meta.name
            .as_deref()
            .map(sanitize_markdown)
            .unwrap_or_else(|| "(missing)".to_string())
    ));
    lines.push(format!(
        "- Description: {}",
        meta.description
            .as_deref()
            .map(sanitize_markdown)
            .unwrap_or_else(|| "(missing)".to_string())
    ));
    lines.push(format!(
        "- Exits: {}",
        if meta.exits.is_empty() {
            "(none)".to_string()
        } else {
            meta.exits
                .iter()
                .map(|exit| sanitize_markdown(exit))
                .collect::<Vec<_>>()
                .join(", ")
        }
    ));
    lines.join("\n")
}

fn format_item_hover(id: &str, meta: &ItemMetadata) -> String {
    let mut lines = vec![format!("**Item:** {}", sanitize_markdown(id))];
    lines.push(format!(
        "- Name: {}",
        meta.name
            .as_deref()
            .map(sanitize_markdown)
            .unwrap_or_else(|| "(missing)".to_string())
    ));
    lines.push(format!(
        "- Description: {}",
        meta.description
            .as_deref()
            .map(sanitize_markdown)
            .unwrap_or_else(|| "(missing)".to_string())
    ));
    lines.push(format!(
        "- Portable: {}",
        meta.portable
            .map(|p| if p { "true" } else { "false" }.to_string())
            .unwrap_or_else(|| "(none)".to_string())
    ));
    lines.push(format!(
        "- Location: {}",
        meta.location
            .as_deref()
            .map(sanitize_markdown)
            .unwrap_or_else(|| "(missing)".to_string())
    ));
    lines.push(format!(
        "- Container state: {}",
        meta.container_state
            .as_deref()
            .map(sanitize_markdown)
            .unwrap_or_else(|| "(none)".to_string())
    ));
    let format_list = |values: &[String]| -> String {
        if values.is_empty() {
            "(none)".to_string()
        } else {
            values
                .iter()
                .map(|value| sanitize_markdown(value))
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    lines.push(format!("- Abilities: {}", format_list(&meta.abilities)));
    lines.push(format!(
        "- Requirements: {}",
        format_list(&meta.requirements)
    ));
    lines.join("\n")
}

fn format_npc_hover(id: &str, meta: &NpcMetadata) -> String {
    let mut lines = vec![format!("**NPC:** {}", sanitize_markdown(id))];
    lines.push(format!(
        "- Name: {}",
        meta.name
            .as_deref()
            .map(sanitize_markdown)
            .unwrap_or_else(|| "(missing)".to_string())
    ));
    lines.push(format!(
        "- Description: {}",
        meta.description
            .as_deref()
            .map(sanitize_markdown)
            .unwrap_or_else(|| "(missing)".to_string())
    ));
    lines.push(format!(
        "- Location: {}",
        meta.location
            .as_deref()
            .map(sanitize_markdown)
            .unwrap_or_else(|| "(missing)".to_string())
    ));
    lines.push(format!(
        "- State: {}",
        meta.state
            .as_deref()
            .map(sanitize_markdown)
            .unwrap_or_else(|| "(none)".to_string())
    ));
    lines.join("\n")
}

fn format_flag_hover(id: &str, meta: &FlagMetadata) -> String {
    let mut lines = vec![format!("**Flag:** {}", sanitize_markdown(id))];
    if let Some(trigger) = &meta.defined_in {
        lines.push(format!(
            "- Defined in trigger: {}",
            sanitize_markdown(trigger)
        ));
    }
    if let Some(limit) = meta.sequence_limit {
        lines.push(format!("- Sequence limit: {}", limit));
    }
    if lines.len() == 1 {
        lines.push("- Defined in trigger: (unknown)".to_string());
    }
    lines.join("\n")
}

fn format_set_hover(id: &str, meta: &SetMetadata) -> String {
    let mut lines = vec![format!("**Set:** {}", sanitize_markdown(id))];
    lines.push(format!(
        "- Rooms: {}",
        if meta.rooms.is_empty() {
            "(none)".to_string()
        } else {
            meta.rooms.join(", ")
        }
    ));
    lines.join("\n")
}

fn range_from_node(document: &Document, node: &Node) -> Range {
    Range {
        start: document.position_at(node.start_byte()),
        end: document.position_at(node.end_byte()),
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

fn normalize_flag_reference(name: &str, range: &Range) -> (String, Option<Range>) {
    let Some((base, _)) = name.split_once('#') else {
        return (name.to_string(), None);
    };

    if base.is_empty() {
        return (name.to_string(), None);
    }

    let base_len = base.chars().map(|ch| ch.len_utf16() as u32).sum();
    let end_character = range
        .start
        .character
        .saturating_add(base_len)
        .min(range.end.character);
    let rename_range = Range {
        start: range.start,
        end: Position {
            line: range.start.line,
            character: end_character,
        },
    };

    (base.to_string(), Some(rename_range))
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

    if let Some(block) = named_child_by_kind(room_node, "room_block") {
        let mut cursor = block.walk();
        for child in block.named_children(&mut cursor) {
            match child.kind() {
                "room_name" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let raw = slice_text(text, &name_node);
                        name = Some(normalize_string_literal(raw));
                    }
                }
                "room_desc" => {
                    if let Some(desc_node) = child.child_by_field_name("description") {
                        let raw = slice_text(text, &desc_node);
                        description = Some(normalize_string_literal(raw));
                    }
                }
                "room_exit" => {
                    if let Some(dest) = child.child_by_field_name("dest") {
                        exits.push(slice_text(text, &dest).trim().to_string());
                    }
                }
                _ => {}
            }
        }
    }

    (name, description, exits)
}

fn format_location_node(location_node: &Node, text: &str) -> String {
    if let Some(room) = named_child_by_kind(location_node, "room_id")
        .or_else(|| named_child_by_kind(location_node, "_room_ref"))
    {
        return format!("room {}", sanitize_markdown(slice_text(text, &room).trim()));
    }
    if let Some(chest) = named_child_by_kind(location_node, "chest_id") {
        return format!(
            "chest {}",
            sanitize_markdown(slice_text(text, &chest).trim())
        );
    }
    if let Some(npc) = named_child_by_kind(location_node, "npc_id") {
        return format!("npc {}", sanitize_markdown(slice_text(text, &npc).trim()));
    }
    if let Some(spawn_note) = named_child_by_kind(location_node, "spawn_note") {
        return format!(
            "nowhere {}",
            sanitize_markdown(&normalize_string_literal(slice_text(text, &spawn_note)))
        );
    }

    let raw = slice_text(text, location_node).trim();
    sanitize_markdown(
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

    if let Some(block) = named_child_by_kind(item_node, "item_block") {
        let mut cursor = block.walk();
        for child in block.named_children(&mut cursor) {
            match child.kind() {
                "item_name_stmt" => {
                    if let Some(name_node) = child.child_by_field_name("item_name") {
                        name = Some(normalize_string_literal(slice_text(text, &name_node)));
                    }
                }
                "item_desc_stmt" => {
                    if let Some(desc_node) = child.child_by_field_name("item_description") {
                        description = Some(normalize_string_literal(slice_text(text, &desc_node)));
                    }
                }
                "item_portable_stmt" => {
                    if let Some(port_node) = child.child_by_field_name("portable") {
                        portable = Some(slice_text(text, &port_node).trim() == "true");
                    }
                }
                "item_loc_stmt" => {
                    if let Some(loc_node) = named_child_by_kind(&child, "item_location") {
                        location = Some(format_location_node(&loc_node, text));
                    } else {
                        let raw = slice_text(text, &child).trim();
                        location = Some(sanitize_markdown(
                            raw.strip_prefix("location")
                                .map(|rest| rest.trim())
                                .unwrap_or(raw),
                        ));
                    }
                }
                "item_container_stmt" => {
                    if let Some(state_node) = named_child_by_kind(&child, "container_state") {
                        container_state = Some(slice_text(text, &state_node).trim().to_string());
                    }
                }
                "item_ability_stmt" => {
                    abilities.push(sanitize_markdown(slice_text(text, &child).trim()));
                }
                "item_requires_stmt" => {
                    requirements.push(sanitize_markdown(slice_text(text, &child).trim()));
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

    if let Some(block) = named_child_by_kind(npc_node, "npc_block") {
        let mut cursor = block.walk();
        for child in block.named_children(&mut cursor) {
            match child.kind() {
                "npc_name_stmt" => {
                    if let Some(name_node) = child.child_by_field_name("npc_name") {
                        name = Some(normalize_string_literal(slice_text(text, &name_node)));
                    }
                }
                "npc_desc_stmt" => {
                    if let Some(desc_node) = child.child_by_field_name("npc_description") {
                        description = Some(normalize_string_literal(slice_text(text, &desc_node)));
                    }
                }
                "npc_loc_stmt" => {
                    if let Some(loc_node) = named_child_by_kind(&child, "npc_location") {
                        location = Some(format_location_node(&loc_node, text));
                    } else {
                        let raw = slice_text(text, &child).trim();
                        location = Some(sanitize_markdown(
                            raw.strip_prefix("location")
                                .map(|rest| rest.trim())
                                .unwrap_or(raw),
                        ));
                    }
                }
                "npc_state_stmt" => {
                    if let Some(state_node) = named_child_by_kind(&child, "npc_state") {
                        state = Some(slice_text(text, &state_node).trim().to_string());
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
                return Some(normalize_string_literal(slice_text(text, &name_node)));
            }
            break;
        }
        current = parent;
    }
    None
}

fn extract_flag_metadata(action_node: &Node, text: &str) -> (Option<String>, Option<i64>) {
    let defined_in = find_trigger_name(*action_node, text);
    let limit = if action_node.kind() == "action_add_seq" {
        let mut cursor = action_node.walk();
        let mut result = None;
        for child in action_node.named_children(&mut cursor) {
            if child.kind() == "number" {
                if let Ok(value) = slice_text(text, &child).trim().parse::<i64>() {
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
    if let Some(list_node) = named_child_by_kind(set_node, "set_list")
        .or_else(|| named_child_by_kind(set_node, "room_list"))
    {
        let mut cursor = list_node.walk();
        let mut rooms = Vec::new();
        for child in list_node.named_children(&mut cursor) {
            match child.kind() {
                "room_id" | "_room_ref" => {
                    rooms.push(sanitize_markdown(slice_text(text, &child).trim()))
                }
                _ => {}
            }
        }
        rooms
    } else {
        Vec::new()
    }
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

fn field_name_for_child<'tree>(parent: &Node<'tree>, child: &Node<'tree>) -> Option<&'static str> {
    for i in 0..parent.child_count() {
        if let Some(candidate) = parent.child(i) {
            if candidate.id() == child.id() {
                return parent.field_name_for_child(i as u32);
            }
        }
    }
    None
}

fn symbol_kind_from_kind(kind: &str) -> Option<SymbolKind> {
    match kind {
        "room_id" | "_room_ref" => Some(SymbolKind::Room),
        "item_id" | "_item_ref" => Some(SymbolKind::Item),
        "npc_id" | "_npc_ref" => Some(SymbolKind::Npc),
        "flag_name" | "_flag_ref" => Some(SymbolKind::Flag),
        "set_name" | "_set_ref" => Some(SymbolKind::Set),
        _ => None,
    }
}

fn symbol_kind_from_field(field_name: &str) -> Option<SymbolKind> {
    match field_name {
        "room_id" | "dest" | "room" | "from_room" | "to_room" => Some(SymbolKind::Room),
        "item_id" | "tool_id" | "target_id" | "container_id" | "chest_id" => Some(SymbolKind::Item),
        "npc_id" => Some(SymbolKind::Npc),
        "flag_name" | "flag" => Some(SymbolKind::Flag),
        "set_name" => Some(SymbolKind::Set),
        _ => None,
    }
}

fn is_definition_node<'tree>(node: &Node<'tree>, symbol_type: SymbolKind) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        let kind = parent.kind();
        let is_definition = match symbol_type {
            SymbolKind::Room => kind == "room_def",
            SymbolKind::Item => kind == "item_def",
            SymbolKind::Npc => kind == "npc_def",
            SymbolKind::Flag => kind == "action_add_flag" || kind == "action_add_seq",
            SymbolKind::Set => kind == "set_decl",
        };

        if is_definition {
            return true;
        }

        current = parent.parent();
    }

    false
}

fn is_definition_field(parent_kind: &str, field_name: &str, symbol_type: SymbolKind) -> bool {
    match symbol_type {
        SymbolKind::Room => parent_kind == "room_def" && field_name == "room_id",
        SymbolKind::Item => parent_kind == "item_def" && field_name == "item_id",
        SymbolKind::Npc => parent_kind == "npc_def" && field_name == "npc_id",
        SymbolKind::Flag => {
            (parent_kind == "action_add_flag" && field_name == "flag")
                || (parent_kind == "action_add_seq" && field_name == "flag_name")
        }
        SymbolKind::Set => parent_kind == "set_decl" && field_name == "name",
    }
}

fn symbol_kind_from_children<'tree>(
    node: &Node<'tree>,
    offset: usize,
    stack: &mut Vec<Node<'tree>>,
) -> Option<SymbolKind> {
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
            if let Some(symbol_type) = symbol_kind_from_field(field_name) {
                if is_definition_field(node.kind(), field_name, symbol_type) {
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

fn symbol_kind_from_syntax<'tree>(node: Node<'tree>, offset: usize) -> Option<SymbolKind> {
    let mut stack = vec![node];
    let mut visited = HashSet::new();

    while let Some(n) = stack.pop() {
        if !visited.insert(n.id()) {
            continue;
        }

        let parent = n.parent();

        if let Some(symbol_type) = symbol_kind_from_kind(n.kind()) {
            if is_definition_node(&n, symbol_type) {
                if let Some(parent) = parent {
                    stack.push(parent);
                }
                continue;
            }

            let blocked = parent.as_ref().and_then(|p| {
                field_name_for_child(p, &n)
                    .map(|field| is_definition_field(p.kind(), field, symbol_type))
            });

            if !blocked.unwrap_or(false) {
                return Some(symbol_type);
            }
        }

        if let Some(symbol_type) = symbol_kind_from_children(&n, offset, &mut stack) {
            return Some(symbol_type);
        }

        if let Some(parent) = parent {
            stack.push(parent);
        }
    }

    None
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

    fn completion_at(source: &str, position: Position) -> Option<SymbolKind> {
        let tree = parse_source(source);
        let root = tree.root_node();
        let offset = Document::new(source.to_string())
            .offset(position)
            .expect("offset");

        let mut candidates = vec![offset];
        if offset > 0 {
            candidates.push(offset - 1);
        }

        for candidate in candidates {
            if let Some(node) = node_at_offset(&root, candidate) {
                if let Some(symbol) = symbol_kind_from_syntax(node, candidate) {
                    return Some(symbol);
                }
            }
        }

        None
    }

    fn position_for_token(source: &str, line: usize, token: &str, offset: usize) -> Position {
        let line_str = source.lines().nth(line).expect("line missing");
        let start = line_str.find(token).expect("token missing on line");
        let prefix = &line_str[..start + offset];
        let character = prefix.chars().map(|ch| ch.len_utf16() as u32).sum();
        Position {
            line: line as u32,
            character,
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
        assert_eq!(symbol, Some(SymbolKind::Room));
    }

    #[test]
    fn detects_item_reference_context_in_conditions() {
        let source = "trigger \"test-trigger\" when always {\n    if has item test_item {\n        do show \"\"\n    }\n}\n\nitem test_item {\n    name \"Item\"\n}\n";
        let position = position_for_token(source, 1, "test_item", 2);
        let symbol = completion_at(source, position);
        assert_eq!(symbol, Some(SymbolKind::Item));
    }

    #[test]
    fn detects_npc_reference_context_in_events() {
        let source = "trigger \"npc-trigger\" when talk to npc test_npc {\n    do show \"\"\n}\n\nnpc test_npc {\n    name \"Npc\"\n}\n";
        let position = position_for_token(source, 0, "test_npc", 2);
        let symbol = completion_at(source, position);
        assert_eq!(symbol, Some(SymbolKind::Npc));
    }

    #[test]
    fn detects_flag_reference_context_in_actions() {
        let source = "trigger \"flag-trigger\" when always {\n    if has flag quest_flag {\n        do show \"\"\n    }\n}\n";
        let position = position_for_token(source, 1, "quest_flag", 2);
        let symbol = completion_at(source, position);
        assert_eq!(symbol, Some(SymbolKind::Flag));
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
    fn formats_room_hover_markdown() {
        let meta = RoomMetadata {
            name: Some("Test Room".into()),
            description: Some("A description".into()),
            exits: vec!["north-hall".into(), "south-porch".into()],
        };

        let hover = format_room_hover("test-room", &meta);
        assert!(hover.contains("**Room:** test-room"));
        assert!(hover.contains("Test Room"));
        assert!(hover.contains("north-hall"));
    }

    #[test]
    fn formats_item_hover_lists_abilities() {
        let meta = ItemMetadata {
            name: Some("Widget".into()),
            description: Some("Useful widget".into()),
            portable: Some(true),
            location: Some("room lab".into()),
            container_state: Some("closed".into()),
            abilities: vec!["ability Unlock".into()],
            requirements: vec!["requires ability Use to interact".into()],
        };

        let hover = format_item_hover("widget", &meta);
        assert!(hover.contains("Abilities: ability Unlock"));
        assert!(hover.contains("Requirements: requires ability Use to interact"));
    }
}
