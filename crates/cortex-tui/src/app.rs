//! TUI application state — Claude Code–style chat surface.

use crate::complete::{self, CompletionState};
use crate::host::TuiHost;
use anyhow::Result;
use cortex_memory::SessionSummary;
use cortex_models::{Message, Role, Session};
use cortex_tools::{ApprovalDecision, ApprovalRequest};
use ratatui::widgets::ListState;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::oneshot;

/// A display block in the conversation.
#[derive(Debug, Clone)]
pub struct MessageLine {
    /// Role label: you | cortex | tool | system.
    pub role: String,
    /// Body text (may contain newlines).
    pub content: String,
}

impl MessageLine {
    /// User line.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "you".into(),
            content: content.into(),
        }
    }

    /// Assistant line.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "cortex".into(),
            content: content.into(),
        }
    }

    /// System / status line.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    /// Tool activity line.
    pub fn tool(content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: content.into(),
        }
    }

    /// From a session message.
    pub fn from_message(m: &Message) -> Self {
        let role = match m.role {
            Role::User => "you",
            Role::Assistant => "cortex",
            Role::System => "system",
            Role::Tool => "tool",
        };
        let mut content = m.content.clone();
        if !m.tool_calls.is_empty() {
            let names: Vec<_> = m.tool_calls.iter().map(|t| t.name.as_str()).collect();
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(&format!("→ {}", names.join(", ")));
        }
        if content.len() > 12_000 {
            let boundary = content.floor_char_boundary(12_000);
            content.truncate(boundary);
            content.push('…');
        }
        Self {
            role: role.into(),
            content,
        }
    }
}

/// Pending tool-approval modal state.
pub struct ApprovalModal {
    /// The original approval request.
    pub request: ApprovalRequest,
    /// Channel to send the user's decision back to the [`TuiApprover`].
    pub respond: oneshot::Sender<ApprovalDecision>,
}

/// Result of a background agent turn.
#[derive(Debug)]
pub struct RunUpdate {
    /// Whether the run succeeded.
    pub ok: bool,
    /// Updated session.
    pub session: Session,
    /// Assistant reply text.
    pub assistant: String,
    /// Tool log lines.
    pub logs: Vec<String>,
    /// Status summary.
    pub status: String,
    /// Optional error.
    pub error: Option<String>,
    /// LLM turns consumed.
    pub turns: u32,
    /// Wall duration ms.
    pub duration_ms: u64,
    /// Tool results that succeeded.
    pub tools_ok: u32,
    /// Tool results that failed.
    pub tools_err: u32,
    /// Prompt/input tokens.
    pub prompt_tokens: u32,
    /// Completion/output tokens.
    pub completion_tokens: u32,
}

/// Live UI events from a background run (stream + completion).
#[derive(Debug)]
pub enum UiEvent {
    /// Streaming assistant text delta.
    StreamDelta(String),
    /// Tool / sub-agent log line.
    ToolLog(String),
    /// Transient status (e.g. "planning…").
    Status(String),
    /// Run finished.
    Done(Box<RunUpdate>),
}

