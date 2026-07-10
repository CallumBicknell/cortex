//! TUI application state — Claude Code–style chat surface.

use crate::complete::{self, CompletionState};
use crate::host::TuiHost;
use anyhow::Result;
use cortex_memory::SessionSummary;
use cortex_models::{Message, Role, Session};
use ratatui::widgets::ListState;
use std::collections::VecDeque;
use std::path::PathBuf;

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
            content.truncate(12_000);
            content.push('…');
        }
        Self {
            role: role.into(),
            content,
        }
    }
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
    /// Session list.
    pub sessions: Vec<SessionSummary>,
    /// List widget state.
    pub session_list: ListState,
    /// Active session.
    pub session: Session,
    /// Transcript lines.
    pub lines: Vec<MessageLine>,
    /// When true, the conversation view pins to the latest content (auto-scroll).
    ///
    /// Set false when the user scrolls up so selection / reading is not yanked
    /// around as new stream tokens or tool lines append at the bottom.
    pub follow: bool,
    /// Absolute Paragraph Y offset (lines from the top) when `follow` is false.
    pub scroll_top: u16,
    /// Last computed max top-offset (`total_rows - viewport`), updated on draw.
    pub last_max_scroll: u16,
    /// Recent tool activity (footer strip / optional).
    pub logs: Vec<String>,
    /// Input buffer (may contain newlines).
    pub input: String,
    /// Cursor position in input (byte index, simplified: end of string for now).
    pub input_cursor: usize,
    /// Whether the input box is focused.
    pub input_focused: bool,
    /// Show sessions drawer.
    pub show_sessions: bool,
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
    /// Workspace root (for `@path` completion / attachment expansion).
    pub workspace_path: PathBuf,
    /// Known skill ids for `/skill` autocomplete.
    pub skill_ids: Vec<String>,
    /// Skill id → short description.
    pub skill_details: Vec<(String, String)>,
    /// Active composer completion popup.
    pub completion: Option<CompletionState>,
    /// Previously sent user prompts (oldest → newest) for ↑/↓ history.
    pub input_history: Vec<String>,
    /// Index into `input_history` while browsing, or `None` for a fresh draft.
    pub history_index: Option<usize>,
    /// Draft saved when the user first presses ↑ from a live composer buffer.
    pub history_draft: String,
    /// Messages queued with Enter while a run is still in progress.
    pub pending: VecDeque<String>,
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
            sessions,
            session_list,
            session,
            lines: vec![MessageLine::system(welcome)],
            follow: true,
            scroll_top: 0,
            last_max_scroll: 0,
            logs: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            input_focused: true,
            show_sessions: false,
            running: false,
            yolo: host.yolo,
            max_turns: host.max_turns,
            skills: host.skills.clone(),
            status: "ready".into(),
            streaming: None,
            activity: None,
            workspace_path: host.workspace.clone(),
            skill_ids,
            skill_details,
            completion: None,
            input_history: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            pending: VecDeque::new(),
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
        self.input_cursor = self.input.len();
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
        self.jump_to_bottom();
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
        self.jump_to_bottom();
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
            self.status = "sessions · ↑/↓ · Enter open · Ctrl+B hide".into();
        } else {
            self.input_focused = true;
            self.status = "ready".into();
        }
    }

    /// Select previous session in list.
    pub fn select_prev(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let i = self.session_list.selected().unwrap_or(0);
        let next = if i == 0 {
            self.sessions.len() - 1
        } else {
            i - 1
        };
        self.session_list.select(Some(next));
    }

    /// Select next session in list.
    pub fn select_next(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let i = self.session_list.selected().unwrap_or(0);
        let next = (i + 1) % self.sessions.len();
        self.session_list.select(Some(next));
    }

    /// Load currently selected session.
    pub async fn load_selected(&mut self, host: &TuiHost) -> Result<()> {
        if let Some(i) = self.session_list.selected() {
            if let Some(s) = self.sessions.get(i).cloned() {
                let loaded = host.load_session(s.id).await?;
                self.set_session(loaded);
                self.show_sessions = false;
                self.input_focused = true;
                self.status = format!(
                    "loaded {}",
                    &s.id.to_string()[..8.min(s.id.to_string().len())]
                );
            }
        }
        Ok(())
    }

    /// Append a transcript line.
    pub fn push_line(&mut self, line: MessageLine) {
        self.lines.push(line);
    }

    /// Insert newline into composer.
    pub fn insert_newline(&mut self) {
        self.input.push('\n');
        self.input_cursor = self.input.len();
        self.refresh_completion();
    }

    /// Insert a character at the end of input.
    pub fn insert_char(&mut self, c: char) {
        self.input.push(c);
        self.input_cursor = self.input.len();
        self.refresh_completion();
    }

    /// Insert pasted / multi-char text into the composer.
    ///
    /// Normalizes `\r\n` / `\r` to `\n` and caps extreme pastes so a huge
    /// clipboard dump cannot freeze the TUI.
    pub fn insert_str(&mut self, s: &str) {
        const MAX_PASTE_CHARS: usize = 100_000;
        let mut normalized = s.replace("\r\n", "\n").replace('\r', "\n");
        if normalized.chars().count() > MAX_PASTE_CHARS {
            normalized = normalized.chars().take(MAX_PASTE_CHARS).collect();
            normalized.push_str("\n…[paste truncated]");
        }
        if normalized.is_empty() {
            return;
        }
        self.input.push_str(&normalized);
        self.input_cursor = self.input.len();
        self.refresh_completion();
    }

    /// Backspace.
    pub fn backspace(&mut self) {
        self.input.pop();
        self.input_cursor = self.input.len();
        self.refresh_completion();
    }

    /// Take and clear the input buffer.
    pub fn take_input(&mut self) -> String {
        let s = std::mem::take(&mut self.input);
        self.input_cursor = 0;
        self.completion = None;
        self.history_index = None;
        self.history_draft.clear();
        s
    }

    /// Record a sent user prompt for ↑/↓ history (newest at end).
    pub fn push_input_history(&mut self, prompt: &str) {
        let t = prompt.trim_end();
        if t.is_empty() {
            return;
        }
        // Avoid consecutive duplicates.
        if self.input_history.last().map(|s| s.as_str()) != Some(t) {
            self.input_history.push(t.to_string());
            if self.input_history.len() > 200 {
                let drain = self.input_history.len() - 200;
                self.input_history.drain(0..drain);
            }
        }
        self.history_index = None;
        self.history_draft.clear();
    }

    /// ↑ — older sent message (shell-style).
    pub fn history_prev(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                self.history_draft = self.input.clone();
                let i = self.input_history.len() - 1;
                self.history_index = Some(i);
                self.input = self.input_history[i].clone();
            }
            Some(0) => {
                // Already at oldest.
            }
            Some(i) => {
                let i = i - 1;
                self.history_index = Some(i);
                self.input = self.input_history[i].clone();
            }
        }
        self.input_cursor = self.input.len();
        self.completion = None;
    }

    /// ↓ — newer sent message, or restore the draft after the newest.
    pub fn history_next(&mut self) {
        let Some(i) = self.history_index else {
            return;
        };
        if i + 1 < self.input_history.len() {
            let i = i + 1;
            self.history_index = Some(i);
            self.input = self.input_history[i].clone();
        } else {
            self.history_index = None;
            self.input = std::mem::take(&mut self.history_draft);
        }
        self.input_cursor = self.input.len();
        self.completion = None;
    }

    /// Queue a prompt to run after the current agent turn finishes.
    pub fn enqueue_pending(&mut self, prompt: String) {
        self.pending.push_back(prompt);
    }

    /// Pop the next queued prompt, if any.
    pub fn pop_pending(&mut self) -> Option<String> {
        self.pending.pop_front()
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
        if self.logs.len() > 100 {
            let drain = self.logs.len() - 100;
            self.logs.drain(0..drain);
        }
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
        // Only snap to bottom if the user was already following — never steal
        // the viewport while they're selecting / reading history.
        if self.follow {
            self.jump_to_bottom();
        }
    }

    /// Apply a live UI event from a background run.
    pub fn apply_ui_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::StreamDelta(text) => {
                let buf = self.streaming.get_or_insert_with(String::new);
                buf.push_str(&text);
                if buf.len() > 24_000 {
                    let keep = buf.len() - 16_000;
                    *buf = format!("…{}", &buf[keep..]);
                }
                self.status = "streaming…".into();
            }
            UiEvent::ToolLog(line) => {
                self.activity = Some(line.clone());
                self.logs.push(line);
                if self.logs.len() > 100 {
                    let drain = self.logs.len() - 100;
                    self.logs.drain(0..drain);
                }
            }
            UiEvent::Status(s) => {
                self.status = s;
            }
            UiEvent::Done(update) => {
                self.apply_run_update(*update);
            }
        }
    }

    /// Pin the conversation view to the latest content and resume auto-scroll.
    pub fn jump_to_bottom(&mut self) {
        self.follow = true;
        self.scroll_top = 0;
    }

    /// Scroll transcript up (older content). Leaves follow mode so the view
    /// stays put while new messages append (selection-friendly).
    pub fn scroll_up(&mut self, n: u16) {
        if self.follow {
            self.follow = false;
            self.scroll_top = self.last_max_scroll.saturating_sub(n);
        } else {
            self.scroll_top = self.scroll_top.saturating_sub(n);
        }
    }

    /// Scroll transcript down (newer). Re-enables follow when the bottom is reached.
    pub fn scroll_down(&mut self, n: u16) {
        if self.follow {
            return;
        }
        let next = self.scroll_top.saturating_add(n);
        if next >= self.last_max_scroll {
            self.jump_to_bottom();
        } else {
            self.scroll_top = next;
        }
    }

    /// How far above the bottom the view is (for the footer hint).
    pub fn scroll_above_bottom(&self) -> u16 {
        if self.follow {
            0
        } else {
            self.last_max_scroll.saturating_sub(self.scroll_top)
        }
    }
}

