//! Cortex tools: uniform tool interface, registry, permissions, and builtins.
//!
//! The runtime invokes tools only through [`Tool`] + [`ToolExecutor`]. Concrete
//! tools (filesystem, shell, git, HTTP, MCP adapters later) plug in without
//! changing the loop.

#![deny(missing_docs)]

pub mod builtins;
mod error;
mod executor;
mod permissions;
mod registry;
mod tool;

pub use builtins::{
    default_tool_names, register_default_tools, register_default_tools_with_browser,
    register_memory_tools, register_skill_tools, BrowserBackend, BrowserConfig, BrowserHandle,
    MemoryHandle, MemorySearchTool, SkillStoreHandle,
};
pub use error::{Result, ToolError};
pub use executor::ToolExecutor;
pub use permissions::{PermissionMode, PermissionPolicy};
pub use registry::ToolRegistry;
pub use tool::{
    run_tool, AlwaysAllow, AlwaysDeny, ApprovalDecision, ApprovalRequest, Approver, Tool,
    ToolContext,
};
