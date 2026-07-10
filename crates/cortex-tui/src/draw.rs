//! Claude Code–style chat drawing (minimal chrome, conversation-first).

use crate::app::App;
use crate::complete::{CompleteKind, CompletionState};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

fn short_path(p: &str) -> String {
    if p.len() <= 48 {
        p.to_string()
    } else {
        format!("…{}", &p[p.len() - 46..])
    }
}

/// Draw the full chat UI.
pub fn ui(f: &mut Frame, app: &App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header strip
            Constraint::Min(6),    // conversation
            Constraint::Length(composer_height(app)),
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    draw_header(f, root[0], app);
    draw_conversation(f, root[1], app);
    draw_composer(f, root[2], app);
    draw_footer(f, root[3], app);

    if let Some(comp) = &app.completion {
        draw_completion_popup(f, root[2], comp);
    }

    if app.show_sessions {
        draw_sessions_overlay(f, f.area(), app);
    }
}

fn composer_height(app: &App) -> u16 {
    let lines = app.input.lines().count().max(1) as u16;
    // borders + content, cap so chat stays usable
    (lines + 2).clamp(3, 10)
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let yolo = if app.yolo { "yolo" } else { "safe" };
    let run = if app.running { "  ●" } else { "" };
    let text = format!(
        " cortex  ·  {}  ·  {}  ·  {}  ·  {}{}",
        app.model_label,
        short_path(&app.workspace),
        yolo,
        short_path(&app.database),
        run
    );
    let p = Paragraph::new(text).style(
        Style::default()
            .fg(Color::Rgb(160, 170, 180))
            .bg(Color::Rgb(18, 18, 22)),
    );
    f.render_widget(p, area);
}

fn draw_conversation(f: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = Vec::new();

    for m in &app.lines {
        // blank gap between blocks
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        match m.role.as_str() {
            "you" => {
                lines.push(Line::from(Span::styled(
                    "You",
                    Style::default()
                        .fg(Color::Rgb(120, 200, 140))
                        .add_modifier(Modifier::BOLD),
                )));
                push_body(&mut lines, &m.content, Color::Rgb(220, 220, 220));
            }
            "cortex" => {
                lines.push(Line::from(Span::styled(
                    "Cortex",
                    Style::default()
                        .fg(Color::Rgb(120, 180, 255))
                        .add_modifier(Modifier::BOLD),
                )));
                push_body(&mut lines, &m.content, Color::Rgb(230, 230, 235));
            }
            "tool" => {
                lines.push(Line::from(Span::styled(
                    format!("  · {}", m.content),
                    Style::default().fg(Color::Rgb(140, 140, 155)),
                )));
            }
            _ => {
                lines.push(Line::from(Span::styled(
                    m.content.lines().next().unwrap_or(""),
                    Style::default()
                        .fg(Color::Rgb(110, 110, 120))
                        .add_modifier(Modifier::ITALIC),
                )));
                for extra in m.content.lines().skip(1) {
                    lines.push(Line::from(Span::styled(
                        extra.to_string(),
                        Style::default().fg(Color::Rgb(110, 110, 120)),
                    )));
                }
            }
        }
    }

    // Live stream
    if let Some(draft) = &app.streaming {
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            "Cortex",
            Style::default()
                .fg(Color::Rgb(120, 180, 255))
                .add_modifier(Modifier::BOLD),
        )));
        push_body(&mut lines, draft, Color::Rgb(230, 230, 235));
        // caret on last line
        if let Some(last) = lines.last_mut() {
            last.spans.push(Span::styled(
                " ▌",
                Style::default().fg(Color::Rgb(120, 180, 255)),
            ));
        }
    }

    if let Some(act) = &app.activity {
        if app.running {
            lines.push(Line::from(Span::styled(
                format!("  · {act}"),
                Style::default()
                    .fg(Color::Rgb(180, 140, 220))
                    .add_modifier(Modifier::ITALIC),
            )));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "Start typing below…",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll, 0))
        .style(Style::default().bg(Color::Rgb(12, 12, 16)));
    f.render_widget(p, area);
}

fn push_body(lines: &mut Vec<Line>, content: &str, color: Color) {
    if content.is_empty() {
        lines.push(Line::from(""));
        return;
    }
    for line in content.lines() {
        lines.push(Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(color),
        )));
    }
}

