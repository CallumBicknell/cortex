//! Tool registry.

use crate::error::{Result, ToolError};
use crate::tool::Tool;
use cortex_models::ToolSpec;
use std::collections::HashMap;
use std::sync::Arc;

/// Registry of named tools.
#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool. Errors if the name is already taken.
    pub fn register(&mut self, tool: Arc<dyn Tool>) -> Result<()> {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            return Err(ToolError::AlreadyRegistered(name));
        }
        self.tools.insert(name, tool);
        Ok(())
    }

    /// Register, replacing any existing tool with the same name.
    pub fn register_or_replace(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Result<Arc<dyn Tool>> {
        self.tools
            .get(name)
            .cloned()
            .ok_or_else(|| ToolError::NotFound(name.to_string()))
    }

    /// Whether a tool exists.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// True if empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Sorted tool names.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.tools.keys().cloned().collect();
        names.sort();
        names
    }

    /// Specs for all tools (sorted by name) — ready for LLM tool-calling.
    pub fn specs(&self) -> Vec<ToolSpec> {
        let mut specs: Vec<_> = self.tools.values().map(|t| t.spec()).collect();
        specs.sort_by(|a, b| a.name.cmp(&b.name));
        specs
    }

    /// Filter specs to a subset of names (unknown names ignored).
    pub fn specs_for(&self, names: &[String]) -> Vec<ToolSpec> {
        names
            .iter()
            .filter_map(|n| self.tools.get(n).map(|t| t.spec()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::filesystem::ReadFileTool;
    use std::sync::Arc;

    #[test]
    fn register_and_list() {
        let mut reg = ToolRegistry::new();
        reg.register(Arc::new(ReadFileTool)).unwrap();
        assert!(reg.contains("read_file"));
        assert_eq!(reg.names(), vec!["read_file".to_string()]);
        assert_eq!(reg.specs().len(), 1);
        assert!(reg.register(Arc::new(ReadFileTool)).is_err());
    }
}
