use clap::Parser;
use std::{fs, io, path::PathBuf};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ropey::Rope;
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, ThemeSet},
    parsing::{ParseState, SyntaxSet, SyntaxReference},
    util::LinesWithEndings,
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};

#[derive(Parser)]
#[command(version = "0.1", about = "A fast C++ editor like Neovim")]
struct RopeTextBuffer {
    rope: Rope,
    string: String,
    modified: bool,
}

impl RopeTextBuffer {
    fn new(rope: Rope) -> Self {
        let string = rope.to_string();
        Self {
            rope,
            string,
            modified: false,
        }
    }

    fn sync_to_rope(&mut self) {
        if self.modified {
            self.rope = Rope::from_str(&self.string);
            self.modified = false;
        }
    }

    fn sync_from_rope(&mut self) {
        self.string = self.rope.to_string();
    }
}

struct CimApp {
    text_buffer: RopeTextBuffer,
    file_path: Option<PathBuf>,
    insert_mode: bool,
    command_buffer: String,
    use_tabs: bool,
    line_numbers: String,
    text_changed: bool,
    syntax_set: SyntaxSet,
    highlighter: HighlightLines<'static>,
    syntax: SyntaxReference,
    cursor_position: (u16, u16),
    scroll_offset: usize, 
    horizontal_offset: usize, 
    viewport_height: usize,
    viewport_width: usize,
    highlighted_lines: Vec<Vec<(SyntectStyle, String)>>,
    last_key_event: Option<KeyEvent>,
}

impl CimApp {
    fn new(file_path: Option<PathBuf>) -> Self {
        let text = if let Some(ref path) = file_path {
            fs::File::open(path)
                .and_then(|f| Rope::from_reader(f))
                .unwrap_or_else(|_| Rope::from_str("Could not open file."))
        } else {
            Rope::new()
        };

        let text_buffer = RopeTextBuffer::new(text.clone());
        let line_count = text.lines().count().max(1);
        let mut line_numbers = String::with_capacity(line_count * 4);

        for n in 1..=line_count {
            line_numbers.push_str(&format!("{:3}\n", n));
        }

        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let syntax = syntax_set
            .find_syntax_by_extension("cpp")
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
            .clone();
        
        let theme = Box::leak(Box::new(theme_set.themes["base16-ocean.dark"].clone()));
        let highlighter = HighlightLines::new(&syntax, theme);

        Self {
            text_buffer,
            file_path,
            insert_mode: false,
            command_buffer: String::new(),
            use_tabs: false,
            line_numbers,
            text_changed: true,
            syntax_set,
            highlighter,
            syntax,
            cursor_position: (0, 0),
            scroll_offset: 0,
            horizontal_offset: 0,
            viewport_height: 0,
            viewport_width: 0,
            highlighted_lines: Vec::new(),
            last_key_event: None,
        }
    }

    fn update_line_numbers(&mut self) {
        if self.text_changed {
            self.text_buffer.sync_to_rope();
            let new_line_count = self.text_buffer.rope.len_lines();
            let current_line_count = self.line_numbers.lines().count();

            if new_line_count != current_line_count {
                let num_width = (new_line_count as f64).log10().ceil().max(3.0) as usize;
                self.line_numbers = (1..=new_line_count)
                    .map(|n| format!("{:width$}\n", n, width = num_width))
                    .collect();
            }
        }
    }

    fn update_highlighting(&mut self) {
        self.highlighted_lines.clear();
        for line in LinesWithEndings::from(&self.text_buffer.string) {
            if let Ok(highlighted) = self.highlighter.highlight_line(line, &self.syntax_set) {
                self.highlighted_lines.push(
                    highlighted
                        .into_iter()
                        .map(|(style, text)| (style, text.to_string()))
                        .collect(),
                );
            }
        }
    }

    fn save_file(&mut self) -> io::Result<()> {
        self.text_buffer.sync_to_rope();
        if let Some(path) = &self.file_path {
            let mut file = fs::File::create(path)?;
            self.text_buffer.rope.write_to(&mut file)?;
        }
        Ok(())
    }

