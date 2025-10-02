# Installation Notes - Important Structure Change

## What Changed

The language server code has been moved to a subdirectory to prevent Zed from trying to compile it as a WASM extension.

### Directory Structure

```
zed-amble-ext/
├── extension.toml          # Extension config (points to binary)
├── languages/              # Language config files
├── grammars/              # Tree-sitter grammar
├── language-server/       # ← Language server lives here now
│   ├── Cargo.toml
│   ├── src/
│   │   └── amble.rs
│   └── target/
│       └── release/
│           └── amble-lsp  # The binary
└── test_lsp.amble
```

## Why This Change Was Needed

**The Problem:**
- Zed saw `Cargo.toml` at the extension root
- Zed tried to compile it as a WASM extension
- But our language server uses `tokio` with full features
- `tokio` full features don't work in WASM
- Build failed with: "Only features sync,macros,io-util,rt,time are supported on wasm"

**The Solution:**
- Move the language server to `language-server/` subdirectory
- Zed no longer tries to compile it
- We build it separately as a native binary
- `extension.toml` points to the binary location

## How to Build Now

```bash
# From the zed-amble-ext directory
cd language-server
cargo build --release
cd ..
```

The binary will be at `language-server/target/release/amble-lsp`.

## How to Install in Zed

```bash
# From the zed-amble-ext directory (not language-server)
zed --dev-extension $(pwd)
```

Or via Zed UI:
1. Extensions panel (`Cmd+Shift+X`)
2. "Install Dev Extension"
3. Select the `zed-amble-ext` directory (not the language-server subdirectory)

## Verification

After installing and restarting Zed:

1. Open `test_lsp.amble`
2. Check Zed logs (`Cmd+Shift+P` → "zed: open log")
3. Look for: "Amble LSP server initialized"
4. Test F12 (Go To Definition)

If you see errors about "failed to compile Rust extension", Zed is still seeing a Cargo.toml somewhere it shouldn't. Make sure you're installing from the `zed-amble-ext` directory, not `language-server`.

## Key Points

✅ **DO:** Build from `language-server/` subdirectory
✅ **DO:** Install extension from `zed-amble-ext/` root
✅ **DO:** Restart Zed after rebuilding the language server

❌ **DON'T:** Put Cargo.toml at the extension root
❌ **DON'T:** Install from the `language-server/` directory

## Files Updated

- `extension.toml` - Added `path = "language-server/target/release/amble-lsp"`
- `QUICKSTART.md` - Updated build instructions
- `README.md` - Updated build path
- `.gitignore` - Updated to ignore `language-server/target/`
- Moved: `Cargo.toml`, `src/`, `target/` → `language-server/`

## This Is Normal!

Many Zed extensions with language servers use this pattern:
- Extension code (if any) at root
- Language server in a subdirectory
- Binary referenced by path in `extension.toml`

The separation keeps the extension (WASM) and language server (native) builds independent.