# Autocomplete Support in Amble Language Server

## Overview

The Amble language server now provides intelligent autocomplete for all major symbol types: **rooms**, **items**, **NPCs**, and **flags**. As you type references to these symbols, the LSP automatically suggests available options from your entire project.

## What Gets Autocompleted?

### 1. Room References
Autocomplete triggers when typing room identifiers in contexts like:
- `exit north -> test-ro...` → suggests all rooms starting with "test-ro"
- `push player to my-roo...` → suggests matching rooms
- `if player in room sta...` → suggests rooms
- `if has visited room dun...` → suggests rooms
- `spawn item in room off...` → suggests rooms

### 2. Item References
Autocomplete triggers when typing item identifiers:
- `use item magic-sw...` → suggests all items starting with "magic-sw"
- `if has item key-...` → suggests matching items
- `do give item old-...` → suggests items
- `spawn item torn-...` → suggests items

### 3. NPC References
Autocomplete triggers when typing NPC identifiers:
- `talk to npc quest-gi...` → suggests matching NPCs
- `if with npc merc...` → suggests NPCs
- `when npc defeated guard-...` → suggests NPCs

### 4. Flag References
Autocomplete triggers when typing flag names:
- `if has flag quest-com...` → suggests matching flags
- `if missing flag tutorial-...` → suggests flags
- `do reset flag player-...` → suggests flags
- `do advance flag mission-...` → suggests flags

## How It Works

### Context-Aware Suggestions
The language server uses tree-sitter to understand **what type of symbol is expected** at your cursor position:
- In an `exit` statement → suggests rooms
- After `has item` → suggests items
- After `with npc` → suggests NPCs
- After `has flag` → suggests flags

No need to manually specify what you're looking for - the LSP knows from context!

### Project-Wide Symbol Collection
Autocomplete suggestions include:
- ✅ Symbols from the current file
- ✅ Symbols from all other `.amble` files in the directory
- ✅ Real-time updates as you add/modify definitions

### Smart Filtering
As you type, the completion list filters down to match:
- Your partial identifier
- Case-sensitive matching
- Only symbols of the appropriate type for the context

## Usage Examples

### Example 1: Room Navigation
```amble
room starting-room {
    name "Starting Room"
    desc "You are at the beginning."
    # As you type "exit north -> st"
    # Autocomplete suggests: starting-room, stone-hall, stairway, etc.
    exit north -> st█
}
```

### Example 2: Flag Conditions
```amble
trigger check-progress {
    when enter room checkpoint
    # As you type "if has flag quest"
    # Autocomplete suggests: quest-started, quest-completed, quest:stage-1
    if has flag quest█
    do msg "Checking quest status..."
}
```

### Example 3: Item Usage
```amble
trigger use-key {
    when enter room locked-door
    # As you type "if has item rusty"
    # Autocomplete suggests: rusty-key, rusty-sword, rusty-lockpick
    if has item rusty█
    do msg "You have the rusty key!"
}
```

### Example 4: NPC Interaction
```amble
trigger talk-quest-giver {
    # As you type "when talk to npc quest"
    # Autocomplete suggests: quest-giver, quest-master, questor-npc
    when talk to npc quest█
    do msg "Hello, adventurer!"
}
```

## Completion Item Details

Each autocomplete suggestion includes:
- **Label**: The symbol identifier (e.g., `test-room-one`)
- **Kind**: Marked as `CONSTANT` (shows appropriate icon in editor)
- **Detail**: Type information (e.g., "Room: test-room-one")
- **Documentation**: Where the symbol is defined (file path)

## Keyboard Shortcuts (Typical in Most Editors)
- **Trigger manually**: `Ctrl+Space` (or `Cmd+Space` on Mac)
- **Accept suggestion**: `Enter` or `Tab`
- **Navigate list**: Arrow keys or `Ctrl+N/P`
- **Dismiss**: `Esc`

## Implementation Details

