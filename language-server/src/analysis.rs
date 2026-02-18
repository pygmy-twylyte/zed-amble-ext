use crate::backend::Backend;
use crate::symbols::{
    sanitize_markdown, FlagMetadata, ItemMetadata, Movability, NpcMetadata, RoomMetadata,
    SetMetadata, SymbolDefinition, SymbolIndex, SymbolKind, SymbolLocation, SymbolMetadata,
    SymbolOccurrence, SymbolReference,
};
use crate::text::Document;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DiagnosticTag, InitializeParams, Position, Range, Url,
};
use tree_sitter::{Node, QueryCursor, StreamingIterator};
use walkdir::{DirEntry, WalkDir};

const IGNORED_DIRECTORIES: &[&str] = &[".git", "node_modules", "target", "dist", "build"];
const HOVER_DESCRIPTION_MAX_CHARS: usize = 100;
const SCHEDULE_WRAPPER_PREFIX: &str = "trigger \"__amble_schedule__\" when always ";

/// Captures a `player_start` location plus source span for diagnostics.
#[derive(Debug, Clone)]
pub(crate) struct PlayerStart {
    pub room_id: String,
    pub range: Range,
    pub uri: Url,
}

#[derive(Debug, Clone)]
struct ScheduleFlagDefinition {
    id: String,
    range: Range,
    defined_in: Option<String>,
    sequence_limit: Option<i64>,
}

