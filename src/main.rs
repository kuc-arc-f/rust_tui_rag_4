use std::ffi::{CString, CStr};
use std::os::raw::c_char;
use std::ptr;
use std::env;
use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use serde::Deserialize;

extern "C" {

    fn rag_search(input: *const c_char) -> *mut c_char;

    // 文字列処理（受信→加工→返却）
    fn process_string(input: *const c_char) -> *mut c_char;
    
    // メモリ解放
    fn free_string(ptr: *mut c_char);    
}

#[derive(Debug, Deserialize)]
struct Item {
    id: i32,
    title: String,
}

fn chat_handler(input: String) -> String
{
    let mut ret = "".to_string();
    let row_line ="------------------------------------------------------------------------------ \n".to_string();
    unsafe {
        let mut input_buff = input.clone();
        let c_input = CString::new(input_buff.clone()).unwrap();
        let result_ptr = rag_search(c_input.as_ptr());
        if !result_ptr.is_null() {
            let result_cstr = CStr::from_ptr(result_ptr);
            let result_str = result_cstr.to_str().unwrap();
            let resp = result_str.to_string();
            free_string(result_ptr);
            ret = resp.clone();
        }
    } 
    return ret;
}
/// 発言者の種類。表示色やラベルを切り替えるために使う。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    System,
    User,
    Ai,
}

impl Role {
    fn label(&self) -> &'static str {
        match self {
            Role::System => "Welcome",
            Role::User => "You:",
            Role::Ai => "AI:",
        }
    }

    fn color(&self) -> Color {
        match self {
            Role::System => Color::Cyan,
            Role::User => Color::Blue,
            Role::Ai => Color::Green,
        }
    }
}

/// 1件のメッセージ（複数行可）。
struct ChatMessage {
    role: Role,
    lines: Vec<String>,
}

impl ChatMessage {
    fn new(role: Role, text: &str) -> Self {
        Self {
            role,
            lines: text.lines().map(|l| l.to_string()).collect(),
        }
    }

    /// 枠線込みで必要な行数（高さ）。
    fn height(&self) -> u16 {
        // 上下ボーダー(2) + 本文行数（最低1行）
        2 + self.lines.len().max(1) as u16
    }
}

/// アプリ全体の状態。
struct App {
    messages: Vec<ChatMessage>,
    input: String,
    /// 直近から何メッセージ分スクロールして遡っているか（0 = 最新が見える状態）。
    scroll_offset: usize,
    should_quit: bool,
    /// 実行中（AI応答待ち）かどうか。
    thinking: bool,
    /// 実行開始時刻。5秒経過で応答を表示する。
    thinking_since: Option<Instant>,
    /// 実行完了時に表示するユーザー発話の控え。
    pending_text: Option<String>,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            messages: Vec::new(),
            input: String::new(),
            scroll_offset: 0,
            should_quit: false,
            thinking: false,
            thinking_since: None,
            pending_text: None,
        };
        app.messages.push(ChatMessage::new(
            Role::System,
            "Welcome\nChat app Example\n\nEnd: Ctrl + c",
        ));
        app
    }

    fn submit_message(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }
        
        self.messages.push(ChatMessage::new(Role::User, &text));

        // 即時応答せず、実行中メッセージを表示してから応答を出す。
        self.pending_text = Some(text);
        self.thinking = true;
        self.thinking_since = Some(Instant::now());

        self.input.clear();
        self.scroll_offset = 0; // 新規メッセージ送信時は最下部に戻る
    }

    /// 実行中状態を更新し、5秒経過でダミー応答を追加する。
    fn tick(&mut self) {
        if !self.thinking {
            return;
        }
        if let Some(since) = self.thinking_since {
            // ここはダミー応答。実際にはAI API呼び出しの結果に差し替える。
            if since.elapsed() >= Duration::from_secs(1) {
                let text = self.pending_text.take().unwrap_or_default();
                let reply = format!("「{}」を受け取りました。", text);
                let resp = chat_handler(text.clone());
           
                self.messages.push(ChatMessage::new(Role::Ai, &resp));
                self.thinking = false;
                self.thinking_since = None;
            }
        }
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset + 1 < self.messages.len() {
            self.scroll_offset += 1;
        }
    }

    fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.should_quit = true;
                    }
                    KeyCode::Enter => app.submit_message(),
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Up => app.scroll_up(),
                    KeyCode::Down => app.scroll_down(),
                    KeyCode::Char(c) => app.input.push(c),
                    _ => {}
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }

        app.tick();
    }
}

