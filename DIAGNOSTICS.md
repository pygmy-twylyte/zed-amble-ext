# Diagnostics in Amble Language Server

## Overview

The Amble language server now provides real-time diagnostics to catch errors before you run your game. The primary focus is detecting **undefined symbol references** - when you reference a room, item, NPC, or flag that hasn't been defined anywhere in your project.

## What Gets Checked

### Undefined References Detection

The LSP checks all references to ensure they have corresponding definitions:

- **Rooms**: Verifies every room reference has a matching `room` definition
- **Items**: Verifies every item reference has a matching `item` definition  
- **NPCs**: Verifies every NPC reference has a matching `npc` definition
- **Flags**: Verifies every flag reference has a matching `add flag` statement

### Error Display

Undefined references are marked with:
- 🔴 **Red squiggly underlines** under the invalid identifier
- **Error message** on hover: "Undefined room: 'room-name'"
- **Source label**: "amble-lsp" to identify the LSP as the source
- **Real-time updates** as you type or change files

## How It Works

### Automatic Checking

Diagnostics run automatically:
- ✅ When you **open** a file
- ✅ When you **edit** a file (on every change)
- ✅ When you **save** a file (triggers directory re-scan)
- ✅ Across all `.amble` files in your project

### Project-Wide Validation

The language server maintains a complete symbol table from all files, so:
- Definitions in one file validate references in another
- Adding a definition automatically clears related errors
- Deleting a definition shows errors wherever it was referenced

## Examples

### Valid References (No Errors)

```amble
# Definition
room start-room {
    name "Starting Room"
    desc "You are here."
}

# Valid reference - no error
room second-room {
    name "Second Room"
    exit west -> start-room
}
```

✅ No diagnostics - `start-room` is defined.

### Undefined Room Reference

```amble
room my-room {
    name "My Room"
    exit north -> undefined-room-name  ⚠️ Error: Undefined room
}
```

🔴 Error message: `Undefined room: 'undefined-room-name'`

### Undefined Item Reference

```amble
trigger get-item {
    when enter room my-room
    if has item magic-sword  ⚠️ Error: Undefined item
    do msg "You have the sword!"
}
```

🔴 Error message: `Undefined item: 'magic-sword'`

### Undefined NPC Reference

```amble
trigger talk-to-merchant {
    when talk to npc merchant-bob  ⚠️ Error: Undefined NPC
    do msg "Hello!"
}
```

🔴 Error message: `Undefined NPC: 'merchant-bob'`

### Undefined Flag Reference

```amble
trigger check-quest {
    when enter room quest-room
    if has flag quest-completed  ⚠️ Error: Undefined flag
    do msg "Quest done!"
}
```

🔴 Error message: `Undefined flag: 'quest-completed'`

Note: Flags must be defined with `add flag` before being referenced.

## Common Scenarios

### Scenario 1: Typo in Reference

```amble
room start {
    name "Start"
}

room next {
    exit west -> strat  ⚠️ Error: Typo - should be "start"
}
```

**Fix**: Correct the typo to `start`.

### Scenario 2: Flag Used Before Definition

```amble
trigger check-flag {
    if has flag game-started  ⚠️ Error: Flag not yet added
    do msg "Game started"
}

# Flag added later - diagnostics will clear once LSP re-scans
trigger init {
    when enter room start
    do add flag game-started
}
```

**Note**: The error clears automatically once the `add flag` statement exists.

### Scenario 3: Cross-File Reference

**File: rooms.amble**
```amble
room lobby {
    exit north -> office  ✅ Valid if office exists in another file
}
```

**File: more-rooms.amble**
```amble
room office {
    name "Office"
}
```

✅ No error - the LSP scans all files and validates cross-file references.

### Scenario 4: Deleted Definition

If you delete a room/item/npc/flag definition, **all references to it** will immediately show errors. This helps you:
- Find all places that need updating
- Avoid broken references in your game
- Refactor safely

## Benefits

### During Development

- ✅ **Catch typos immediately** - No more hunting for misspelled IDs
- ✅ **Prevent broken references** - Know when a symbol doesn't exist
- ✅ **Safe refactoring** - See impact of deleting definitions
- ✅ **Cross-file validation** - Ensure consistency across your project

### Before Testing

- ✅ **Reduce runtime errors** - Catch issues before launching the game
- ✅ **Cleaner codebase** - No dangling references
- ✅ **Confidence** - Know your references are valid