#[derive(Debug, Clone)]
struct ScheduleSymbolReference {
    kind: SymbolKind,
    id: String,
    raw_id: String,
    range: Range,
    rename_range: Option<Range>,
}

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

            let modified = directory_modified(&dir);
            if !self.should_scan_directory(&dir, modified) {
                continue;
            }

            for entry in WalkDir::new(&dir)
                .follow_links(false)
                .into_iter()
                .filter_entry(|entry| should_visit_entry(entry))
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

            self.scanned_directories.insert(dir.clone(), modified);
        }
    }

    fn should_scan_directory(&self, dir: &Path, modified: Option<SystemTime>) -> bool {
        match self.scanned_directories.get(dir) {
            Some(previous) => needs_rescan(previous.value().clone(), modified),
            None => true,
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
                    movability,
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
                            movability,
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

        let (schedule_flag_definitions, schedule_symbol_references) = {
            let mut parser = self.parser.lock();
            (
                collect_schedule_flag_definitions(
                    &document,
                    root_node,
                    text,
                    &mut parser,
                    &self.queries.flag_definitions,
                ),
                collect_schedule_symbol_references(
                    &document,
                    root_node,
                    text,
                    &mut parser,
                    &self.queries.room_references,
                    &self.queries.item_references,
                    &self.queries.npc_references,
                    &self.queries.flag_references,
                    &self.queries.set_references,
                ),
            )
        };

        for schedule_definition in schedule_flag_definitions {
            let location = SymbolLocation {
                uri: uri.clone(),
                range: schedule_definition.range.clone(),
                rename_range: None,
            };

            self.symbols.flags.insert_definition(
                schedule_definition.id.clone(),
                SymbolDefinition {
                    location,
                    metadata: SymbolMetadata::Flag(FlagMetadata {
                        defined_in: schedule_definition.defined_in,
                        sequence_limit: schedule_definition.sequence_limit,
                    }),
                },
            );

            occurrences.push(SymbolOccurrence {
                kind: SymbolKind::Flag,
                id: schedule_definition.id,
                range: schedule_definition.range,
            });
        }

        for schedule_reference in schedule_symbol_references {
            let location = SymbolLocation {
                uri: uri.clone(),
                range: schedule_reference.range.clone(),
                rename_range: schedule_reference.rename_range,
            };

            match schedule_reference.kind {
                SymbolKind::Room => self.symbols.rooms.add_reference(
                    schedule_reference.id.clone(),
                    SymbolReference {
                        location,
                        raw_id: schedule_reference.raw_id,
                    },
                ),
                SymbolKind::Item => self.symbols.items.add_reference(
                    schedule_reference.id.clone(),
                    SymbolReference {
                        location,
                        raw_id: schedule_reference.raw_id,
                    },
                ),
                SymbolKind::Npc => self.symbols.npcs.add_reference(
                    schedule_reference.id.clone(),
                    SymbolReference {
                        location,
                        raw_id: schedule_reference.raw_id,
                    },
                ),
                SymbolKind::Flag => self.symbols.flags.add_reference(
                    schedule_reference.id.clone(),
                    SymbolReference {
                        location,
                        raw_id: schedule_reference.raw_id,
                    },
                ),
                SymbolKind::Set => self.symbols.sets.add_reference(
                    schedule_reference.id.clone(),
                    SymbolReference {
                        location,
                        raw_id: schedule_reference.raw_id,
                    },
                ),
            }

            occurrences.push(SymbolOccurrence {
                kind: schedule_reference.kind,
                id: schedule_reference.id,
                range: schedule_reference.range,
            });
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

        let player_starts = collect_player_starts(&document, root_node, text, uri);
        self.player_starts.insert(uri_str.clone(), player_starts);

        self.document_symbols.insert(uri_str.clone(), occurrences);
        self.documents.insert(uri_str, document);
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

        self.append_duplicate_definition_diagnostics(uri, &mut diagnostics);
        self.append_unused_definition_diagnostics(uri, &mut diagnostics);
        self.append_metadata_diagnostics(uri, &mut diagnostics);
        self.append_world_consistency_diagnostics(uri, &mut diagnostics);
        self.append_flag_sequence_diagnostics(uri, &mut diagnostics);

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }

    /// Flags duplicate definitions, downgrading flags to hints because multiple triggers
    /// may intentionally set the same game state.
    fn append_duplicate_definition_diagnostics(
        &self,
        uri: &Url,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        self.append_duplicate_diagnostics_for_index(
            uri,
            diagnostics,
            SymbolKind::Room,
            &self.symbols.rooms,
        );
        self.append_duplicate_diagnostics_for_index(
            uri,
            diagnostics,
            SymbolKind::Item,
            &self.symbols.items,
        );
        self.append_duplicate_diagnostics_for_index(
            uri,
            diagnostics,
            SymbolKind::Npc,
            &self.symbols.npcs,
        );
        self.append_duplicate_flag_diagnostics(uri, diagnostics);
        self.append_duplicate_diagnostics_for_index(
            uri,
            diagnostics,
            SymbolKind::Set,
            &self.symbols.sets,
        );
    }

    fn append_duplicate_diagnostics_for_index(
        &self,
        uri: &Url,
        diagnostics: &mut Vec<Diagnostic>,
        kind: SymbolKind,
        index: &SymbolIndex,
    ) {
        for entry in index.duplicate_definitions_iter() {
            let id = entry.key().clone();
            let duplicates = entry.value().clone();
            drop(entry);

            let mut definitions = Vec::new();
            if let Some(primary) = index.definition(&id) {
                definitions.push(primary.clone());
            }
            definitions.extend(duplicates);

            for def in definitions {
                if def.location.uri == *uri {
                    diagnostics.push(Diagnostic {
                        range: def.location.range,
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: None,
                        code_description: None,
                        source: Some("amble-lsp".to_string()),
                        message: format!("Duplicate {} definition: '{}'", kind.label(), id),
                        related_information: None,
                        tags: None,
                        data: None,
                    });
                }
            }
        }
    }

    /// Emits a lightweight reminder when multiple triggers define the same flag. Flags represent
    /// global state, so we only warn to help authors keep alternate solutions aligned.
    fn append_duplicate_flag_diagnostics(&self, uri: &Url, diagnostics: &mut Vec<Diagnostic>) {
        for entry in self.symbols.flags.duplicate_definitions_iter() {
            let id = entry.key().clone();
            let duplicates = entry.value().clone();
            drop(entry);

            let mut definitions = Vec::new();
            if let Some(primary) = self.symbols.flags.definition(&id) {
                definitions.push(primary.clone());
            }
            definitions.extend(duplicates);

            for def in definitions {
                if def.location.uri == *uri {
                    diagnostics.push(Diagnostic {
                        range: def.location.range,
                        severity: Some(DiagnosticSeverity::HINT),
                        code: None,
                        code_description: None,
                        source: Some("amble-lsp".to_string()),
                        message: format!(
                            "Flag '{}' is defined in multiple triggers; ensure these paths stay in sync",
                            id
                        ),
                        related_information: None,
                        tags: Some(vec![DiagnosticTag::UNNECESSARY]),
                        data: None,
                    });
                }
            }
        }
    }

    fn append_unused_definition_diagnostics(&self, uri: &Url, diagnostics: &mut Vec<Diagnostic>) {
        self.append_unused_for_index(uri, diagnostics, SymbolKind::Room, &self.symbols.rooms);
        self.append_unused_for_index(uri, diagnostics, SymbolKind::Item, &self.symbols.items);
        self.append_unused_for_index(uri, diagnostics, SymbolKind::Npc, &self.symbols.npcs);
        self.append_unused_for_index(uri, diagnostics, SymbolKind::Flag, &self.symbols.flags);
        self.append_unused_for_index(uri, diagnostics, SymbolKind::Set, &self.symbols.sets);
    }

    fn append_unused_for_index(
        &self,
        uri: &Url,
        diagnostics: &mut Vec<Diagnostic>,
        kind: SymbolKind,
        index: &SymbolIndex,
    ) {
        for entry in index.definitions_iter() {
            let id = entry.key().clone();
            let definition = entry.value().clone();
            drop(entry);

            if definition.location.uri != *uri {
                continue;
            }

            let has_references = {
                if let Some(refs) = index.references(&id) {
                    let used = !refs.is_empty();
                    drop(refs);
                    used
                } else {
                    false
                }
            };

            if !has_references {
                diagnostics.push(Diagnostic {
                    range: definition.location.range,
                    severity: Some(DiagnosticSeverity::HINT),
                    code: None,
                    code_description: None,
                    source: Some("amble-lsp".to_string()),
                    message: format!("{} '{}' is never referenced", kind.label(), id),
                    related_information: None,
                    tags: Some(vec![DiagnosticTag::UNNECESSARY]),
                    data: None,
                });
            }
        }
    }

    fn append_metadata_diagnostics(&self, uri: &Url, diagnostics: &mut Vec<Diagnostic>) {
        self.append_metadata_for_index(uri, diagnostics, &self.symbols.rooms);
        self.append_metadata_for_index(uri, diagnostics, &self.symbols.items);
        self.append_metadata_for_index(uri, diagnostics, &self.symbols.npcs);
    }

    fn append_metadata_for_index(
        &self,
        uri: &Url,
        diagnostics: &mut Vec<Diagnostic>,
        index: &SymbolIndex,
    ) {
        for entry in index.definitions_iter() {
            let id = entry.key().clone();
            let definition = entry.value().clone();
            drop(entry);

            if definition.location.uri != *uri {
                continue;
            }

            for message in metadata_issues_for_definition(&id, &definition) {
                diagnostics.push(Diagnostic {
                    range: definition.location.range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: None,
                    code_description: None,
                    source: Some("amble-lsp".to_string()),
                    message,
                    related_information: None,
                    tags: None,
                    data: None,
                });
            }
        }
    }

    /// Ensures there is at least one `player_start`, and warns if multiple start rooms exist.
    fn append_world_consistency_diagnostics(&self, uri: &Url, diagnostics: &mut Vec<Diagnostic>) {
        let start_entries: Vec<PlayerStart> = self
            .player_starts
            .iter()
            .flat_map(|entry| entry.value().clone())
            .collect();

        if start_entries.is_empty() {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position::default(),
                    end: Position::default(),
                },
                severity: Some(DiagnosticSeverity::WARNING),
                code: None,
                code_description: None,
                source: Some("amble-lsp".to_string()),
                message: "No player start room defined in this workspace".to_string(),
                related_information: None,
                tags: None,
                data: None,
            });
            return;
        }

        if start_entries.len() > 1 {
            let rooms: Vec<String> = start_entries
                .iter()
                .map(|start| start.room_id.clone())
                .collect();
            for start in start_entries.iter().filter(|start| &start.uri == uri) {
                diagnostics.push(Diagnostic {
                    range: start.range.clone(),
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: None,
                    code_description: None,
                    source: Some("amble-lsp".to_string()),
                    message: format!(
                        "Multiple player starts defined (rooms: {})",
                        rooms.join(", ")
                    ),
                    related_information: None,
                    tags: None,
                    data: None,
                });
            }
        }
    }

    /// Validates that sequence-style flag references stay within bounds and avoids referencing
    /// non-sequence flags with a `#N` suffix.
    fn append_flag_sequence_diagnostics(&self, uri: &Url, diagnostics: &mut Vec<Diagnostic>) {
        for entry in self.symbols.flags.definitions_iter() {
            let id = entry.key().clone();
            let definition = entry.value().clone();
            drop(entry);

            let meta = match &definition.metadata {
                SymbolMetadata::Flag(meta) => meta.clone(),
                _ => continue,
            };

            if let Some(refs) = self.symbols.flags.references(&id) {
                for reference in refs.iter() {
                    if reference.location.uri != *uri {
                        continue;
                    }
                    if let Some(index) = flag_sequence_index(&reference.raw_id) {
                        if let Some(limit) = meta.sequence_limit {
                            if index >= limit {
                                diagnostics.push(Diagnostic {
                                    range: reference.location.range,
                                    severity: Some(DiagnosticSeverity::WARNING),
                                    code: None,
                                    code_description: None,
                                    source: Some("amble-lsp".to_string()),
                                    message: format!(
                                        "Flag '{}' sequence limit is {} but reference uses index {}",
                                        id, limit, index
                                    ),
                                    related_information: None,
                                    tags: None,
                                    data: None,
                                });
                            }
                        } else {
                            diagnostics.push(Diagnostic {
                                range: reference.location.range,
                                severity: Some(DiagnosticSeverity::WARNING),
                                code: None,
                                code_description: None,
                                source: Some("amble-lsp".to_string()),
                                message: format!(
                                    "Flag '{}' is defined as a single flag but referenced as '{}'",
                                    id, reference.raw_id
                                ),
                                related_information: None,
                                tags: None,
                                data: None,
                            });
                        }
                    }
                }
                drop(refs);
            }
        }
    }
}

