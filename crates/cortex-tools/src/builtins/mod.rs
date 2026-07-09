//! Builtin tools shipped with Cortex.

pub mod browser;
pub mod diff;
pub mod docker;
pub mod filesystem;
pub mod git;
pub mod http;
pub mod memory;
pub mod search;
pub mod shell;

use crate::error::Result;
use crate::registry::ToolRegistry;
use diff::ApplyPatchTool;
use docker::DockerRunTool;
use filesystem::{EditFileTool, GlobFilesTool, ListDirTool, ReadFileTool, WriteFileTool};
use git::{GitAddTool, GitCommitTool, GitDiffTool, GitLogTool, GitStatusTool};
use http::HttpRequestTool;
use search::WebSearchTool;
use shell::ShellTool;
use std::sync::Arc;

pub use browser::{register_browser_tools, BrowserBackend, BrowserConfig, BrowserHandle};
pub use memory::{MemoryHandle, MemorySearchTool};

/// Register the default builtin tool set (including browser tools with default Obscura config).
pub fn register_default_tools(registry: &mut ToolRegistry) -> Result<()> {
    register_default_tools_with_browser(registry, BrowserHandle::from_env_or_default())
}

/// Register builtins with an explicit browser handle/config.
pub fn register_default_tools_with_browser(
    registry: &mut ToolRegistry,
    browser: BrowserHandle,
) -> Result<()> {
    registry.register(Arc::new(ReadFileTool))?;
    registry.register(Arc::new(WriteFileTool))?;
    registry.register(Arc::new(EditFileTool))?;
    registry.register(Arc::new(ListDirTool))?;
    registry.register(Arc::new(GlobFilesTool))?;
    registry.register(Arc::new(ApplyPatchTool))?;
    registry.register(Arc::new(ShellTool))?;
    registry.register(Arc::new(GitStatusTool))?;
    registry.register(Arc::new(GitDiffTool))?;
    registry.register(Arc::new(GitLogTool))?;
    registry.register(Arc::new(GitAddTool))?;
    registry.register(Arc::new(GitCommitTool))?;
    registry.register(Arc::new(HttpRequestTool::new()))?;
    registry.register(Arc::new(DockerRunTool))?;
    registry.register(Arc::new(WebSearchTool::from_env()))?;
    register_browser_tools(registry, browser);
    Ok(())
}

/// Names of all default tools.
pub fn default_tool_names() -> Vec<&'static str> {
    vec![
        "read_file",
        "write_file",
        "edit_file",
        "list_dir",
        "glob_files",
        "apply_patch",
        "shell",
        "git_status",
        "git_diff",
        "git_log",
        "git_add",
        "git_commit",
        "http_request",
        "docker_run",
        "web_search",
        "browser_navigate",
        "browser_evaluate",
        "browser_snapshot",
        "browser_content",
        "browser_click",
        "browser_close",
        "memory_search",
    ]
}

/// Register optional memory tools when a handle is available.
pub fn register_memory_tools(registry: &mut ToolRegistry, handle: MemoryHandle) {
    let _ = registry.register(Arc::new(MemorySearchTool::new(handle)));
}
