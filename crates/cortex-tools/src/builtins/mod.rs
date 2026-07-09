//! Builtin tools shipped with Cortex.

pub mod filesystem;
pub mod git;
pub mod http;
pub mod shell;

use crate::error::Result;
use crate::registry::ToolRegistry;
use filesystem::{EditFileTool, GlobFilesTool, ListDirTool, ReadFileTool, WriteFileTool};
use git::{GitAddTool, GitCommitTool, GitDiffTool, GitLogTool, GitStatusTool};
use http::HttpRequestTool;
use shell::ShellTool;
use std::sync::Arc;

/// Register the default builtin tool set.
pub fn register_default_tools(registry: &mut ToolRegistry) -> Result<()> {
    registry.register(Arc::new(ReadFileTool))?;
    registry.register(Arc::new(WriteFileTool))?;
    registry.register(Arc::new(EditFileTool))?;
    registry.register(Arc::new(ListDirTool))?;
    registry.register(Arc::new(GlobFilesTool))?;
    registry.register(Arc::new(ShellTool))?;
    registry.register(Arc::new(GitStatusTool))?;
    registry.register(Arc::new(GitDiffTool))?;
    registry.register(Arc::new(GitLogTool))?;
    registry.register(Arc::new(GitAddTool))?;
    registry.register(Arc::new(GitCommitTool))?;
    registry.register(Arc::new(HttpRequestTool::new()))?;
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
        "shell",
        "git_status",
        "git_diff",
        "git_log",
        "git_add",
        "git_commit",
        "http_request",
    ]
}