fn metadata_issues_for_definition(id: &str, def: &SymbolDefinition) -> Vec<String> {
    match &def.metadata {
        SymbolMetadata::Room(meta) => {
            let mut issues = Vec::new();
            if text_missing(&meta.name) {
                issues.push(format!("Room '{}' is missing a name", id));
            }
            if text_missing(&meta.description) {
                issues.push(format!("Room '{}' is missing a description", id));
            }
            issues
        }
        SymbolMetadata::Item(meta) => {
            let mut issues = Vec::new();
            if text_missing(&meta.location) {
                issues.push(format!("Item '{}' is missing a location", id));
            }
            if meta.movability.is_none() {
                issues.push(format!("Item '{}' is missing a movability setting", id));
            }
            issues
        }
        SymbolMetadata::Npc(meta) => {
            let mut issues = Vec::new();
            if text_missing(&meta.location) {
                issues.push(format!("NPC '{}' is missing a location", id));
            }
            if text_missing(&meta.state) {
                issues.push(format!("NPC '{}' is missing a starting state", id));
            }
            issues
        }
        _ => Vec::new(),
    }
}

fn text_missing(value: &Option<String>) -> bool {
    value
        .as_ref()
        .map(|text| text.trim().is_empty())
        .unwrap_or(true)
}

fn should_visit_entry(entry: &DirEntry) -> bool {
    if entry.file_type().is_dir() {
        if let Some(name) = entry.file_name().to_str() {
            return !IGNORED_DIRECTORIES
                .iter()
                .any(|ignored| ignored.eq_ignore_ascii_case(name));
        }
    }
    true
}

fn directory_modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

