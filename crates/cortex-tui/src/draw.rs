//! Ratatui drawing.

use crate::app::App;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

fn short_path(p: &str) -> String {
    if p.len() <= 36 {
        p.to_string()
    } else {
        format!("…{}", &p[p.len() - 34..])
    }
}

/// Draw the full TUI frame.
pub fn ui(f: &mut Frame, app: &App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    draw_header(f, root[0], app);
    draw_body(f, root[1], app);
    draw_input(f, root[2], app);
    draw_status(f, root[3], app);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let yolo = if app.yolo { "YOLO" } else { "safe" };
    let run = if app.running { " ● RUN" } else { "" };
    let title = format!(
        " Cortex  ·  {}  ·  {}  ·  {}  ·  db:{}{}",
        app.model_label,
        yolo,
        app.workspace,
        short_path(&app.database),
        run
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" agent OS ")
        .border_style(Style::default().fg(Color::Cyan));
    let p = Paragraph::new(title)
        .style(Style::default().fg(Color::White))
        .block(block);
    f.render_widget(p, area);
}

fn draw_body(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(22),
            Constraint::Percentage(53),
            Constraint::Percentage(25),
        ])
        .split(area);

    draw_sessions(f, cols[0], app);
    draw_transcript(f, cols[1], app);
    draw_logs(f, cols[2], app);
}

fn draw_sessions(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .sessions
        .iter()
        .map(|s| {
            let id = s.id.to_string();
            let short = if id.len() > 8 { &id[..8] } else { &id };
            let line = format!("{} · {} msg · {:?}", short, s.message_count, s.status);
            ListItem::new(line)
        })
        .collect();

    let border = if app.input_focused {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Yellow)
    };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" sessions (↑↓) ")
                .border_style(border),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    // ListState is Copy; render mutates a local copy because we only have &App.
    let mut state = app.session_list;
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_transcript(f: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = Vec::new();
    for m in &app.lines {
        let color = match m.role.as_str() {
            "you" => Color::Green,
            "cortex" => Color::Cyan,
            "tool" => Color::Magenta,
            _ => Color::DarkGray,
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("[{}] ", m.role),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(m.content.replace('\n', " ⏎ ")),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from("…"));
    }
    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll, 0))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" transcript ")
                .border_style(Style::default().fg(Color::Cyan)),
        );
    f.render_widget(p, area);
}

fn draw_logs(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .logs
        .iter()
        .rev()
        .take(40)
        .map(|l| ListItem::new(l.as_str()))
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" tools / log ")
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(list, area);
}

fn draw_input(f: &mut Frame, area: Rect, app: &App) {
    let border = if app.input_focused {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let title = if app.running {
        " input (running — Ctrl-C cancel) "
    } else if app.input_focused {
        " input (Enter send · Tab focus) "
    } else {
        " input (i / Enter to focus) "
    };
    let text = if app.input.is_empty() && app.input_focused {
        "│".to_string()
    } else if app.input_focused {
        format!("{}│", app.input)
    } else {
        app.input.clone()
    };
    let p = Paragraph::new(text)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border),
        );
    f.render_widget(p, area);
}

fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let help = " q quit · Tab focus · n new · r reload · y yolo · ↑↓ sessions · Enter open/send ";
    let line = format!(" {}  ·  {} ", app.status, help);
    let p = Paragraph::new(line).style(Style::default().fg(Color::DarkGray).bg(Color::Black));
    f.render_widget(p, area);
}
