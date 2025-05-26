use crate::input::handle_key_event;
use crate::{buffer::RopeTextBuffer, highlight::Highlighter};
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use std::io;
use std::path::PathBuf;
use syntect::highlighting::Style as SyntectStyle;
use xi_rope::Rope;
pub enum EditorAction {
    Exit,
    Save,
    SaveExit,
    ChangeMode(bool),
    MoveCursor((i16, i16)),
    MoveWord(i16),
    LineStart,
    LineEnd,
    PageUp,
    PageDown,
    StartCommand,
    InsertChar(char),
    DeleteChar,
    Backspace,
    Tab,
}

pub struct CimEditor {
    pub buffer: RopeTextBuffer,
    pub file_path: Option<PathBuf>,
    pub mode: EditorMode,
    pub command_buffer: String,
    pub highlighter: Highlighter,
    pub cursor_position: (u16, u16),
    pub scroll_offset: usize,
    pub horizontal_offset: usize,
    pub viewport_height: usize,
    pub viewport_width: usize,
    pub text_changed: bool,
    pub last_key_event: Option<KeyEvent>,
    pub line_numbers: String,
    pub highlighted_lines: Vec<Vec<(SyntectStyle, String)>>,
}

#[derive(PartialEq)]
pub enum EditorMode {
    Normal,
    Insert,
    Command,
}

impl CimEditor {
    pub fn new(file_path: Option<PathBuf>) -> io::Result<Self> {
        let buffer = if let Some(ref path) = file_path {
            let content = std::fs::read_to_string(path)?;
            RopeTextBuffer::new(Rope::from(content))
        } else {
            RopeTextBuffer::new(Rope::from(""))
        };

        let mut highlighter = Highlighter::new();
        highlighter.set_syntax_for_file(file_path.as_deref());

        let line_count = buffer.rope().measure::<xi_rope::LinesMetric>().max(1);
        let lines: Vec<String> = (1..=line_count).map(|n| format!("{:3}", n)).collect();
        let line_numbers = lines.join("\n");

        let highlighted_lines = highlighter.highlight(buffer.rope());

        Ok(Self {
            buffer,
            file_path,
            mode: EditorMode::Normal,
            command_buffer: String::new(),
            highlighter,
            cursor_position: (0, 0),
            scroll_offset: 0,
            horizontal_offset: 0,
            viewport_height: 0,
            viewport_width: 0,
            text_changed: true,
            last_key_event: None,
            line_numbers,
            highlighted_lines,
        })
    }
    pub fn update_line_numbers(&mut self) {
        let new_line_count = self.buffer.rope().measure::<xi_rope::LinesMetric>();
        let current_lines = self.line_numbers.lines().count();
        
        if new_line_count == current_lines {
            return;
        }
    
        self.line_numbers = if new_line_count > current_lines {
            let mut new_lines = self.line_numbers.clone();
            for n in (current_lines + 1)..=new_line_count {
                new_lines.push_str(&format!("\n{:4}", n));
            }
            new_lines
        } else {
            (1..=new_line_count)
                .map(|n| format!("{:4}", n))
                .collect::<Vec<_>>()
                .join("\n")
        };
    }
    pub fn update_after_edit(&mut self) {
        self.text_changed = true;
        self.normalize_cursor();
        self.update_viewport();
    }
    pub fn move_cursor_word(&mut self, direction: i16) {
        if direction == 0 {
            return;
        }

        let (x, y) = self.cursor_position;
        let line_count = self.buffer.rope().measure::<xi_rope::LinesMetric>();

        if line_count == 0 {
            return;
        }

        let current_line_idx = y as usize;
        if current_line_idx >= line_count {
            return;
        }

        let line_start = self.buffer.rope().offset_of_line(current_line_idx);
        let line_end = self.buffer.rope().offset_of_line(current_line_idx + 1);
        let line = self.buffer.rope().slice(line_start..line_end).to_string();

        let line = line.trim_end_matches(&['\r', '\n'][..]);

        if line.is_empty() {
            return;
        }

        let chars: Vec<char> = line.chars().collect();
        let current_x = x as usize;

        if current_x >= chars.len() {
            return;
        }

        if direction > 0 {
            let mut new_x = current_x;

            if chars[new_x].is_alphanumeric() || chars[new_x] == '_' {
                while new_x < chars.len() && (chars[new_x].is_alphanumeric() || chars[new_x] == '_')
                {
                    new_x += 1;
                }
            }

            while new_x < chars.len() && !(chars[new_x].is_alphanumeric() || chars[new_x] == '_') {
                new_x += 1;
            }

            self.cursor_position = (new_x as u16, y);
        } else {
            if current_x == 0 {
                return;
            }

            let mut new_x = current_x - 1;

            while new_x > 0 && !(chars[new_x].is_alphanumeric() || chars[new_x] == '_') {
                new_x -= 1;
            }

            while new_x > 0 && (chars[new_x - 1].is_alphanumeric() || chars[new_x - 1] == '_') {
                new_x -= 1;
            }

            self.cursor_position = (new_x as u16, y);
        }

        self.update_viewport();
    }

