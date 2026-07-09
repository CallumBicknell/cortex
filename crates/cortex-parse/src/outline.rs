//! Symbol outline extraction via tree-sitter queries.

use crate::error::{ParseError, Result};
use crate::language::SourceLanguage;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tree_sitter::{Node, Parser};

/// One symbol in a file outline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Symbol {
    /// Kind: function, struct, enum, impl, class, module, …
    pub kind: String,
    /// Display name.
    pub name: String,
    /// 1-based start line.
    pub start_line: usize,
    /// 1-based end line.
    pub end_line: usize,
    /// Optional parent/container name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// Outline of a source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outline {
    /// Path as provided.
    pub path: String,
    /// Detected language.
    pub language: String,
    /// Symbols ordered by appearance.
    pub symbols: Vec<Symbol>,
}

/// Parse source bytes into a tree-sitter tree and extract symbols.
pub fn outline_source(path: &str, source: &str, lang: SourceLanguage) -> Result<Outline> {
    let mut parser = Parser::new();
    parser
        .set_language(&lang.language())
        .map_err(|e| ParseError::Parse(e.to_string()))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| ParseError::Parse("tree-sitter returned None".into()))?;

    let symbols = match lang {
        SourceLanguage::Rust => extract_rust(tree.root_node(), source.as_bytes())?,
        SourceLanguage::Python => extract_python(tree.root_node(), source.as_bytes())?,
        SourceLanguage::Solidity => extract_solidity(tree.root_node(), source.as_bytes())?,
    };

    Ok(Outline {
        path: path.to_string(),
        language: lang.name().to_string(),
        symbols,
    })
}

/// Read a file and outline it (language from extension).
pub fn outline_file(path: impl AsRef<Path>) -> Result<Outline> {
    let path = path.as_ref();
    let lang = SourceLanguage::from_path(path)?;
    let source = std::fs::read_to_string(path)?;
    outline_source(&path.display().to_string(), &source, lang)
}

/// Format outline for agent context.
pub fn format_outline(outline: &Outline) -> String {
    let mut out = format!("# Outline: {} ({})\n", outline.path, outline.language);
    if outline.symbols.is_empty() {
        out.push_str("(no symbols found)\n");
        return out;
    }
    for s in &outline.symbols {
        let parent = s
            .parent
            .as_ref()
            .map(|p| format!(" in {p}"))
            .unwrap_or_default();
        out.push_str(&format!(
            "L{}-{}  {:10} {}{}\n",
            s.start_line, s.end_line, s.kind, s.name, parent
        ));
    }
    out
}

fn extract_rust(root: Node, source: &[u8]) -> Result<Vec<Symbol>> {
    // Walk without queries for portability across grammar versions.
    let mut symbols = Vec::new();
    walk_rust(root, source, None, &mut symbols);
    Ok(symbols)
}

fn walk_rust(node: Node, source: &[u8], parent: Option<&str>, out: &mut Vec<Symbol>) {
    let kind = node.kind();
    match kind {
        "function_item" => {
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol("function", &name, node, parent));
            }
        }
        "struct_item" => {
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol("struct", &name, node, parent));
            }
        }
        "enum_item" => {
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol("enum", &name, node, parent));
            }
        }
        "trait_item" => {
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol("trait", &name, node, parent));
            }
        }
        "mod_item" => {
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol("mod", &name, node, parent));
            }
        }
        "impl_item" => {
            // type name or trait for Type
            let type_name = node
                .child_by_field_name("type")
                .map(|n| node_text(n, source))
                .unwrap_or_else(|| "impl".into());
            let trait_name = node
                .child_by_field_name("trait")
                .map(|n| node_text(n, source));
            let name = match trait_name {
                Some(t) => format!("{t} for {type_name}"),
                None => type_name.clone(),
            };
            out.push(symbol("impl", &name, node, parent));
            // Methods inside impl body
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "declaration_list" {
                    walk_rust(child, source, Some(&name), out);
                }
            }
            return; // already walked body
        }
        "const_item" | "static_item" | "type_item" => {
            if let Some(name) = child_text(node, "name", source) {
                let k = match kind {
                    "const_item" => "const",
                    "static_item" => "static",
                    _ => "type",
                };
                out.push(symbol(k, &name, node, parent));
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_rust(child, source, parent, out);
    }
}

fn extract_python(root: Node, source: &[u8]) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    walk_python(root, source, None, &mut symbols);
    Ok(symbols)
}

fn extract_solidity(root: Node, source: &[u8]) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    walk_solidity(root, source, None, &mut symbols);
    Ok(symbols)
}

