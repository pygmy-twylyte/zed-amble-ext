use tree_sitter::Query;

const ROOM_DEF_QUERY: &str = r#"
(room_def
  room_id: (room_id) @room.definition)
"#;

const ROOM_REF_QUERY: &str = r#"
(_room_ref
  (room_id) @room.reference)
"#;

const ITEM_DEF_QUERY: &str = r#"
(item_def
  item_id: (item_id) @item.definition)
"#;

const ITEM_REF_QUERY: &str = r#"
(_item_ref
  (item_id) @item.reference)
"#;

const NPC_DEF_QUERY: &str = r#"
(npc_def
  npc_id: (npc_id) @npc.definition)
"#;

const NPC_REF_QUERY: &str = r#"
(_npc_ref
  (npc_id) @npc.reference)
"#;

const FLAG_DEF_QUERY: &str = r#"
[
  (action_add_flag
    flag: (flag_name) @flag.definition)
  (action_add_seq
    flag_name: (flag_name) @flag.definition)
]
"#;

const FLAG_REF_QUERY: &str = r#"
(_flag_ref) @flag.reference
"#;

const SET_DEF_QUERY: &str = r#"
(set_decl
  name: (set_name) @set.definition)
"#;

const SET_REF_QUERY: &str = r#"
(set_name) @set.reference
"#;

pub struct Queries {
    pub room_definitions: Query,
    pub room_references: Query,
    pub item_definitions: Query,
    pub item_references: Query,
    pub npc_definitions: Query,
    pub npc_references: Query,
    pub flag_definitions: Query,
    pub flag_references: Query,
    pub set_definitions: Query,
    pub set_references: Query,
}

impl Queries {
    pub fn new() -> Self {
        let language = tree_sitter_amble::language();
        Self {
            room_definitions: Query::new(&language, ROOM_DEF_QUERY)
                .expect("Bad room definition query"),
            room_references: Query::new(&language, ROOM_REF_QUERY)
                .expect("Bad room reference query"),
            item_definitions: Query::new(&language, ITEM_DEF_QUERY)
                .expect("Bad item definition query"),
            item_references: Query::new(&language, ITEM_REF_QUERY)
                .expect("Bad item reference query"),
            npc_definitions: Query::new(&language, NPC_DEF_QUERY)
                .expect("Bad npc definition query"),
            npc_references: Query::new(&language, NPC_REF_QUERY)
                .expect("Bad npc reference query"),
            flag_definitions: Query::new(&language, FLAG_DEF_QUERY)
                .expect("Bad flag definition query"),
            flag_references: Query::new(&language, FLAG_REF_QUERY)
                .expect("Bad flag reference query"),
            set_definitions: Query::new(&language, SET_DEF_QUERY)
                .expect("Bad set definition query"),
            set_references: Query::new(&language, SET_REF_QUERY)
                .expect("Bad set reference query"),
        }
    }
}
