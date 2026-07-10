//! Claude Code–style chat drawing (minimal chrome, conversation-first).

use crate::app::App;
use crate::complete::{CompleteKind, CompletionState};
use cortex_models::SessionStatus;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

fn short_path(p: &str) -> String {
    if p.len() <= 48 {
        p.to_string()
    } else {
        let byte = p.floor_char_boundary(p.len() - 46);
        format!("…{}", &p[byte..])
    }
}

/// Draw the full chat UI.
pub fn ui(f: &mut Frame, app: &App) {
    let header_len = if app.compact { 0 } else { 1 };
    let composer_inner_w = f.area().width.saturating_sub(2).max(8);
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_len), // header strip (hidden in compact)
            Constraint::Min(6),             // conversation
            Constraint::Length(composer_height(app, composer_inner_w)),
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    if !app.compact {
        draw_header(f, root[0], app);
    }
    draw_conversation(f, root[1], app);
    draw_composer(f, root[2], app);
    draw_footer(f, root[3], app);

    if let Some(comp) = &app.completion {
        draw_completion_popup(f, root[2], comp);
    }

    if app.show_sessions {
        draw_sessions_overlay(f, f.area(), app);
    }

    if app.approval.is_some() {
        draw_approval_modal(f, f.area(), app);
    }
}

fn composer_height(app: &App, inner_width: u16) -> u16 {
    let rows = composer_visual_rows(app, inner_width).max(1);
    // borders (+2), grow with content, keep chat readable
    (rows + 2).clamp(3, 16)
}

/// How many terminal rows the composer body occupies when wrapped to `inner_width`.
fn composer_visual_rows(app: &App, inner_width: u16) -> u16 {
    let body = composer_body(app);
    let width = inner_width.max(1);
    body.split('\n')
        .map(|line| {
            let w = line.chars().count() as u16;
            if w == 0 {
                1
            } else {
                w.div_ceil(width)
            }
        })
        .fold(0u16, |a, b| a.saturating_add(b))
        .max(1)
}

