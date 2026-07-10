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
    // Do NOT enable mouse capture — it steals the pointer from the terminal
    // and blocks native drag-select / copy in the conversation. Scroll with
    // PgUp/PgDn (and Ctrl+L to jump to bottom) instead.
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
        terminal.draw(|f| draw::ui(f, &mut app))?;

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
                        if app.input_focused && !app.show_sessions {
                            app.insert_str(&text);
                        }
                    }
                    Some(Ok(Event::Resize(_, _))) => {}
                    // Mouse events ignored (capture off) so the terminal can
                    // drag-select and copy conversation text natively.
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
                    // Drain queue: start next message typed while we were thinking.
                    if let Some(next) = app.pop_pending() {
                        let remaining = app.pending.len();
                        if let Err(e) =
                            start_agent_turn(&mut app, &host, next, &mut run_cancel, &tx)
                        {
                            app.status = format!("queue start failed: {e}");
                        } else if remaining > 0 {
                            app.status =
                                format!("running… · {remaining} more queued");
                        }
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
            app.jump_to_bottom();
            return Ok(false);
        }
        _ => {}
    }

    // Newline without send: Shift+Enter or Ctrl+J
    if app.input_focused
        && ((code == KeyCode::Enter && mods.contains(KeyModifiers::SHIFT))
            || (code == KeyCode::Char('j') && mods.contains(KeyModifiers::CONTROL)))
    {
        app.insert_newline();
        return Ok(false);
    }

    // Toggle yolo
    if code == KeyCode::Char('y') && mods.contains(KeyModifiers::CONTROL) {
        app.yolo = !app.yolo;
        app.status = format!("yolo={}", app.yolo);
        return Ok(false);
    }

    // Copy last assistant reply to the system clipboard
    if code == KeyCode::Char('o') && mods.contains(KeyModifiers::CONTROL) {
        match copy_last_assistant(app) {
            Ok(n) => app.status = format!("copied last reply ({n} chars)"),
            Err(e) => app.status = format!("copy failed: {e}"),
        }
        return Ok(false);
    }

    // Completion navigation (when popup is open) — also while thinking
    if app.completion.is_some() && app.input_focused {
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

    // Cursor + smart history when completion popup is closed
    if app.input_focused && app.completion.is_none() {
        match code {
            KeyCode::Left => {
                app.move_left();
                return Ok(false);
            }
            KeyCode::Right => {
                app.move_right();
                return Ok(false);
            }
            KeyCode::Home => {
                app.move_home();
                return Ok(false);
            }
            KeyCode::End => {
                app.move_end();
                return Ok(false);
            }
            KeyCode::Up => {
                app.move_up_or_history();
                return Ok(false);
            }
            KeyCode::Down => {
                app.move_down_or_history();
                return Ok(false);
            }
            KeyCode::Delete => {
                app.delete_forward();
                return Ok(false);
            }
            _ => {}
        }
    }

    match code {
        KeyCode::Esc => {
            if app.completion.is_some() {
                app.clear_completion();
            } else if !app.input.is_empty() {
                app.input.clear();
                app.input_cursor = 0;
                app.clear_completion();
                app.history_index = None;
                app.history_draft.clear();
            } else if app.running {
                // Empty input + Esc cancels the run (typing-friendly: Esc first clears draft).
                if let Some(c) = run_cancel.take() {
                    c.cancel();
                }
                app.running = false;
                app.status = "cancelled".into();
                app.streaming = None;
                app.activity = None;
            }
        }
        KeyCode::Tab if app.input_focused => {
            if app.completion.is_none() {
                app.refresh_completion();
            }
            if app.completion.is_some() {
                app.accept_completion();
            }
        }
        KeyCode::Enter if app.input_focused && !mods.contains(KeyModifiers::SHIFT) => {
            let prompt = app.take_input();
            let prompt = prompt.trim_end().to_string();
            if prompt.is_empty() {
                return Ok(false);
            }

            let parsed = parse_prompt(&prompt, &app.skill_ids);
            if let Some(meta) = parsed.meta {
                return handle_meta(app, meta);
            }

            app.push_input_history(&parsed.display);

            // While thinking: queue the message and keep typing free.
            if app.running {
                app.enqueue_pending(parsed.display.clone());
                app.push_line(MessageLine::system(format!(
                    "queued ({}): {}",
                    app.pending.len(),
                    truncate_for_status(&parsed.display, 60)
                )));
                app.status = format!("queued {} · running… (Ctrl+C cancel)", app.pending.len());
                return Ok(false);
            }

            if let Err(e) = start_agent_turn(app, host, parsed.display, run_cancel, tx) {
                app.status = format!("start failed: {e}");
            }
        }
        KeyCode::Char(c) if app.input_focused && !mods.contains(KeyModifiers::CONTROL) => {
            app.insert_char(c);
        }
        KeyCode::Backspace if app.input_focused => {
            app.backspace();
        }
        _ => {}
    }

    Ok(false)
}

