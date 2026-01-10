use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use tower_lsp::lsp_types::{Range, Url};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Room,
    Item,
    Npc,
    Flag,
    Set,
}

impl SymbolKind {
    pub fn label(self) -> &'static str {
        match self {
            SymbolKind::Room => "Room",
            SymbolKind::Item => "Item",
            SymbolKind::Npc => "NPC",
            SymbolKind::Flag => "Flag",
            SymbolKind::Set => "Set",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SymbolLocation {
    pub uri: Url,
    pub range: Range,
    pub rename_range: Option<Range>,
}

impl SymbolLocation {
    pub fn rename_range(&self) -> Range {
        self.rename_range.clone().unwrap_or(self.range)
    }
}

#[derive(Debug, Clone)]
pub struct SymbolReference {
    pub location: SymbolLocation,
    pub raw_id: String,
}

#[derive(Debug, Clone)]
pub struct SymbolDefinition {
    pub location: SymbolLocation,
    pub metadata: SymbolMetadata,
}

#[derive(Debug, Clone)]
pub enum SymbolMetadata {
    Room(RoomMetadata),
    Item(ItemMetadata),
    Npc(NpcMetadata),
    Flag(FlagMetadata),
    Set(SetMetadata),
}

#[derive(Debug, Clone)]
pub struct RoomMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub exits: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ItemMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub movability: Option<Movability>,
    pub location: Option<String>,
    pub container_state: Option<String>,
    pub abilities: Vec<String>,
    pub requirements: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct NpcMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FlagMetadata {
    pub defined_in: Option<String>,
    pub sequence_limit: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct SetMetadata {
    pub rooms: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SymbolOccurrence {
    pub kind: SymbolKind,
    pub id: String,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub enum Movability {
    Free,
    Fixed(Option<String>),
    Restricted(Option<String>),
}

#[derive(Debug, Default)]
pub struct SymbolIndex {
    definitions: DashMap<String, SymbolDefinition>,
    duplicates: DashMap<String, Vec<SymbolDefinition>>,
    references: DashMap<String, Vec<SymbolReference>>,
}

impl SymbolIndex {
    pub fn clear_document(&self, uri: &Url) {
        let mut removed_ids = Vec::new();
        self.definitions.retain(|id, def| {
            if def.location.uri == *uri {
                removed_ids.push(id.clone());
                false
            } else {
                true
            }
        });
        for mut entry in self.references.iter_mut() {
            entry
                .value_mut()
                .retain(|reference| reference.location.uri != *uri);
        }
        for mut entry in self.duplicates.iter_mut() {
            entry
                .value_mut()
                .retain(|definition| definition.location.uri != *uri);
        }
        self.duplicates.retain(|_, defs| !defs.is_empty());

        for id in removed_ids {
            if let Some(mut extra) = self.duplicates.get_mut(&id) {
                let mut promoted = None;
                if !extra.value().is_empty() {
                    let new_def = extra.value_mut().remove(0);
                    promoted = Some(new_def);
                }
                let should_remove = extra.value().is_empty();
                drop(extra);
                if let Some(definition) = promoted {
                    self.definitions.insert(id.clone(), definition);
                }
                if should_remove {
                    self.duplicates.remove(&id);
                }
            }
        }
    }

    pub fn insert_definition(&self, id: String, def: SymbolDefinition) {
        match self.definitions.entry(id.clone()) {
            Entry::Occupied(_) => {
                self.duplicates.entry(id).or_insert_with(Vec::new).push(def);
            }
            Entry::Vacant(entry) => {
                entry.insert(def);
                self.duplicates.remove(&id);
            }
        }
    }

    pub fn add_reference(&self, id: String, reference: SymbolReference) {
        self.references
            .entry(id)
            .or_insert_with(Vec::new)
            .push(reference);
    }

    pub fn definition(
        &self,
        id: &str,
    ) -> Option<dashmap::mapref::one::Ref<'_, String, SymbolDefinition>> {
        self.definitions.get(id)
    }

    pub fn references(
        &self,
        id: &str,
    ) -> Option<dashmap::mapref::one::Ref<'_, String, Vec<SymbolReference>>> {
        self.references.get(id)
    }

    pub fn has_definition(&self, id: &str) -> bool {
        self.definitions.contains_key(id)
    }

    pub fn definitions_iter(&self) -> dashmap::iter::Iter<'_, String, SymbolDefinition> {
        self.definitions.iter()
    }

    pub fn references_iter(&self) -> dashmap::iter::Iter<'_, String, Vec<SymbolReference>> {
        self.references.iter()
    }

    pub fn duplicate_definitions_iter(&self) -> dashmap::iter::Iter<'_, String, Vec<SymbolDefinition>> {
        self.duplicates.iter()
    }
}

#[derive(Debug, Default)]
pub struct SymbolStore {
    pub rooms: SymbolIndex,
    pub items: SymbolIndex,
    pub npcs: SymbolIndex,
    pub flags: SymbolIndex,
    pub sets: SymbolIndex,
}

impl SymbolStore {
    pub fn index(&self, kind: SymbolKind) -> &SymbolIndex {
        match kind {
            SymbolKind::Room => &self.rooms,
            SymbolKind::Item => &self.items,
            SymbolKind::Npc => &self.npcs,
            SymbolKind::Flag => &self.flags,
            SymbolKind::Set => &self.sets,
        }
    }

    pub fn clear_document(&self, uri: &Url) {
        self.rooms.clear_document(uri);
        self.items.clear_document(uri);
        self.npcs.clear_document(uri);
        self.flags.clear_document(uri);
        self.sets.clear_document(uri);
    }
}

pub(crate) fn sanitize_markdown(value: &str) -> String {
    value.trim().replace('|', "\\|").replace('\n', "<br>")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_location(path: &str) -> SymbolLocation {
        use tower_lsp::lsp_types::Position;

        SymbolLocation {
            uri: Url::parse(&format!("file:///{}", path)).unwrap(),
            range: Range {
                start: Position::default(),
                end: Position::default(),
            },
            rename_range: None,
        }
    }

    fn room_definition(path: &str) -> SymbolDefinition {
        SymbolDefinition {
            location: test_location(path),
            metadata: SymbolMetadata::Room(RoomMetadata {
                name: Some("Room".into()),
                description: Some("Desc".into()),
                exits: vec![],
            }),
        }
    }

    #[test]
    fn tracks_duplicates_and_promotes_after_clear() {
        let index = SymbolIndex::default();
        index.insert_definition("room_a".into(), room_definition("rooms/a.amble"));
        index.insert_definition("room_a".into(), room_definition("rooms/b.amble"));

        let duplicates: Vec<_> = index
            .duplicate_definitions_iter()
            .map(|entry| entry.key().clone())
            .collect();
        assert_eq!(duplicates, vec!["room_a".to_string()]);

        index.clear_document(&Url::parse("file:///rooms/a.amble").unwrap());

        assert!(index
            .duplicate_definitions_iter()
            .next()
            .is_none());
        let current = index.definition("room_a").unwrap();
        assert_eq!(
            current.location.uri,
            Url::parse("file:///rooms/b.amble").unwrap()
        );
    }
}