fn needs_rescan(previous: Option<SystemTime>, current: Option<SystemTime>) -> bool {
    match (previous, current) {
        (None, _) => true,
        (Some(_), None) => true,
        (Some(prev), Some(curr)) => match curr.duration_since(prev) {
            Ok(elapsed) => !elapsed.is_zero(),
            Err(_) => true,
        },
    }
}

pub(crate) fn format_hover(
    id: &str,
    def: &SymbolDefinition,
    relative_path: Option<&str>,
) -> String {
    match &def.metadata {
        SymbolMetadata::Room(meta) => format_room_hover(id, meta, relative_path),
        SymbolMetadata::Item(meta) => format_item_hover(id, meta, relative_path),
        SymbolMetadata::Npc(meta) => format_npc_hover(id, meta, relative_path),
        SymbolMetadata::Flag(meta) => format_flag_hover(id, meta, relative_path),
        SymbolMetadata::Set(meta) => format_set_hover(id, meta, relative_path),
    }
}

fn format_room_hover(id: &str, meta: &RoomMetadata, relative_path: Option<&str>) -> String {
    let mut lines = vec![entity_title_line("ROOM", meta.name.as_deref(), id)];
    if let Some(location_line) = definition_path_line(relative_path) {
        lines.push(location_line);
    }
    lines.push(format!(
        "- **Description:** {}",
        truncate_description(meta.description.as_deref())
    ));
    lines.push(format!(
        "- **Exits:** {}",
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

fn format_item_hover(id: &str, meta: &ItemMetadata, relative_path: Option<&str>) -> String {
    let mut lines = vec![entity_title_line("ITEM", meta.name.as_deref(), id)];
    if let Some(location_line) = definition_path_line(relative_path) {
        lines.push(location_line);
    }
    lines.push(format!(
        "- **Description:** {}",
        truncate_description(meta.description.as_deref())
    ));
    lines.push(format!(
        "- **Movability:** {}",
        describe_movability(meta.movability.as_ref())
    ));
    lines.push(format!(
        "- **Location:** {}",
        meta.location
            .as_deref()
            .map(sanitize_markdown)
            .unwrap_or_else(|| "(missing)".to_string())
    ));
    lines.push(format!(
        "- **Container state:** {}",
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
    lines.push(format!("- **Abilities:** {}", format_list(&meta.abilities)));
    lines.push(format!(
        "- **Requires:** {}",
        format_list(&meta.requirements)
    ));
    lines.join("\n")
}

fn format_npc_hover(id: &str, meta: &NpcMetadata, relative_path: Option<&str>) -> String {
    let mut lines = vec![entity_title_line("NPC", meta.name.as_deref(), id)];
    if let Some(location_line) = definition_path_line(relative_path) {
        lines.push(location_line);
    }
    lines.push(format!(
        "- **Description:** {}",
        truncate_description(meta.description.as_deref())
    ));
    lines.push(format!(
        "- **Location:** {}",
        meta.location
            .as_deref()
            .map(sanitize_markdown)
            .unwrap_or_else(|| "(missing)".to_string())
    ));
    lines.push(format!(
        "- **State:** {}",
        meta.state
            .as_deref()
            .map(sanitize_markdown)
            .unwrap_or_else(|| "(none)".to_string())
    ));
    lines.join("\n")
}

fn format_flag_hover(id: &str, meta: &FlagMetadata, relative_path: Option<&str>) -> String {
    let mut lines = vec![entity_title_line("FLAG", None, id)];
    if let Some(location_line) = definition_path_line(relative_path) {
        lines.push(location_line);
    }
    if let Some(trigger) = &meta.defined_in {
        lines.push(format!(
            "- **Defined in trigger:** {}",
            sanitize_markdown(trigger)
        ));
    }
    if let Some(limit) = meta.sequence_limit {
        lines.push(format!("- **Sequence limit:** {}", limit));
    }
    if lines.len() == 1 {
        lines.push("- **Defined in trigger:** (unknown)".to_string());
    }
    lines.join("\n")
}

fn format_set_hover(id: &str, meta: &SetMetadata, relative_path: Option<&str>) -> String {
    let mut lines = vec![entity_title_line("SET", None, id)];
    if let Some(location_line) = definition_path_line(relative_path) {
        lines.push(location_line);
    }
    lines.push(format!(
        "- **Rooms:** {}",
        if meta.rooms.is_empty() {
            "(none)".to_string()
        } else {
            meta.rooms
                .iter()
                .map(|room| sanitize_markdown(room))
                .collect::<Vec<_>>()
                .join(", ")
        }
    ));
    lines.join("\n")
}

fn definition_path_line(relative_path: Option<&str>) -> Option<String> {
    relative_path.map(|path| {
        let shortened = shorten_to_data_root(path);
        format!("- **File:** {}", sanitize_markdown(&shortened))
    })
}

fn entity_title_line(kind: &str, display_name: Option<&str>, id: &str) -> String {
    let kind_label = kind.to_ascii_uppercase();
    let sanitized_id = sanitize_markdown(id);
    let display = display_name
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(sanitize_markdown);

    if let Some(name) = display {
        if name == sanitized_id {
            format!("**{}:** {}", kind_label, sanitized_id)
        } else {
            format!("**{}:** {} ({})", kind_label, name, sanitized_id)
        }
    } else {
        format!("**{}:** {}", kind_label, sanitized_id)
    }
}

fn shorten_to_data_root(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let components: Vec<&str> = normalized
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    if let Some(idx) = components
        .iter()
        .position(|segment| segment.eq_ignore_ascii_case("data"))
    {
        if idx + 2 <= components.len() {
            let world_relative = components[idx + 2..].join("/");
            if !world_relative.is_empty() {
                return world_relative;
            }
        }
    }

    components.join("/")
}

fn describe_movability(movability: Option<&Movability>) -> String {
    match movability {
        Some(Movability::Free) => "free".to_string(),
        Some(Movability::Fixed(note)) => match note {
            Some(text) if !text.trim().is_empty() => {
                format!("fixed ({})", sanitize_markdown(text))
            }
            _ => "fixed".to_string(),
        },
        Some(Movability::Restricted(note)) => match note {
            Some(text) if !text.trim().is_empty() => {
                format!("restricted ({})", sanitize_markdown(text))
            }
            _ => "restricted".to_string(),
        },
        None => "(none)".to_string(),
    }
}

fn truncate_description(value: Option<&str>) -> String {
    match value {
        Some(text) if !text.trim().is_empty() => {
            let sanitized = sanitize_markdown(text);
            truncate_string(sanitized, HOVER_DESCRIPTION_MAX_CHARS)
        }
        _ => "(missing)".to_string(),
    }
}

fn truncate_string(value: String, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value
    } else {
        let truncated: String = value.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
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
    Option<Movability>,
    Option<String>,
    Option<String>,
    Vec<String>,
    Vec<String>,
) {
    let mut name = None;
    let mut description = None;
    let mut movability = None;
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
                "item_movability_stmt" => {
                    if let Some(mov_node) = child.child_by_field_name("movability") {
                        movability = extract_movability(&mov_node, text);
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
                    let ability = child
                        .child_by_field_name("ability")
                        .map(|node| sanitize_markdown(slice_text(text, &node).trim()));
                    let target = child
                        .child_by_field_name("target_id")
                        .map(|node| sanitize_markdown(slice_text(text, &node).trim()));

                    match ability {
                        Some(name) => {
                            if let Some(target) = target {
                                abilities.push(format!("{} ({})", name, target));
                            } else {
                                abilities.push(name);
                            }
                        }
                        None => {
                            abilities.push(sanitize_markdown(slice_text(text, &child).trim()));
                        }
                    }
                }
                "item_requires_stmt" => {
                    let ability = child
                        .child_by_field_name("ability")
                        .map(|node| sanitize_markdown(slice_text(text, &node).trim()));
                    let interaction = child
                        .child_by_field_name("interaction")
                        .map(|node| sanitize_markdown(slice_text(text, &node).trim()));

                    match (ability, interaction) {
                        (Some(ability), Some(interaction)) => {
                            requirements.push(format!("{} -> {}", ability, interaction));
                        }
                        _ => requirements.push(sanitize_markdown(slice_text(text, &child).trim())),
                    }
                }
                _ => {}
            }
        }
    }

    (
        name,
        description,
        movability,
        location,
        container_state,
        abilities,
        requirements,
    )
}

fn extract_movability(node: &Node, text: &str) -> Option<Movability> {
    let raw = slice_text(text, node).trim();
    let lowered = raw.to_ascii_lowercase();
    if lowered == "free" {
        return Some(Movability::Free);
    }

    let note = node
        .child_by_field_name("note")
        .map(|note_node| normalize_string_literal(slice_text(text, &note_node)));

    if lowered.starts_with("fixed") {
        Some(Movability::Fixed(note))
    } else if lowered.starts_with("restricted") {
        Some(Movability::Restricted(note))
    } else {
        None
    }
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
    let limit = extract_flag_sequence_limit(action_node, text);
    (defined_in, limit)
}

fn extract_flag_sequence_limit(action_node: &Node, text: &str) -> Option<i64> {
    if action_node.kind() != "action_add_seq" {
        return None;
    }

    let mut cursor = action_node.walk();
    for child in action_node.named_children(&mut cursor) {
        if child.kind() == "number" {
            if let Ok(value) = slice_text(text, &child).trim().parse::<i64>() {
                return Some(value);
            }
        }
    }

    None
}

fn collect_schedule_nodes<'tree>(root: Node<'tree>) -> Vec<(Node<'tree>, Node<'tree>)> {
    let mut schedule_nodes = Vec::new();
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        if node.kind() == "action_schedule" {
            if let Some(body_node) = node.child_by_field_name("body") {
                schedule_nodes.push((node, body_node));
            }
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            stack.push(child);
        }
    }

    schedule_nodes
}

fn remap_schedule_capture_range(
    document: &Document,
    body_node: &Node,
    body_len: usize,
    capture_node: &Node,
) -> Option<Range> {
    if capture_node.start_byte() < SCHEDULE_WRAPPER_PREFIX.len() {
        return None;
    }

    let relative_start = capture_node.start_byte() - SCHEDULE_WRAPPER_PREFIX.len();
    let relative_end = capture_node
        .end_byte()
        .saturating_sub(SCHEDULE_WRAPPER_PREFIX.len());
    if relative_end > body_len || relative_end < relative_start {
        return None;
    }

    let absolute_start = body_node.start_byte() + relative_start;
    let absolute_end = body_node.start_byte() + relative_end;
    Some(Range {
        start: document.position_at(absolute_start),
        end: document.position_at(absolute_end),
    })
}

fn collect_schedule_flag_definitions(
    document: &Document,
    root: Node,
    text: &str,
    parser: &mut tree_sitter::Parser,
    flag_definition_query: &tree_sitter::Query,
) -> Vec<ScheduleFlagDefinition> {
    let mut result = Vec::new();
    for (schedule_node, body_node) in collect_schedule_nodes(root) {
        let body_text = slice_text(text, &body_node);
        if body_text.trim().is_empty() {
            continue;
        }

        let wrapped = format!("{}{}", SCHEDULE_WRAPPER_PREFIX, body_text);
        let Some(tree) = parser.parse(&wrapped, None) else {
            continue;
        };

        let defined_in = find_trigger_name(schedule_node, text);
        let mut cursor = QueryCursor::new();
        let mut matches =
            cursor.matches(flag_definition_query, tree.root_node(), wrapped.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let flag_name = slice_text(&wrapped, &node).trim();
                if flag_name.is_empty() || node.start_byte() < SCHEDULE_WRAPPER_PREFIX.len() {
                    continue;
                }

                let Some(range) =
                    remap_schedule_capture_range(document, &body_node, body_text.len(), &node)
                else {
                    continue;
                };

                let sequence_limit = node
                    .parent()
                    .and_then(|action_node| extract_flag_sequence_limit(&action_node, &wrapped));

                result.push(ScheduleFlagDefinition {
                    id: flag_name.to_string(),
                    range,
                    defined_in: defined_in.clone(),
                    sequence_limit,
                });
            }
        }
    }

    result
}

