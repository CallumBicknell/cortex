# Code parsing (tree-sitter outlines)

Cortex can extract **structural outlines** from source files using
[tree-sitter](https://tree-sitter.github.io/). This is **not** a full language
server — no diagnostics or hover. Workspace symbol search and name-based
definition lookup are available via `workspace_symbols` / `code_definition`.

## Supported languages

| Language | Extensions | Symbols |
|----------|------------|---------|
| Rust | `.rs` | functions, structs, enums, traits, impls, mods, consts/types |
| Python | `.py`, `.pyi` | functions, classes, methods |
| Solidity | `.sol` | contracts, interfaces, libraries, functions, modifiers, events, errors, state vars, structs, enums |

## CLI

```bash
cortex parse outline crates/cortex-runtime/src/agent_loop.rs
cortex parse outline examples/foundry-vault/src/VulnerableVault.sol
cortex parse outline path/to/mod.py --json
```

## Tools

| Tool | Role |
|------|------|
| `code_outline` | File outline |
| `workspace_symbols` | Search indexed symbols |
| `code_definition` | Find definitions by name |

## Library

```rust
use cortex_parse::{outline_file, format_outline};

let outline = outline_file("src/Vault.sol")?;
println!("{}", format_outline(&outline));
```

## Not yet (full LSP / later)

- True go-to-definition / references via language servers
- Diagnostics / hover
- TypeScript, Go grammars
- Incremental re-parse of dirty buffers
