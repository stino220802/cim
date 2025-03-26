use clap::Parser;
use eframe::egui::{self, Stroke, Key, Align2, FontId, TextStyle, Visuals, Style};
use std::{fs, path::PathBuf};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SyntectStyle, ThemeSet};
use syntect::parsing::{SyntaxSet, Scope, ParseState};
use syntect::util::LinesWithEndings;

/// Cim: A C++-focused text editor
#[derive(Parser)]
#[command(version = "0.1", about = "A fast C++ editor like Neovim")]
struct Args {
    path: Option<PathBuf>,
}

struct CimApp {
    text: String,
    file_path: Option<PathBuf>,
    insert_mode: bool,
    command_buffer: String,
    use_tabs: bool,
    line_numbers: String, 
    text_changed: bool,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    highlight_state: ParseState,
    highlighted_lines: Vec<Vec<(SyntectStyle, String)>>,
}

impl CimApp {
    fn new(file_path: Option<PathBuf>) -> Self {
        let text = if let Some(ref path) = file_path {
            fs::read_to_string(path).unwrap_or_else(|_| "Could not open file.".to_string())
        } else {
            "".to_string()
        };
        let line_count = text.lines().count().max(1);
        let line_numbers = (1..=line_count)
            .map(|n| format!("{:3}\n", n))
            .collect::<String>();
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let syntax = syntax_set.find_syntax_by_extension("cpp").unwrap_or_else(|| syntax_set.find_syntax_plain_text());
        let highlight_state = ParseState::new(syntax);

        let mut app = Self {
            text,
            file_path,
            insert_mode: true,
            command_buffer: String::new(),
            use_tabs: false,
            line_numbers,
            text_changed: false,
            syntax_set,
            theme_set,
            highlight_state,
            highlighted_lines: Vec::new(),
        };
        
        app.highlight_text(); 
        app
    }

    fn update_line_numbers(&mut self) {
        if self.text_changed {
            let line_count = self.text.lines().count().max(1);
            self.line_numbers = (1..=line_count)
                .map(|n| format!("{:3}\n", n))
                .collect::<String>();
            self.text_changed = false;
        }
    }
    fn highlight_text(&mut self) {
        let syntax = self.syntax_set.find_syntax_by_extension("cpp")
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        
        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);
        
        self.highlighted_lines.clear();

        for line in LinesWithEndings::from(&self.text) {
            match highlighter.highlight_line(line, &self.syntax_set) {
                Ok(line_highlights) => {
                    let highlighted_line = line_highlights.into_iter()
                        .map(|(style, text)| (style, text.to_string()))
                        .collect::<Vec<_>>();
                    self.highlighted_lines.push(highlighted_line);
                }
                Err(_) => {
                    self.highlighted_lines.push(vec![(
                        SyntectStyle::default(), 
                        line.to_string()
                    )]);
                }
            }
        }
    }
}