/// Kick off one agent turn (shared by Enter and the pending queue).
fn start_agent_turn(
    app: &mut App,
    host: &TuiHost,
    prompt: String,
    run_cancel: &mut Option<CancellationToken>,
    tx: &mpsc::UnboundedSender<UiEvent>,
) -> Result<()> {
    let parsed = parse_prompt(&prompt, &app.skill_ids);
    // Meta should not be queued as agent work.
    if parsed.meta.is_some() {
        return Ok(());
    }

    // Keep slash-skill intent visible to the model (tokens were stripped for
    // cleanliness, but without a signal the LLM often false-refuses browsing).
    let mut agent_body = parsed.agent_prompt.clone();
    if !parsed.skills.is_empty() {
        let list = parsed.skills.join(", ");
        agent_body = format!(
            "[Skill packs explicitly activated for this turn: {list}]\n\
             Use the tools from these packs. Do not claim you lack those capabilities.\n\n\
             {agent_body}"
        );
    }
    let agent_prompt = expand_attachments(&app.workspace_path, &parsed.attachments, &agent_body);

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
    if app.status == "ready" || app.status.is_empty() || app.status.starts_with("done") {
        app.status = "running…".into();
    } else if !app.status.contains("running") {
        app.status = format!("{} · running…", app.status);
    }
    app.streaming = None;
    app.activity = None;
    // New user turns resume follow so you see the reply stream.
    app.jump_to_bottom();

    let host = host.clone_for_run();
    let session = app.session.clone();
    let yolo = app.yolo;
    let max_turns = app.max_turns;
    let tx = tx.clone();
    tokio::spawn(async move {
        host.run_turn(session, agent_prompt, yolo, max_turns, skills, cancel, tx)
            .await;
    });
    Ok(())
}

fn truncate_for_status(s: &str, max: usize) -> String {
    let flat: String = s.chars().map(|c| if c == '\n' { ' ' } else { c }).collect();
    if flat.chars().count() <= max {
        flat
    } else {
        let t: String = flat.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

/// Copy the most recent assistant message to the OS clipboard.
fn copy_last_assistant(app: &App) -> std::result::Result<usize, String> {
    let text = app
        .lines
        .iter()
        .rev()
        .find(|m| m.role == "cortex")
        .map(|m| m.content.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "no assistant message to copy".to_string())?;
    copy_to_clipboard(text)?;
    Ok(text.len())
}

/// Best-effort clipboard write via common CLI tools (no extra crate).
fn copy_to_clipboard(text: &str) -> std::result::Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else {
        &[
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
        ]
    };

    let mut last_err = String::from("no clipboard tool found (install wl-copy, xclip, or xsel)");
    for (bin, args) in candidates {
        let Ok(mut child) = Command::new(bin)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            continue;
        };
        if let Some(mut stdin) = child.stdin.take() {
            if let Err(e) = stdin.write_all(text.as_bytes()) {
                last_err = format!("{bin}: write failed: {e}");
                let _ = child.kill();
                continue;
            }
        }
        match child.wait() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => last_err = format!("{bin} exited {status}"),
            Err(e) => last_err = format!("{bin}: {e}"),
        }
    }
    Err(last_err)
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
                "Commands: /help  /skills  /new  /sessions  /yolo  /copy  /quit\n\
                 Skills: type / then Tab — e.g. /git fix the commit\n\
                 Files: type @ then Tab — e.g. fix @src/main.rs\n\
                 Keys: Enter send (queues while thinking) · Shift+Enter / Ctrl+J newline ·\n\
                 ←/→ cursor · ↑/↓ line then history · Tab complete · PgUp/PgDn scroll ·\n\
                 Ctrl+O copy last reply · drag-select chat · Ctrl+B sessions · Ctrl+C cancel",
            ));
        }
        MetaCommand::Copy => match copy_last_assistant(app) {
            Ok(n) => {
                app.push_line(MessageLine::system(format!(
                    "Copied last assistant reply ({n} chars) to the clipboard."
                )));
                app.status = format!("copied ({n} chars)");
            }
            Err(e) => {
                app.push_line(MessageLine::system(format!("Copy failed: {e}")));
                app.status = format!("copy failed: {e}");
            }
        },
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
