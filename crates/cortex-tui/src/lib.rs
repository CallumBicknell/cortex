//! Terminal UI for Cortex — interactive chat, sessions, and run logs.
//!
//! Launch with [`run`]. Requires a prepared [`TuiHost`] (provider, tools, store).

#![deny(missing_docs)]

mod app;
mod draw;
mod host;

pub use host::TuiHost;

use anyhow::{Context, Result};
use app::{App, MessageLine, UiEvent};
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

/// Run the Cortex TUI until the user quits.
pub async fn run(host: TuiHost) -> Result<()> {
    info!("starting cortex TUI");
    enable_raw_mode().context("enable raw mode")?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend).context("create terminal")?;
    terminal.clear()?;

    let result = run_loop(&mut terminal, host).await;

    // Always restore terminal.
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
                        if key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL)
                        {
                            if let Some(c) = run_cancel.take() {
                                c.cancel();
                                app.status = "cancelled run".into();
                                app.running = false;
                            } else {
                                break;
                            }
                            continue;
                        }

                        match key.code {
                            KeyCode::Esc | KeyCode::Char('q') if !app.input_focused || key.modifiers.contains(KeyModifiers::CONTROL) => {
                                if app.running {
                                    if let Some(c) = run_cancel.take() {
                                        c.cancel();
                                    }
                                    app.running = false;
                                    app.status = "cancelled".into();
                                } else {
                                    break;
                                }
                            }
                            KeyCode::Tab => {
                                app.cycle_focus();
                            }
                            KeyCode::Char('y') if !app.input_focused => {
                                app.yolo = !app.yolo;
                                app.status = format!("yolo={}", app.yolo);
                            }
                            KeyCode::Char('n') if !app.input_focused => {
                                app.new_session();
                            }
                            KeyCode::Char('r') if !app.input_focused => {
                                if let Err(e) = app.reload_sessions(&host).await {
                                    app.status = format!("reload failed: {e}");
                                } else {
                                    app.status = "sessions reloaded".into();
                                }
                            }
                            KeyCode::Up if !app.input_focused => {
                                app.select_prev();
                            }
                            KeyCode::Down if !app.input_focused => {
                                app.select_next();
                            }
                            KeyCode::Enter if app.input_focused && !app.running => {
                                let prompt = app.input.trim().to_string();
                                if !prompt.is_empty() {
                                    app.input.clear();
                                    app.push_line(MessageLine::user(prompt.clone()));
                                    let cancel = CancellationToken::new();
                                    run_cancel = Some(cancel.clone());
                                    app.running = true;
                                    app.status = "running…".into();
                                    let host = host.clone_for_run();
                                    let session = app.session.clone();
                                    let yolo = app.yolo;
                                    let max_turns = app.max_turns;
                                    let skills = app.skills.clone();
                                    let tx = tx.clone();
                                    app.streaming = None;
                                    tokio::spawn(async move {
                                        host.run_turn(
                                            session, prompt, yolo, max_turns, skills, cancel, tx,
                                        )
                                        .await;
                                    });
                                }
                            }
                            KeyCode::Enter if !app.input_focused => {
                                if let Err(e) = app.load_selected(&host).await {
                                    app.status = format!("load failed: {e}");
                                }
                            }
                            KeyCode::Char(c) if app.input_focused => {
                                app.input.push(c);
                            }
                            KeyCode::Backspace if app.input_focused => {
                                app.input.pop();
                            }
                            KeyCode::Char('i') if !app.input_focused => {
                                app.input_focused = true;
                            }
                            _ => {}
                        }
                    }
                    Some(Ok(Event::Resize(_, _))) => {}
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
