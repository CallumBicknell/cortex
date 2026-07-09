//! Classify tools for concurrent execution safety.

/// Tools that are safe to run concurrently with each other (read-only / no shared
/// mutable workspace side effects). Writes, shell, network mutators, and nested
/// agents stay serial.
pub fn is_parallel_safe(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "read_file"
            | "list_dir"
            | "glob_files"
            | "git_status"
            | "git_diff"
            | "git_log"
            | "code_outline"
            | "workspace_symbols"
            | "code_definition"
            | "memory_search"
            | "skill_list"
            | "browser_snapshot"
            | "browser_content"
            | "web_search"
    )
}

/// Tools that mutate workspace files (used for post-edit verify hooks).
pub fn is_file_mutating(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "write_file" | "edit_file" | "apply_patch" | "write_audit_report"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_are_safe_writes_are_not() {
        assert!(is_parallel_safe("read_file"));
        assert!(is_parallel_safe("code_outline"));
        assert!(!is_parallel_safe("write_file"));
        assert!(!is_parallel_safe("shell"));
        assert!(!is_parallel_safe("spawn_subagent"));
        assert!(!is_parallel_safe("audit_lenses"));
        assert!(is_file_mutating("edit_file"));
        assert!(!is_file_mutating("read_file"));
    }
}
