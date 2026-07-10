//! Terminal UI for Cortex — Claude Code–style interactive chat.
//!
//! Launch with [`run`]. Requires a prepared [`TuiHost`] (provider, tools, store).

#![deny(missing_docs)]

mod app;
mod complete;
mod draw;
mod host;
mod mentions;

pub use host::TuiHost;

use anyhow::{Context, Result};
use app::{App, MessageLine, UiEvent};
use crossterm::event::{
    DisableBracketedPaste, EnableBracketedPaste, Event, EventStream, KeyCode, KeyEventKind,
    KeyModifiers,
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
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend).context("create terminal")?;
    terminal.clear()?;

    let result = run_loop(&mut terminal, host).await;

    stdout().execute(DisableBracketedPaste).ok();
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
) -> Result<bool> {
    // Global cancel / quit
    if code == KeyCode::Char('c') && mods.contains(KeyModifiers::CONTROL) {
        if let Some(c) = run_cancel.take() {
            c.cancel();
            app.status = "cancelled".into();
            app.running = false;
            app.streaming = None;
            app.activity = None;
            return Ok(false);
        }
        return Ok(true);
    }

    // Sessions drawer
    if code == KeyCode::Char('b') && mods.contains(KeyModifiers::CONTROL) {
        app.toggle_sessions();
        return Ok(false);
    }

    if app.show_sessions {
        match code {
            KeyCode::Esc => {
                app.show_sessions = false;
                app.input_focused = true;
                app.status = "ready".into();
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

    match code {
        KeyCode::Esc => {
            if app.running {
                if let Some(c) = run_cancel.take() {
                    c.cancel();
                }
                app.running = false;
                app.status = "cancelled".into();
                app.streaming = None;
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
            tokio::spawn(async move {
                host.run_turn(session, agent_prompt, yolo, max_turns, skills, cancel, tx)
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
                "Commands: /help  /skills  /new  /sessions  /yolo  /quit\n\
                 Skills: type / then Tab — e.g. /git fix the commit\n\
                 Files: type @ then Tab — e.g. fix @src/main.rs\n\
                 Keys: Enter send · Tab complete · ↑/↓ select · Ctrl+J newline · Ctrl+B sessions · Ctrl+C cancel",
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
    }
    Ok(false)
}
