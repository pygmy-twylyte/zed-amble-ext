# zed-amble-ext

Extension for the Zed editor providing syntax highlighting, tree-sitter parsing, and language server support for the Amble game engine's DSL.

## Features

### Syntax Highlighting
- Full syntax highlighting for Amble DSL files (`.amble`)
- Tree-sitter based grammar for accurate parsing

### Language Server Protocol (LSP)
- **Go To Definition**: Jump from room references to their definitions (F12)
- **Find All References**: Find all uses of a room throughout your project (Shift+F12)
- Works across all `.amble` files in the same directory

## Installation

1. Install the extension in Zed
2. Open any `.amble` file
3. The language server will automatically start

## LSP Usage

Once installed, you can:
- Click on any room identifier and press F12 to jump to its definition
- Press Shift+F12 to see all references to a room
- Works with room definitions, exit targets, trigger conditions, and more

For detailed LSP documentation, see [LSP_README.md](LSP_README.md).

## Development

### Building the Language Server

```bash
cd language-server
cargo build --release
```

The LSP binary will be at `language-server/target/release/amble-lsp`.

## Repository

- Extension: https://github.com/pygmy-twylyte/zed-amble-ext
- Tree-sitter Grammar: https://github.com/pygmy-twylyte/tree-sitter-amble