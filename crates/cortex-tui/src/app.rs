//! TUI application state — Claude Code–style chat surface.

use crate::host::TuiHost;
use anyhow::Result;
use cortex_memory::SessionSummary;
use cortex_models::{Message, Role, Session};
use ratatui::widgets::ListState;

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
    /// Scroll offset from bottom (0 = stick to bottom).
    pub scroll: u16,
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
        let welcome = format!(
            "Cortex · {} · {}\n\nType a message and press Enter to send.\nCtrl+J newline · Ctrl+B sessions · Ctrl+C cancel · /quit to exit",
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
            scroll: 0,
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
        })
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
    }

    /// Insert a character at the end of input.
    pub fn insert_char(&mut self, c: char) {
        self.input.push(c);
        self.input_cursor = self.input.len();
    }

    /// Backspace.
    pub fn backspace(&mut self) {
        self.input.pop();
        self.input_cursor = self.input.len();
    }

    /// Take and clear the input buffer.
    pub fn take_input(&mut self) -> String {
        let s = std::mem::take(&mut self.input);
        self.input_cursor = 0;
        s
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

    /// Scroll transcript up (older).
    pub fn scroll_up(&mut self, n: u16) {
        self.scroll = self.scroll.saturating_add(n);
    }

    /// Scroll transcript down (newer).
    pub fn scroll_down(&mut self, n: u16) {
        self.scroll = self.scroll.saturating_sub(n);
    }
}
