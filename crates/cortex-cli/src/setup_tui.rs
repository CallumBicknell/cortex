//! Ratatui first-run setup wizard.

use crate::setup_config::{write_setup_models_toml, DetectedEnv, SetupPreset};
use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use std::io::stdout;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Choose,
    EditCustom,
    EditModel,
    Confirm,
    Done,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CustomField {
    Id,
    BaseUrl,
    Model,
    ApiKeyEnv,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuItem {
    Auto,
    Mock,
    Ollama,
    OpenAI,
    Anthropic,
    OpenRouter,
    Custom,
}

impl MenuItem {
    fn title(self, detect: &DetectedEnv) -> String {
        match self {
            Self::Auto => format!("Auto — {}", detect.auto_preset().label()),
            Self::Mock => "Mock (offline, no API key)".into(),
            Self::Ollama => format!(
                "Ollama (local){}",
                if detect.ollama_up { "  ● up" } else { "" }
            ),
            Self::OpenAI => format!("OpenAI{}", if detect.openai_key { "  ● key" } else { "" }),
            Self::Anthropic => format!(
                "Anthropic{}",
                if detect.anthropic_key {
                    "  ● key"
                } else {
                    ""
                }
            ),
            Self::OpenRouter => format!(
                "OpenRouter{}",
                if detect.openrouter_key {
                    "  ● key"
                } else {
                    ""
                }
            ),
            Self::Custom => "Custom OpenAI-compatible…".into(),
        }
    }
}

struct Wizard {
    home: PathBuf,
    models_path: PathBuf,
    detect: DetectedEnv,
    step: Step,
    menu: Vec<MenuItem>,
    menu_idx: usize,
    list_state: ListState,
    /// Preset being edited / confirmed.
    working: Option<SetupPreset>,
    custom_id: String,
    custom_base_url: String,
    custom_model: String,
    custom_api_key_env: String,
    custom_field: CustomField,
    model_edit: String,
    status: String,
    result_alias: Option<String>,
}

/// Run the full-screen setup wizard. Returns the default model alias written.
pub fn run_setup_tui(home: &Path, models_path: &Path) -> Result<String> {
    let detect = DetectedEnv::detect();
    let menu = vec![
        MenuItem::Auto,
        MenuItem::Mock,
        MenuItem::Ollama,
        MenuItem::OpenAI,
        MenuItem::Anthropic,
        MenuItem::OpenRouter,
        MenuItem::Custom,
    ];
    let mut list_state = ListState::default();
    list_state.select(Some(0));

    let mut wiz = Wizard {
        home: home.to_path_buf(),
        models_path: models_path.to_path_buf(),
        detect,
        step: Step::Choose,
        menu,
        menu_idx: 0,
        list_state,
        working: None,
        custom_id: "custom".into(),
        custom_base_url: "https://api.openai.com/v1".into(),
        custom_model: "gpt-4.1".into(),
        custom_api_key_env: "OPENAI_API_KEY".into(),
        custom_field: CustomField::Id,
        model_edit: String::new(),
        status: "↑/↓ select · Enter · s = mock · Esc quit".into(),
        result_alias: None,
    };

    enable_raw_mode().context("enable raw mode")?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend).context("create terminal")?;
    terminal.clear()?;

    let outcome = loop {
        terminal.draw(|f| draw(f, &mut wiz))?;
        if matches!(wiz.step, Step::Done | Step::Cancelled) {
            break wiz.step;
        }
        if !event::poll(Duration::from_millis(200)).context("poll")? {
            continue;
        }
        let Event::Key(key) = event::read().context("read key")? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match wiz.step {
            Step::Choose => on_choose(&mut wiz, key.code),
            Step::EditCustom => on_custom(&mut wiz, key.code),
            Step::EditModel => on_model(&mut wiz, key.code),
            Step::Confirm => on_confirm(&mut wiz, key.code)?,
            Step::Done | Step::Cancelled => break wiz.step,
        }
    };

    disable_raw_mode().ok();
    stdout().execute(LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    match outcome {
        Step::Done => wiz
            .result_alias
            .ok_or_else(|| anyhow::anyhow!("setup finished without alias")),
        _ => anyhow::bail!("setup wizard cancelled"),
    }
}

fn on_choose(wiz: &mut Wizard, code: KeyCode) {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => wiz.step = Step::Cancelled,
        KeyCode::Char('s') => {
            wiz.working = Some(SetupPreset::Mock);
            wiz.step = Step::Confirm;
            wiz.status = "Enter save · Esc back".into();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            wiz.menu_idx = if wiz.menu_idx == 0 {
                wiz.menu.len() - 1
            } else {
                wiz.menu_idx - 1
            };
            wiz.list_state.select(Some(wiz.menu_idx));
        }
        KeyCode::Down | KeyCode::Char('j') => {
            wiz.menu_idx = (wiz.menu_idx + 1) % wiz.menu.len();
            wiz.list_state.select(Some(wiz.menu_idx));
        }
        KeyCode::Enter => match wiz.menu[wiz.menu_idx] {
            MenuItem::Auto => {
                let p = wiz.detect.auto_preset();
                if matches!(p, SetupPreset::Mock) {
                    wiz.working = Some(p);
                    wiz.step = Step::Confirm;
                    wiz.status = "Enter save · Esc back".into();
                } else {
                    start_model_edit(wiz, p);
                }
            }
            MenuItem::Mock => {
                wiz.working = Some(SetupPreset::Mock);
                wiz.step = Step::Confirm;
                wiz.status = "Enter save · Esc back".into();
            }
            MenuItem::Custom => {
                wiz.step = Step::EditCustom;
                wiz.custom_field = CustomField::Id;
                wiz.status = "Tab fields · Enter on last to confirm · Esc back".into();
            }
            MenuItem::Ollama => start_model_edit(
                wiz,
                SetupPreset::Ollama {
                    model: "qwen2.5-coder".into(),
                },
            ),
            MenuItem::OpenAI => start_model_edit(
                wiz,
                SetupPreset::OpenAI {
                    model: "gpt-4.1".into(),
                },
            ),
            MenuItem::Anthropic => start_model_edit(
                wiz,
                SetupPreset::Anthropic {
                    model: "claude-sonnet-4-20250514".into(),
                },
            ),
            MenuItem::OpenRouter => start_model_edit(
                wiz,
                SetupPreset::OpenRouter {
                    model: "anthropic/claude-sonnet-4".into(),
                },
            ),
        },
        _ => {}
    }
}

