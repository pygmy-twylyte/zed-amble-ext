use std::collections::HashMap;
use tree_sitter::{Node, Parser};

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

pub fn format_document(text: &str) -> String {
    let mut parser = Parser::new();
    if parser.set_language(&tree_sitter_amble::language()).is_err() {
        return fallback_format(text);
    }

    if let Some(tree) = parser.parse(text, None) {
        let events = collect_brace_events(tree.root_node());
        let mut formatted = format_with_events(text, events);
        if let Some(tree) = parser.parse(&formatted, None) {
            formatted = ParenthesizedListFormatter::new(&formatted).apply(tree.root_node());
        }
        return formatted;
    }

    fallback_format(text)
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
            update_multiline_state(line, &mut in_multiline);
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            if has_newline {
                result.push('\n');
            }
            update_multiline_state(line, &mut in_multiline);
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

        update_multiline_state(trimmed_start, &mut in_multiline);
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
            update_multiline_state(line, &mut in_multiline);
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            if has_newline {
                result.push('\n');
            }
            update_multiline_state(line, &mut in_multiline);
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

        let mut delta = brace_delta(normalized);
        if closes_block {
            delta += 1;
        }
        indent_level = ((indent_level as isize) + delta).max(0) as usize;

        update_multiline_state(trimmed_start, &mut in_multiline);
    }

    if !result.ends_with('\n') {
        result.push('\n');
    }

    result
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
    walk_brace_nodes(root, &mut events);
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
        walk_brace_nodes(child, events);
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
            slice_text(self.text, node).trim().to_string()
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

fn slice_text<'a>(text: &'a str, node: &Node) -> &'a str {
    &text[node.byte_range()]
}

#[cfg(test)]
mod tests {
    use super::format_document;

    #[test]
    fn formats_item_block() {
        let source = "item sample {\n  name \"Sample\"\n  portable true\n}\n";
        let expected = "item sample {\n    name \"Sample\"\n    portable true\n}\n";
        assert_eq!(format_document(source), expected);
    }

    #[test]
    fn preserves_multiline_text_blocks() {
        let source = "item example {\n  text \"\"\"line1\nline2\"\"\"\n}\n";
        let expected = "item example {\n    text \"\"\"line1\nline2\"\"\"\n}\n";
        assert_eq!(format_document(source), expected);
    }

    #[test]
    fn ignores_braces_inside_raw_strings() {
        let source = "item raw {\n  name r#\"{curly}\"#\n}\n";
        let expected = "item raw {\n    name r#\"{curly}\"#\n}\n";
        assert_eq!(format_document(source), expected);
    }

    #[test]
    fn formats_any_group_single_line_with_spacing() {
        let source = "trigger \"example\" when always {\n    if any(missing item quest_scroll, has flag quest_started) {\n        do show \"\"\n    }\n}\n";
        let expected = "trigger \"example\" when always {\n    if any( missing item quest_scroll, has flag quest_started ) {\n        do show \"\"\n    }\n}\n";
        assert_eq!(format_document(source), expected);
    }

    #[test]
    fn formats_any_group_multiline_with_nested_all() {
        let source = "trigger \"example\" when always {\n    if any(missing item some_item, has flag some_flag, all(with npc guide_bot, flag in progress guide_bot_intro, missing item guide_token)) {\n        do show \"\"\n    }\n}\n";
        let expected = "trigger \"example\" when always {\n    if any(\n        missing item some_item,\n        has flag some_flag,\n        all(\n            with npc guide_bot,\n            flag in progress guide_bot_intro,\n            missing item guide_token,\n        ),\n    ) {\n        do show \"\"\n    }\n}\n";
        assert_eq!(format_document(source), expected);
    }

    #[test]
    fn formats_any_group_trailing_commas_without_duplicates() {
        let source = "trigger \"example\" when always {\n    if any(has flag flag_1, has flag flag_2, has flag flag_3,) {\n        do show \"\"\n    }\n}\n";
        let expected = "trigger \"example\" when always {\n    if any(\n        has flag flag_1,\n        has flag flag_2,\n        has flag flag_3,\n    ) {\n        do show \"\"\n    }\n}\n";
        assert_eq!(format_document(source), expected);
    }

    #[test]
    fn formats_set_lists_into_multiline_blocks() {
        let source = "let set hallway = (room_a, room_b, room_c)\n";
        let expected = "let set hallway = (\n    room_a,\n    room_b,\n    room_c,\n)\n";
        assert_eq!(format_document(source), expected);
    }

    #[test]
    fn formats_required_items_with_parenthesis_spacing() {
        let source = "room foyer {\n    exit north -> hall {\n        required_items(item_key, item_badge)\n    }\n}\n";
        let expected = "room foyer {\n    exit north -> hall {\n        required_items( item_key, item_badge )\n    }\n}\n";
        assert_eq!(format_document(source), expected);
    }

    #[test]
    fn formats_overlay_conditions_with_two_items_single_line() {
        let source = "room entry {\n    overlay if (flag set foo, item present bar) {\n        text \"\"\n    }\n}\n";
        let expected = "room entry {\n    overlay if ( flag set foo, item present bar ) {\n        text \"\"\n    }\n}\n";
        assert_eq!(format_document(source), expected);
    }

    #[test]
    fn formats_overlay_conditions_multiline_when_three_items() {
        let source = "room entry {\n    overlay if (flag set foo, item present bar, player has item baz) {\n        text \"\"\n    }\n}\n";
        let expected = "room entry {\n    overlay if (\n        flag set foo,\n        item present bar,\n        player has item baz,\n    ) {\n        text \"\"\n    }\n}\n";
        assert_eq!(format_document(source), expected);
    }
}
