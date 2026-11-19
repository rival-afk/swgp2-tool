//! swgp2-tui - terminal editor for Super Woden GP 2 saves. No flags, just run it.
//!
//! Keys:  up/down select field   Enter edit    s save (auto-backup)
//!        Tab switch save slot   b backup      r restore latest backup
//!        d dump JSON to file    q quit

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;
use serde_json::Value as Json;
use std::io;
use std::path::PathBuf;
use std::time::Duration;
use swgp2_tool::app;

struct Field {
    path: String,
    label: String,
    hint: String,
    text: String,
}

enum Mode {
    Normal,
    Editing(String),
    ConfirmRestore,
}

struct App {
    slots: Vec<PathBuf>,
    slot: usize,
    save_path: Option<PathBuf>,
    fields: Vec<Field>,
    selected: usize,
    mode: Mode,
    message: (String, Color),
}

impl App {
    fn new() -> Self {
        let slots = app::slot_choices();
        let mut app = App {
            slots,
            slot: 0,
            save_path: None,
            fields: Vec::new(),
            selected: 0,
            mode: Mode::Normal,
            message: (String::new(), Color::Gray),
        };
        app.reload();
        app
    }

    fn reload(&mut self) {
        self.save_path = self.slots.get(self.slot).cloned();
        self.fields.clear();
        self.selected = 0;
        let Some(path) = self.save_path.clone() else {
            self.msg("no save slots found (game never run?)", Color::Red);
            return;
        };
        match app::load(&path) {
            Ok(g) => {
                self.fields = app::COMMON_FIELDS
                    .iter()
                    .map(|f| Field {
                        path: f.path.to_string(),
                        label: f.label.to_string(),
                        hint: f.hint.to_string(),
                        text: app::field_text(&g, f.path),
                    })
                    .collect();
                self.msg(format!("loaded {}", path.display()), Color::Green);
            }
            Err(e) => self.msg(format!("cannot load save: {e}"), Color::Red),
        }
    }

    fn msg(&mut self, text: impl Into<String>, color: Color) {
        self.message = (text.into(), color);
    }

    fn dirty_edits(&self) -> Vec<(String, Json)> {
        self.fields
            .iter()
            .map(|f| (f.path.clone(), app::parse_value(&f.text)))
            .collect()
    }

    fn save_all(&mut self) {
        let Some(path) = self.save_path.clone() else { return };
        match app::patch_save(&path, &self.dirty_edits()) {
            Ok(backup) => {
                self.msg(format!("saved (backup: {})", backup.display()), Color::Green);
                self.reload();
            }
            Err(e) => self.msg(format!("error: {e}"), Color::Red),
        }
    }

    fn backup(&mut self) {
        let Some(path) = self.save_path.clone() else { return };
        match swgp2_tool::save::create_backup(&path) {
            Ok(b) => self.msg(format!("backup created: {}", b.display()), Color::Green),
            Err(e) => self.msg(format!("error: {e}"), Color::Red),
        }
    }

    fn restore_latest(&mut self) {
        let Some(path) = self.save_path.clone() else { return };
        match swgp2_tool::save::list_backups(&path) {
            Ok(backups) => match backups.last() {
                Some(latest) => match app::restore_backup(&path, latest) {
                    Ok(cur) => {
                        self.msg(
                            format!("restored {} (current saved to {})", latest.display(), cur.display()),
                            Color::Green,
                        );
                        self.reload();
                    }
                    Err(e) => self.msg(format!("error: {e}"), Color::Red),
                },
                None => self.msg("no backups yet", Color::Yellow),
            },
            Err(e) => self.msg(format!("error: {e}"), Color::Red),
        }
    }

    fn dump(&mut self) {
        let Some(path) = self.save_path.clone() else { return };
        match app::dump_json(&path) {
            Ok(json) => {
                let out = std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("swgp2-dump.json");
                match std::fs::write(&out, json) {
                    Ok(()) => self.msg(format!("dumped to {}", out.display()), Color::Green),
                    Err(e) => self.msg(format!("error: {e}"), Color::Red),
                }
            }
            Err(e) => self.msg(format!("error: {e}"), Color::Red),
        }
    }
}

fn main() -> io::Result<()> {
    // No flags: any argument just gets a hint, then the TUI opens anyway.
    if std::env::args().len() > 1 {
        eprintln!("swgp2-tui takes no arguments - just run it");
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new();
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match &mut app.mode {
            Mode::Editing(buf) => match key.code {
                KeyCode::Enter => {
                    let text = buf.trim().to_string();
                    if let Some(field) = app.fields.get_mut(app.selected) {
                        field.text = text;
                    }
                    app.mode = Mode::Normal;
                    app.msg("press s to save", Color::Yellow);
                }
                KeyCode::Esc => app.mode = Mode::Normal,
                KeyCode::Backspace => {
                    buf.pop();
                }
                KeyCode::Char(c) => buf.push(c),
                _ => {}
            },
            Mode::ConfirmRestore => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    app.mode = Mode::Normal;
                    app.restore_latest();
                }
                _ => {
                    app.mode = Mode::Normal;
                    app.msg("restore cancelled", Color::Gray);
                }
            },
            Mode::Normal => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Up | KeyCode::Char('k') => {
                    if app.selected > 0 {
                        app.selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if app.selected + 1 < app.fields.len() {
                        app.selected += 1;
                    }
                }
                KeyCode::Enter => {
                    if let Some(field) = app.fields.get(app.selected) {
                        app.mode = Mode::Editing(field.text.clone());
                    }
                }
                KeyCode::Tab => {
                    if !app.slots.is_empty() {
                        app.slot = (app.slot + 1) % app.slots.len();
                        app.reload();
                    }
                }
                KeyCode::Char('s') => app.save_all(),
                KeyCode::Char('b') => app.backup(),
                KeyCode::Char('r') => app.mode = Mode::ConfirmRestore,
                KeyCode::Char('d') => app.dump(),
                _ => {}
            },
        }
    }
}

fn ui(f: &mut ratatui::Frame<'_>, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // save file
            Constraint::Min(8),    // fields
            Constraint::Length(3), // footer / message
        ])
        .split(f.area());

    let save_line = match &app.save_path {
        Some(p) => p.display().to_string(),
        None => "(no save found)".to_string(),
    };
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("SWGP2 save editor", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("   Tab: switch slot   Enter: edit   s: save   b: backup   r: restore   d: dump   q: quit"),
        ]),
        Line::from(save_line),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Super Woden GP 2 "));
    f.render_widget(header, chunks[0]);

    let items: Vec<ListItem> = app
        .fields
        .iter()
        .map(|fld| {
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(format!("{:<28}", fld.label), Style::default().fg(Color::White)),
                    Span::styled(&fld.text, Style::default().fg(Color::Yellow)),
                ]),
                Line::from(Span::styled(
                    format!("    {}", fld.hint),
                    Style::default().fg(Color::DarkGray),
                )),
            ])
        })
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Fields "))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");
    let mut state = ListState::default();
    state.select(Some(app.selected));
    f.render_stateful_widget(list, chunks[1], &mut state);

    let footer_text = match &app.mode {
        Mode::Editing(buf) => format!("edit: {buf}_   (Enter commit, Esc cancel)"),
        Mode::ConfirmRestore => "restore latest backup? y = yes, any other key = no".to_string(),
        Mode::Normal => app.message.0.clone(),
    };
    let footer_color = match &app.mode {
        Mode::Editing(_) | Mode::ConfirmRestore => Color::Yellow,
        Mode::Normal => app.message.1,
    };
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(footer_color))
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title(" Status "));
    f.render_widget(footer, chunks[2]);
}