    pub fn normalize_cursor(&mut self) {
        let (x, y) = self.cursor_position;
        let line_count = self.buffer.rope().measure::<xi_rope::LinesMetric>();

        let new_y = if line_count == 0 {
            0
        } else {
            y.min(line_count as u16 - 1)
        };

        let new_x = if new_y != y {
            0
        } else if line_count > 0 {
            let line_start = self.buffer.rope().offset_of_line(new_y as usize);
            let line_end = self.buffer.rope().offset_of_line(new_y as usize + 1);
            let line = self.buffer.rope().slice(line_start..line_end).to_string();
            let line_length = line.trim_end_matches(&['\r', '\n'][..]).chars().count();

            if self.mode == EditorMode::Normal && line_length > 0 {
                x.min(line_length as u16 - 1)
            } else {
                x.min(line_length as u16)
            }
        } else {
            0
        };

        self.cursor_position = (new_x, new_y);
        self.update_viewport();
    }
    pub fn save(&mut self) -> io::Result<()> {
        if let Some(path) = &self.file_path {
            self.buffer.save_to_file(path)?;
            self.text_changed = false;
        }
        Ok(())
    }

    pub fn move_cursor(&mut self, direction: (i16, i16)) {
        let (mut x, mut y) = self.cursor_position;
        let total_lines = self.buffer.rope().measure::<xi_rope::LinesMetric>();
        let max_y = total_lines.saturating_sub(1) as u16;

        y = y.saturating_add_signed(direction.1).min(max_y);

        let line_start = self.buffer.rope().offset_of_line(y as usize);
        let next_line_start = self.buffer.rope().offset_of_line(y as usize + 1);
        let is_last_line = (y as usize) == (total_lines - 1);

        let line_slice = if is_last_line {
            self.buffer.rope().slice(line_start..next_line_start)
        } else {
            self.buffer.rope().slice(line_start..next_line_start - 1)
        };
        let line_char_len = line_slice.to_string().chars().count() as u16;

        let line = self
            .buffer
            .rope()
            .slice(line_start..next_line_start)
            .to_string();
        let max_x = line.chars().count().saturating_sub(1) as u16;

        x = x.saturating_add_signed(direction.0).min(max_x);

        self.cursor_position = (x, y);
        self.update_viewport();
    }

    pub fn update_viewport(&mut self) {
        let (x, y) = self.cursor_position;
        let line_count = self.buffer.rope().measure::<xi_rope::LinesMetric>();
        
        // Vertical scrolling
        let margin = 2.min(self.viewport_height / 4);
        self.scroll_offset = match y {
            y if y < (self.scroll_offset + margin) as u16 => 
                y.saturating_sub(margin as u16) as usize,
            y if y >= (self.scroll_offset + self.viewport_height - margin) as u16 => 
                (y as usize + margin).saturating_sub(self.viewport_height),
            _ => self.scroll_offset
        }.min(line_count.saturating_sub(self.viewport_height));
    
        // Horizontal scrolling
        let h_margin = 5.min(self.viewport_width / 4);
        self.horizontal_offset = match x {
            x if x < (self.horizontal_offset + h_margin) as u16 => 
                x.saturating_sub(h_margin as u16) as usize,
            x if x >= (self.horizontal_offset + self.viewport_width - h_margin) as u16 => 
                (x as usize + h_margin).saturating_sub(self.viewport_width),
            _ => self.horizontal_offset
        };
    }


