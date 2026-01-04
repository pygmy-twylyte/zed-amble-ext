use dashmap::DashMap;
use tower_lsp::lsp_types::{Position, Range};

pub type DocumentStore = DashMap<String, Document>;

type LineOffset = usize;

#[derive(Debug, Clone)]
pub struct Document {
    text: String,
    line_index: LineIndex,
}

impl Document {
    pub fn new(text: String) -> Self {
        let line_index = LineIndex::new(&text);
        Self { text, line_index }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn offset(&self, position: Position) -> Option<usize> {
        self.line_index.offset(&self.text, position)
    }

    pub fn position_at(&self, byte_offset: usize) -> Position {
        self.line_index.position_at(&self.text, byte_offset)
    }

    pub fn range(&self) -> Range {
        self.line_index.range(&self.text)
    }
}

#[derive(Debug, Clone)]
struct LineIndex {
    line_starts: Vec<LineOffset>,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (idx, ch) in text.char_indices() {
            if ch == '\n' {
                line_starts.push(idx + 1);
            }
        }
        Self { line_starts }
    }

    fn offset(&self, text: &str, position: Position) -> Option<usize> {
        let line = position.line as usize;
        let line_start = *self.line_starts.get(line)?;
        let line_end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or_else(|| text.len());
        let mut line_slice = &text[line_start..line_end];
        if line_slice.ends_with('\n') {
            line_slice = &line_slice[..line_slice.len().saturating_sub(1)];
        }

        let mut current_units = 0u32;
        for (byte_idx, ch) in line_slice.char_indices() {
            if current_units == position.character {
                return Some(line_start + byte_idx);
            }
            current_units += ch.len_utf16() as u32;
        }

        if current_units == position.character {
            return Some(line_start + line_slice.len());
        }

        None
    }

    fn position_at(&self, text: &str, byte_offset: usize) -> Position {
        let clamped = byte_offset.min(text.len());
        let line = self.line_for_offset(clamped);
        let line_start = *self.line_starts.get(line).unwrap_or(&0);
        let column_bytes = clamped.saturating_sub(line_start);
        let line_slice = &text[line_start..(line_start + column_bytes).min(text.len())];
        let column_units = line_slice.chars().map(|ch| ch.len_utf16() as u32).sum();

        Position {
            line: line as u32,
            character: column_units,
        }
    }

    fn range(&self, text: &str) -> Range {
        let line_index = self.line_starts.len().saturating_sub(1) as u32;
        let last_start = self.line_starts.last().copied().unwrap_or(0);
        let last_len = if text.ends_with('\n') {
            0
        } else {
            text[last_start..]
                .chars()
                .map(|ch| ch.len_utf16() as u32)
                .sum()
        };

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

    fn line_for_offset(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next_line) => next_line.saturating_sub(1),
        }
    }
}