fn start_model_edit(wiz: &mut Wizard, preset: SetupPreset) {
    wiz.model_edit = match &preset {
        SetupPreset::Ollama { model }
        | SetupPreset::OpenAI { model }
        | SetupPreset::Anthropic { model }
        | SetupPreset::OpenRouter { model }
        | SetupPreset::Custom { model, .. } => model.clone(),
        SetupPreset::Mock => String::new(),
    };
    wiz.working = Some(preset);
    wiz.step = Step::EditModel;
    wiz.status = "Edit model id · Enter continue · Esc back".into();
}

fn on_model(wiz: &mut Wizard, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            wiz.step = Step::Choose;
            wiz.working = None;
            wiz.status = "↑/↓ select · Enter · s = mock · Esc quit".into();
        }
        KeyCode::Enter => {
            let mid = wiz.model_edit.trim();
            if mid.is_empty() {
                wiz.status = "model id cannot be empty".into();
                return;
            }
            let next = match wiz.working.take() {
                Some(SetupPreset::Ollama { .. }) => SetupPreset::Ollama { model: mid.into() },
                Some(SetupPreset::OpenAI { .. }) => SetupPreset::OpenAI { model: mid.into() },
                Some(SetupPreset::Anthropic { .. }) => SetupPreset::Anthropic { model: mid.into() },
                Some(SetupPreset::OpenRouter { .. }) => {
                    SetupPreset::OpenRouter { model: mid.into() }
                }
                Some(other) => other,
                None => {
                    wiz.status = "no preset".into();
                    return;
                }
            };
            wiz.working = Some(next);
            wiz.step = Step::Confirm;
            wiz.status = "Enter save · Esc back".into();
        }
        KeyCode::Backspace => {
            wiz.model_edit.pop();
        }
        KeyCode::Char(c) if !c.is_control() => wiz.model_edit.push(c),
        _ => {}
    }
}

