use std::ffi::{CString, CStr};
use std::os::raw::c_char;
use std::ptr;
use std::env;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, List, ListItem, Padding, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
    Frame, Terminal,
};
use std::{
    io,
    time::{Duration, Instant},
};
use serde::Deserialize;

extern "C" {
    // メモリ解放
    fn free_string(ptr: *mut c_char);    
    fn rag_add() -> *mut c_char;
    fn rag_search(input: *const c_char) -> *mut c_char;

    // 文字列処理（受信→加工→返却）
    fn process_string(input: *const c_char) -> *mut c_char;    
}
#[derive(Debug, Deserialize)]
struct Item {
    id: i32,
    title: String,
}

// ─── Color Palette (App Dark Theme) ────────────────────────────────────────
const BG: Color = Color::Rgb(13, 13, 13);
const SURFACE: Color = Color::Rgb(22, 22, 26);
const BORDER: Color = Color::Rgb(45, 45, 55);
const ACCENT: Color = Color::Rgb(100, 220, 180); // teal-green
const ACCENT_DIM: Color = Color::Rgb(50, 120, 100);
const USER_COLOR: Color = Color::Rgb(130, 180, 255); // soft blue
const ASSISTANT_COLOR: Color = Color::Rgb(100, 220, 180);
const MUTED: Color = Color::Rgb(90, 90, 110);
const TEXT: Color = Color::Rgb(210, 210, 220);
const TEXT_DIM: Color = Color::Rgb(140, 140, 155);
const ERROR_COLOR: Color = Color::Rgb(255, 100, 100);

// ─── Message Types ────────────────────────────────────────────────────────────
#[derive(Clone, PartialEq)]
enum Role {
    User,
    Assistant,
    System,
}

#[derive(Clone)]
struct Message {
    role: Role,
    content: String,
    timestamp: String,
}

impl Message {
    fn new(role: Role, content: impl Into<String>) -> Self {
        let now = chrono::Local::now();
        Self {
            role,
            content: content.into(),
            timestamp: now.format("%H:%M").to_string(),
        }
    }
}

// ─── App State ────────────────────────────────────────────────────────────────
struct App {
    messages: Vec<Message>,
    input: String,
    cursor_pos: usize,
    scroll_offset: usize,
    scroll_state: ScrollbarState,
    thinking: bool,
    think_frame: usize,
    last_tick: Instant,
    mode: InputMode,
    model: String,
    token_count: usize,
    query: String,
}

#[derive(PartialEq)]
enum InputMode {
    Normal,
    Insert,
}

impl App {
    fn new() -> Self {
        let welcome = Message {
            role: Role::System,
            content: "RAG App へようこそ。コードや質問を入力してください。\n[i] で入力モード / [Esc] で通常モード / [Enter] で送信 / [Ctrl+C] で終了".into(),
            timestamp: chrono::Local::now().format("%H:%M").to_string(),
        };
        Self {
            messages: vec![welcome],
            input: String::new(),
            cursor_pos: 0,
            scroll_offset: 0,
            scroll_state: ScrollbarState::default(),
            thinking: false,
            think_frame: 0,
            last_tick: Instant::now(),
            mode: InputMode::Normal,
            model: "OpenRouter".into(),
            token_count: 0,
            query: String::new(),
        }
    }

    fn send_message(&mut self) {
        self.messages = vec![]; 
        if self.input.trim().is_empty() {
            return;
        }    
        self.query = self.input.clone();    
        let content = self.input.drain(..).collect::<String>();
        self.cursor_pos = 0;
        self.token_count += content.split_whitespace().count();
        self.messages.push(Message::new(Role::User, content));
        self.thinking = true;
        self.think_frame = 0;
    }

    fn receive_response(&mut self) {
        let mut input_buff = self.query.clone();
        unsafe {
            let c_input = CString::new(input_buff.clone()).unwrap();
            let result_ptr = rag_search(c_input.as_ptr());
            if !result_ptr.is_null() {
                let result_cstr = CStr::from_ptr(result_ptr);
                let result_str = result_cstr.to_str().unwrap();
                let resp = result_str.to_string();
                /*
                let items: Vec<String> = resp
                    .lines()
                    .map(|s| s.to_string())
                    .collect();
                for item in items {
                    let s3 = format!("{}", item);
                    self.messages.push(s3);
                }
                */
                self.token_count += resp.split_whitespace().count();
                self.messages.push(Message::new(Role::Assistant, resp));
                self.thinking = false;
                self.scroll_offset = self.messages.len().saturating_sub(1);

                free_string(result_ptr);
            }    
        }
    }

