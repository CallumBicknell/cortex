# Code parsing (tree-sitter outlines)

Cortex can extract **structural outlines** from source files using
[tree-sitter](https://tree-sitter.github.io/). This is **not** a full language
server — no go-to-definition, diagnostics, or completions yet.

## Supported languages

| Language | Extensions | Symbols |
|----------|------------|---------|
| Rust | `.rs` | functions, structs, enums, traits, impls, mods, consts/types |
| Python | `.py`, `.pyi` | functions, classes, methods |

## CLI

```bash
cortex parse outline crates/cortex-runtime/src/agent_loop.rs
cortex parse outline path/to/mod.py --json
```

## Tool

`code_outline` (always-on via the `coding` skill):

```json
{ "path": "crates/cortex-tools/src/lib.rs" }
```

## Library

```rust
use cortex_parse::{outline_file, format_outline};

let outline = outline_file("src/main.rs")?;
println!("{}", format_outline(&outline));
```

## Not yet (LSP / later)

- go-to-definition / references
- diagnostics / hover
- TypeScript, Go, Solidity grammars
- Incremental re-parse of dirty buffers
- Workspace-wide symbol index