/// Live TUI state.
pub struct App {
    /// Workspace path display.
    pub workspace: String,
    /// Model label.
    pub model_label: String,
    /// Database path display.
    pub database: String,
    /// Custom session label (set by /rename).
    pub session_label: String,
    /// Session list.
    pub sessions: Vec<SessionSummary>,
    /// List widget state.
    pub session_list: ListState,
    /// Active session.
    pub session: Session,
    /// Transcript lines.
    pub lines: Vec<MessageLine>,
    /// Scroll offset from bottom (0 = stick to bottom).
    pub scroll: u16,
    /// Recent tool activity (footer strip / optional).
    pub logs: Vec<String>,
    /// Input buffer (may contain newlines).
    pub input: String,
    /// Cursor position in input (char index, not byte index).
    pub input_cursor: usize,
    /// Whether the input box is focused.
    pub input_focused: bool,
    /// Show sessions drawer.
    pub show_sessions: bool,
    /// Session search filter (applied in drawer).
    pub session_search: String,
    /// Agent currently running.
    pub running: bool,
    /// Auto-approve tools.
    pub yolo: bool,
    /// Max turns.
    pub max_turns: u32,
    /// Skills override.
    pub skills: Vec<String>,
    /// Status bar text.
    pub status: String,
    /// Live streaming assistant draft (while a run is in progress).
    pub streaming: Option<String>,
    /// Last activity line (tool chip under stream).
    pub activity: Option<String>,
    /// Pending tool-approval modal (blocks input when `Some`).
    pub approval: Option<ApprovalModal>,
    /// Last run prompt tokens.
    pub last_prompt_tokens: u32,
    /// Last run completion tokens.
    pub last_completion_tokens: u32,
    /// Undo stack for composer (input, cursor) pairs.
    pub input_undo: Vec<(String, usize)>,
    /// Compact mode (less spacing, smaller header).
    pub compact: bool,
    /// Workspace root (for `@path` completion / attachment expansion).
    pub workspace_path: PathBuf,
    /// Known skill ids for `/skill` autocomplete.
    pub skill_ids: Vec<String>,
    /// Skill id → short description.
    pub skill_details: Vec<(String, String)>,
    /// Active composer completion popup.
    pub completion: Option<CompletionState>,
    /// Sent prompt history (most recent last).
    pub history: Vec<String>,
    /// History browsing index (`None` = not browsing).
    pub history_index: Option<usize>,
    /// Draft saved when entering history browsing mode.
    pub history_draft: String,
    /// Auto-follow new streaming content (reset scroll to bottom).
    pub auto_follow: bool,
    /// Whether the streaming cursor is visible (toggled for blink animation).
    pub cursor_visible: bool,
    /// Last time the cursor blink state toggled.
    pub last_blink: Instant,
    /// Tool start time for elapsed display (tool_name -> start time).
    pub tool_start: Option<Instant>,
}

impl App {
    /// Create app and load sessions.
    pub async fn new(host: &TuiHost) -> Result<Self> {
        let sessions = host.list_sessions(30).await.unwrap_or_default();
        let session = Session::new(
            host.workspace.to_string_lossy(),
            format!("{}/{}", host.provider_id, host.model),
        );
        let mut session_list = ListState::default();
        if !sessions.is_empty() {
            session_list.select(Some(0));
        }
        let skills = host.list_skills();
        let skill_ids: Vec<String> = skills.iter().map(|(id, _)| id.clone()).collect();
        let skill_details = skills;
        let welcome = format!(
            "Cortex · {} · {}\n\nType a message and press Enter to send.\n\
             /skill · @path · Tab complete · Ctrl+J newline · Ctrl+B sessions · /quit",
            host.model_alias,
            host.workspace.display()
        );
        // Fresh conversation by default (Claude Code–style). Resume via Ctrl+B sessions.
        Ok(Self {
            workspace: host.workspace.display().to_string(),
            model_label: format!("{} · {}/{}", host.model_alias, host.provider_id, host.model),
            database: host.database.display().to_string(),
            session_label: String::new(),
            sessions,
            session_list,
            session,
            lines: vec![MessageLine::system(welcome)],
            scroll: 0,
            logs: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            input_focused: true,
            show_sessions: false,
            session_search: String::new(),
            running: false,
            yolo: host.yolo,
            max_turns: host.max_turns,
            skills: host.skills.clone(),
            status: "ready".into(),
            streaming: None,
            activity: None,
            approval: None,
            last_prompt_tokens: 0,
            last_completion_tokens: 0,
            input_undo: Vec::new(),
            compact: false,
            workspace_path: host.workspace.clone(),
            skill_ids,
            skill_details,
            completion: None,
            history: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            auto_follow: true,
            cursor_visible: true,
            last_blink: Instant::now(),
            tool_start: None,
        })
    }