fn on_custom(wiz: &mut Wizard, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            wiz.step = Step::Choose;
            wiz.status = "↑/↓ select · Enter · s = mock · Esc quit".into();
        }
        KeyCode::Tab | KeyCode::Down => {
            wiz.custom_field = match wiz.custom_field {
                CustomField::Id => CustomField::BaseUrl,
                CustomField::BaseUrl => CustomField::Model,
                CustomField::Model => CustomField::ApiKeyEnv,
                CustomField::ApiKeyEnv => CustomField::Id,
            };
        }
        KeyCode::BackTab | KeyCode::Up => {
            wiz.custom_field = match wiz.custom_field {
                CustomField::Id => CustomField::ApiKeyEnv,
                CustomField::BaseUrl => CustomField::Id,
                CustomField::Model => CustomField::BaseUrl,
                CustomField::ApiKeyEnv => CustomField::Model,
            };
        }
        KeyCode::Enter => {
            if wiz.custom_field != CustomField::ApiKeyEnv {
                wiz.custom_field = match wiz.custom_field {
                    CustomField::Id => CustomField::BaseUrl,
                    CustomField::BaseUrl => CustomField::Model,
                    CustomField::Model => CustomField::ApiKeyEnv,
                    CustomField::ApiKeyEnv => CustomField::ApiKeyEnv,
                };
                return;
            }
            let preset = SetupPreset::Custom {
                id: wiz.custom_id.trim().into(),
                base_url: wiz.custom_base_url.trim().into(),
                model: wiz.custom_model.trim().into(),
                api_key_env: wiz.custom_api_key_env.trim().into(),
            };
            if let Err(e) = crate::setup_config::validate_id(preset.alias()) {
                wiz.status = format!("id: {e}");
                return;
            }
            if wiz.custom_base_url.trim().is_empty() || wiz.custom_model.trim().is_empty() {
                wiz.status = "base_url and model required".into();
                return;
            }
            wiz.working = Some(preset);
            wiz.step = Step::Confirm;
            wiz.status = "Enter save · Esc back".into();
        }
        KeyCode::Backspace => {
            custom_buf(wiz).pop();
        }
        KeyCode::Char(c) if !c.is_control() => {
            custom_buf(wiz).push(c);
        }
        _ => {}
    }
}

fn custom_buf(wiz: &mut Wizard) -> &mut String {
    match wiz.custom_field {
        CustomField::Id => &mut wiz.custom_id,
        CustomField::BaseUrl => &mut wiz.custom_base_url,
        CustomField::Model => &mut wiz.custom_model,
        CustomField::ApiKeyEnv => &mut wiz.custom_api_key_env,
    }
}

fn on_confirm(wiz: &mut Wizard, code: KeyCode) -> Result<()> {
    match code {
        KeyCode::Esc => {
            wiz.step = Step::Choose;
            wiz.working = None;
            wiz.status = "↑/↓ select · Enter · s = mock · Esc quit".into();
        }
        KeyCode::Enter | KeyCode::Char('y') => {
            let Some(preset) = wiz.working.clone() else {
                wiz.status = "nothing to save".into();
                return Ok(());
            };
            write_setup_models_toml(&wiz.models_path, &preset)?;
            wiz.result_alias = Some(preset.alias().to_string());
            wiz.step = Step::Done;
        }
        _ => {}
    }
    Ok(())
}

fn draw(f: &mut Frame, wiz: &mut Wizard) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(f.area());

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " Cortex setup ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("· {} ", wiz.home.display())),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" agent OS ")
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        root[0],
    );

    match wiz.step {
        Step::Choose => draw_choose(f, root[1], wiz),
        Step::EditCustom => draw_custom(f, root[1], wiz),
        Step::EditModel => draw_model(f, root[1], wiz),
        Step::Confirm => draw_confirm(f, root[1], wiz),
        Step::Done => f.render_widget(
            Paragraph::new(format!(
                "Saved default_model = \"{}\"\n\n{}",
                wiz.result_alias.as_deref().unwrap_or("?"),
                wiz.models_path.display()
            ))
            .block(Block::default().borders(Borders::ALL).title(" done ")),
            root[1],
        ),
        Step::Cancelled => f.render_widget(
            Paragraph::new("Cancelled.")
                .block(Block::default().borders(Borders::ALL).title(" cancelled ")),
            root[1],
        ),
    }

    f.render_widget(
        Paragraph::new(wiz.status.as_str())
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title(" help ")),
        root[2],
    );
}