    fn move_cursor(&mut self, direction: (i16, i16)) {
        let (mut x, mut y) = self.cursor_position;
        let max_y = self.text_buffer.rope.len_lines().saturating_sub(1) as u16;
        
        y = y.saturating_add_signed(direction.1).min(max_y);
        let line_len = self.text_buffer.rope.line(y as usize).len_chars() as u16;
        x = x.saturating_add_signed(direction.0).min(line_len);
        
        self.cursor_position = (x, y);
        
        if y < self.scroll_offset as u16 {
            self.scroll_offset = y as usize;
        } else if y >= (self.scroll_offset + self.viewport_height) as u16 {
            self.scroll_offset = (y as usize).saturating_sub(self.viewport_height - 1);
        }
        
        let line_display_width = self.viewport_width.saturating_sub(5); 
        if x < self.horizontal_offset as u16 {
            self.horizontal_offset = x as usize;
        } else if x >= (self.horizontal_offset + line_display_width) as u16 {
            self.horizontal_offset = (x as usize).saturating_sub(line_display_width - 1);
        }
    }

    fn get_visible_text(&self) -> String {
        let start_line = self.scroll_offset;
        let end_line = (self.scroll_offset + self.viewport_height).min(self.text_buffer.rope.len_lines());
        
        let mut visible_text = String::new();
        for line_num in start_line..end_line {
            let line = self.text_buffer.rope.line(line_num);
            let line_str = line.to_string();
            
            let h_offset = self.horizontal_offset.min(line_str.len());
            let visible_line = if h_offset < line_str.len() {
                &line_str[h_offset..]
            } else {
                ""
            };
            
            visible_text.push_str(visible_line);
            if !visible_line.ends_with('\n') && line_num < self.text_buffer.rope.len_lines() - 1 {
                visible_text.push('\n');
            }
        }
        visible_text
    }
    
    fn is_duplicate_key_event(&self, key: &KeyEvent) -> bool {
        if let Some(last_key) = &self.last_key_event {
            return last_key.code == key.code && 
                  last_key.modifiers == key.modifiers;
        }
        false
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let args: Vec<String> = std::env::args().collect();
    let file_path = args.get(1).map(|p| PathBuf::from(p));
    let mut app = CimApp::new(file_path);

    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    res
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut CimApp) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if app.insert_mode && app.is_duplicate_key_event(&key) {
                app.last_key_event = None;
                continue;
            }
            
            app.last_key_event = Some(key);
            
            match key {
                KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => return Ok(()),
            
                KeyEvent {
                    code: KeyCode::Char('q'),
                    modifiers: KeyModifiers::NONE,
                    ..
                } if !app.insert_mode => return Ok(()),
            
                KeyEvent {
                    code: KeyCode::Char('i'),
                    modifiers: KeyModifiers::NONE,
                    ..
                } if !app.insert_mode => {
                    app.insert_mode = true;
                }
            
                KeyEvent {
                    code: KeyCode::Esc,
                    modifiers: KeyModifiers::NONE,
                    ..
                } if app.insert_mode => {
                    app.insert_mode = false;
                }
            
                KeyEvent {
                    code: KeyCode::Char(':'),
                    modifiers: KeyModifiers::NONE,
                    ..
                } if !app.insert_mode => {
                    app.command_buffer.push(':');
                }
            
                KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                    ..
                } if !app.command_buffer.is_empty() => {
                    match app.command_buffer.as_str() {
                        ":w" => app.save_file()?,
                        ":q" => return Ok(()),
                        ":wq" => {
                            app.save_file()?;
                            return Ok(());
                        }
                        _ => {}
                    }
                    app.command_buffer.clear();
                }
            
                KeyEvent {
                    code: KeyCode::Backspace,
                    modifiers: KeyModifiers::NONE,
                    ..
                } if !app.command_buffer.is_empty() => {
                    app.command_buffer.pop();
                }
            
                KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    if app.insert_mode {
                        let (x, y) = app.cursor_position;
                        let pos = app.text_buffer.rope.line_to_char(y as usize) + x as usize;
                        app.text_buffer.string.insert(pos, c);
                        app.text_buffer.modified = true;
                        app.text_changed = true;
                        app.move_cursor((1, 0));
                    } else if !app.command_buffer.is_empty() {
                        app.command_buffer.push(c);
                    }
                }
            
                KeyEvent {
                    code: KeyCode::Up,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => app.move_cursor((0, -1)),
            
                KeyEvent {
                    code: KeyCode::Down,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => app.move_cursor((0, 1)),
            
                KeyEvent {
                    code: KeyCode::Left,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => app.move_cursor((-1, 0)),
            
                KeyEvent {
                    code: KeyCode::Right,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => app.move_cursor((1, 0)),
            
                KeyEvent {
                    code: KeyCode::PageUp,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    let jump = app.viewport_height.saturating_sub(1) as i16;
                    app.move_cursor((0, -jump));
                },
            
                KeyEvent {
                    code: KeyCode::PageDown,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    let jump = app.viewport_height.saturating_sub(1) as i16;
                    app.move_cursor((0, jump));
                },
            
                KeyEvent {
                    code: KeyCode::Home,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    app.cursor_position.0 = 0;
                    app.horizontal_offset = 0;
                },
            
                KeyEvent {
                    code: KeyCode::End,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    let line_len = app.text_buffer.rope.line(app.cursor_position.1 as usize).len_chars() as u16;
                    app.cursor_position.0 = line_len;
                    
                    if line_len > app.viewport_width.saturating_sub(5) as u16 {
                        app.horizontal_offset = line_len.saturating_sub(app.viewport_width as u16) as usize;
                    }
                }
            
                KeyEvent {
                    code: KeyCode::Backspace,
                    modifiers: KeyModifiers::NONE,
                    ..
                } if app.insert_mode => {
                    let (x, y) = app.cursor_position;
                    if x > 0 {
                        let pos = app.text_buffer.rope.line_to_char(y as usize) + x as usize - 1;
                        app.text_buffer.string.remove(pos);
                        app.text_buffer.modified = true;
                        app.text_changed = true;
                        app.move_cursor((-1, 0));
                    } else if y > 0 {
                        
                        let prev_line_len = app.text_buffer.rope.line(y as usize - 1).len_chars();
                        let pos = app.text_buffer.rope.line_to_char(y as usize - 1) + prev_line_len;
                        app.text_buffer.string.remove(pos);
                        app.text_buffer.modified = true;
                        app.text_changed = true;
                        app.cursor_position = (prev_line_len as u16, y - 1);
                    }
                }
            
                KeyEvent {
                    code: KeyCode::Delete,
                    modifiers: KeyModifiers::NONE,
                    ..
                } if app.insert_mode => {
                    let (x, y) = app.cursor_position;
                    let pos = app.text_buffer.rope.line_to_char(y as usize) + x as usize;
                    if pos < app.text_buffer.string.len() {
                        app.text_buffer.string.remove(pos);
                        app.text_buffer.modified = true;
                        app.text_changed = true;
                    }
                }
            
                KeyEvent {
                    code: KeyCode::Tab,
                    modifiers: KeyModifiers::NONE,
                    ..
                } if app.insert_mode => {
                    let (x, y) = app.cursor_position;
                    let pos = app.text_buffer.rope.line_to_char(y as usize) + x as usize;
                    let spaces = if app.use_tabs { "\t" } else { "    " };
                    app.text_buffer.string.insert_str(pos, spaces);
                    app.text_buffer.modified = true;
                    app.text_changed = true;
                    app.move_cursor((spaces.len() as i16, 0));
                }
            
                KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                    ..
                } if app.insert_mode => {
                    let (x, y) = app.cursor_position;
                    let pos = app.text_buffer.rope.line_to_char(y as usize) + x as usize;
                    app.text_buffer.string.insert(pos, '\n');
                    app.text_buffer.modified = true;
                    app.text_changed = true;
                    app.cursor_position = (0, y + 1);
                    app.horizontal_offset = 0; 
                }
                
                KeyEvent {
                    code: KeyCode::Left,
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    app.horizontal_offset = app.horizontal_offset.saturating_sub(10);
                    if app.cursor_position.0 as usize > app.horizontal_offset {
                        app.cursor_position.0 = app.horizontal_offset as u16;
                    }
                }
                
                KeyEvent {
                    code: KeyCode::Right,
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    app.horizontal_offset = app.horizontal_offset.saturating_add(10);
                    let line_len = app.text_buffer.rope.line(app.cursor_position.1 as usize).len_chars();
                    if (app.cursor_position.0 as usize) < app.horizontal_offset && app.horizontal_offset < line_len {
                        app.cursor_position.0 = app.horizontal_offset as u16;
                    }
                }
            
                _ => {}
            }
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut CimApp) {
    if app.text_changed {
        app.update_line_numbers();
        app.update_highlighting();
        app.text_changed = false;
    }

    let size = f.size();
    app.viewport_height = size.height as usize - 2; 
    app.viewport_width = size.width as usize;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(size);

    let mode = if app.insert_mode { "INSERT" } else { "NORMAL" };
    let filename = app
        .file_path
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("[No Name]");
    
    let (x, y) = app.cursor_position;
    let status = Paragraph::new(Spans::from(vec![
        Span::styled(
            format!(" {} | {} | Ln {}, Col {} | Scroll V:{} H:{} ", 
                    mode, filename, y + 1, x + 1, app.scroll_offset, app.horizontal_offset),
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(Block::default());
    f.render_widget(status, chunks[0]);

    let editor_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(4), Constraint::Min(1)].as_ref())
        .split(chunks[1]);

    let line_numbers_text = (app.scroll_offset + 1..=app.scroll_offset + app.viewport_height)
        .take(app.text_buffer.rope.len_lines().saturating_sub(app.scroll_offset))
        .map(|n| format!("{:3}\n", n))
        .collect::<String>();
    
    let line_numbers = Paragraph::new(line_numbers_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default());
    f.render_widget(line_numbers, editor_chunks[0]);

    let visible_text = app.get_visible_text();
    let mut text = Text::default();
    
    for (i, line) in visible_text.lines().enumerate() {
        let global_line_num = app.scroll_offset + i;
        if global_line_num < app.highlighted_lines.len() {
            let mut spans = Vec::new();
            let mut char_count = 0;
            for (style, segment) in &app.highlighted_lines[global_line_num] {
                if char_count + segment.chars().count() <= app.horizontal_offset {
                    char_count += segment.chars().count();
                    continue;
                }
                
                if char_count < app.horizontal_offset {
                    let skip_chars = app.horizontal_offset - char_count;
                    let visible_segment: String = segment.chars().skip(skip_chars).collect();
                    if !visible_segment.is_empty() {
                        spans.push(Span::styled(
                            visible_segment,
                            Style::default()
                                .fg(Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b))
                                .bg(Color::Rgb(style.background.r, style.background.g, style.background.b)),
                        ));
                    }
                } else {
                    spans.push(Span::styled(
                        segment,
                        Style::default()
                            .fg(Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b))
                            .bg(Color::Rgb(style.background.r, style.background.g, style.background.b)),
                    ));
                }
                char_count += segment.chars().count();
            }
            
            if !spans.is_empty() {
                text.lines.push(Spans::from(spans));
            } else {
                text.lines.push(Spans::from(""));
            }
        } else {
            text.lines.push(Spans::from(line));
        }
    }

    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(paragraph, editor_chunks[1]);

    if !app.command_buffer.is_empty() || !app.insert_mode {
        let command = Paragraph::new(Spans::from(vec![
            Span::styled(
                format!("Command: {}", app.command_buffer),
                Style::default().fg(Color::Yellow),
            ),
        ]))
        .block(Block::default());
        f.render_widget(command, chunks[2]);
    }

    if app.insert_mode {
        f.set_cursor(
            editor_chunks[1].x + (app.cursor_position.0 as usize).saturating_sub(app.horizontal_offset) as u16,
            editor_chunks[1].y + (app.cursor_position.1 as usize).saturating_sub(app.scroll_offset) as u16,
        );
    }
}