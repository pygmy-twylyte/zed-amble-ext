# Repository Guidelines

## Project Structure & Module Organization
- `src/lib.rs` is the Zed entry point; it wires the extension runtime to the `bin/amble-lsp` executable via `zed_extension_api`.
- `language-server/` hosts the `tower-lsp` backend (`src/amble.rs`) plus its own `Cargo.*` and `target/`; rebuilds land in `language-server/target/release/amble-lsp`.
- `grammars/amble/` contains the tree-sitter grammar, Node bindings, and query fixtures that compile to `grammars/amble.wasm`.
- Highlight, fold, indent, and symbol queries live under `languages/amble/*.scm`, while reusable editor snippets sit in `snippets/`.
- Sample worlds in `fixtures/` and the root `test_*.amble` files double as regression inputs when exercising new language features.

## Build, Test, and Development Commands
```bash
./build.sh                               # Builds language-server in release mode and copies bin/amble-lsp
cargo test --package amble-lsp           # Run Rust tests for the LSP backend (add cases per feature)
cargo fmt --all && cargo clippy --all    # Enforce formatting and linting across both crates
(cd grammars/amble && npm test)          # Validates Node bindings and grammar shape
zed --dev-extension "$(pwd)"             # Load the extension into Zed for manual verification
```
Building locally relies on a stable Rust toolchain (2021 edition) and Node 18+ inside `grammars/amble`. Always rerun `./build.sh` before testing inside Zed so `src/lib.rs` can find the fresh binary.

## Coding Style & Naming Conventions
- Run `cargo fmt` before committing; default 4-space indents and imports grouped by crate.
- Prefer `snake_case` for functions/modules and `PascalCase` for types/enums in both crates. Keep handler helpers pure and side-effect free for easier testing.
- Tree-sitter query files remain `kebab-case` (`highlights.scm`, `locals.scm`); node names should mirror grammar symbols declared in `grammar.js`.
- Add doc comments when behavior is non-obvious, especially around the DashMap caches in `language-server/src/amble.rs`.

## Testing Guidelines
- Each new LSP feature should include a focused `#[cfg(test)]` helper test plus a concrete `.amble` fixture under `fixtures/<feature>/` or `test_<scenario>.amble`.
- When touching grammar rules or queries, run `npm test` and open the file in `tree-sitter playground` (`npm start`) to visually confirm captures.
- Manual validation flows: `zed --dev-extension "$(pwd)"`, open `fixtures/*.amble`, and exercise `F12`, symbol search, and diagnostics to ensure maps update correctly.

## Commit & Pull Request Guidelines
- Follow the existing concise style (`Component: reason`, e.g., `Grammar: align set refs`) with bodies explaining context, linked issues, or upstream grammar revisions.
- Keep commits scoped (grammar, language server, or queries) so reviewers can bisect quickly; include the commands you ran in the body when touching build/test infra.
- Pull requests must describe motivation, outline test coverage (`cargo test`, `npm test`, manual Zed checks), and attach screenshots/gifs when syntax colors or outlines change.
- Reference any upstream tree-sitter or engine versions in both the PR and `extension.toml` when bumps occur; reviewers will look for matching rev hashes.