    /// Refresh autocomplete from the current input buffer.
    pub fn refresh_completion(&mut self) {
        self.completion = complete::refresh_completion(
            &self.input,
            &self.skill_ids,
            &self.skill_details,
            &self.workspace_path,
        );
    }

    /// Accept the selected completion item into the input buffer.
    ///
    /// Directory `@path/` completions stay open so nesting can continue; file
    /// and `/skill` completions dismiss the popup (next Enter sends).
    pub fn accept_completion(&mut self) -> bool {
        let Some(state) = self.completion.clone() else {
            return false;
        };
        if state.items.is_empty() {
            self.completion = None;
            return false;
        }
        let keep_open = state
            .current()
            .map(|i| i.insert.ends_with('/'))
            .unwrap_or(false);
        self.input = complete::apply_completion(&self.input, &state);
        self.input_cursor = self.input.chars().count();
        if keep_open {
            self.refresh_completion();
        } else {
            self.completion = None;
        }
        true
    }

    /// Dismiss completion popup.
    pub fn clear_completion(&mut self) {
        self.completion = None;
    }

    /// Start a fresh session.
    pub fn new_session(&mut self) {
        let model = self.session.model.clone();
        let ws = self.session.workspace.clone();
        self.session = Session::new(ws, model);
        self.lines = vec![MessageLine::system("New session.")];
        self.logs.clear();
        self.streaming = None;
        self.activity = None;
        self.session_list.select(None);
        self.status = "new session".into();
        self.scroll = 0;
    }

    /// Replace active session and rebuild transcript.
    pub fn set_session(&mut self, session: Session) {
        self.lines = session
            .messages
            .iter()
            .filter(|m| m.role != Role::System || m.content.starts_with('['))
            .map(MessageLine::from_message)
            .collect();
        if self.lines.is_empty() {
            self.lines
                .push(MessageLine::system("Empty session — send a message."));
        }
        self.session = session;
        self.scroll = 0;
        self.streaming = None;
        self.activity = None;
    }

    /// Reload session list from store.
    pub async fn reload_sessions(&mut self, host: &TuiHost) -> Result<()> {
        self.sessions = host.list_sessions(30).await?;
        Ok(())
    }

    /// Toggle sessions drawer.
    pub fn toggle_sessions(&mut self) {
        self.show_sessions = !self.show_sessions;
        if self.show_sessions {
            self.input_focused = false;
            self.session_search.clear();
            self.status = "sessions · ↑/↓ · Enter open · / search · d delete · Ctrl+B hide".into();
        } else {
            self.input_focused = true;
            self.session_search.clear();
            self.status = "ready".into();
        }
    }

