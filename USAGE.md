# Using the Amble Extension in Zed

This guide will help you build, install, and test the Amble language extension with LSP support in Zed.

## Prerequisites

- Zed editor installed
- Rust toolchain installed (`cargo` command available)
- Git (for version control)

## Building the Extension

### 1. Build the Language Server

From the repository root:

```bash
cd zed-amble-ext
cargo build --release
```

The language server binary will be created at `target/release/amble-lsp`.

### 2. Verify the Build

Check that the binary was created successfully:

```bash
ls -lh target/release/amble-lsp
```

You should see a binary file approximately 6-7 MB in size.

## Installing the Extension in Zed

### Development Installation

Zed supports loading extensions from a local directory for development:

1. **Open Zed**

2. **Open the Extensions Panel**
   - Press `Cmd+Shift+X` (Mac) or `Ctrl+Shift+X` (Linux/Windows)
   - Or use the menu: View → Extensions

3. **Install from Dev**
   - Click "Install Dev Extension"
   - Navigate to the `zed-amble-ext` directory
   - Select the directory

Alternatively, you can use the Zed CLI:

```bash
zed --dev-extension /path/to/zed-amble-ext
```

### Verifying Installation

1. Open Zed's extension panel (`Cmd+Shift+X`)
2. Look for "Amble" in the installed extensions list
3. Check that it shows as "Installed (Dev)" or similar

## Testing the Language Server

### Quick Test with Provided Test File

1. **Open the test file** in Zed:
   ```bash
   zed test_lsp.amble
   ```

2. **Test Go To Definition**:
   - Click on any room reference (e.g., `test-room-two` in an exit statement on line 7)
   - Press `F12` (or right-click → "Go to Definition")
   - You should jump to the room definition on line 11

3. **Test Find All References**:
   - Click on a room identifier (e.g., `test-room-one` on line 3)
   - Press `Shift+F12` (or right-click → "Find All References")
   - You should see a list of all places where `test-room-one` is used

### Testing with Your Own Files

1. **Create a new .amble file** or open existing ones in the same directory

2. **Define some rooms**:
   ```amble
   room entrance {
       name "The Entrance"
       desc "A grand entrance hall."
       exit north -> hallway
   }
   
   room hallway {
       name "Long Hallway"
       desc "A long, dimly lit hallway."
       exit south -> entrance
   }
   ```

3. **Test the LSP features**:
   - Click on `hallway` in the exit statement
   - Press `F12` to jump to the definition
   - Press `Shift+F12` to find all references

### Expected Behavior

✅ **Working correctly:**
- Clicking a room identifier and pressing F12 jumps to the `room room-id {` line
- Pressing Shift+F12 shows all uses including:
  - The definition line
  - All exit targets (`exit dir -> room-id`)
  - All trigger conditions (`enter room room-id`, etc.)
  - All actions (`push player to room-id`, etc.)

❌ **Common issues:**
- If nothing happens when pressing F12:
  - Check the Zed log: `Cmd+Shift+P` → "zed: open log"
  - Look for "amble-lsp" messages
  - Verify the language server started

## Checking Language Server Status

### View Zed Logs

1. Open the command palette: `Cmd+Shift+P` (Mac) or `Ctrl+Shift+P` (Linux/Windows)
2. Type "log" and select "zed: open log"
3. Look for messages like:
   - "Amble LSP server initialized"
   - "Opened document: file://..."

### Language Server Running

You can verify the language server is running:

```bash
ps aux | grep amble-lsp
```

You should see the `amble-lsp` process running when you have an `.amble` file open.

## Supported Room Reference Contexts

The language server recognizes rooms in these contexts:

| Context | Example | Supported |
|---------|---------|-----------|
| Definition | `room my-room {` | ✅ |
| Exit target | `exit north -> my-room` | ✅ |
| Enter event | `enter room my-room` | ✅ |
| Leave event | `leave room my-room` | ✅ |
| Player location | `player in room my-room` | ✅ |
| Push action | `push player to my-room` | ✅ |
| Visited check | `has visited room my-room` | ✅ |
| Reached goal | `reached room my-room` | ✅ |
| Lock/unlock | `lock exit from my-room` | ✅ |
| Multiple rooms | `in rooms room-1, room-2` | ✅ |

## Troubleshooting

### Language Server Not Starting

1. **Rebuild the language server**:
   ```bash
   cargo clean
   cargo build --release
   ```

2. **Check Zed's extensions directory**:
   - The binary should be accessible to Zed
   - Check file permissions: `chmod +x target/release/amble-lsp`

3. **Restart Zed**:
   - Quit Zed completely
   - Reopen and try again

### Go To Definition Not Working

1. **Verify the room is defined**:
   - Make sure there's a `room room-id {` line somewhere in the same directory

2. **Check file extensions**:
   - Files must have the `.amble` extension
   - The language server only scans `.amble` files in the same directory (not subdirectories yet)

3. **Cursor positioning**:
   - Place the cursor directly on the room identifier
   - Not on the `room` keyword or the `{` bracket

### No References Found

1. **Save all files**:
   - The language server re-scans on save
   - Press `Cmd+S` to save

2. **Check the directory**:
   - Only files in the same directory are scanned
   - Subdirectories are not yet supported in this version

### Syntax Highlighting Not Working

1. **Check file extension**: Must be `.amble`
2. **Restart Zed** after installing the extension
3. **Check extension is enabled** in the Extensions panel

## Rebuilding After Changes

If you modify the language server code:

1. **Rebuild**:
   ```bash
   cargo build --release
   ```

2. **Restart Zed** (or reload the extension)

3. **Test again** with your `.amble` files

## Next Steps

Once you have the basic LSP working:

- Test with larger `.amble` files
- Try multiple files in the same directory
- Experiment with different room reference patterns
- Report any issues or unexpected behavior

For detailed information about the language server implementation, see [LSP_README.md](LSP_README.md).

## Getting Help

If you encounter issues:

1. Check the Zed log for error messages
2. Verify the build completed successfully
3. Try the provided `test_lsp.amble` file first
4. Check that the file has the `.amble` extension

## Known Limitations (v0.1.0)

- Only supports room definitions/references (not items, NPCs, flags, etc.)
- Only scans files in the same directory (no subdirectories)
- No autocompletion yet
- No diagnostics/warnings for undefined rooms yet
- Uses regex parsing (may miss some edge cases)