fn walk_solidity(node: Node, source: &[u8], parent: Option<&str>, out: &mut Vec<Symbol>) {
    let kind = node.kind();
    match kind {
        "contract_declaration" | "interface_declaration" | "library_declaration" => {
            let k = match kind {
                "interface_declaration" => "interface",
                "library_declaration" => "library",
                _ => "contract",
            };
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol(k, &name, node, parent));
                // Walk body with container as parent
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "contract_body" {
                        walk_solidity(child, source, Some(&name), out);
                    }
                }
                return;
            }
        }
        "function_definition" => {
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol("function", &name, node, parent));
                return;
            }
        }
        "constructor_definition" | "constructor" => {
            out.push(symbol("constructor", "constructor", node, parent));
            return;
        }
        "fallback_receive_definition" => {
            let text = node_text(node, source);
            let name = if text.contains("receive") {
                "receive"
            } else if text.contains("fallback") {
                "fallback"
            } else {
                "fallback_or_receive"
            };
            out.push(symbol("function", name, node, parent));
            return;
        }
        "modifier_definition" => {
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol("modifier", &name, node, parent));
                return;
            }
        }
        "event_definition" => {
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol("event", &name, node, parent));
                return;
            }
        }
        "error_declaration" => {
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol("error", &name, node, parent));
                return;
            }
        }
        "struct_declaration" => {
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol("struct", &name, node, parent));
                return;
            }
        }
        "enum_declaration" => {
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol("enum", &name, node, parent));
                return;
            }
        }
        "state_variable_declaration" => {
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol("state_var", &name, node, parent));
                return;
            }
        }
        "constant_variable_declaration" => {
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol("constant", &name, node, parent));
                return;
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_solidity(child, source, parent, out);
    }
}

fn walk_python(node: Node, source: &[u8], parent: Option<&str>, out: &mut Vec<Symbol>) {
    let kind = node.kind();
    match kind {
        "function_definition" => {
            if let Some(name) = child_text(node, "name", source) {
                let k = if parent.is_some() {
                    "method"
                } else {
                    "function"
                };
                out.push(symbol(k, &name, node, parent));
            }
        }
        "class_definition" => {
            if let Some(name) = child_text(node, "name", source) {
                out.push(symbol("class", &name, node, parent));
                // Walk body with this class as parent
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "block" {
                        walk_python(child, source, Some(&name), out);
                    }
                }
                return;
            }
        }
        "decorated_definition" => {
            // Prefer the definition inside
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_python(child, source, parent, out);
            }
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_python(child, source, parent, out);
    }
}

fn symbol(kind: &str, name: &str, node: Node, parent: Option<&str>) -> Symbol {
    Symbol {
        kind: kind.into(),
        name: name.into(),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        parent: parent.map(|s| s.to_string()),
    }
}

fn child_text(node: Node, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .map(|n| node_text(n, source))
}

fn node_text(node: Node, source: &[u8]) -> String {
    String::from_utf8_lossy(&source[node.byte_range()]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_outline_functions_and_struct() {
        let src = r#"
pub struct Foo {
    x: i32,
}

impl Foo {
    pub fn new() -> Self { Self { x: 0 } }
    pub fn bar(&self) {}
}

pub fn top_level() {}

pub trait T { fn m(&self); }
"#;
        let outline = outline_source("lib.rs", src, SourceLanguage::Rust).unwrap();
        let kinds: Vec<_> = outline.symbols.iter().map(|s| s.kind.as_str()).collect();
        assert!(kinds.contains(&"struct"), "{kinds:?}");
        assert!(kinds.contains(&"impl"), "{kinds:?}");
        assert!(kinds.contains(&"function"), "{kinds:?}");
        assert!(kinds.contains(&"trait"), "{kinds:?}");
        assert!(outline.symbols.iter().any(|s| s.name == "top_level"));
        assert!(outline.symbols.iter().any(|s| s.name == "new"));
    }

    #[test]
    fn python_outline_class_and_fn() {
        let src = r#"
def top():
    pass

class C:
    def method(self):
        pass
"#;
        let outline = outline_source("m.py", src, SourceLanguage::Python).unwrap();
        assert!(outline
            .symbols
            .iter()
            .any(|s| s.kind == "function" && s.name == "top"));
        assert!(outline
            .symbols
            .iter()
            .any(|s| s.kind == "class" && s.name == "C"));
        assert!(outline
            .symbols
            .iter()
            .any(|s| s.kind == "method" && s.name == "method" && s.parent.as_deref() == Some("C")));
    }

    #[test]
    fn format_nonempty() {
        let outline = outline_source("a.rs", "fn main() {}", SourceLanguage::Rust).unwrap();
        let text = format_outline(&outline);
        assert!(text.contains("main"));
    }

    #[test]
    fn solidity_outline_contract_and_fn() {
        let src = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract Vault {
    mapping(address => uint256) public balances;
    event Withdraw(address indexed who, uint256 amount);
    error ZeroAmount();

    function withdraw() external {
        uint256 bal = balances[msg.sender];
        balances[msg.sender] = 0;
        (bool ok,) = msg.sender.call{value: bal}("");
        require(ok);
    }

    modifier onlyPositive(uint256 x) {
        require(x > 0);
        _;
    }
}

interface IVault {
    function deposit() external payable;
}
"#;
        let outline = outline_source("Vault.sol", src, SourceLanguage::Solidity).unwrap();
        assert!(
            outline
                .symbols
                .iter()
                .any(|s| s.kind == "contract" && s.name == "Vault"),
            "{:?}",
            outline.symbols
        );
        assert!(
            outline
                .symbols
                .iter()
                .any(|s| s.kind == "function" && s.name == "withdraw"),
            "{:?}",
            outline.symbols
        );
        assert!(
            outline
                .symbols
                .iter()
                .any(|s| s.kind == "interface" && s.name == "IVault"),
            "{:?}",
            outline.symbols
        );
        assert!(outline.symbols.iter().any(|s| s.kind == "event"));
        assert!(outline.symbols.iter().any(|s| s.kind == "modifier"));
    }
}
