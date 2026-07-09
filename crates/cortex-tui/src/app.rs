//! TUI application state.

use crate::host::TuiHost;
use anyhow::Result;
use cortex_memory::SessionSummary;
use cortex_models::{Message, Role, Session};
use ratatui::widgets::ListState;

/// A display line in the transcript.
#[derive(Debug, Clone)]
pub struct MessageLine {
    /// Role label.
    pub role: String,
    /// Body text.
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
            content.push_str(&format!("\n[tools: {}]", names.join(", ")));
        }
        if content.len() > 4000 {
            content.truncate(4000);
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
    /// Scroll offset from bottom (0 = bottom).
    pub scroll: u16,
    /// Tool / event logs.
    pub logs: Vec<String>,
    /// Input buffer.
    pub input: String,
    /// Whether the input box is focused.
    pub input_focused: bool,
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
        let mut app = Self {
            workspace: host.workspace.display().to_string(),
            model_label: format!("{} ({}/{})", host.model_alias, host.provider_id, host.model),
            database: host.database.display().to_string(),
            sessions,
            session_list,
            session,
            lines: vec![MessageLine::system(
                "Cortex TUI — type a message and press Enter. Tab focus · n new · r reload · y yolo · q quit · Ctrl-C cancel run",
            )],
            scroll: 0,
            logs: Vec::new(),
            input: String::new(),
            input_focused: true,
            running: false,
            yolo: host.yolo,
            max_turns: host.max_turns,
            skills: host.skills.clone(),
            status: "ready".into(),
        };
        // If sessions exist, load the first one into the transcript.
        if let Some(s) = app.sessions.first().cloned() {
            if let Ok(loaded) = host.load_session(s.id).await {
                app.set_session(loaded);
            }
        }
        Ok(app)
    }

    /// Start a fresh session.
    pub fn new_session(&mut self) {
        let model = self.session.model.clone();
        let ws = self.session.workspace.clone();
        self.session = Session::new(ws, model);
        self.lines = vec![MessageLine::system("New session started.")];
        self.logs.clear();
        self.session_list.select(None);
        self.status = "new session".into();
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
    }

    /// Reload session list from store.
    pub async fn reload_sessions(&mut self, host: &TuiHost) -> Result<()> {
        self.sessions = host.list_sessions(30).await?;
        Ok(())
    }

    /// Cycle focus: input ↔ session list.
    pub fn cycle_focus(&mut self) {
        self.input_focused = !self.input_focused;
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

    /// Load currently selected session (call after Up/Down + Enter on list).
    pub async fn load_selected(&mut self, host: &TuiHost) -> Result<()> {
        if let Some(i) = self.session_list.selected() {
            if let Some(s) = self.sessions.get(i).cloned() {
                let loaded = host.load_session(s.id).await?;
                self.set_session(loaded);
                self.status = format!("loaded {}", s.id);
            }
        }
        Ok(())
    }

    /// Append a transcript line.
    pub fn push_line(&mut self, line: MessageLine) {
        self.lines.push(line);
    }

    /// Apply a finished run.
    pub fn apply_run_update(&mut self, update: RunUpdate) {
        self.session = update.session;
        if !update.assistant.is_empty() {
            self.push_line(MessageLine::assistant(update.assistant));
        }
        self.logs.extend(update.logs);
        if self.logs.len() > 200 {
            let drain = self.logs.len() - 200;
            self.logs.drain(0..drain);
        }
        if let Some(err) = update.error {
            self.push_line(MessageLine::system(format!("error: {err}")));
            self.status = format!("error · {}", update.status);
        } else {
            self.status = update.status;
        }
        if !update.ok {
            self.status = format!("! {}", self.status);
        }
    }
}