    fn tick(&mut self) {
        if self.last_tick.elapsed() >= Duration::from_millis(80) {
            self.last_tick = Instant::now();
            if self.thinking {
                self.think_frame = self.think_frame.wrapping_add(1);
                // Simulate response after ~1 second (12 frames × 80ms)
                if self.think_frame >= 12 {
                    self.receive_response();
                }
            }
        }
    }

    fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    fn delete_char(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self
                .input[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input.drain(prev..self.cursor_pos);
            self.cursor_pos = prev;
        }
    }

    fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        self.scroll_offset = (self.scroll_offset + 1).min(self.messages.len().saturating_sub(1));
    }
}

// ─── Simulated AI Response ────────────────────────────────────────────────────
fn simulate_response(input: &str) -> String {
    let lower = input.to_lowercase();
    if lower.contains("hello") || lower.contains("こんにちは") || lower.contains("hi") {
        "こんにちは！何かお手伝いできることはありますか？コードのデバッグ、説明、新しい機能の実装など、お気軽にどうぞ。".into()
    } else if lower.contains("rust") {
        "Rust は安全性とパフォーマンスを兼ね備えた素晴らしい言語です。\n\n```rust\nfn main() {\n    let msg = \"Hello, Rust!\";\n    println!(\"{}\", msg);\n}\n```\n\n何か具体的な Rust のコードについて質問はありますか？".into()
    } else if lower.contains("ratatui") {
        "Ratatui は Rust 製の TUI (Terminal User Interface) ライブラリです。\nこのチャット画面自体も Ratatui で構築されています！\n\nレイアウト、ウィジェット、イベントハンドリングなど、何について詳しく知りたいですか？".into()
    } else if lower.contains("explain") || lower.contains("説明") {
        format!(
            "「{}」について説明します。\n\nこれは非常に興味深いトピックです。具体的にどの側面に焦点を当てて説明しましょうか？",
            input.chars().take(30).collect::<String>()
        )
    } else {
        format!(
            "了解しました。「{}」について処理中...\n\nこちらが私の回答です。実際の実装では OpenAI Codex や Claude API に接続してください。",
            input.chars().take(40).collect::<String>()
        )
    }
}

// ─── Render ───────────────────────────────────────────────────────────────────
fn ui(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    // Background
    frame.render_widget(
        Block::default().style(Style::default().bg(BG)),
        size,
    );

    // Main layout: header / chat / input
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Min(5),     // chat area
            Constraint::Length(4),  // input
            Constraint::Length(1),  // status bar
        ])
        .split(size);

    render_header(frame, app, chunks[0]);
    render_chat(frame, app, chunks[1]);
    render_input(frame, app, chunks[2]);
    render_statusbar(frame, app, chunks[3]);
}

fn render_header(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(22)])
        .split(area);

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled("❯ ", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled("RAG App", Style::default().fg(TEXT).add_modifier(Modifier::BOLD)),
        Span::styled("  Chat", Style::default().fg(TEXT_DIM)),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM | Borders::LEFT | Borders::TOP)
            .border_style(Style::default().fg(BORDER))
            .border_type(BorderType::Rounded)
            .padding(Padding::horizontal(1)),
    )
    .style(Style::default().bg(SURFACE));
    frame.render_widget(title, cols[0]);

    // Model badge
    let model_line = Line::from(vec![
        Span::styled("⬡ ", Style::default().fg(ACCENT_DIM)),
        Span::styled(
            app.model.as_str(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
    ]);
    let model_badge = Paragraph::new(model_line)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER))
                .border_type(BorderType::Rounded)
                .padding(Padding::horizontal(1)),
        )
        .style(Style::default().bg(SURFACE));
    frame.render_widget(model_badge, cols[1]);
}