### For Team Collaboration

- ✅ **Consistency** - Everyone sees the same errors
- ✅ **Coordination** - Know when someone deletes a shared symbol
- ✅ **Onboarding** - New team members see errors for invalid assumptions

## Error Severity

Currently all undefined references are marked as **ERROR** (red squiggles).

Future enhancements may include:
- **WARNING** severity for less critical issues
- **INFO** or **HINT** severity for suggestions
- Customizable severity levels

## Limitations

### Current Version

- Only checks for **undefined references** (not unused definitions)
- Does not suggest fixes or alternatives (yet)
- No "quick fix" actions (yet)
- Case-sensitive matching only

### Not Yet Implemented

- Unused definition detection (symbols defined but never referenced)
- Typo suggestions ("Did you mean 'start-room'?")
- Quick fixes (auto-correct typos, create missing definitions)
- Circular reference detection
- Duplicate definition warnings

## Testing

The `test_diagnostics.amble` file includes comprehensive test scenarios:
- Valid definitions and references (should show no errors)
- Undefined room references
- Undefined item references
- Undefined NPC references
- Undefined flag references
- Mixed valid/invalid references
- Overlay conditions with undefined references
- Goal conditions with undefined references

To test manually:
1. Open `test_diagnostics.amble` in Zed
2. Look for red squiggly underlines under undefined references
3. Hover over squiggles to see error messages
4. Try adding a definition and watch the error disappear
5. Try deleting a definition and watch errors appear on references

## Implementation Details

### Real-Time Updates

Diagnostics are re-computed:
- On **every keystroke** in `did_change()`
- On **file save** in `did_save()` (after directory re-scan)
- On **file open** in `did_open()` (after initial scan)

### Cross-File Awareness

The language server maintains:
- Global symbol tables for all definitions (rooms, items, NPCs, flags)
- Global reference lists for all references
- Document map for all loaded files

When checking diagnostics for a file, it compares references in that file against the global definition tables.

### Performance

- **Fast**: Uses O(1) hash map lookups (`DashMap`)
- **Efficient**: Only publishes diagnostics for the changed file
- **Concurrent**: Thread-safe data structures
- **Incremental**: Re-parses only changed documents

### Data Flow

1. User types → `did_change()` triggered
2. LSP re-parses document → updates symbol tables
3. `check_diagnostics()` called for the file
4. Iterates through all references in the file
5. Checks if each reference has a definition
6. Creates diagnostic if no definition found
7. Publishes diagnostics to editor
8. Editor displays red squiggles

## Technical Notes

- **Severity**: All diagnostics use `DiagnosticSeverity::ERROR`
- **Source**: Labeled as "amble-lsp" in diagnostic info
- **Range**: Precise location of the invalid identifier
- **Message format**: `"Undefined <type>: '<identifier>'"`

## Future Enhancements

### Planned Features

1. **Typo suggestions**: "Did you mean 'start-room'?"
2. **Quick fixes**: Click to create missing definition
3. **Unused symbol warnings**: Flag definitions that are never used
4. **Duplicate definition warnings**: Multiple definitions of same symbol
5. **Circular dependency detection**: Room loops, item spawning loops
6. **Custom severity levels**: Configurable error vs. warning levels

### Possible Improvements

- Fuzzy matching for close symbol names
- Context-aware suggestions based on symbol type
- "Go to definition" from diagnostic
- Batch fixes for multiple errors
- Ignore/suppress specific diagnostics

## Related Features

| Feature | What It Does | When It Helps |
|---------|--------------|---------------|
| **Diagnostics** | Shows errors for undefined references | Catching mistakes early |
| **Autocomplete** | Suggests valid symbols as you type | Preventing errors before they happen |
| **Go To Definition** | Jumps to where symbol is defined | Understanding existing code |
| **Find References** | Shows all uses of a symbol | Impact analysis, refactoring |

Together, these features provide comprehensive error prevention and code navigation for Amble development.

## Related Files

- `language-server/src/amble.rs` - Implementation of `check_diagnostics()`
- `test_diagnostics.amble` - Comprehensive test scenarios
- `AUTOCOMPLETE.md` - Related feature documentation
- `IMPLEMENTATION_STATUS.md` - Project status tracking

---

**Last Updated**: 2025-01-18  
**Status**: Production Ready  
**Feature Version**: 0.1.0