### Tree-Sitter Context Detection
The language server walks up the syntax tree from the cursor position to find:
- `_room_ref` nodes → return Room completions
- `_item_ref` nodes → return Item completions
- `_npc_ref` nodes → return NPC completions
- `_flag_ref` nodes → return Flag completions

This approach leverages the grammar's built-in semantic information, making context detection robust and accurate.

### Completion Triggers
The LSP advertises these trigger characters:
- `>` - Triggers after `exit north ->`
- ` ` (space) - Triggers after keywords like `flag`, `item`, `npc`, `room`

### Performance
- **Fast**: DashMap lookups are O(1)
- **Scalable**: Works efficiently with hundreds of symbols
- **Concurrent**: Thread-safe symbol storage
- **Incremental**: Updates as files change

### Data Flow
1. **User types** in an `.amble` file
2. **Trigger character** detected (space, `>`, etc.)
3. **LSP queries** tree-sitter for cursor context
4. **Context matched** to symbol type
5. **Definitions retrieved** from appropriate DashMap
6. **Completions sent** to editor
7. **User selects** from filtered list

## Benefits

### During Content Creation
- ✅ **Faster writing** - No need to look up symbol names
- ✅ **Fewer typos** - Select from a list instead of typing
- ✅ **Discovery** - See what symbols are available
- ✅ **Confidence** - Know you're referencing real symbols

### During Refactoring
- ✅ **Safety net** - Autocomplete shows what exists
- ✅ **Quick reference** - See all rooms/items/npcs at a glance

### For Team Collaboration
- ✅ **Consistency** - Use existing symbols instead of creating duplicates
- ✅ **Onboarding** - New team members discover symbols naturally

## Limitations

### Current Version
- Autocomplete triggers on context, not partial text matching yet
- No fuzzy matching (typing "qstgvr" won't find "quest-giver")
- No ranking by relevance or usage frequency
- No snippet completions (e.g., complete trigger templates)

### Not Yet Implemented
- Custom sorting/ranking of results
- Documentation hover previews in completion list
- Snippet-based completions for common patterns
- Completion resolve (lazy loading of documentation)

## Future Enhancements

Possible improvements:
1. **Fuzzy matching** - Match non-contiguous characters
2. **Smart ranking** - Most recently used or most frequently referenced first
3. **Snippets** - Auto-complete entire trigger/room/item blocks
4. **Rich documentation** - Show item descriptions, room names in completion docs
5. **Cross-reference hints** - Show how many times a symbol is used
6. **Insert imports** - Auto-add necessary definitions if missing

## Testing

The `test_lsp.amble` file includes autocomplete test scenarios:
- Room reference completion in exit statements
- Flag completion in conditional statements
- Mixed symbol type completion in complex triggers

To test manually:
1. Open `test_lsp.amble` in Zed
2. Navigate to an autocomplete test section
3. Start typing a partial identifier
4. Press `Ctrl+Space` to manually trigger
5. Verify suggestions appear and are filtered correctly

## Technical Notes

- **Thread Safety**: Uses `DashMap` for concurrent access to symbol tables
- **Position Handling**: Converts LSP positions to byte offsets for tree-sitter queries
- **Node Traversal**: Walks up syntax tree until context node found
- **Error Handling**: Returns `None` if context unclear (no suggestions shown)

## Related Files

- `language-server/src/amble.rs` - Implementation of `completion()` handler
- `grammars/amble/grammar.js` - Grammar defines `_*_ref` supertypes used for context
- `test_lsp.amble` - Test file with autocomplete examples

## Comparison with Other Features

| Feature | What It Does | When It Helps |
|---------|--------------|---------------|
| **Autocomplete** | Suggests valid symbols as you type | Writing new references |
| **Go To Definition** | Jumps to where symbol is defined | Understanding existing code |
| **Find References** | Shows all uses of a symbol | Impact analysis, refactoring |
| **Diagnostics** (planned) | Warns about undefined references | Catching errors |

Together, these features provide a comprehensive IDE experience for Amble development.