fn collect_schedule_symbol_references(
    document: &Document,
    root: Node,
    text: &str,
    parser: &mut tree_sitter::Parser,
    room_reference_query: &tree_sitter::Query,
    item_reference_query: &tree_sitter::Query,
    npc_reference_query: &tree_sitter::Query,
    flag_reference_query: &tree_sitter::Query,
    set_reference_query: &tree_sitter::Query,
) -> Vec<ScheduleSymbolReference> {
    let mut result = Vec::new();

    for (_schedule_node, body_node) in collect_schedule_nodes(root) {
        let body_text = slice_text(text, &body_node);
        if body_text.trim().is_empty() {
            continue;
        }

        let wrapped = format!("{}{}", SCHEDULE_WRAPPER_PREFIX, body_text);
        let Some(tree) = parser.parse(&wrapped, None) else {
            continue;
        };
        let schedule_root = tree.root_node();

        collect_schedule_references_for_query(
            &mut result,
            document,
            &body_node,
            body_text.len(),
            &wrapped,
            schedule_root,
            SymbolKind::Room,
            room_reference_query,
        );
        collect_schedule_references_for_query(
            &mut result,
            document,
            &body_node,
            body_text.len(),
            &wrapped,
            schedule_root,
            SymbolKind::Item,
            item_reference_query,
        );
        collect_schedule_references_for_query(
            &mut result,
            document,
            &body_node,
            body_text.len(),
            &wrapped,
            schedule_root,
            SymbolKind::Npc,
            npc_reference_query,
        );
        collect_schedule_references_for_query(
            &mut result,
            document,
            &body_node,
            body_text.len(),
            &wrapped,
            schedule_root,
            SymbolKind::Flag,
            flag_reference_query,
        );
        collect_schedule_references_for_query(
            &mut result,
            document,
            &body_node,
            body_text.len(),
            &wrapped,
            schedule_root,
            SymbolKind::Set,
            set_reference_query,
        );
    }

    result
}

