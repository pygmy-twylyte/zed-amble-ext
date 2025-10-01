# Migration Plan: Regex → Tree-Sitter

## Goal
Replace regex-based parsing with tree-sitter AST queries for more robust and maintainable code.

## Benefits
- ✅ One source of truth (the grammar)
- ✅ Handles complex nesting properly
- ✅ Automatic updates when grammar changes
- ✅ More accurate (no regex edge cases)
- ✅ Incremental parsing (future optimization)

## Phase 1: Setup Tree-Sitter (CURRENT)

### Step 1.1: Add Dependencies ✅
- Added `tree-sitter = "0.22"` to Cargo.toml

### Step 1.2: Create Tree-Sitter Language Binding
We need to create a small Rust crate that exports the Amble grammar.

**Create**: `language-server/tree-sitter-amble/`
```
tree-sitter-amble/
├── Cargo.toml
├── build.rs        # Compiles grammar.c
└── src/
    └── lib.rs      # Exports language()
```

Copy grammar source from: `grammars/amble/src/`

### Step 1.3: Test Parsing
Create a simple test that:
1. Loads the grammar
2. Parses a test string
3. Prints the AST

**Test string:**
```rust
let test = "room test-room { name \"Test\" }";
```

**Expected output:**
```
(source_file
  (room_def
    (room_id)
    (room_block)))
```

---

## Phase 2: Query Room Definitions

### Step 2.1: Write Tree-Sitter Query
Query to find room definitions:
```scheme
(room_def
  (room_id) @room.definition)
```

### Step 2.2: Extract Positions
From each match:
- Get the node text (room ID)
- Get the node range (start/end position)
- Store in `room_definitions` map

### Step 2.3: Test
Parse a file with multiple rooms, verify all definitions are found.

---

## Phase 3: Query Room References

### Step 3.1: Identify Reference Patterns in Grammar
From grammar.js, `_room_ref` appears in:
- Exit destinations
- Trigger conditions (enter/leave room)
- Conditionals (player in room, has visited room)
- Actions (push player to room)
- Item locations (location room)
- And more...

### Step 3.2: Write Queries for Each Context
We can use a single query that captures all `_room_ref`:
```scheme
(_room_ref
  (room_id) @room.reference)
```

Or specific queries:
```scheme
; Exit references
(room_exit
  (_room_ref (room_id) @room.reference))

; Enter/leave events
(enter_room
  (_room_ref (room_id) @room.reference))
```

### Step 3.3: Extract and Store
Similar to definitions, extract ID and range for each reference.

### Step 3.4: Test
Verify F12 works from:
- Exit statements
- Item locations
- Trigger conditions
- Actions

---

## Phase 4: Remove Regex Code

### Step 4.1: Clean Up
Remove:
- All `Regex::new()` calls
- The regex patterns
- The `regex` dependency

Keep:
- The data structures (DashMap, etc.)
- The LSP protocol handlers
- The position/offset conversion functions

### Step 4.2: Verify
Test with:
- `test_lsp.amble`
- `test_comprehensive.amble`  
- Real Amble files

---

## Implementation Notes

### Tree-Sitter Query Syntax
```scheme
; Capture a node
(node_name) @capture.name

; Match specific field
(parent
  field_name: (child) @capture)

; Alternatives
[
  (pattern1)
  (pattern2)
] @capture
```

### Getting Node Information
```rust
// Get node text
let text = &source[node.byte_range()];

// Get position
let start_pos = node.start_position(); // (row, column)
let end_pos = node.end_position();

// Convert to LSP Position
let lsp_start = Position {
    line: start_pos.row as u32,
    character: start_pos.column as u32,
};
```

### Performance Considerations
- Parse once per file change
- Cache the parsed tree
- Use incremental parsing (future optimization)

---

## Testing Strategy

### Unit Tests
Add to `language-server/src/amble.rs`:
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_room_definition() {
        let source = "room test { name \"Test\" }";
        // Parse and verify
    }
}
```

### Integration Tests
Use the test files:
- Verify definitions found
- Verify references found
- Verify F12 works across files

---

## Rollback Plan
If issues arise:
1. Git checkout previous version
2. Rebuild with regex version
3. Document the blocker
4. Plan a different approach

---

## Next Steps

**Right now:** Step 1.2 - Create tree-sitter-amble binding

**Command to proceed:**
```bash
cd /home/dave/Code/zed-amble-ext/language-server
mkdir tree-sitter-amble
# Create the binding crate
```

Would you like to proceed with Step 1.2?

## Progress Log

### ✅ Phase 1 Complete - Setup Tree-Sitter
- Added dependencies (tree-sitter = "0.25")
- Created tree-sitter-amble binding crate
- Successfully parsed test document
- Verified AST output

**Key Finding:** Grammar requires tree-sitter 0.25 (ABI version 15)

### 🔄 Next: Phase 2 - Query Room Definitions
Ready to write queries to extract room definitions from the AST.


## ✅ Phase 2 Complete - Tree-Sitter Queries Working!

Successfully replaced regex-based parsing with tree-sitter queries:
- Room definitions found using `(room_def room_id: (room_id) @room.definition)`
- Room references found using `(_room_ref (room_id) @room.reference)`
- Go To Definition works across all files
- Find All References works including `location room` statements

### Verified Working
- ✅ Definitions and references across multiple files
- ✅ All reference contexts (exits, locations, triggers, actions)
- ✅ No regex patterns needed
- ✅ Grammar is single source of truth

### Cleanup Status
- ✅ Removed `regex` dependency from Cargo.toml
- ⚠️  Debug eprintln statements still present (minor, can be removed later)
- ⚠️  Unused helper functions still present (not causing issues)

## Next Steps (Optional Future Work)
- Remove debug eprintln statements for cleaner logs
- Add support for other symbol types (items, NPCs, flags, goals)
- Add autocompletion
- Add hover information
- Add diagnostics

