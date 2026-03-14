//! Tree-sitter grammar for the Amble DSL

use tree_sitter::Language;

extern "C" {
    fn tree_sitter_amble() -> Language;
}

/// Returns the tree-sitter language for Amble
pub fn language() -> Language {
    unsafe { tree_sitter_amble() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::StreamingIterator;

    #[test]
    fn test_can_load_grammar() {
        let lang = language();
        assert!(lang.node_kind_count() > 0);
    }

    #[test]
    fn test_parse_room_definition() {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        parser
            .set_language(&language())
            .expect("Error loading Amble grammar");

        let source_code = r#"room test-room {
    name "Test Room"
    desc "A simple test room."
}"#;

        let tree = parser.parse(source_code, None).unwrap();
        let root_node = tree.root_node();

        assert_eq!(root_node.kind(), "source_file");
        assert!(root_node.child_count() > 0);

        let mut found_room_def = false;
        for i in 0..root_node.child_count() {
            if let Some(child) = root_node.child(i) {
                if child.kind() == "room_def" {
                    found_room_def = true;
                }
            }
        }

        assert!(found_room_def, "should have found a room_def node");
    }

    #[test]
    fn test_query_room_definitions() {
        use tree_sitter::{Parser, Query, QueryCursor};

        let mut parser = Parser::new();
        parser.set_language(&language()).unwrap();

        let source_code = r#"
room test-room {
    name "Test Room"
    desc "A test room."
    exit north -> other-room
}

room other-room {
    name "Other Room"
    desc "Another room."
}
"#;

        let tree = parser.parse(source_code, None).unwrap();
        let root_node = tree.root_node();

        let query_source = r#"
(room_def
  room_id: (room_id) @room.definition)
"#;

        let query = Query::new(&language(), query_source).unwrap();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, root_node, source_code.as_bytes());
        let mut room_ids = Vec::new();

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let text = &source_code[capture.node.byte_range()];
                room_ids.push(text.to_string());
            }
        }

        assert_eq!(room_ids.len(), 2);
        assert!(room_ids.contains(&"test-room".to_string()));
        assert!(room_ids.contains(&"other-room".to_string()));
    }

    #[test]
    fn test_query_room_references() {
        use tree_sitter::{Parser, Query, QueryCursor};

        let mut parser = Parser::new();
        parser.set_language(&language()).unwrap();

        let source_code = r#"
room test-room {
    exit north -> other-room
}

item box {
    location room test-room
}

trigger "test" when enter room other-room {
    do show "test"
}
"#;

        let tree = parser.parse(source_code, None).unwrap();
        let root_node = tree.root_node();

        let query_source = r#"
(_room_ref
  (room_id) @room.reference)
"#;

        let query = Query::new(&language(), query_source).unwrap();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, root_node, source_code.as_bytes());
        let mut room_refs = Vec::new();

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let text = &source_code[capture.node.byte_range()];
                room_refs.push(text.to_string());
            }
        }

        assert!(room_refs.len() >= 2, "should find at least 2 room references");
        assert!(room_refs.contains(&"other-room".to_string()));
        assert!(room_refs.contains(&"test-room".to_string()));
    }

    #[test]
    fn test_parse_take_when_variants() {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        parser.set_language(&language()).unwrap();

        let source_code = r#"
trigger "simple" when take foo { do show "x" }
trigger "npc" when take foo from npc bar { do show "x" }
trigger "item" when take foo from item chest { do show "x" }
"#;

        let tree = parser.parse(source_code, None).unwrap();
        assert!(
            !tree.root_node().has_error(),
            "updated take forms should parse cleanly"
        );
    }
}