/// Build the composer text (prefixes + caret), shared by height calc and draw.
fn composer_body(app: &App) -> String {
    let mut display = String::new();
    let cur = app.input_cursor.min(app.input.chars().count());
    let byte_cur = app
        .input
        .char_indices()
        .nth(cur)
        .map(|(i, _)| i)
        .unwrap_or(app.input.len());
    display.push_str(&app.input[..byte_cur]);
    if app.input_focused {
        display.push('▌');
    }
    display.push_str(&app.input[byte_cur..]);
    if display.is_empty() {
        display = if app.input_focused {
            "▌".into()
        } else {
            String::new()
        };
    }

    let mut out = String::new();
    let raw = if display.ends_with('\n') {
        format!("{display}\u{200b}")
    } else {
        display
    };
    for (i, line) in raw.split('\n').enumerate() {
        let line = line.trim_end_matches('\u{200b}');
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
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let yolo = if app.yolo { "yolo" } else { "safe" };
    let run = if app.running { "  ●" } else { "" };
    let session = if app.session_label.is_empty() {
        short_path(&app.database)
    } else {
        let label: String = app.session_label.chars().take(24).collect();
        label
    };
    let text = format!(
        " cortex  ·  {}  ·  {}  ·  {}  ·  {}{}",
        app.model_label,
        short_path(&app.workspace),
        yolo,
        session,
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
        // Streaming cursor — blinks when active.
        if let Some(last) = lines.last_mut() {
            if app.cursor_visible {
                last.spans.push(Span::styled(
                    " ▌",
                    Style::default().fg(Color::Rgb(120, 180, 255)),
                ));
            }
        }
    }

    if let Some(act) = &app.activity {
        if app.running {
            let elapsed = app
                .tool_start
                .map(|t| {
                    let secs = t.elapsed().as_secs_f64();
                    if secs >= 1.0 {
                        format!(" ({secs:.1}s)")
                    } else {
                        String::new()
                    }
                })
                .unwrap_or_default();
            lines.push(Line::from(Span::styled(
                format!("  · {act}{elapsed}"),
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

    // Show "↓ new" indicator when scrolled up.
    if app.scroll > 0 {
        let below = app.lines.len().saturating_sub(app.scroll as usize);
        let msg = if below > 0 {
            format!("  ↓ {below} messages below (Ctrl+L to jump)")
        } else {
            "  ↓ new content below (Ctrl+L to jump)".to_string()
        };
        lines.push(Line::from(Span::styled(
            msg,
            Style::default()
                .fg(Color::Rgb(100, 160, 120))
                .add_modifier(Modifier::ITALIC),
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
        " thinking… (Ctrl+C cancel) ".to_string()
    } else if app.completion.is_some() {
        " Tab/Enter accept · ↑↓ · Esc dismiss ".to_string()
    } else {
        let chars = app.input.chars().count();
        let lines = app.input.lines().count().max(1);
        format!(" message · {chars} chars · {lines} lines · Tab ")
    };

    let body = composer_body(app);
    let inner_w = area.width.saturating_sub(2).max(1);
    let inner_h = area.height.saturating_sub(2).max(1);

    // Scroll composer content so the caret stays in view.
    let total_rows = composer_visual_rows(app, inner_w);
    let scroll_y = if total_rows > inner_h {
        // Count visual rows up to the caret position.
        let cur = app.input_cursor.min(app.input.chars().count());
        let byte_cur = app
            .input
            .char_indices()
            .nth(cur)
            .map(|(i, _)| i)
            .unwrap_or(app.input.len());
        let input_prefix = &app.input[..byte_cur];
        // Count wraps in the input prefix (each hard newline + soft wraps).
        let mut row = 0u16;
        let mut col = 0u16;
        // Account for the "❯ " prefix on the first line.
        col = col.saturating_add(2);
        for ch in input_prefix.chars() {
            if ch == '\n' {
                row = row.saturating_add(1);
                col = 0;
            } else {
                col = col.saturating_add(1);
                if col >= inner_w {
                    row = row.saturating_add(1);
                    col = 0;
                }
            }
        }
        let max_scroll = total_rows.saturating_sub(inner_h);
        row.saturating_sub(inner_h.saturating_sub(1))
            .min(max_scroll)
    } else {
        0
    };

    let p = Paragraph::new(body)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0))
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
    let tokens = if app.last_prompt_tokens > 0 || app.last_completion_tokens > 0 {
        format!(
            "  ↑{} ↓{}",
            compact_tokens(app.last_prompt_tokens),
            compact_tokens(app.last_completion_tokens)
        )
    } else {
        String::new()
    };
    let elapsed = if let Some(start) = app.turn_start {
        let secs = start.elapsed().as_secs();
        if secs >= 60 {
            format!("  {:02}:{:02}", secs / 60, secs % 60)
        } else {
            format!("  {secs}s")
        }
    } else {
        String::new()
    };
    let streaming_chars = if let Some(draft) = &app.streaming {
        let chars = draft.chars().count();
        format!("  {chars} chars")
    } else {
        String::new()
    };
    let help = if area.width < 60 {
        " ↵ send  ↑↓ history  ^J nl  ^B sessions "
    } else if area.width < 80 {
        " ↵ send  ↑↓ history  Tab complete  ^J nl  ^B sessions  /quit "
    } else {
        " ↵ send  ↑↓ history  Tab complete  /skill  @path  ^J nl  ^B sessions  ^C cancel  /quit "
    };
    let line = format!(
        " {}{tokens}{elapsed}{streaming_chars}  ·{} ",
        app.status, help
    );
    let p = Paragraph::new(line).style(
        Style::default()
            .fg(Color::Rgb(100, 100, 110))
            .bg(Color::Rgb(14, 14, 18)),
    );
    f.render_widget(p, area);
}

/// Format token count compactly (e.g. 1234 → "1.2k", 500 → "500").
fn compact_tokens(n: u32) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
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

    let filtered = app.filtered_sessions();

    // Search bar at top of modal
    let search_line = if app.session_search.is_empty() {
        Line::from(Span::styled(
            "  type to filter…",
            Style::default().fg(Color::Rgb(100, 100, 110)),
        ))
    } else {
        Line::from(vec![
            Span::styled("  /", Style::default().fg(Color::Rgb(100, 100, 110))),
            Span::styled(
                app.session_search.clone(),
                Style::default()
                    .fg(Color::Rgb(180, 200, 255))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ({} found)", filtered.len()),
                Style::default().fg(Color::Rgb(100, 100, 110)),
            ),
        ])
    };

    let mut items: Vec<ListItem> = Vec::new();
    items.push(ListItem::new(search_line));

    // Separator
    items.push(ListItem::new(Line::from(Span::styled(
        "─".repeat(w.saturating_sub(2) as usize),
        Style::default().fg(Color::Rgb(50, 50, 60)),
    ))));

    if filtered.is_empty() {
        items.push(ListItem::new(if app.session_search.is_empty() {
            "(no sessions yet)".to_string()
        } else {
            "(no matches)".to_string()
        }));
    } else {
        let active_id = &app.session.id;
        for (_, s) in &filtered {
            let id = s.id.to_string();
            let short = if id.len() > 8 { &id[..8] } else { &id };
            let is_current = s.id == *active_id;

            // Relative time
            let ago = relative_time(s.updated_at);

            // Model abbreviation (strip provider prefix)
            let model_short = s.model.rsplit('/').next().unwrap_or(&s.model);
            let model_display = if model_short.len() > 20 {
                format!("{}…", &model_short[..19])
            } else {
                model_short.to_string()
            };

            // Status badge
            let status_str = match s.status {
                SessionStatus::Completed => "✓",
                SessionStatus::Failed => "✗",
                SessionStatus::Active => "●",
                SessionStatus::Paused => "❍",
                SessionStatus::Archived => "⋄",
            };

            let id_style = if is_current {
                Style::default()
                    .fg(Color::Rgb(180, 210, 255))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(140, 150, 160))
            };

            let mut spans = vec![
                Span::styled(short.to_string(), id_style),
                Span::styled(
                    format!("  {status_str} "),
                    Style::default().fg(Color::Rgb(120, 140, 160)),
                ),
                Span::styled(
                    format!("{:2} msgs", s.message_count),
                    Style::default().fg(Color::Rgb(160, 160, 170)),
                ),
                Span::styled(
                    format!("  {model_display}"),
                    Style::default().fg(Color::Rgb(100, 130, 170)),
                ),
                Span::styled(
                    format!("  {ago}"),
                    Style::default().fg(Color::Rgb(90, 90, 100)),
                ),
            ];
            if is_current {
                spans.push(Span::styled(
                    "  (current)",
                    Style::default()
                        .fg(Color::Rgb(140, 190, 255))
                        .add_modifier(Modifier::ITALIC),
                ));
            }
            items.push(ListItem::new(Line::from(spans)));
        }
    }

    let total_sessions = app.sessions.len();
    let title = if app.session_search.is_empty() {
        format!(" sessions ({total_sessions}) · ↑/↓ open · / search · d delete · n new ")
    } else {
        let matched = filtered.len();
        format!(" sessions ({matched}/{total_sessions}) · ↑/↓ open · Esc clear search ")
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
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
    // Offset selection index by 2 to account for search bar + separator rows
    let adjusted = state.selected().map(|i| i + 2);
    state.select(adjusted);
    f.render_stateful_widget(list, rect, &mut state);
}

/// Format a datetime as relative time string.
fn relative_time(dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(dt);
    let secs = diff.num_seconds();

    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else if secs < 604800 {
        format!("{}d ago", secs / 86400)
    } else {
        dt.format("%b %d").to_string()
    }
}

/// Centered modal for tool-approval requests.
fn draw_approval_modal(f: &mut Frame, area: Rect, app: &App) {
    let Some(modal) = &app.approval else {
        return;
    };

    let req = &modal.request;
    let tool = &req.tool_name;
    let summary = &req.summary;

    // Format arguments: show as compact JSON, truncated.
    let args_raw = req.arguments.to_string();
    let args_display = if args_raw.len() > 80 {
        let boundary = args_raw.floor_char_boundary(79);
        format!("{}…", &args_raw[..boundary])
    } else {
        args_raw
    };

    // Build lines for the modal body.
    let mut body_lines: Vec<Line> = Vec::new();
    body_lines.push(Line::from(Span::styled(
        format!("Tool: {tool}"),
        Style::default()
            .fg(Color::Rgb(255, 200, 80))
            .add_modifier(Modifier::BOLD),
    )));
    body_lines.push(Line::from(""));
    body_lines.push(Line::from(Span::styled(
        summary.clone(),
        Style::default().fg(Color::Rgb(200, 200, 210)),
    )));
    body_lines.push(Line::from(""));
    body_lines.push(Line::from(Span::styled(
        format!("args: {args_display}"),
        Style::default().fg(Color::Rgb(140, 140, 160)),
    )));
    body_lines.push(Line::from(""));
    body_lines.push(Line::from(Span::styled(
        "  y = Allow     n / Esc = Deny",
        Style::default()
            .fg(Color::Rgb(160, 170, 180))
            .add_modifier(Modifier::BOLD),
    )));

    // Size the modal to content.
    let line_count = body_lines.len() as u16;
    let content_width = body_lines
        .iter()
        .map(|l| l.width() as u16)
        .max()
        .unwrap_or(20);
    let w = (content_width + 6)
        .min(area.width.saturating_sub(4))
        .max(36);
    let h = (line_count + 4).min(area.height.saturating_sub(4)).max(10);

    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect::new(x, y, w, h);

    f.render_widget(Clear, rect);

    let p = Paragraph::new(body_lines)
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(Color::Rgb(24, 24, 32)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ⚠  Approve tool call ")
                .border_style(Style::default().fg(Color::Rgb(220, 180, 60)))
                .style(Style::default().bg(Color::Rgb(24, 24, 32))),
        );
    f.render_widget(p, rect);
}