fn render_chat(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let inner_width = area.width.saturating_sub(6) as usize; // borders + scrollbar + padding

    let mut items: Vec<ListItem> = Vec::new();

    for msg in &app.messages {
        let lines = build_message_lines(msg, inner_width);
        items.push(ListItem::new(lines));
    }

    // Thinking indicator
    if app.thinking {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let spinner = frames[app.think_frame % frames.len()];
        let thinking_line = Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(spinner, Style::default().fg(ACCENT)),
            Span::styled(
                " RAG App が思考中...",
                Style::default()
                    .fg(ACCENT_DIM)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]);
        items.push(ListItem::new(vec![Line::from(""), thinking_line]));
    }

    let total = items.len();
    let visible = area.height.saturating_sub(2) as usize;
    let max_offset = total.saturating_sub(visible);
    // Auto-scroll: if near bottom, keep scrolled to latest
    if app.scroll_offset >= max_offset.saturating_sub(1) {
        app.scroll_offset = max_offset;
    }

    app.scroll_state = app
        .scroll_state
        .content_length(total)
        .position(app.scroll_offset);

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT | Borders::TOP)
                .border_style(Style::default().fg(BORDER))
                .border_type(BorderType::Rounded)
                .padding(Padding::new(1, 2, 1, 0)),
        )
        .style(Style::default().bg(BG));

    // Render with scroll offset
    let inner = area.inner(Margin { horizontal: 0, vertical: 0 });
    frame.render_stateful_widget(
        list,
        inner,
        &mut ratatui::widgets::ListState::default().with_offset(app.scroll_offset),
    );

    // Scrollbar
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"))
            .thumb_style(Style::default().fg(ACCENT_DIM))
            .track_style(Style::default().fg(BORDER)),
        area.inner(Margin {
            horizontal: 0,
            vertical: 1,
        }),
        &mut app.scroll_state,
    );
}

fn build_message_lines(msg: &Message, _width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Separator / role header
    lines.push(Line::from(""));

    match msg.role {
        Role::User => {
            lines.push(Line::from(vec![
                Span::styled(
                    "  You  ",
                    Style::default()
                        .fg(BG)
                        .bg(USER_COLOR)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", msg.timestamp),
                    Style::default().fg(MUTED),
                ),
            ]));
        }
        Role::Assistant => {
            lines.push(Line::from(vec![
                Span::styled(
                    "  RAG App  ",
                    Style::default()
                        .fg(BG)
                        .bg(ASSISTANT_COLOR)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", msg.timestamp),
                    Style::default().fg(MUTED),
                ),
            ]));
        }
        Role::System => {
            lines.push(Line::from(vec![Span::styled(
                "  System  ",
                Style::default()
                    .fg(BG)
                    .bg(MUTED)
                    .add_modifier(Modifier::BOLD),
            )]));
        }
    }

    // Content lines with syntax-aware coloring
    for raw_line in msg.content.lines() {
        let line = render_content_line(raw_line, &msg.role);
        lines.push(line);
    }

    lines
}

fn render_content_line(line: &str, role: &Role) -> Line<'static> {
    let trimmed = line.trim_start();

    // Code block lines (simple heuristic)
    if line.starts_with("```") {
        return Line::from(vec![Span::styled(
            format!("  {}", line),
            Style::default().fg(BORDER),
        )]);
    }
    if line.starts_with("    ") || (trimmed.starts_with("fn ") || trimmed.starts_with("let ") || trimmed.starts_with("use ")) {
        return Line::from(vec![Span::styled(
            format!("  {}", line),
            Style::default().fg(Color::Rgb(180, 210, 255)),
        )]);
    }

    let color = match role {
        Role::User => TEXT,
        Role::Assistant => TEXT,
        Role::System => TEXT_DIM,
    };

    // Bold markers
    if trimmed.starts_with("# ") {
        return Line::from(vec![Span::styled(
            format!("  {}", line),
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD),
        )]);
    }

    Line::from(vec![Span::styled(
        format!("  {}", line),
        Style::default().fg(color),
    )])
}

