//! Terminal UI for Cortex — Claude Code–style interactive chat.
//!
//! Launch with [`run`]. Requires a prepared [`TuiHost`] (provider, tools, store).

#![deny(missing_docs)]

mod app;
mod approver;
mod complete;
mod draw;
mod host;
mod mentions;

pub use host::TuiHost;

use anyhow::{Context, Result};
use app::{App, MessageLine, UiEvent};
use approver::TuiApprovalRequest;
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event,
    EventStream, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use futures::StreamExt;
use mentions::{expand_attachments, parse_prompt, MetaCommand};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, stdout};
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Run the Cortex chat TUI until the user quits.
pub async fn run(host: TuiHost) -> Result<()> {
    info!("starting cortex chat TUI");
    enable_raw_mode().context("enable raw mode")?;
    stdout().execute(EnterAlternateScreen)?;
    // Bracketed paste: terminals send Event::Paste instead of raw key spam
    // (which would fire Enter mid-paste and break multi-line clipboard dumps).
    stdout().execute(EnableBracketedPaste)?;
    stdout().execute(EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend).context("create terminal")?;
    terminal.clear()?;

    let result = run_loop(&mut terminal, host).await;

    stdout().execute(DisableBracketedPaste).ok();
    stdout().execute(DisableMouseCapture).ok();
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
        // Blink cursor every 500ms when focused and not running.
        if app.input_focused && !app.running && app.last_blink.elapsed().as_millis() >= 500 {
            app.cursor_visible = !app.cursor_visible;
            app.last_blink = Instant::now();
        }
        // Also blink during streaming.
        if app.running && app.last_blink.elapsed().as_millis() >= 500 {
            app.cursor_visible = !app.cursor_visible;
            app.last_blink = Instant::now();
        }

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
                    Some(Ok(Event::Paste(text))) => {
                        if app.input_focused && !app.running && !app.show_sessions {
                            app.insert_str(&text);
                        }
                    }
                    Some(Ok(Event::Resize(_, _))) => {}
                    Some(Ok(Event::Mouse(mouse))) => {
                        match mouse.kind {
                            MouseEventKind::ScrollUp => {
                                app.scroll_up(3);
                            }
                            MouseEventKind::ScrollDown => {
                                app.scroll_down(3);
                            }
                            _ => {}
                        }
                    }
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
            app.auto_follow = true;
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

    // Completion navigation (when popup is open)
    if app.completion.is_some() && app.input_focused && !app.running {
        match code {
            KeyCode::Up => {
                if let Some(c) = app.completion.as_mut() {
                    c.select_prev();
                }
                return Ok(false);
            }
            KeyCode::Down => {
                if let Some(c) = app.completion.as_mut() {
                    c.select_next();
                }
                return Ok(false);
            }
            KeyCode::Tab => {
                app.accept_completion();
                return Ok(false);
            }
            KeyCode::Enter => {
                // Accept completion instead of sending.
                app.accept_completion();
                return Ok(false);
            }
            KeyCode::Esc => {
                app.clear_completion();
                return Ok(false);
            }
            _ => {}
        }
    }

    // History and cursor navigation (when no completion popup, no sessions drawer, not running)
    if app.input_focused && !app.running && app.completion.is_none() && !app.show_sessions {
        match code {
            KeyCode::Up => {
                app.history_up();
                return Ok(false);
            }
            KeyCode::Down => {
                app.history_down();
                return Ok(false);
            }
            KeyCode::Left => {
                app.cursor_left();
                return Ok(false);
            }
            KeyCode::Right => {
                app.cursor_right();
                return Ok(false);
            }
            KeyCode::Home => {
                app.cursor_home();
                return Ok(false);
            }
            KeyCode::End => {
                app.cursor_end();
                return Ok(false);
            }
            KeyCode::Delete => {
                app.delete();
                return Ok(false);
            }
            _ => {}
        }
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
            } else if app.completion.is_some() {
                app.clear_completion();
            } else if !app.input.is_empty() {
                app.input.clear();
                app.input_cursor = 0;
                app.clear_completion();
            }
        }
        KeyCode::Tab if app.input_focused && !app.running => {
            if app.completion.is_none() {
                app.refresh_completion();
            }
            if app.completion.is_some() {
                app.accept_completion();
            }
        }
        // Undo composer edit (Ctrl+Z).
        KeyCode::Char('z') if app.input_focused && mods.contains(KeyModifiers::CONTROL) => {
            app.undo();
            return Ok(false);
        }
        // Toggle compact mode (Ctrl+U).
        KeyCode::Char('u') if app.input_focused && mods.contains(KeyModifiers::CONTROL) => {
            app.compact = !app.compact;
            app.status = if app.compact {
                "compact mode on".into()
            } else {
                "compact mode off".into()
            };
            return Ok(false);
        }
        KeyCode::Enter if app.input_focused && !app.running => {
            let prompt = app.take_input();
            let prompt = prompt.trim_end().to_string();
            if prompt.is_empty() {
                return Ok(false);
            }

            let parsed = parse_prompt(&prompt, &app.skill_ids);
            if let Some(meta) = parsed.meta {
                return handle_meta(app, meta);
            }
            if prompt == "/export" {
                match export_transcript(app) {
                    Ok(path) => {
                        app.push_line(MessageLine::system(format!("Exported to {path}")));
                        app.status = format!("exported → {path}");
                    }
                    Err(e) => {
                        app.status = format!("export failed: {e}");
                    }
                }
                return Ok(false);
            }

            // Expand @paths for the agent; keep original text in the transcript.
            let agent_prompt = expand_attachments(
                &app.workspace_path,
                &parsed.attachments,
                &parsed.agent_prompt,
            );

            let mut skills = app.skills.clone();
            for s in &parsed.skills {
                if !skills.iter().any(|x| x == s) {
                    skills.push(s.clone());
                }
            }

            if !parsed.skills.is_empty() || !parsed.attachments.is_empty() {
                let mut bits = Vec::new();
                if !parsed.skills.is_empty() {
                    bits.push(format!("skills: {}", parsed.skills.join(", ")));
                }
                if !parsed.attachments.is_empty() {
                    bits.push(format!("@ {}", parsed.attachments.join(", ")));
                }
                app.status = bits.join(" · ");
            }

            app.push_line(MessageLine::user(parsed.display));
            let cancel = CancellationToken::new();
            *run_cancel = Some(cancel.clone());
            app.running = true;
            if app.status == "ready" || app.status.is_empty() {
                app.status = "running…".into();
            } else {
                app.status = format!("{} · running…", app.status);
            }
            app.streaming = None;
            app.activity = None;
            app.scroll = 0;

            let host = host.clone_for_run();
            let session = app.session.clone();
            let yolo = app.yolo;
            let max_turns = app.max_turns;
            let tx = tx.clone();
            let approval_tx = approval_tx.clone();
            tokio::spawn(async move {
                host.run_turn(
                    session,
                    agent_prompt,
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
        // Readline shortcuts (Ctrl+A/E/W/U/K).
        KeyCode::Char('a')
            if app.input_focused && !app.running && mods.contains(KeyModifiers::CONTROL) =>
        {
            app.cursor_home();
        }
        KeyCode::Char('e')
            if app.input_focused && !app.running && mods.contains(KeyModifiers::CONTROL) =>
        {
            app.cursor_end();
        }
        KeyCode::Char('w')
            if app.input_focused && !app.running && mods.contains(KeyModifiers::CONTROL) =>
        {
            app.delete_word_backward();
        }
        KeyCode::Char('u')
            if app.input_focused && !app.running && mods.contains(KeyModifiers::CONTROL) =>
        {
            app.delete_to_start();
        }
        KeyCode::Char('k')
            if app.input_focused && !app.running && mods.contains(KeyModifiers::CONTROL) =>
        {
            app.delete_to_end();
        }
        // Word movement (Ctrl+Left/Right).
        KeyCode::Left
            if app.input_focused && !app.running && mods.contains(KeyModifiers::CONTROL) =>
        {
            app.cursor_word_left();
        }
        KeyCode::Right
            if app.input_focused && !app.running && mods.contains(KeyModifiers::CONTROL) =>
        {
            app.cursor_word_right();
        }
        _ => {}
    }

    Ok(false)
}

/// Export the current transcript as a markdown file.
fn export_transcript(app: &App) -> Result<String, String> {
    use std::fs;
    use std::path::PathBuf;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("cortex_export_{timestamp}.md");

    // Try workspace dir first, fall back to current dir.
    let ws = std::path::Path::new(&app.workspace);
    let dir = if ws.is_dir() {
        ws.to_path_buf()
    } else {
        PathBuf::from(".")
    };
    let path = dir.join(&filename);

    let mut md = String::new();
    md.push_str("# Cortex Session Export\n\n");
    md.push_str(&format!(
        "**Date:** {}\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
    ));
    md.push_str(&format!("**Model:** {}\n\n", app.model_label));
    md.push_str("---\n\n");

    for line in &app.lines {
        match line.role.as_str() {
            "you" => {
                md.push_str("## You\n\n");
                md.push_str(&line.content);
                md.push_str("\n\n");
            }
            "cortex" => {
                md.push_str("## Cortex\n\n");
                md.push_str(&line.content);
                md.push_str("\n\n");
            }
            "tool" => {
                md.push_str(&format!("> {}\n\n", line.content));
            }
            "system" => {
                md.push_str(&format!("*{}*\n\n", line.content));
            }
            _ => {
                md.push_str(&format!("### {}\n\n{}\n\n", line.role, line.content));
            }
        }
    }

    fs::write(&path, &md).map_err(|e| format!("write failed: {e}"))?;
    Ok(path.display().to_string())
}

fn handle_meta(app: &mut App, meta: MetaCommand) -> Result<bool> {
    match meta {
        MetaCommand::Quit => return Ok(true),
        MetaCommand::New => {
            app.new_session();
        }
        MetaCommand::Sessions => {
            app.toggle_sessions();
        }
        MetaCommand::Yolo => {
            app.yolo = !app.yolo;
            app.status = format!("yolo={}", app.yolo);
        }
        MetaCommand::Help => {
            app.push_line(MessageLine::system(
                "Commands: /help  /skills  /new  /sessions  /export  /undo  /yolo  /quit\n\
                 Skills: type / then Tab — e.g. /git fix the commit\n\
                 Files: type @ then Tab — e.g. fix @src/main.rs\n\
                 Keys: Enter send · Tab complete · ↑/↓ history · Ctrl+J newline · Ctrl+B sessions · Ctrl+C cancel",
            ));
        }
        MetaCommand::Skills => {
            let mut body = String::from("Skills (type /name in the composer):\n");
            for (id, desc) in &app.skill_details {
                let short: String = desc.chars().take(72).collect();
                body.push_str(&format!("  /{id}  — {short}\n"));
            }
            body.push_str(
                "\nTip: /skill-id activates that pack for the turn (plus always-on skills).",
            );
            app.push_line(MessageLine::system(body));
        }
        MetaCommand::Export => match export_transcript(app) {
            Ok(path) => {
                app.push_line(MessageLine::system(format!("Exported to {path}")));
                app.status = format!("exported → {path}");
            }
            Err(e) => {
                app.status = format!("export failed: {e}");
            }
        },
        MetaCommand::Undo => {
            let last_user = app.lines.iter().rposition(|l| l.role == "you");
            if let Some(user_idx) = last_user {
                let assistant_idx = app.lines[user_idx + 1..]
                    .iter()
                    .position(|l| l.role == "cortex")
                    .map(|i| user_idx + 1 + i);
                if let Some(assist_idx) = assistant_idx {
                    let prompt = app.lines[user_idx].content.clone();
                    app.lines.drain(assist_idx..=user_idx);
                    app.input = prompt;
                    app.input_cursor = app.input.chars().count();
                    app.status = "undid last exchange".into();
                } else {
                    app.status = "no assistant response to undo".into();
                }
            } else {
                app.status = "nothing to undo".into();
            }
        }
    }
    Ok(false)
}