impl eframe::App for CimApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.style_mut(|style| {
            style.visuals = egui::Visuals::dark();
            style.visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.style_mut().override_font_id = Some(egui::FontId::monospace(16.0));
            ui.separator();
            ui.separator();

            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.insert_mode = false;
                self.command_buffer.clear();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::I)) {
                self.insert_mode = true;
                ctx.request_repaint();
            }

            let line_number_width = 50.0;
            let text_width = (self.text.len() as f32 * 8.0).max(ui.available_width());

            
            
            if self.text_changed {
                self.highlight_text();
            }
            self.update_line_numbers();
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(line_number_width);
                            ui.style_mut().visuals.widgets.noninteractive.bg_fill = ui.visuals().panel_fill;
                            ui.label(&self.line_numbers);
                        });

                        ui.vertical(|ui| {
                            ui.visuals_mut().selection.stroke.color = egui::Color32::TRANSPARENT;
                            ui.set_width(ui.available_width().max(text_width));

                            let mut layouter = |ui: &egui::Ui, _string: &str, _wrap_width: f32| {
                                let mut job = egui::text::LayoutJob::default();
                                let mut char_index = 0;

                                for line in &self.highlighted_lines {
                                    for (style, text) in line {
                                        let color = egui::Color32::from_rgb(
                                            style.foreground.r,
                                            style.foreground.g,
                                            style.foreground.b,
                                        );
                                        job.append(
                                            text, 
                                            0.0, 
                                            egui::TextFormat::simple(
                                                FontId::monospace(16.0), 
                                                color
                                            )
                                        );
                                        char_index += text.len();
                                    }
                                }
                                ui.fonts(|f| f.layout_job(job))
                            };

                            let mut text_edit = egui::TextEdit::multiline(&mut self.text)
                                .layouter(&mut layouter)
                                .lock_focus(self.insert_mode)
                                .desired_width(text_width)
                                .interactive(true);

                            let response = ui.add_sized([text_width, ui.available_height()], text_edit);

                            if response.changed() {
                                self.text_changed = true;
                            }

                            if response.dragged() {
                                if let Some(mouse_pos) = ctx.input(|i| i.pointer.hover_pos()) {
                                    let ui_rect = ui.clip_rect();
                                    let margin = 30.0;
                                    let max_scroll_speed = 30.0;

                                    let mut scroll_delta = egui::Vec2::ZERO;

                                    let top_dist = (mouse_pos.y - ui_rect.min.y).max(0.0);
                                    if top_dist < margin {
                                        let factor = 1.0 - (top_dist / margin);
                                        scroll_delta.y = max_scroll_speed * factor;
                                    }

                                    let bottom_dist = (ui_rect.max.y - mouse_pos.y).max(0.0);
                                    if bottom_dist < margin {
                                        let factor = 1.0 - (bottom_dist / margin);
                                        scroll_delta.y = -max_scroll_speed * factor;
                                    }

                                    let left_dist = (mouse_pos.x - ui_rect.min.x).max(0.0);
                                    if left_dist < margin {
                                        let factor = 1.0 - (left_dist / margin);
                                        scroll_delta.x = max_scroll_speed * factor;
                                    }

                                    let right_dist = (ui_rect.max.x - mouse_pos.x).max(0.0);
                                    if right_dist < margin {
                                        let factor = 1.0 - (right_dist / margin);
                                        scroll_delta.x = -max_scroll_speed * factor;
                                    }

                                    if scroll_delta != egui::Vec2::ZERO {
                                        ui.scroll_with_delta(scroll_delta);
                                        ctx.request_repaint();
                                    }
                                }

                                if let Some(state) = egui::TextEdit::load_state(ctx, response.id) {
                                    if let Some(cursor_range) = state.cursor.char_range() {
                                        let start = cursor_range.primary.index.min(cursor_range.secondary.index);
                                        let end = cursor_range.primary.index.max(cursor_range.secondary.index);

                                        let lines: Vec<&str> = self.text.lines().collect();
                                        let mut char_count = 0;
                                        let mut start_line = 0;
                                        let mut end_line = 0;

                                        let text_len = self.text.len();
                                        let clamped_start = start.min(text_len);
                                        let clamped_end = end.min(text_len);

                                        for (line_num, line) in lines.iter().enumerate() {
                                            let line_length = line.len() + 1;
                                            if char_count <= clamped_start && clamped_start <= char_count.saturating_add(line_length) {
                                                start_line = line_num;
                                            }
                                            if char_count <= clamped_end && clamped_end <= char_count.saturating_add(line_length) {
                                                end_line = line_num;
                                            }
                                            char_count = char_count.saturating_add(line_length);
                                            if char_count > text_len {
                                                char_count = text_len;
                                                break;
                                            }
                                        }

                                        let line_height = 16.0;
                                        let visible_lines = if end_line >= start_line {
                                            (end_line - start_line + 1) as f32
                                        } else {
                                            1.0
                                        };

                                        let scroll_rect = egui::Rect::from_min_size(
                                            egui::pos2(0.0, (start_line as f32) * line_height),
                                            egui::vec2(text_width, visible_lines * line_height),
                                        );

                                        let current_scroll = ui.clip_rect();
                                        if !current_scroll.contains_rect(scroll_rect) {
                                            ui.scroll_to_rect(scroll_rect, None);
                                        }
                                    }
                                }
                            }

                            if self.insert_mode {
                                ctx.memory_mut(|mem| mem.request_focus(response.id));

                                if response.has_focus() && ctx.input(|i| i.key_pressed(egui::Key::Tab)) {
                                    let spaces = if self.use_tabs { "\t" } else { "    " };

                                    if let Some(mut state) = egui::TextEdit::load_state(ctx, response.id) {
                                        let cursor_pos = state.cursor.char_range().map(|r| r.primary.index).unwrap_or(self.text.len());
                                        self.text.insert_str(cursor_pos, spaces);

                                        let new_cursor = egui::text::CCursor::new(cursor_pos + spaces.len());
                                        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(new_cursor)));
                                        state.store(ctx, response.id);
                                    } else {
                                        self.text.push_str(spaces);
                                    }
                                    self.text_changed = true;
                                    ctx.request_repaint();
                                }
                            } else {
                                for event in ctx.input(|i| i.events.iter().cloned().collect::<Vec<_>>()) {
                                    if let egui::Event::Text(text) = event {
                                        if self.command_buffer.is_empty() && text == ":" {
                                            self.command_buffer.push_str(&text);
                                        } else if !self.command_buffer.is_empty() {
                                            self.command_buffer.push_str(&text);
                                        }
                                    }
                                    if ctx.input(|i| i.key_pressed(egui::Key::Backspace)) {
                                        self.command_buffer.pop();
                                    }
                                }

                                if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    match self.command_buffer.trim() {
                                        ":w" => {
                                            if let Some(ref path) = self.file_path {
                                                if let Err(err) = std::fs::write(path, &self.text) {
                                                    eprintln!("Error saving file: {}", err);
                                                }
                                            }
                                        }
                                        ":q" => {
                                            std::process::exit(0);
                                        }
                                        ":wq" => {
                                            if let Some(ref path) = self.file_path {
                                                if let Err(err) = std::fs::write(path, &self.text) {
                                                    eprintln!("Error saving file: {}", err);
                                                }
                                            }
                                            std::process::exit(0);
                                        }
                                        _ => {}
                                    }
                                    self.command_buffer.clear();
                                }
                            }
                        });
                    });
                });

            let temp = ui.spacing_mut().item_spacing.x;
            ui.add_space(temp);
        });

        egui::TopBottomPanel::bottom("command_bar").show(ctx, |ui| {
            if !self.insert_mode {
                ui.horizontal_centered(|ui| {
                    ui.label(format!("Command: {}", self.command_buffer));
                });
            }
        });

        egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let mode = if self.insert_mode { "INSERT" } else { "NORMAL" };
                let filename = self.file_path.as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("[No Name]");

                ui.label(format!(" {} | File: {} ", mode, filename));
            });
        });

        if ctx.input(|i| i.key_pressed(egui::Key::I)) {
            self.insert_mode = true;
            self.command_buffer.clear();
            ctx.request_repaint();
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    let args = Args::parse();
    let app = CimApp::new(args.path);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_maximized(true),
        ..Default::default()
    };
    eframe::run_native("Cim", options, Box::new(|_| Ok(Box::new(app))))
}