fn draw_composer(f: &mut Frame, area: Rect, app: &App) {
    let border = if app.running {
        Style::default().fg(Color::Rgb(180, 120, 60))
    } else if app.completion.is_some() {
        Style::default().fg(Color::Rgb(120, 160, 220))
    } else if app.input_focused {
        Style::default().fg(Color::Rgb(100, 160, 120))
    } else {
        Style::default().fg(Color::Rgb(60, 60, 70))
    };

    let title = if app.running {
        " thinking… (Ctrl+C cancel) "
    } else if app.completion.is_some() {
        " Tab/Enter accept · ↑↓ · Esc dismiss "
    } else {
        " message · /skill · @path · Tab "
    };

    let mut display = app.input.clone();
    if app.input_focused && !app.running {
        display.push('▌');
    }
    if display.is_empty() {
        display = if app.running {
            String::new()
        } else {
            "▌".into()
        };
    }

    // Prefix first line with ❯
    let body = if app.input.is_empty() && app.input_focused && !app.running {
        "❯ ▌".to_string()
    } else {
        let mut out = String::new();
        for (i, line) in display.lines().enumerate() {
            if i == 0 {
                out.push_str("❯ ");
            } else {
                out.push_str("  ");
            }
            out.push_str(line);
            out.push('\n');
        }
        if out.ends_with('\n') {
            out.pop();
        }
        out
    };

    let p = Paragraph::new(body)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::White).bg(Color::Rgb(18, 18, 24)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border)
                .title(title)
                .title_style(Style::default().fg(Color::Rgb(140, 140, 150))),
        );
    f.render_widget(p, area);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let help = " ↵ send  ↑↓ history  Tab complete  /skill  @path  ^J nl  ^B sessions  /quit ";
    let line = format!(" {}  ·{} ", app.status, help);
    let p = Paragraph::new(line).style(
        Style::default()
            .fg(Color::Rgb(100, 100, 110))
            .bg(Color::Rgb(14, 14, 18)),
    );
    f.render_widget(p, area);
}

/// Floating completion list above the composer.
fn draw_completion_popup(f: &mut Frame, composer_area: Rect, state: &CompletionState) {
    let n = state.items.len().min(10) as u16;
    if n == 0 {
        return;
    }
    let height = n + 2; // border
    let width = composer_area.width.clamp(24, 72);
    let x = composer_area.x;
    let y = composer_area.y.saturating_sub(height);
    let rect = Rect::new(x, y, width, height);
    f.render_widget(Clear, rect);

    let title = match state.kind {
        CompleteKind::Slash => " / skills & commands ",
        CompleteKind::Path => " @ paths ",
    };

    let items: Vec<ListItem> = state
        .items
        .iter()
        .map(|it| {
            let detail: String = it.detail.chars().take(40).collect();
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:24}", it.label),
                    Style::default().fg(Color::Rgb(200, 210, 220)),
                ),
                Span::styled(
                    format!(" {detail}"),
                    Style::default().fg(Color::Rgb(110, 120, 130)),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Rgb(100, 150, 210)))
                .style(Style::default().bg(Color::Rgb(22, 24, 32))),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Rgb(20, 22, 28))
                .bg(Color::Rgb(140, 190, 255))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(
        state.selected.min(state.items.len().saturating_sub(1)),
    ));
    f.render_stateful_widget(list, rect, &mut list_state);
}

fn draw_sessions_overlay(f: &mut Frame, area: Rect, app: &App) {
    // Centered modal list
    let w = (area.width * 60 / 100)
        .max(40)
        .min(area.width.saturating_sub(4));
    let h = (area.height * 50 / 100)
        .max(10)
        .min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect::new(x, y, w, h);

    f.render_widget(Clear, rect);

    let items: Vec<ListItem> = if app.sessions.is_empty() {
        vec![ListItem::new("(no sessions yet)")]
    } else {
        app.sessions
            .iter()
            .map(|s| {
                let id = s.id.to_string();
                let short = if id.len() > 8 { &id[..8] } else { &id };
                ListItem::new(format!(
                    "{}  ·  {} msgs  ·  {:?}",
                    short, s.message_count, s.status
                ))
            })
            .collect()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" sessions · Enter open · Esc/Ctrl+B close ")
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Rgb(20, 22, 28))),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(120, 180, 255))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    let mut state = app.session_list;
    f.render_stateful_widget(list, rect, &mut state);
}
