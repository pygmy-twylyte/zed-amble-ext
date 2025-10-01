# Maintenance Guide - Amble LSP Extension

This guide covers maintaining, updating, and extending the Amble language server extension for Zed.

## Quick Reference

### Rebuild After Changes
```bash
cd /home/dave/Code/zed-amble-ext
./build.sh
# Then restart Zed
```

### Update Extension in Zed
1. Quit Zed completely
2. Reopen Zed
3. Extension auto-recompiles if source changed
4. Check logs: `Cmd+Shift+P` → "zed: open log"

### Test Changes
```bash
cd /home/dave/Code/zed-amble-ext
zed test_lsp.amble
# Or
zed test_comprehensive.amble
```

## Directory Structure

```
zed-amble-ext/
├── extension.toml          # Extension metadata & config
├── Cargo.toml              # Extension WASM build
├── src/lib.rs              # Extension implementation (how to start LSP)
├── bin/amble-lsp           # LSP binary (copy of built binary)
├── language-server/        # LSP source code
│   ├── Cargo.toml          # LSP dependencies
│   └── src/amble.rs        # Main LSP implementation
├── languages/amble/        # Language configuration
│   └── config.toml         # Language settings
└── grammars/amble/         # Tree-sitter grammar (external)
```

## Making Changes

### Updating the Language Server

**When to edit:** Adding features, fixing bugs, supporting new patterns

**File to edit:** `language-server/src/amble.rs`

**Workflow:**
```bash
# 1. Edit the file
vim language-server/src/amble.rs

# 2. Build and test
cd language-server
cargo build --release
cargo test  # If you add tests

# 3. Copy to extension
cd ..
cp language-server/target/release/amble-lsp bin/

# 4. Restart Zed to pick up changes
```

**Common changes:**
- Add new regex patterns in `analyze_document()`
- Add new data structures in `Backend`
- Update `goto_definition()` or `references()` logic

### Updating the Extension Implementation

**When to edit:** Changing how the LSP starts, adding initialization options

**File to edit:** `src/lib.rs`

**Workflow:**
```bash
# 1. Edit the file
vim src/lib.rs

# 2. Let Zed rebuild it
# Just restart Zed - it will recompile the extension

# 3. Check logs for compilation
tail -f ~/.local/share/zed/logs/Zed.log
```

### Updating Configuration

**When to edit:** Changing language settings, extension metadata

**Files:**
- `extension.toml` - Extension metadata, language server config
- `languages/amble/config.toml` - Language behavior (brackets, comments, etc.)

**Important:** If you change language names, ensure exact case match:
```toml
# extension.toml
[language_servers.amble-lsp]
language = "Amble"  # Must match exactly ↓

# languages/amble/config.toml
name = "Amble"  # Must match exactly ↑
```

## Adding New Features

### Example: Add Support for Item References

**Step 1: Update data structures** (`language-server/src/amble.rs`)
```rust
#[derive(Debug, Clone)]
struct ItemDefinition {
    uri: Url,
    range: Range,
}

#[derive(Debug, Clone)]
struct ItemReference {
    uri: Url,
    range: Range,
}

// In Backend struct:
item_definitions: Arc<DashMap<String, ItemDefinition>>,
item_references: Arc<DashMap<String, Vec<ItemReference>>>,
```

**Step 2: Add regex patterns** (in `analyze_document()`)
```rust
// Item definitions: item <item-id> {
let item_def_regex = Regex::new(
    r"(?m)^[ \t]*item[ \t]+([a-zA-Z_][a-zA-Z0-9_-]*)[ \t]*\{"
).unwrap();

// Item references: various contexts
let take_item_regex = Regex::new(
    r"take[ \t]+item[ \t]+([a-zA-Z_][a-zA-Z0-9_-]*)"
).unwrap();
```

**Step 3: Parse and store** (in `analyze_document()`)
```rust
// Parse definitions
for cap in item_def_regex.captures_iter(text) {
    // ... similar to room parsing
}

// Parse references
for cap in take_item_regex.captures_iter(text) {
    // ... similar to room parsing
}
```

**Step 4: Update lookup** (modify `get_room_id_at_position()` or create new method)
```rust
fn get_symbol_at_position(&self, uri: &Url, position: Position) 
    -> Option<(SymbolType, String)> 
{
    // Check rooms, items, NPCs, etc.
}
```

**Step 5: Update LSP methods** (in `goto_definition()` and `references()`)
```rust
// Check both rooms and items
if let Some(def) = self.item_definitions.get(&item_id) {
    // Return item definition
}
```

**Step 6: Rebuild and test**
```bash
./build.sh
# Restart Zed and test
```

## Dependency Updates

### Update Language Server Dependencies

```bash
cd language-server

# Check for updates
cargo outdated

# Update specific package
cargo update -p tower-lsp

# Update all dependencies
cargo update

# Test still compiles
cargo build --release
```

### Update Extension Dependencies

```bash
# Check for updates
cargo outdated

# Update zed_extension_api
cargo update -p zed_extension_api

# Restart Zed to test
```

### Check for Breaking Changes

After updating dependencies, verify:
1. Compilation succeeds
2. LSP starts without errors
3. Go To Definition works
4. Find References works
5. No new errors in Zed logs

