//! Terminal UI for Cortex — Claude Code–style interactive chat.
//!
//! Launch with [`run`]. Requires a prepared [`TuiHost`] (provider, tools, store).

#![deny(missing_docs)]

mod app;
mod approver;
mod draw;
mod host;

pub use host::TuiHost;

use anyhow::{Context, Result};
use app::{App, MessageLine, UiEvent};
use approver::TuiApprovalRequest;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, stdout};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Run the Cortex chat TUI until the user quits.
pub async fn run(host: TuiHost) -> Result<()> {
    info!("starting cortex chat TUI");
    enable_raw_mode().context("enable raw mode")?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend).context("create terminal")?;
    terminal.clear()?;

    let result = run_loop(&mut terminal, host).await;

    disable_raw_mode().ok();
    stdout().execute(LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    host: TuiHost,
) -> Result<()> {
    let mut app = App::new(&host).await?;
    let mut events = EventStream::new();
    let (tx, mut rx) = mpsc::unbounded_channel::<UiEvent>();
    let (approval_tx, mut approval_rx) = mpsc::unbounded_channel::<TuiApprovalRequest>();
    let mut run_cancel: Option<CancellationToken> = None;

    loop {
        terminal.draw(|f| draw::ui(f, &app))?;

        tokio::select! {
            maybe_ev = events.next() => {
                match maybe_ev {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                        if handle_key(
                            &mut app,
                            &host,
                            key.code,
                            key.modifiers,
                            &mut run_cancel,
                            &tx,
                            &approval_tx,
                        )
                        .await?
                        {
                            break;
                        }
                    }
                    Some(Ok(Event::Resize(_, _))) => {}
                    Some(Ok(Event::Mouse(_))) => {}
                    Some(Err(e)) => {
                        app.status = format!("event error: {e}");
                    }
                    None => break,
                    _ => {}
                }
            }
            Some(event) = rx.recv() => {
                let is_done = matches!(event, UiEvent::Done(_));
                app.apply_ui_event(event);
                if is_done {
                    run_cancel = None;
                    app.running = false;
                    if let Err(e) = app.reload_sessions(&host).await {
                        app.status = format!("{}, reload warn: {e}", app.status);
                    }
                }
            }
            Some(req) = approval_rx.recv() => {
                app.approval = Some(app::ApprovalModal {
                    request: req.request,
                    respond: req.respond,
                });
            }
        }
    }

    Ok(())
}