fn collect_schedule_references_for_query(
    output: &mut Vec<ScheduleSymbolReference>,
    document: &Document,
    body_node: &Node,
    body_len: usize,
    wrapped: &str,
    root: Node,
    kind: SymbolKind,
    query: &tree_sitter::Query,
) {
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, root, wrapped.as_bytes());
    while let Some(m) = matches.next() {
        for capture in m.captures {
            let node = capture.node;
            let raw_id = slice_text(wrapped, &node).trim();
            if raw_id.is_empty() {
                continue;
            }

            if let Some(parent) = node.parent() {
                if is_definition_parent_for_kind(parent.kind(), kind) {
                    continue;
                }
            }

            let Some(range) = remap_schedule_capture_range(document, body_node, body_len, &node)
            else {
                continue;
            };

            let (id, rename_range) = if kind == SymbolKind::Flag {
                normalize_flag_reference(raw_id, &range)
            } else {
                (raw_id.to_string(), None)
            };

            output.push(ScheduleSymbolReference {
                kind,
                id,
                raw_id: raw_id.to_string(),
                range,
                rename_range,
            });
        }
    }
}

fn is_definition_parent_for_kind(parent_kind: &str, kind: SymbolKind) -> bool {
    match kind {
        SymbolKind::Room => parent_kind == "room_def",
        SymbolKind::Item => parent_kind == "item_def",
        SymbolKind::Npc => parent_kind == "npc_def",
        SymbolKind::Flag => parent_kind == "action_add_flag" || parent_kind == "action_add_seq",
        SymbolKind::Set => parent_kind == "set_decl",
    }
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

/// Walks the syntax tree and records every `player_start room ...` statement for diagnostics.
fn collect_player_starts(
    document: &Document,
    root: Node,
    text: &str,
    uri: &Url,
) -> Vec<PlayerStart> {
    let mut result = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "player_start" {
            if let Some(room_node) = node.child_by_field_name("room_id") {
                let room_id = slice_text(text, &room_node).trim().to_string();
                if !room_id.is_empty() {
                    let range = range_from_node(document, &room_node);
                    result.push(PlayerStart {
                        room_id,
                        range,
                        uri: uri.clone(),
                    });
                }
            }
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            stack.push(child);
        }
    }

    result
}

