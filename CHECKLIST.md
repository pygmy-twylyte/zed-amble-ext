# Amble LSP Extension - Installation Checklist

Use this checklist to verify your Amble language server extension is properly installed and working in Zed.

## Pre-Installation

- [ ] Zed editor is installed and running
- [ ] Rust toolchain is installed (`cargo --version` works)
- [ ] Git repository is cloned to local machine
- [ ] You're in the `zed-amble-ext` directory

## Build Phase

- [ ] Run `cargo build --release`
- [ ] Build completes without errors
- [ ] Build completes without warnings
- [ ] Binary exists at `target/release/amble-lsp`
- [ ] Binary is executable (`ls -l target/release/amble-lsp` shows `x` permission)
- [ ] Binary size is approximately 6-7 MB

## Installation Phase

Choose one method:

### Method A: Command Line
- [ ] Run `zed --dev-extension $(pwd)` from the extension directory
- [ ] Zed opens without errors

### Method B: UI
- [ ] Open Zed
- [ ] Press `Cmd+Shift+X` (or `Ctrl+Shift+X`)
- [ ] Click "Install Dev Extension"
- [ ] Navigate to `zed-amble-ext` directory
- [ ] Click to install

## Verification Phase

- [ ] Extension appears in Zed's Extensions panel
- [ ] Extension shows as "Installed" or "Dev"
- [ ] No error messages in Extensions panel

## Basic Functionality Test

### Open Test File
- [ ] Open `test_lsp.amble` in Zed
- [ ] File has syntax highlighting
- [ ] File shows in Amble language mode (check status bar)

### Test Go To Definition
- [ ] Click on `test-room-two` on line 7 (in the `exit north` statement)
- [ ] Press `F12`
- [ ] Cursor jumps to line 11 (the `room test-room-two {` definition)
- [ ] No error messages appear

### Test Find All References
- [ ] Click on `test-room-one` on line 3 (the room definition line)
- [ ] Press `Shift+F12`
- [ ] A references panel appears
- [ ] Shows 4 locations total:
  - [ ] Line 3: Definition
  - [ ] Line 7: Exit reference
  - [ ] Line 14: Exit reference  
  - [ ] Line 34: Push action reference

### Test Multiple References
- [ ] Click on `test-room-two` anywhere in the file
- [ ] Press `Shift+F12`
- [ ] References panel shows multiple locations:
  - [ ] Line 11: Definition
  - [ ] Line 7: Exit reference
  - [ ] Line 15: Exit reference
  - [ ] Line 32: Enter room condition
  - [ ] Line 33: Player in room condition

## Advanced Functionality Test

### Cross-Reference Types
- [ ] Test exit target: Click `test-room-three` on line 8, press F12, jumps to line 17
- [ ] Test trigger condition: Click `test-room-two` on line 32, press F12, jumps to line 11
- [ ] Test action: Click `test-room-one` on line 34, press F12, jumps to line 3

### Multi-File Test (Optional)
- [ ] Create a new `.amble` file in the same directory
- [ ] Add a room definition
- [ ] Reference that room in `test_lsp.amble`
- [ ] F12 works across files
- [ ] Save both files
- [ ] References update correctly

## Language Server Status Check

- [ ] Open Zed logs: `Cmd+Shift+P` → "zed: open log"
- [ ] Search for "amble"
- [ ] See "Amble LSP server initialized" message
- [ ] See "Opened document" messages for `.amble` files
- [ ] No error messages in logs

## System Status Check

- [ ] Run `ps aux | grep amble-lsp` in terminal
- [ ] See `amble-lsp` process running (when `.amble` file is open)
- [ ] Process terminates when all `.amble` files are closed

## Troubleshooting Checks (If Something Failed)

### If Build Failed
- [ ] Check Rust version: `rustc --version` (should be 1.70+)
- [ ] Clean and rebuild: `cargo clean && cargo build --release`
- [ ] Check for disk space
- [ ] Review error messages in build output

### If Installation Failed
- [ ] Verify path is correct (absolute path works better)
- [ ] Try restarting Zed completely
- [ ] Check Zed version is recent
- [ ] Try re-running installation command

### If F12 Doesn't Work
- [ ] Verify cursor is on the room identifier (not keyword "room")
- [ ] Check file has `.amble` extension
- [ ] Verify room is actually defined somewhere
- [ ] Check Zed logs for LSP errors
- [ ] Try closing and reopening the file
- [ ] Try restarting Zed

### If Find References Doesn't Work
- [ ] Save all files first (`Cmd+S`)
- [ ] Verify cursor is on room identifier
- [ ] Check that references actually exist
- [ ] Restart Zed and try again

### If Syntax Highlighting Doesn't Work
- [ ] Verify file extension is `.amble`
- [ ] Check language mode in status bar
- [ ] Restart Zed
- [ ] Re-install extension

## Final Verification

- [x] All tests passed
- [ ] Ready to use with real Amble projects
- [ ] Documented any issues encountered
- [ ] Read USAGE.md for advanced features

## Notes

Record any issues or observations here:

```
(Add your notes here)
```

## Getting Help

If multiple items failed:
1. Check USAGE.md for detailed troubleshooting
2. Review LSP_README.md for architecture details
3. Try QUICKSTART.md for streamlined setup
4. Check Zed logs for specific error messages
5. Verify all files are in the correct locations

## Success!

If all checkboxes are marked, congratulations! Your Amble language server is fully operational.

Next steps:
- Try it with your own `.amble` files
- Explore different room reference patterns
- Consider what features you'd like to see next