#[cfg(test)]
mod history_tests {
    use super::*;

    fn bare_app() -> App {
        App {
            workspace: String::new(),
            model_label: String::new(),
            database: String::new(),
            sessions: Vec::new(),
            session_list: ListState::default(),
            session: Session::new(".", "m"),
            lines: Vec::new(),
            follow: true,
            scroll_top: 0,
            last_max_scroll: 0,
            logs: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            input_focused: true,
            show_sessions: false,
            running: false,
            yolo: true,
            max_turns: 8,
            skills: Vec::new(),
            status: String::new(),
            streaming: None,
            activity: None,
            workspace_path: PathBuf::from("."),
            skill_ids: Vec::new(),
            skill_details: Vec::new(),
            completion: None,
            input_history: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            pending: VecDeque::new(),
        }
    }

    #[test]
    fn history_up_down_roundtrip() {
        let mut app = bare_app();
        app.push_input_history("first");
        app.push_input_history("second");
        app.input = "draft".into();
        app.history_prev();
        assert_eq!(app.input, "second");
        app.history_prev();
        assert_eq!(app.input, "first");
        app.history_next();
        assert_eq!(app.input, "second");
        app.history_next();
        assert_eq!(app.input, "draft");
        assert!(app.history_index.is_none());
    }

    #[test]
    fn queue_fifo() {
        let mut app = bare_app();
        app.enqueue_pending("a".into());
        app.enqueue_pending("b".into());
        assert_eq!(app.pop_pending().as_deref(), Some("a"));
        assert_eq!(app.pop_pending().as_deref(), Some("b"));
        assert!(app.pop_pending().is_none());
    }

    #[test]
    fn scroll_up_freezes_follow_with_stable_top() {
        let mut app = bare_app();
        app.follow = true;
        app.last_max_scroll = 80;
        app.scroll_up(10);
        assert!(!app.follow);
        assert_eq!(app.scroll_top, 70);
        // Further growth of last_max_scroll must not move scroll_top.
        app.last_max_scroll = 120;
        assert_eq!(app.scroll_top, 70);
        app.scroll_down(5);
        assert_eq!(app.scroll_top, 75);
        app.scroll_down(1000);
        assert!(app.follow);
    }
}
