# Amble Extension Quick Start

Get the Amble language server running in Zed in under 5 minutes.

## Prerequisites

- Zed editor installed
- Rust toolchain installed (`cargo --version` works)

## 1. Build the Language Server

```bash
cd /home/dave/Code/zed-amble-ext
./build.sh
```

This builds the language server and copies it to `bin/amble-lsp`.

**Manual build (if script doesn't work):**
```bash
cd language-server
cargo build --release
cd ..
mkdir -p bin
cp language-server/target/release/amble-lsp bin/
```

## 2. Install in Zed

**Option A: Command Line**
```bash
cd /home/dave/Code/zed-amble-ext
zed --dev-extension $(pwd)
```

**Option B: In Zed UI**
1. Press `Cmd+Shift+X` (Extensions panel)
2. Click "Install Dev Extension"
3. Select the `zed-amble-ext` directory

Zed will compile the extension and copy the binary. Wait for it to finish (watch the logs).

## 3. Test It Works

**Restart Zed completely** (quit and reopen), then:

```bash
cd /home/dave/Code/zed-amble-ext
zed test_lsp.amble
```

**Test Go To Definition:**
1. Click on `test-room-two` on line 7 (in the `exit north ->` statement)
2. Press `F12`
3. ✅ You should jump to line 11 (the `room test-room-two {` definition)

**Test Find All References:**
1. Click on `test-room-one` on line 3 (the room definition)
2. Press `Shift+F12`
3. ✅ You should see a list showing 4 locations:
   - Line 3: Definition
   - Line 7: Exit reference
   - Line 14: Exit reference
   - Line 34: Push action reference

## That's It! 🎉

You now have:
- ✅ Syntax highlighting for `.amble` files
- ✅ Go To Definition for room references (F12)
- ✅ Find All References for rooms (Shift+F12)

## What Works Right Now

The LSP recognizes rooms in these contexts:

```amble
room my-room {                  # Definition
    exit north -> my-room       # Exit target
}

trigger test {
    when enter room my-room     # Enter/leave events
    if player in room my-room   # Conditionals
    do push player to my-room   # Actions
}
```

## Troubleshooting

**Nothing happens when I press F12?**
- Make sure you clicked on the room identifier (not the keyword "room")
- Check the file has `.amble` extension
- View logs: `Cmd+Shift+P` → "zed: open log"
- Look for: `[lsp] starting language server process`

**Language server not starting?**
```bash
# Check if binary exists in installed location
ls -la ~/.local/share/zed/extensions/installed/amble/bin/amble-lsp

# Rebuild and reinstall
./build.sh
# Then quit Zed, reopen, and reinstall the extension
```

**Build fails?**
```bash
# Clean build
cd language-server
cargo clean
cargo build --release
cd ..
cp language-server/target/release/amble-lsp bin/
```

## Key Insight - Case Sensitivity! 🔑

The extension config requires **exact case matching**:

```toml
# In extension.toml
[language_servers.amble-lsp]
language = "Amble"  # Must match languages/amble/config.toml exactly!
```

If you change the language name, both files must match exactly.

## Next Steps

- See [USAGE.md](USAGE.md) for detailed testing
- See [LSP_README.md](LSP_README.md) for architecture
- See [SUCCESS.md](SUCCESS.md) for complete solution documentation
- Try it with your own `.amble` files!

## Current Limitations

- Only room references (items/NPCs coming later)
- Only scans same directory (no subdirectories yet)
- No autocomplete yet (planned)

## Rebuilding After Code Changes

```bash
# Just run the build script
./build.sh

# Then restart Zed (quit completely and reopen)
```

The extension will automatically recompile when Zed restarts.