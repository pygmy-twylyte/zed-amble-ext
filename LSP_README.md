# Amble Language Server Protocol (LSP)

A minimal language server implementation for the Amble DSL, providing IDE features for Amble game development files.

## Current Features

### Go To Definition
- Jump from any room reference to its definition
- Works across all `.amble` files in the same directory

### Find All References
- Find all uses of a room identifier throughout your project
- Includes both the definition and all references

## Supported Room Reference Contexts

The LSP recognizes room identifiers in the following contexts:

- **Room definitions**: `room room-id { ... }`
- **Exit targets**: `exit direction -> room-id`
- **Trigger conditions**: 
  - `enter room room-id`
  - `leave room room-id`
  - `player in room room-id`
  - `has visited room room-id`
- **Trigger actions**:
  - `push player to room-id`
  - `lock exit from room-id direction ...`
  - `unlock exit from room-id direction ...`
- **Goal conditions**: `reached room room-id`
- **Multiple rooms**: `in rooms room-1, room-2, room-3`

## Building the Language Server

```bash
cargo build --release
```

The binary will be located at `target/release/amble-lsp`.

## Installation in Zed

The language server is automatically built and used when the extension is installed. The configuration is already set up in:

- `extension.toml` - Declares the language server
- `languages/amble/config.toml` - Associates it with Amble files

## Usage in Zed

Once the extension is installed:

1. Open any `.amble` file
2. Place your cursor on a room identifier (either in a definition or reference)
3. Use the following commands:
   - **Go To Definition**: Right-click → "Go to Definition" (or F12)
   - **Find All References**: Right-click → "Find All References" (or Shift+F12)

## Testing

A test file is provided: `test_lsp.amble`

This file contains:
- Multiple room definitions
- Various types of room references
- Trigger examples with room conditions and actions

### Manual Testing Steps

1. Open `test_lsp.amble` in Zed
2. Click on any room identifier (e.g., `test-room-two` in an exit)
3. Press F12 to jump to the definition
4. Press Shift+F12 to see all references

## Architecture

The language server uses:
- **tower-lsp**: LSP protocol implementation
- **regex**: Pattern matching for parsing Amble syntax
- **dashmap**: Thread-safe hash maps for storing definitions and references
- **tokio**: Async runtime

### How It Works

1. When a document is opened, the LSP:
   - Parses the document for room definitions and references
   - Scans all other `.amble` files in the same directory
   - Builds an index of definitions and references

2. When you request "Go To Definition":
   - The LSP identifies the room ID at the cursor position
   - Returns the location of the room definition

3. When you request "Find All References":
   - The LSP identifies the room ID at the cursor position
   - Returns all locations where that room is referenced

## Future Features (Planned)

- **Autocompletion**: Suggest room IDs when typing
- **Hover Information**: Show room name and description on hover
- **Diagnostics**: Warn about undefined rooms, unreachable rooms, etc.
- **Additional Symbols**:
  - Item definitions and references
  - NPC definitions and references
  - Flag definitions and references
  - Goal definitions and references
- **Workspace Support**: Scan subdirectories recursively
- **Rename Refactoring**: Rename a room and update all references
- **Document Symbols**: Outline view of all rooms in a file

## Development Notes

### Scope Limitations (Current)

- Only scans `.amble` files in the same directory (no subdirectories)
- Only supports room references (items, NPCs, flags, etc. are not yet implemented)
- Uses regex-based parsing (no tree-sitter integration yet)

### Known Issues

- The regex-based parser may miss some edge cases in complex syntax
- No validation of room identifiers (undefined rooms don't show errors yet)

## Contributing

To add support for additional symbol types (items, NPCs, etc.):

1. Add new definition/reference storage in the `Backend` struct
2. Add regex patterns in `analyze_document()` to match the new symbols
3. Update `get_*_at_position()` methods to check new symbol types
4. Update `goto_definition()` and `references()` to handle new types

## Version

Current version: 0.1.0 (Minimal prototype)