/// Returns true if the UI should exit.
async fn handle_key(
    app: &mut App,
    host: &TuiHost,
    code: KeyCode,
    mods: KeyModifiers,
    run_cancel: &mut Option<CancellationToken>,
    tx: &mpsc::UnboundedSender<UiEvent>,
    approval_tx: &mpsc::UnboundedSender<TuiApprovalRequest>,
) -> Result<bool> {
    // Global cancel / quit
    if code == KeyCode::Char('c') && mods.contains(KeyModifiers::CONTROL) {
        if let Some(c) = run_cancel.take() {
            c.cancel();
            // Keep partial reply if present.
            if let Some(draft) = app.streaming.take() {
                if !draft.trim().is_empty() {
                    app.push_line(MessageLine::assistant(format!("{draft} (cancelled)")));
                }
            }
            app.status = "cancelled".into();
            app.running = false;
            app.activity = None;
            return Ok(false);
        }
        return Ok(true);
    }

    // Tool-approval modal: intercept all keys when open.
    if app.approval.is_some() {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(modal) = app.approval.take() {
                    let _ = modal.respond.send(cortex_tools::ApprovalDecision::Allow);
                    app.status = "approved".into();
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                if let Some(modal) = app.approval.take() {
                    let _ = modal.respond.send(cortex_tools::ApprovalDecision::Deny);
                    app.status = "denied".into();
                }
            }
            _ => {}
        }
        return Ok(false);
    }

    // Sessions drawer
    if code == KeyCode::Char('b') && mods.contains(KeyModifiers::CONTROL) {
        app.toggle_sessions();
        return Ok(false);
    }

    if app.show_sessions {
        match code {
            KeyCode::Esc => {
                if app.session_search.is_empty() {
                    app.show_sessions = false;
                    app.input_focused = true;
                    app.status = "ready".into();
                } else {
                    app.session_search.clear();
                    app.session_list.select(Some(0));
                    app.status =
                        "sessions · ↑/↓ · Enter open · / search · d delete · Ctrl+B hide".into();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
            KeyCode::Down | KeyCode::Char('j') => app.select_next(),
            KeyCode::Enter => {
                if let Err(e) = app.load_selected(host).await {
                    app.status = format!("load failed: {e}");
                }
            }
            KeyCode::Char('n') => {
                app.new_session();
                app.show_sessions = false;
                app.input_focused = true;
            }
            KeyCode::Char('r') => {
                if let Err(e) = app.reload_sessions(host).await {
                    app.status = format!("reload failed: {e}");
                } else {
                    app.status = "sessions reloaded".into();
                }
            }
            KeyCode::Char('d') => {
                if let Err(e) = app.archive_selected(host).await {
                    app.status = format!("archive failed: {e}");
                }
            }
            KeyCode::Char(c)
                if c != '/' && c != 'j' && c != 'k' && c != 'n' && c != 'r' && c != 'd' =>
            {
                app.session_search_insert(c);
            }
            KeyCode::Backspace => {
                app.session_search_backspace();
            }
            _ => {}
        }
        return Ok(false);
    }

    // Scroll
    match code {
        KeyCode::PageUp => {
            app.scroll_up(8);
            return Ok(false);
        }
        KeyCode::PageDown => {
            app.scroll_down(8);
            return Ok(false);
        }
        KeyCode::Char('l') if mods.contains(KeyModifiers::CONTROL) => {
            app.scroll = 0;
            return Ok(false);
        }
        _ => {}
    }

    // Newline: Ctrl+J
    if code == KeyCode::Char('j') && mods.contains(KeyModifiers::CONTROL) {
        if app.input_focused && !app.running {
            app.insert_newline();
        }
        return Ok(false);
    }

    // Toggle yolo
    if code == KeyCode::Char('y') && mods.contains(KeyModifiers::CONTROL) {
        app.yolo = !app.yolo;
        app.status = format!("yolo={}", app.yolo);
        return Ok(false);
    }

    match code {
        KeyCode::Esc => {
            if app.running {
                if let Some(c) = run_cancel.take() {
                    c.cancel();
                }
                // Keep partial reply if present.
                if let Some(draft) = app.streaming.take() {
                    if !draft.trim().is_empty() {
                        app.push_line(MessageLine::assistant(format!("{draft} (cancelled)")));
                    }
                }
                app.running = false;
                app.status = "cancelled".into();
                app.activity = None;
            } else if !app.input.is_empty() {
                app.input.clear();
                app.input_cursor = 0;
            }
        }
        KeyCode::Enter if app.input_focused && !app.running => {
            let prompt = app.take_input();
            let prompt = prompt.trim_end().to_string();
            if prompt.is_empty() {
                return Ok(false);
            }
            // Slash commands
            if prompt == "/quit" || prompt == "/exit" || prompt == "/q" {
                return Ok(true);
            }
            if prompt == "/new" || prompt == "/clear" {
                app.new_session();
                return Ok(false);
            }
            if prompt == "/sessions" {
                app.toggle_sessions();
                return Ok(false);
            }
            if prompt == "/yolo" {
                app.yolo = !app.yolo;
                app.status = format!("yolo={}", app.yolo);
                return Ok(false);
            }
            if prompt == "/help" {
                app.push_line(MessageLine::system(
                    "Commands: /new  /sessions  /yolo  /quit\nKeys: Enter send · Ctrl+J newline · Ctrl+B sessions · Ctrl+C cancel · PgUp/PgDn scroll",
                ));
                return Ok(false);
            }

            app.push_line(MessageLine::user(prompt.clone()));
            let cancel = CancellationToken::new();
            *run_cancel = Some(cancel.clone());
            app.running = true;
            app.status = "running…".into();
            app.streaming = None;
            app.activity = None;
            app.scroll = 0;

            let host = host.clone_for_run();
            let session = app.session.clone();
            let yolo = app.yolo;
            let max_turns = app.max_turns;
            let skills = app.skills.clone();
            let tx = tx.clone();
            let approval_tx = approval_tx.clone();
            tokio::spawn(async move {
                host.run_turn(
                    session,
                    prompt,
                    yolo,
                    max_turns,
                    skills,
                    cancel,
                    tx,
                    approval_tx,
                )
                .await;
            });
        }
        KeyCode::Char(c)
            if app.input_focused && !app.running && !mods.contains(KeyModifiers::CONTROL) =>
        {
            app.insert_char(c);
        }
        KeyCode::Backspace if app.input_focused && !app.running => {
            app.backspace();
        }
        _ => {}
    }

    Ok(false)
}