/// Parses the numeric suffix of a flag reference like `quest#3`, returning the index if present.
fn flag_sequence_index(raw_id: &str) -> Option<i64> {
    let (_, suffix) = raw_id.split_once('#')?;
    let digits: String = suffix
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
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
    use crate::queries::Queries;
    use tower_lsp::lsp_types::Url;
    use tree_sitter::{Parser, Query};

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

    fn text_for_range(source: &str, range: &Range) -> String {
        let document = Document::new(source.to_string());
        let start = document.offset(range.start).expect("range start offset");
        let end = document.offset(range.end).expect("range end offset");
        source[start..end].to_string()
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

        let hover = format_room_hover("test-room", &meta, Some("rooms/test-room.amble"));
        assert!(hover.contains("**ROOM:** Test Room (test-room)"));
        assert!(hover.contains("north-hall"));
        assert!(hover.contains("**File:** rooms/test-room.amble"));
    }

    #[test]
    fn room_hover_truncates_long_description() {
        let long_desc: String = std::iter::repeat('a').take(400).collect();
        let meta = RoomMetadata {
            name: Some("Test Room".into()),
            description: Some(long_desc.clone()),
            exits: vec![],
        };

        let hover = format_room_hover("test-room", &meta, None);
        let expected = format!(
            "- **Description:** {}...",
            "a".repeat(HOVER_DESCRIPTION_MAX_CHARS)
        );

        assert!(hover.contains("- **Description:** "));
        assert!(!hover.contains(&long_desc));
        assert!(hover.contains(&expected));
    }

    #[test]
    fn formats_item_hover_lists_abilities() {
        let meta = ItemMetadata {
            name: Some("Widget".into()),
            description: Some("Useful widget".into()),
            movability: Some(Movability::Free),
            location: Some("room lab".into()),
            container_state: Some("closed".into()),
            abilities: vec!["Unlock".into()],
            requirements: vec!["requires ability Use to interact".into()],
        };

        let hover = format_item_hover("widget", &meta, Some("items/widget.amble"));
        assert!(hover.contains("**ITEM:** Widget (widget)"));
        assert!(hover.contains("**Abilities:** Unlock"));
        assert!(hover.contains("**Requires:** requires ability Use to interact"));
        assert!(hover.contains("**Movability:** free"));
        assert!(hover.contains("**File:** items/widget.amble"));
    }

    #[test]
    fn item_hover_formats_requirement_pairs() {
        let meta = ItemMetadata {
            name: Some("Widget".into()),
            description: Some("Useful widget".into()),
            movability: Some(Movability::Free),
            location: Some("room lab".into()),
            container_state: Some("closed".into()),
            abilities: vec![],
            requirements: vec!["ignite -> burn".into(), "cutWood -> cut".into()],
        };

        let hover = format_item_hover("widget", &meta, None);
        assert!(hover.contains("**Requires:** ignite -> burn, cutWood -> cut"));
    }

    #[test]
    fn item_hover_formats_ability_targets() {
        let meta = ItemMetadata {
            name: Some("Widget".into()),
            description: Some("Useful widget".into()),
            movability: Some(Movability::Free),
            location: Some("room lab".into()),
            container_state: Some("closed".into()),
            abilities: vec!["Unlock (security_crate)".into()],
            requirements: vec![],
        };

        let hover = format_item_hover("widget", &meta, None);
        assert!(hover.contains("**Abilities:** Unlock (security_crate)"));
    }

    #[test]
    fn hover_paths_trim_data_root_prefix() {
        let meta = ItemMetadata {
            name: Some("Utility".into()),
            description: Some("Helpful".into()),
            movability: Some(Movability::Free),
            location: Some("room hub".into()),
            container_state: None,
            abilities: vec![],
            requirements: vec![],
        };

        let hover = format_item_hover(
            "utility_item",
            &meta,
            Some("amble_script/data/Amble/global/useful_items.amble"),
        );

        assert!(hover.contains("**ITEM:** Utility (utility_item)"));
        assert!(hover.contains("**File:** global/useful_items.amble"));
    }

    #[test]
    fn extract_item_metadata_formats_requirements() {
        let source = "item widget {\n    requires ignite to burn\n    requires cutWood to cut\n}\n";
        let tree = parse_source(source);
        let root = tree.root_node();
        let item_node =
            named_child_by_kind(&root, "item_def").expect("missing item_def for requirements test");
        let (_, _, _, _, _, _, requirements) = extract_item_metadata(&item_node, source);
        assert_eq!(
            requirements,
            vec!["ignite -> burn".to_string(), "cutWood -> cut".to_string()]
        );
    }

    #[test]
    fn extract_item_metadata_formats_abilities() {
        let source = "item widget {\n    ability Unlock security_crate\n    ability Read\n}\n";
        let tree = parse_source(source);
        let root = tree.root_node();
        let item_node =
            named_child_by_kind(&root, "item_def").expect("missing item_def for abilities test");
        let (_, _, _, _, _, abilities, _) = extract_item_metadata(&item_node, source);
        assert_eq!(
            abilities,
            vec!["Unlock (security_crate)".to_string(), "Read".to_string()]
        );
    }

    fn sample_location() -> SymbolLocation {
        SymbolLocation {
            uri: Url::parse("file:///test.amble").unwrap(),
            range: Range {
                start: Position::default(),
                end: Position::default(),
            },
            rename_range: None,
        }
    }

    #[test]
    fn detects_missing_room_metadata_fields() {
        let def = SymbolDefinition {
            location: sample_location(),
            metadata: SymbolMetadata::Room(RoomMetadata {
                name: None,
                description: None,
                exits: vec![],
            }),
        };
        let issues = metadata_issues_for_definition("room_a", &def);
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().any(|msg| msg.contains("name")));
        assert!(issues.iter().any(|msg| msg.contains("description")));
    }

    #[test]
    fn detects_missing_item_metadata_fields() {
        let def = SymbolDefinition {
            location: sample_location(),
            metadata: SymbolMetadata::Item(ItemMetadata {
                name: Some("Item".into()),
                description: Some("desc".into()),
                movability: None,
                location: None,
                container_state: None,
                abilities: vec![],
                requirements: vec![],
            }),
        };
        let issues = metadata_issues_for_definition("item_a", &def);
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().any(|msg| msg.contains("location")));
        assert!(issues.iter().any(|msg| msg.contains("movability")));
    }

    #[test]
    fn detects_missing_npc_metadata_fields() {
        let def = SymbolDefinition {
            location: sample_location(),
            metadata: SymbolMetadata::Npc(NpcMetadata {
                name: Some("Npc".into()),
                description: Some("desc".into()),
                location: None,
                state: None,
            }),
        };
        let issues = metadata_issues_for_definition("npc_a", &def);
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().any(|msg| msg.contains("location")));
        assert!(issues.iter().any(|msg| msg.contains("state")));
    }

    #[test]
    fn parses_flag_sequence_indices() {
        assert_eq!(flag_sequence_index("quest#3"), Some(3));
        assert_eq!(flag_sequence_index("quest#0"), Some(0));
        assert_eq!(flag_sequence_index("quest"), None);
        assert_eq!(flag_sequence_index("quest#x5"), None);
    }

    #[test]
    fn collects_flag_definitions_from_schedule_body() {
        let source = r#"trigger "example" when always {
    do schedule in 3 note "later" {
        do add flag some_flag_defined_here
        do add seq flag some_sequence_flag limit 3
    }
}
"#;

        let tree = parse_source(source);
        let root = tree.root_node();
        let document = Document::new(source.to_string());

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_amble::language())
            .expect("load amble grammar");

        let query = Query::new(
            &tree_sitter_amble::language(),
            r#"
[
  (action_add_flag
    flag: (flag_name) @flag.definition)
  (action_add_seq
    flag_name: (flag_name) @flag.definition)
]
"#,
        )
        .expect("build flag definition query");

        let definitions =
            collect_schedule_flag_definitions(&document, root, source, &mut parser, &query);
        assert_eq!(definitions.len(), 2);

        let mut plain_flag_found = false;
        let mut sequence_flag_found = false;
        for definition in definitions {
            let text = text_for_range(source, &definition.range);
            if definition.id == "some_flag_defined_here" {
                plain_flag_found = true;
                assert_eq!(text, "some_flag_defined_here");
                assert_eq!(definition.defined_in.as_deref(), Some("example"));
                assert_eq!(definition.sequence_limit, None);
            }

            if definition.id == "some_sequence_flag" {
                sequence_flag_found = true;
                assert_eq!(text, "some_sequence_flag");
                assert_eq!(definition.defined_in.as_deref(), Some("example"));
                assert_eq!(definition.sequence_limit, Some(3));
            }
        }

        assert!(plain_flag_found);
        assert!(sequence_flag_found);
    }

    #[test]
    fn collects_symbol_references_from_schedule_body() {
        let source = r#"trigger "example" when always {
    do schedule in 3 note "later" {
        do spawn item widget into room lab
        do spawn npc guide_bot into room lab
        do advance flag quest#2
        do add flag created_here
    }
}
"#;

        let tree = parse_source(source);
        let root = tree.root_node();
        let document = Document::new(source.to_string());

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_amble::language())
            .expect("load amble grammar");

        let queries = Queries::new();
        let references = collect_schedule_symbol_references(
            &document,
            root,
            source,
            &mut parser,
            &queries.room_references,
            &queries.item_references,
            &queries.npc_references,
            &queries.flag_references,
            &queries.set_references,
        );

        assert!(references
            .iter()
            .any(|reference| reference.kind == SymbolKind::Item && reference.id == "widget"));
        assert!(references
            .iter()
            .any(|reference| reference.kind == SymbolKind::Npc && reference.id == "guide_bot"));
        assert!(references
            .iter()
            .any(|reference| reference.kind == SymbolKind::Room && reference.id == "lab"));
        assert!(!references
            .iter()
            .any(|reference| reference.raw_id == "created_here"));

        let quest_reference = references
            .iter()
            .find(|reference| reference.kind == SymbolKind::Flag && reference.raw_id == "quest#2")
            .expect("missing schedule flag reference");
        assert_eq!(quest_reference.id, "quest");
        assert_eq!(text_for_range(source, &quest_reference.range), "quest#2");
        let rename_range = quest_reference
            .rename_range
            .clone()
            .expect("flag sequence reference should set rename range");
        assert_eq!(text_for_range(source, &rename_range), "quest");
    }
}
