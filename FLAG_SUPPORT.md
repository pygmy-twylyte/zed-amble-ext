# Flag Support in Amble Language Server

## Overview

Flags are a key feature of the Amble DSL used to track game state, quest progress, and trigger conditions. The language server now provides full LSP support for flags, including Go To Definition and Find All References.

## What are Flags?

In Amble, flags are boolean or sequential state markers that are:
- **Created** by the `add flag` action within trigger blocks
- **Referenced** in conditions (`has flag`, `missing flag`, `flag complete`, etc.)
- **Manipulated** by actions (`reset flag`, `remove flag`, `advance flag`)

Unlike rooms, items, and NPCs, flags don't have a dedicated "definition statement" - they are implicitly defined when first added via the `add flag` action.

## Features Implemented

### 1. Go To Definition (F12)
When you click on any flag reference and press F12, you'll jump to the location where that flag is first added/defined.

**Example:**
```amble
trigger init-game {
    when enter room start
    do add flag player-started  # <-- This is the definition
}

trigger check-progress {
    when enter room checkpoint
    if has flag player-started  # <-- F12 here jumps to definition above
    do msg "Welcome back!"
}
```

### 2. Find All References (Shift+F12)
When you click on a flag (either at its definition or any reference) and press Shift+F12, you'll see all locations where that flag is used throughout your entire project.

**Example locations found:**
- Definition: `add flag player-started`
- Conditions: `has flag player-started`, `missing flag player-started`
- Actions: `reset flag player-started`, `remove flag player-started`
- Overlays: `overlay if flag player-started { ... }`
- Goal conditions: `done when has flag player-started`

### 3. Cross-File Support
Like rooms, items, and NPCs, flag references work across multiple `.amble` files in your project.

## Implementation Details

### Tree-Sitter Grammar Integration

The implementation leverages the existing tree-sitter grammar structure:

**Flag Definitions Query:**
```
(action_add_flag
  flag: (flag_name) @flag.definition)
```

**Flag References Query:**
```
(_flag_ref) @flag.reference
```

The `_flag_ref` supertype captures all flag references including:
- `has flag <name>`
- `missing flag <name>`
- `flag complete <name>`
- `flag in progress <name>`
- `reset flag <name>`
- `remove flag <name>`
- `advance flag <name>`

### Data Structures

```rust
struct FlagDefinition {
    uri: Url,      // File location
    range: Range,  // Position in file
}

struct FlagReference {
    uri: Url,
    range: Range,
}
```

The backend maintains:
- `flag_definitions: DashMap<String, FlagDefinition>` - Map from flag name to definition
- `flag_references: DashMap<String, Vec<FlagReference>>` - Map from flag name to all references

### Analysis Pipeline

1. **Document Scanning**: When a `.amble` file is opened or modified
2. **Tree-Sitter Parsing**: The file is parsed into an AST
3. **Query Execution**: Two queries run:
   - Find all `add flag` statements (definitions)
   - Find all `_flag_ref` nodes (references)
4. **Symbol Storage**: Flags and their locations are stored in DashMaps
5. **LSP Response**: When Go To Definition or Find References is triggered, the stored data is used to respond

## Usage in Your Code

### Flag Definitions
```amble
trigger quest-start {
    when talk to npc quest-giver
    do add flag quest-accepted
    do add flag quest:stage-1
}
```

### Flag Conditions
```amble
trigger quest-check {
    when enter room quest-location
    if has flag quest-accepted
    if missing flag quest-completed
    do msg "You're on the quest!"
}
```

### Flag Actions
```amble
trigger quest-progress {
    when use item quest-item
    if has flag quest:stage-1
    do advance flag quest:stage-1    # Advances sequential flag
    do reset flag attempts            # Resets to initial state
}

trigger quest-fail {
    when certain-condition
    do remove flag quest-accepted     # Completely removes the flag
}
```

### Overlays with Flags
```amble
room quest-room {
    name "Quest Room"
    desc "A mysterious room."
    
    overlay if flag quest-completed {
        desc "The room now looks different, quest complete!"
    }
}
```

## Testing

The `test_lsp.amble` file includes comprehensive flag tests:
- Flag definitions via `add flag`
- Flag condition checks: `has flag`, `missing flag`
- Flag actions: `advance flag`, `reset flag`, `remove flag`
- Multiple references to the same flag across different triggers

## Technical Notes

1. **First Definition Wins**: If a flag is added in multiple places, the first occurrence (by file scan order) is considered the "definition"
2. **Case Sensitivity**: Flag names are case-sensitive
3. **Incremental Parsing**: The language server re-parses documents on change, maintaining accurate flag locations
4. **Performance**: Uses `DashMap` for concurrent access and fast lookups

## Future Enhancements

Possible improvements for flag support:
- Warning when a flag is referenced but never defined
- Hover tooltips showing where a flag is defined and used
- Auto-completion for flag names
- Rename refactoring support
- Flag usage statistics (how many times referenced)

## Related Files

- `language-server/src/amble.rs` - Main LSP implementation
- `grammars/amble/grammar.js` - Tree-sitter grammar defining flag syntax
- `test_lsp.amble` - Test file with flag examples