    pub fn handle_input(&mut self, key: KeyEvent) -> Option<EditorAction> {
        match self.mode {
            EditorMode::Normal | EditorMode::Command => {
                if let Some(action) = handle_key_event(key) {
                    self.handle_action(action)
                } else {
                    None
                }
            }
            EditorMode::Insert => match key {
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => {
                    self.mode = EditorMode::Normal;
                    Some(EditorAction::ChangeMode(false))
                }
                KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                } => {
                    self.delete_char();
                    Some(EditorAction::Backspace)
                }
                KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                    ..
                } => {
                    self.insert_char(c);
                    Some(EditorAction::InsertChar(c))
                }
                KeyEvent {
                    code: KeyCode::Tab,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    self.insert_tab();
                    Some(EditorAction::Tab)
                }
                KeyEvent {
                    code: KeyCode::Up,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    self.move_cursor((0, -1));
                    Some(EditorAction::MoveCursor((0, -1)))
                }

                KeyEvent {
                    code: KeyCode::Down,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    self.move_cursor((0, 1));
                    Some(EditorAction::MoveCursor((0, 1)))
                }

                KeyEvent {
                    code: KeyCode::Left,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    self.move_cursor((-1, 0));
                    Some(EditorAction::MoveCursor((-1, 0)))
                }

                KeyEvent {
                    code: KeyCode::Right,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    self.move_cursor((1, 0));
                    Some(EditorAction::MoveCursor((1, 0)))
                }
                _ => None,
            },
        }
    }

    pub fn change_mode(&mut self, insert_mode: bool) {
        self.mode = if insert_mode {
            EditorMode::Insert
        } else {
            EditorMode::Normal
        };
    }

    pub fn insert_char(&mut self, c: char) {
        let (x, y) = self.cursor_position;

        if c == '\n' {
            let line_start = self.buffer.rope().offset_of_line(y as usize);
            let line_end = self.buffer.rope().offset_of_line(y as usize + 1);
            let line = self.buffer.rope().slice(line_start..line_end).to_string();

            let byte_pos = if x as usize >= line.chars().count() {
                line.len()
            } else {
                line.char_indices()
                    .nth(x as usize)
                    .map_or(line.len(), |(idx, _)| idx)
            };

            let insert_pos = line_start + byte_pos;
            self.buffer.rope_mut().edit(insert_pos..insert_pos, "\n");

            self.cursor_position = (0, y + 1);
            self.buffer.set_modified(true);
            self.update_after_edit();
            return;
        }

        let line_start = self.buffer.rope().offset_of_line(y as usize);
        let line_end = self.buffer.rope().offset_of_line(y as usize + 1);
        let line = self.buffer.rope().slice(line_start..line_end).to_string();

        let byte_pos = if x as usize >= line.chars().count() {
            line.len()
        } else {
            line.char_indices()
                .nth(x as usize)
                .map_or(line.len(), |(idx, _)| idx)
        };

        let insert_pos = line_start + byte_pos;
        self.buffer.insert_char(insert_pos, c);

        self.cursor_position.0 += 1;
        self.update_after_edit();
    }

    pub fn delete_char(&mut self) {
        let (x, y) = self.cursor_position;

        if x > 0 {
            let line_start = self.buffer.rope().offset_of_line(y as usize);
            let line = self
                .buffer
                .rope()
                .slice(line_start..self.buffer.rope().offset_of_line(y as usize + 1))
                .to_string();

            let byte_pos = line
                .char_indices()
                .nth(x as usize - 1)
                .map_or(0, |(idx, _)| idx);

            let delete_pos = line_start + byte_pos;

            let char_to_delete = line.chars().nth(x as usize - 1).unwrap_or(' ');
            let char_len = char_to_delete.len_utf8();

            self.buffer
                .rope_mut()
                .edit(delete_pos..(delete_pos + char_len), "");

            self.cursor_position.0 -= 1;
            self.buffer.set_modified(true);
            self.update_after_edit();
        } else if y > 0 {
            let current_line_start = self.buffer.rope().offset_of_line(y as usize);
            let prev_line_start = self.buffer.rope().offset_of_line(y as usize - 1);
            let prev_line_end = current_line_start - 1;

            let prev_line = self
                .buffer
                .rope()
                .slice(prev_line_start..prev_line_end)
                .to_string();
            let prev_line_len = prev_line.chars().count() as u16;

            self.buffer
                .rope_mut()
                .edit(prev_line_end..current_line_start, "");

            self.cursor_position = (prev_line_len, y - 1);
            self.buffer.set_modified(true);
            self.update_after_edit();
        }
    }
    pub fn insert_tab(&mut self) {
        for _ in 0..4 {
            self.insert_char(' ');
        }
    }

    pub fn page_up(&mut self) {
        let scroll_amount = self.viewport_height.min(self.cursor_position.1 as usize);
        self.cursor_position.1 -= scroll_amount as u16;

        if self.scroll_offset > scroll_amount {
            self.scroll_offset -= scroll_amount;
        } else {
            self.scroll_offset = 0;
        }

        self.normalize_cursor();
    }

    pub fn page_down(&mut self) {
        let max_line = self
            .buffer
            .rope()
            .measure::<xi_rope::LinesMetric>()
            .saturating_sub(1) as u16;

        let new_y = (self.cursor_position.1 + self.viewport_height as u16).min(max_line);
        let moved_amount = new_y - self.cursor_position.1;
        self.cursor_position.1 = new_y;

        self.scroll_offset += moved_amount as usize;

        self.normalize_cursor();
    }

    pub fn go_to_line_start(&mut self) {
        self.cursor_position.0 = 0;
        self.update_viewport();
    }

    pub fn go_to_line_end(&mut self) {
        let y = self.cursor_position.1 as usize;
        let line_start = self.buffer.rope().offset_of_line(y);
        let next_line_start = self.buffer.rope().offset_of_line(y + 1);
        let line = self
            .buffer
            .rope()
            .slice(line_start..next_line_start)
            .to_string();

        let line_len = line.trim_end_matches(&['\r', '\n'][..]).chars().count() as u16;
        self.cursor_position.0 = if self.mode == EditorMode::Insert {
            line_len
        } else {
            line_len.saturating_sub(1)
        };

        self.update_viewport();
    }
    pub fn current_line_length(&self) -> usize {
        let rope = self.buffer.rope();
        if rope.is_empty() {
            return 0;
        }
        let line_num = self.cursor_position.1 as usize;
        if line_num >= rope.measure::<xi_rope::LinesMetric>() {
            return 0;
        }
        let line_start = rope.offset_of_line(line_num);
        let line_end = rope.offset_of_line(line_num + 1);
        rope.slice(line_start..line_end)
            .to_string()
            .trim_end_matches(&['\r', '\n'][..])
            .chars()
            .count()
    }

    pub fn total_lines(&self) -> usize {
        self.buffer.rope().measure::<xi_rope::LinesMetric>()
    }
    fn handle_action(&mut self, action: EditorAction) -> Option<EditorAction> {
        match action {
            EditorAction::ChangeMode(b) => {
                self.change_mode(b);
                Some(action)
            }
            EditorAction::MoveCursor(dir) => {
                self.move_cursor(dir);
                None
            }
            EditorAction::MoveWord(dir) => {
                self.move_cursor_word(dir);
                None
            }
            EditorAction::LineStart => {
                self.go_to_line_start();
                None
            }
            EditorAction::LineEnd => {
                self.go_to_line_end();
                None
            }
            EditorAction::PageUp => {
                self.page_up();
                None
            }
            EditorAction::PageDown => {
                self.page_down();
                None
            }
            EditorAction::InsertChar(c) => {
                if self.mode == EditorMode::Insert {
                    self.insert_char(c);
                }
                None
            }

            EditorAction::DeleteChar => {
                self.delete_char();
                None
            }
            EditorAction::Save => {
                self.save().ok()?;
                None
            }
            EditorAction::SaveExit => {
                self.save().ok()?;
                Some(EditorAction::Exit)
            }
            EditorAction::StartCommand => {
                self.mode = EditorMode::Command;
                self.command_buffer.clear();
                Some(action)
            }
            _ => Some(action),
        }
    }
}