## Version Management

### Updating Version Numbers

**When to update:**
- Major features: Increment major version (0.x.0 → 1.0.0)
- New features: Increment minor version (0.1.0 → 0.2.0)
- Bug fixes: Increment patch version (0.1.0 → 0.1.1)

**Files to update:**
```toml
# extension.toml
version = "0.10.0"  # Extension version

# language-server/Cargo.toml
version = "0.1.0"  # LSP version (can be independent)

# Cargo.toml (extension)
version = "0.1.0"  # Extension impl version
```

### Publishing Updates

If you later publish to Zed's extension registry:
1. Update version in `extension.toml`
2. Commit all changes
3. Tag the release: `git tag v0.10.0`
4. Push: `git push --tags`

## Debugging

### Language Server Not Starting

**Check 1: Binary exists**
```bash
ls -la ~/.local/share/zed/extensions/installed/amble/bin/amble-lsp
```

**Check 2: Binary is executable**
```bash
chmod +x ~/.local/share/zed/extensions/installed/amble/bin/amble-lsp
```

**Check 3: Test binary manually**
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}' | \
  ~/.local/share/zed/extensions/installed/amble/bin/amble-lsp
```

**Check 4: Extension compiled**
```bash
ls -la ~/.local/share/zed/extensions/installed/amble/extension.wasm
```

**Check 5: Logs**
```bash
tail -f ~/.local/share/zed/logs/Zed.log | grep -i amble
```

### Go To Definition Not Working

**Debug steps:**
1. Add `eprintln!` statements in `get_room_id_at_position()`
2. Rebuild: `./build.sh`
3. Restart Zed
4. Check stderr output in logs

**Common issues:**
- Cursor not on identifier
- Room not defined in same directory
- Case mismatch in room ID
- File not saved (LSP needs updated content)

### Find References Not Working

**Check:**
1. Room is actually referenced somewhere
2. All `.amble` files in directory are saved
3. References in correct format (check regex patterns)

## Performance Tuning

### If Slow with Large Files

**Option 1: Optimize regex patterns**
- Compile regex once (already done)
- Use more specific patterns
- Avoid backtracking

**Option 2: Incremental parsing**
- Currently parses entire file on change
- Could optimize to parse only changed sections

**Option 3: Caching**
- Already uses DashMap for fast lookups
- Consider caching compiled regex

### If High Memory Usage

**Check memory profile:**
```bash
# While Zed is running with large project
ps aux | grep amble-lsp
```

**Optimize:**
- Clear old document cache
- Limit directory scanning depth
- Use more efficient data structures

## Testing

### Manual Testing Checklist

- [ ] Go To Definition from exit targets
- [ ] Go To Definition from trigger conditions  
- [ ] Go To Definition from actions
- [ ] Find All References shows all occurrences
- [ ] Works across multiple files
- [ ] Updates on file save
- [ ] Handles undefined rooms gracefully
- [ ] No crashes with malformed syntax

### Automated Testing (Future)

Add to `language-server/src/amble.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_room_definition() {
        let text = "room test-room {\n    name \"Test\"\n}";
        // ... test parsing logic
    }

    #[test]
    fn test_find_references() {
        // ... test reference finding
    }
}
```

Run tests:
```bash
cd language-server
cargo test
```

## Common Maintenance Tasks

### Monthly
- [ ] Check for dependency updates
- [ ] Review open issues/bugs
- [ ] Test with latest Zed version
- [ ] Update documentation if needed

### When Zed Updates
- [ ] Test extension still loads
- [ ] Check for new zed_extension_api features
- [ ] Verify LSP still starts
- [ ] Test all features work

### When Adding Features
- [ ] Update LSP_README.md
- [ ] Update SUCCESS.md feature list
- [ ] Add tests to test files
- [ ] Update QUICKSTART if UI changes

## Troubleshooting Reference

| Problem | Solution |
|---------|----------|
| "Binary not found" | Copy binary to bin/: `cp language-server/target/release/amble-lsp bin/` |
| "Extension won't compile" | Check Cargo.toml, ensure zed_extension_api version compatible |
| "LSP starts but doesn't work" | Add debug logging, check Zed stderr |
| "Changes not taking effect" | Quit Zed completely and restart |
| "Case-sensitive errors" | Check language name matches exactly in all configs |
| "WASM compile errors" | Make sure not using tokio or other WASM-incompatible crates in extension |

## Getting Help

1. Check logs: `~/.local/share/zed/logs/Zed.log`
2. Review SUCCESS.md for working configuration
3. Check Zed extension API docs: https://zed.dev/docs/extensions
4. Look at other Zed extensions for examples

## Backup & Recovery

### Before Major Changes
```bash
cd /home/dave/Code/zed-amble-ext
git add .
git commit -m "Backup before changes"
git tag backup-$(date +%Y%m%d)
```

### Restore Working Version
```bash
git checkout backup-20251001
./build.sh
# Reinstall in Zed
```

## Notes for Future Development

- Consider tree-sitter integration for better parsing
- Consider recursive directory scanning
- Consider workspace-wide indexing
- Consider autocompletion support
- Consider hover information
- Consider diagnostics/linting
- Consider rename refactoring

The current regex-based approach works well for the prototype but may need enhancement as features grow.