    /// Filtered sessions based on search text.
    pub fn filtered_sessions(&self) -> Vec<(usize, &SessionSummary)> {
        if self.session_search.is_empty() {
            self.sessions.iter().enumerate().collect()
        } else {
            let q = self.session_search.to_ascii_lowercase();
            self.sessions
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    let id = s.id.to_string().to_ascii_lowercase();
                    let model = s.model.to_ascii_lowercase();
                    let status = format!("{}", s.status).to_ascii_lowercase();
                    id.contains(&q) || model.contains(&q) || status.contains(&q)
                })
                .collect()
        }
    }

    /// Insert a character into the session search filter.
    pub fn session_search_insert(&mut self, c: char) {
        self.session_search.push(c);
        self.session_list.select(Some(0));
    }

    /// Delete last character from session search filter.
    pub fn session_search_backspace(&mut self) {
        self.session_search.pop();
        self.session_list.select(Some(0));
    }

    /// Archive (soft-delete) the currently selected session.
    pub async fn archive_selected(&mut self, host: &TuiHost) -> Result<()> {
        let filtered = self.filtered_sessions();
        if let Some(i) = self.session_list.selected() {
            if let Some((_, s)) = filtered.get(i) {
                let id = s.id;
                host.archive_session(id).await?;
                self.reload_sessions(host).await?;
                self.session_search.clear();
                self.session_list.select(Some(0));
                self.status = "session archived".into();
            }
        }
        Ok(())
    }

    /// Select previous session in list.
    pub fn select_prev(&mut self) {
        let count = self.filtered_sessions().len();
        if count == 0 {
            return;
        }
        let i = self.session_list.selected().unwrap_or(0);
        let next = if i == 0 { count - 1 } else { i - 1 };
        self.session_list.select(Some(next));
    }

    /// Select next session in list.
    pub fn select_next(&mut self) {
        let count = self.filtered_sessions().len();
        if count == 0 {
            return;
        }
        let i = self.session_list.selected().unwrap_or(0);
        let next = (i + 1) % count;
        self.session_list.select(Some(next));
    }

    /// Load currently selected session.
    pub async fn load_selected(&mut self, host: &TuiHost) -> Result<()> {
        let filtered = self.filtered_sessions();
        if let Some(i) = self.session_list.selected() {
            if let Some((_, s)) = filtered.get(i) {
                let id = s.id;
                let short_id = {
                    let id_str = id.to_string();
                    if id_str.len() > 8 {
                        id_str[..8].to_string()
                    } else {
                        id_str
                    }
                };
                let loaded = host.load_session(id).await?;
                self.set_session(loaded);
                self.show_sessions = false;
                self.input_focused = true;
                self.status = format!("loaded {short_id}");
            }
        }
        Ok(())
    }

    /// Append a transcript line.
    pub fn push_line(&mut self, line: MessageLine) {
        self.lines.push(line);
    }

    /// Insert newline into composer at cursor.
    pub fn insert_newline(&mut self) {
        self.save_undo();
        let byte = self.char_to_byte(self.input_cursor);
        self.input.insert(byte, '\n');
        self.input_cursor += 1;
        self.refresh_completion();
    }

    /// Insert a character at cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.save_undo();
        let byte = self.char_to_byte(self.input_cursor);
        self.input.insert(byte, c);
        self.input_cursor += 1;
        self.refresh_completion();
    }

    /// Insert pasted / multi-char text into the composer at cursor.
    ///
    /// Normalizes `\r\n` / `\r` to `\n` and caps extreme pastes so a huge
    /// clipboard dump cannot freeze the TUI.
    pub fn insert_str(&mut self, s: &str) {
        const MAX_PASTE_CHARS: usize = 100_000;
        let mut normalized = s.replace("\r\n", "\n").replace('\r', "\n");
        let char_count = normalized.chars().count();
        if char_count > MAX_PASTE_CHARS {
            normalized = normalized.chars().take(MAX_PASTE_CHARS).collect();
            normalized.push_str("\n…[paste truncated]");
        }
        if normalized.is_empty() {
            return;
        }
        let inserted_chars = normalized.chars().count();
        let byte = self.char_to_byte(self.input_cursor);
        self.input.insert_str(byte, &normalized);
        self.input_cursor += inserted_chars;
        self.refresh_completion();
    }

    /// Backspace — delete char before cursor.
    pub fn backspace(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        self.save_undo();
        let byte = self.char_to_byte(self.input_cursor);
        let prev_char = self.input_cursor - 1;
        let prev_byte = self.char_to_byte(prev_char);
        self.input.drain(prev_byte..byte);
        self.input_cursor = prev_char;
        self.refresh_completion();
    }

    /// Delete char after cursor (Delete key).
    pub fn delete(&mut self) {
        if self.input_cursor >= self.input.chars().count() {
            return;
        }
        self.save_undo();
        let byte = self.char_to_byte(self.input_cursor);
        let ch = self.input[byte..].chars().next().unwrap_or('\0');
        self.input.drain(byte..byte + ch.len_utf8());
        self.refresh_completion();
    }

    /// Move cursor one char left.
    pub fn cursor_left(&mut self) {
        if self.input_cursor > 0 {
            self.input_cursor -= 1;
        }
    }

    /// Move cursor one char right.
    pub fn cursor_right(&mut self) {
        if self.input_cursor < self.input.chars().count() {
            self.input_cursor += 1;
        }
    }

    /// Move cursor to start of input.
    pub fn cursor_home(&mut self) {
        self.input_cursor = 0;
    }

    /// Move cursor to end of input.
    pub fn cursor_end(&mut self) {
        self.input_cursor = self.input.chars().count();
    }

    /// Move cursor one word left (to start of previous word).
    pub fn cursor_word_left(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let chars: Vec<char> = self.input.chars().collect();
        let mut i = self.input_cursor;
        // Skip non-word chars.
        while i > 0 && !chars[i - 1].is_alphanumeric() {
            i -= 1;
        }
        // Skip word chars.
        while i > 0 && chars[i - 1].is_alphanumeric() {
            i -= 1;
        }
        self.input_cursor = i;
    }

    /// Move cursor one word right (to start of next word).
    pub fn cursor_word_right(&mut self) {
        let chars: Vec<char> = self.input.chars().collect();
        let len = chars.len();
        if self.input_cursor >= len {
            return;
        }
        let mut i = self.input_cursor;
        // Skip word chars.
        while i < len && chars[i].is_alphanumeric() {
            i += 1;
        }
        // Skip non-word chars.
        while i < len && !chars[i].is_alphanumeric() {
            i += 1;
        }
        self.input_cursor = i;
    }

    /// Delete from cursor to start of line (Ctrl+U).
    pub fn delete_to_start(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        self.save_undo();
        let byte = self.char_to_byte(self.input_cursor);
        self.input.drain(0..byte);
        self.input_cursor = 0;
        self.refresh_completion();
    }

    /// Delete from cursor to end of line (Ctrl+K).
    pub fn delete_to_end(&mut self) {
        let byte = self.char_to_byte(self.input_cursor);
        if byte >= self.input.len() {
            return;
        }
        self.save_undo();
        self.input.drain(byte..);
        self.refresh_completion();
    }

    /// Delete word backward (Ctrl+W).
    pub fn delete_word_backward(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        self.save_undo();
        let old = self.input_cursor;
        self.cursor_word_left();
        let byte = self.char_to_byte(self.input_cursor);
        let old_byte = self.char_to_byte(old);
        self.input.drain(byte..old_byte);
        self.refresh_completion();
    }

    /// Convert a char index to a byte offset in the input string.
    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.input
            .char_indices()
            .nth(char_idx)
            .map(|(byte, _)| byte)
            .unwrap_or(self.input.len())
    }

    /// Undo the last composer edit (Ctrl+Z).
    pub fn undo(&mut self) {
        if let Some((prev_input, prev_cursor)) = self.input_undo.pop() {
            self.input = prev_input;
            self.input_cursor = prev_cursor;
        }
    }

    /// Save current composer state to the undo stack (before a mutation).
    fn save_undo(&mut self) {
        self.input_undo
            .push((self.input.clone(), self.input_cursor));
        // Cap at 100 entries to bound memory.
        if self.input_undo.len() > 100 {
            self.input_undo.drain(0..self.input_undo.len() - 100);
        }
    }

    /// Take and clear the input buffer.
    pub fn take_input(&mut self) -> String {
        let s = std::mem::take(&mut self.input);
        self.input_cursor = 0;
        self.completion = None;
        self.history_index = None;
        self.history_draft = String::new();
        // Save non-empty prompts to history (skip duplicates).
        if !s.trim().is_empty() && self.history.last().map(|h| h.as_str()) != Some(s.trim()) {
            self.history.push(s.trim().to_string());
        }
        s
    }

    /// Move history cursor up (to older prompts).
    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                // Entering history mode: save current draft, go to most recent.
                self.history_draft = self.input.clone();
                let idx = self.history.len() - 1;
                self.input = self.history[idx].clone();
                self.input_cursor = self.input.chars().count();
                self.history_index = Some(idx);
            }
            Some(0) => {} // Already at oldest.
            Some(idx) => {
                let new = idx - 1;
                self.input = self.history[new].clone();
                self.input_cursor = self.input.chars().count();
                self.history_index = Some(new);
            }
        }
        self.completion = None;
    }

    /// Move history cursor down (to newer prompts).
    pub fn history_down(&mut self) {
        match self.history_index {
            None => {}
            Some(idx) => {
                if idx + 1 >= self.history.len() {
                    // Exiting history: restore draft.
                    self.input = self.history_draft.clone();
                    self.input_cursor = self.input.chars().count();
                    self.history_index = None;
                } else {
                    let new = idx + 1;
                    self.input = self.history[new].clone();
                    self.input_cursor = self.input.chars().count();
                    self.history_index = Some(new);
                }
            }
        }
        self.completion = None;
    }

    /// Apply a finished run.
    pub fn apply_run_update(&mut self, update: RunUpdate) {
        self.streaming = None;
        self.activity = None;
        self.session = update.session;
        if !update.assistant.is_empty() {
            self.push_line(MessageLine::assistant(update.assistant));
        }
        for log in &update.logs {
            // Keep a short activity trail as system chips, not walls of text.
            if log.starts_with('[') || log.starts_with('→') || log.starts_with('─') {
                self.push_line(MessageLine::tool(log.clone()));
            }
        }
        self.logs.extend(update.logs);
        self.trim_logs();
        let summary = format!(
            "{} · {} turns · tools {}/{} · {}ms",
            if update.ok { "done" } else { "failed" },
            update.turns,
            update.tools_ok,
            update.tools_ok + update.tools_err,
            update.duration_ms
        );
        if let Some(err) = update.error {
            self.push_line(MessageLine::system(format!("error: {err}")));
            self.status = format!("error · {summary}");
        } else {
            self.status = summary;
        }
        if !update.ok {
            self.status = format!("! {}", self.status);
        }
        self.last_prompt_tokens = update.prompt_tokens;
        self.last_completion_tokens = update.completion_tokens;
        self.scroll = 0;
    }

    /// Apply a live UI event from a background run.
    pub fn apply_ui_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::StreamDelta(text) => {
                let buf = self.streaming.get_or_insert_with(String::new);
                buf.push_str(&text);
                if buf.len() > 24_000 {
                    let keep = buf.len() - 16_000;
                    let boundary = buf.floor_char_boundary(keep);
                    *buf = format!("…{}", &buf[boundary..]);
                }
                self.status = "streaming…".into();
                if self.auto_follow {
                    self.scroll = 0;
                }
            }
            UiEvent::ToolLog(line) => {
                // Track tool start time for elapsed display.
                if line.starts_with("→ ") {
                    self.tool_start = Some(Instant::now());
                }
                self.activity = Some(line.clone());
                self.logs.push(line);
                self.trim_logs();
            }
            UiEvent::Status(s) => {
                self.status = s;
            }
            UiEvent::Done(update) => {
                self.apply_run_update(*update);
            }
        }
    }

    /// Scroll transcript up (older).
    pub fn scroll_up(&mut self, n: u16) {
        self.scroll = self.scroll.saturating_add(n);
        self.auto_follow = false;
    }

    /// Scroll transcript down (newer).
    pub fn scroll_down(&mut self, n: u16) {
        self.scroll = self.scroll.saturating_sub(n);
    }

    /// Trim log buffer to max entries.
    fn trim_logs(&mut self) {
        const MAX_LOGS: usize = 100;
        if self.logs.len() > MAX_LOGS {
            let drain = self.logs.len() - MAX_LOGS;
            self.logs.drain(0..drain);
        }
    }
}