fn ui(f: &mut Frame, app: &App) {
    // 上段: メッセージ一覧 / 中段: 実行中メッセージ（実行中のみ）/ 下段: テキスト入力
    let chunks = if app.thinking {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(3),
                Constraint::Length(5),
            ])
            .split(f.size())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(5)])
            .split(f.size())
    };

    if app.thinking {
        render_messages(f, chunks[0], app);
        render_thinking(f, chunks[1], app);
        render_input(f, chunks[2], app);
    } else {
        render_messages(f, chunks[0], app);
        render_input(f, chunks[1], app);
    }
}

/// メッセージ一覧を、最新が下に来るように積み上げて描画する。
fn render_messages(f: &mut Frame, area: Rect, app: &App) {
    let available_height = area.height;

    // scroll_offset だけ末尾から遡った位置を「一番下」として、
    // そこから上方向に収まるだけメッセージを積み上げる。
    let anchor = app.messages.len().saturating_sub(app.scroll_offset);
    let mut shown: Vec<&ChatMessage> = Vec::new();
    let mut used_height: u16 = 0;

    for msg in app.messages[..anchor].iter().rev() {
        let h = msg.height();
        if used_height + h > available_height && !shown.is_empty() {
            break;
        }
        used_height += h;
        shown.push(msg);
    }
    shown.reverse();

    // 上に余白を入れてチャットを画面下端に寄せる。
    let filler = available_height.saturating_sub(used_height);
    let mut constraints: Vec<Constraint> = Vec::new();
    if filler > 0 {
        constraints.push(Constraint::Length(filler));
    }
    for msg in &shown {
        constraints.push(Constraint::Length(msg.height()));
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let offset = if filler > 0 { 1 } else { 0 };
    for (i, msg) in shown.iter().enumerate() {
        render_message_box(f, rows[i + offset], msg);
    }
}

fn render_message_box(f: &mut Frame, area: Rect, msg: &ChatMessage) {
    let title = Span::styled(
        msg.role.label(),
        Style::default()
            .fg(msg.role.color())
            .add_modifier(Modifier::BOLD),
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(title);

    let lines: Vec<Line> = msg.lines.iter().map(|l| Line::raw(l.clone())).collect();
    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

/// テキスト入力の上に配置する「実行中」表示。
fn render_thinking(f: &mut Frame, area: Rect, app: &App) {
    let remaining = match app.thinking_since {
        Some(since) => {
            let elapsed = since.elapsed().as_secs();
            1u64.saturating_sub(elapsed)
        }
        None => 0,
    };
    let label = Span::styled(
        "Status",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(label);

    //let text = format!("Please wait.... あと約{}秒", remaining);
    let text = "Please wait...".to_string();
    let line = Line::from(Span::styled(text, Style::default().fg(Color::Yellow)));

    let paragraph = Paragraph::new(vec![line])
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

/// 下段の入力欄。1行目: ラベル+入力中テキスト+カーソル、2行目: ヒント。
fn render_input(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            "",
            Style::default(),
        ));

    let input_line = Line::from(vec![
        Span::styled(
            "Input: ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(app.input.clone()),
        Span::styled("▏", Style::default().fg(Color::White)),
    ]);
    let hint_line = Line::from(Span::styled(
        "Type your text and press Enter:",
        Style::default().fg(Color::DarkGray),
    ));

    let paragraph = Paragraph::new(vec![input_line, hint_line])
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}