fn render_input(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let (border_color, mode_indicator, mode_color) = match app.mode {
        InputMode::Insert => (ACCENT, "INSERT", ACCENT),
        InputMode::Normal => (BORDER, "NORMAL", MUTED),
    };

    // Display input with cursor
    let display_text = if app.mode == InputMode::Insert {
        let (before, after) = app.input.split_at(app.cursor_pos.min(app.input.len()));
        let cursor_char = after.chars().next().unwrap_or(' ');
        let after_cursor = if after.is_empty() { "" } else { &after[cursor_char.len_utf8()..] };

        vec![Line::from(vec![
            Span::styled(before.to_string(), Style::default().fg(TEXT)),
            Span::styled(
                cursor_char.to_string(),
                Style::default().fg(BG).bg(ACCENT),
            ),
            Span::styled(after_cursor.to_string(), Style::default().fg(TEXT)),
        ])]
    } else {
        let placeholder = if app.input.is_empty() {
            "[i] で入力開始...".to_string()
        } else {
            app.input.clone()
        };
        vec![Line::from(vec![Span::styled(
            placeholder,
            Style::default().fg(if app.input.is_empty() { MUTED } else { TEXT }),
        )])]
    };

    let input_widget = Paragraph::new(display_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .border_type(BorderType::Rounded)
                .title(Line::from(vec![
                    Span::styled(" ", Style::default()),
                    Span::styled(
                        mode_indicator,
                        Style::default()
                            .fg(mode_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" ", Style::default()),
                ]))
                .title_alignment(Alignment::Left)
                .padding(Padding::horizontal(1)),
        )
        .style(Style::default().bg(SURFACE))
        .wrap(Wrap { trim: false });

    frame.render_widget(input_widget, area);
}

fn render_statusbar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(25)])
        .split(area);

    let left = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓", Style::default().fg(ACCENT_DIM)),
        Span::styled(" スクロール", Style::default().fg(MUTED)),
        Span::styled("  Enter", Style::default().fg(ACCENT_DIM)),
        Span::styled(" 送信", Style::default().fg(MUTED)),
        Span::styled("  Esc", Style::default().fg(ACCENT_DIM)),
        Span::styled(" 通常モード", Style::default().fg(MUTED)),
        Span::styled("  Ctrl+C", Style::default().fg(ERROR_COLOR)),
        Span::styled(" 終了", Style::default().fg(MUTED)),
    ]))
    .style(Style::default().bg(BG));
    frame.render_widget(left, cols[0]);

    let right = Paragraph::new(Line::from(vec![
        Span::styled("tokens: ", Style::default().fg(MUTED)),
        Span::styled(
            app.token_count.to_string(),
            Style::default().fg(ACCENT_DIM),
        ),
        Span::styled(
            format!("  msgs: {}", app.messages.len()),
            Style::default().fg(MUTED),
        ),
        Span::styled(" ", Style::default()),
    ]))
    .alignment(Alignment::Right)
    .style(Style::default().bg(BG));
    frame.render_widget(right, cols[1]);
}

// ─── Main ─────────────────────────────────────────────────────────────────────
fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let tick_rate = Duration::from_millis(80);

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(app.last_tick.elapsed())
            .unwrap_or(Duration::ZERO);

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // Global: Ctrl+C always exits
                if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
                    break;
                }

                match app.mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('i') | KeyCode::Char('a') => {
                            app.mode = InputMode::Insert;
                            app.cursor_pos = app.input.len();
                        }
                        KeyCode::Char('q') => break,
                        KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
                        KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
                        KeyCode::Char('G') => {
                            app.scroll_offset = app.messages.len().saturating_sub(1);
                        }
                        KeyCode::Char('g') => {
                            app.scroll_offset = 0;
                        }
                        _ => {}
                    },
                    InputMode::Insert => match key.code {
                        KeyCode::Esc => {
                            app.mode = InputMode::Normal;
                        }
                        KeyCode::Enter => {
                            if !app.thinking {
                                app.send_message();
                                app.scroll_offset = app.messages.len().saturating_sub(1);
                            }
                        }
                        KeyCode::Char(c) => app.insert_char(c),
                        KeyCode::Backspace => app.delete_char(),
                        KeyCode::Left => {
                            if app.cursor_pos > 0 {
                                app.cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if app.cursor_pos < app.input.len() {
                                app.cursor_pos += 1;
                            }
                        }
                        KeyCode::Home => app.cursor_pos = 0,
                        KeyCode::End => app.cursor_pos = app.input.len(),
                        _ => {}
                    },
                }
            }
        }

        app.tick();
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
