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
    pub portable: Option<bool>,
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

#[derive(Debug, Default)]
pub struct SymbolIndex {
    definitions: DashMap<String, SymbolDefinition>,
    references: DashMap<String, Vec<SymbolReference>>,
}

impl SymbolIndex {
    pub fn clear_document(&self, uri: &Url) {
        self.definitions.retain(|_, def| def.location.uri != *uri);
        for mut entry in self.references.iter_mut() {
            entry
                .value_mut()
                .retain(|reference| reference.location.uri != *uri);
        }
    }

    pub fn insert_definition(&self, id: String, def: SymbolDefinition) {
        self.definitions.insert(id, def);
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