fn draw_choose(f: &mut Frame, area: Rect, wiz: &mut Wizard) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);

    let items: Vec<ListItem> = wiz
        .menu
        .iter()
        .map(|m| ListItem::new(m.title(&wiz.detect)))
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" default provider ")
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    let mut state = wiz.list_state;
    f.render_stateful_widget(list, cols[0], &mut state);
    wiz.list_state = state;

    let mut det = vec![Line::from(Span::styled(
        "Auto-detect",
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    for line in wiz.detect.summary_lines() {
        det.push(Line::from(line));
    }
    det.push(Line::from(""));
    det.push(Line::from("● key/env present (secret never shown)."));
    det.push(Line::from("Custom = Groq, Together, vLLM, Azure, …"));
    det.push(Line::from("Anthropic uses native Messages API."));
    f.render_widget(
        Paragraph::new(det).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" environment "),
        ),
        cols[1],
    );
}

fn draw_custom(f: &mut Frame, area: Rect, wiz: &Wizard) {
    let fields = [
        (CustomField::Id, "id", wiz.custom_id.as_str()),
        (
            CustomField::BaseUrl,
            "base_url",
            wiz.custom_base_url.as_str(),
        ),
        (CustomField::Model, "model", wiz.custom_model.as_str()),
        (
            CustomField::ApiKeyEnv,
            "api_key_env",
            wiz.custom_api_key_env.as_str(),
        ),
    ];
    let mut lines = vec![Line::from("OpenAI-compatible custom provider")];
    for (field, name, val) in fields {
        let focus = field == wiz.custom_field;
        let style = if focus {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let caret = if focus { "▌" } else { "" };
        lines.push(Line::from(vec![
            Span::styled(format!("{name:12} "), style),
            Span::raw(format!("{val}{caret}")),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(
        "Examples: groq · https://api.groq.com/openai/v1",
    ));
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" custom ")
                .border_style(Style::default().fg(Color::Magenta)),
        ),
        area,
    );
}

fn draw_model(f: &mut Frame, area: Rect, wiz: &Wizard) {
    let label = wiz
        .working
        .as_ref()
        .map(|p| p.label())
        .unwrap_or_else(|| "model".into());
    let body = vec![
        Line::from(format!("Provider: {label}")),
        Line::from(""),
        Line::from(vec![
            Span::styled("model id: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{}▌", wiz.model_edit)),
        ]),
    ];
    f.render_widget(
        Paragraph::new(body).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" model ")
                .border_style(Style::default().fg(Color::Green)),
        ),
        area,
    );
}

fn draw_confirm(f: &mut Frame, area: Rect, wiz: &Wizard) {
    let mut lines = vec![Line::from(Span::styled(
        "Write models.toml?",
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    if let Some(p) = &wiz.working {
        lines.push(Line::from(format!("  default_model = \"{}\"", p.alias())));
        lines.push(Line::from(format!("  {}", p.label())));
        lines.push(Line::from(format!("  {}", wiz.models_path.display())));
        match p {
            SetupPreset::OpenAI { .. } => {
                lines.push(Line::from("  needs OPENAI_API_KEY"));
            }
            SetupPreset::Anthropic { .. } => {
                lines.push(Line::from("  needs ANTHROPIC_API_KEY"));
            }
            SetupPreset::OpenRouter { .. } => {
                lines.push(Line::from("  needs OPENROUTER_API_KEY"));
            }
            SetupPreset::Ollama { .. } => {
                lines.push(Line::from("  needs ollama serve + pull model"));
            }
            SetupPreset::Custom { api_key_env, .. } if !api_key_env.is_empty() => {
                lines.push(Line::from(format!("  needs env {api_key_env}")));
            }
            _ => {}
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Enter / y  save    Esc  back"));
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" confirm ")
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        area,
    );
}
