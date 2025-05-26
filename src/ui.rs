use crate::editor::CimEditor;
use crate::editor::EditorMode;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::fs::OpenOptions;
use std::io::Write;
use chrono::Local;

pub fn draw_ui<B: Backend>(f: &mut Frame<B>, app: &mut CimEditor) {
    let size = f.size();
    
    if app.text_changed {
        app.update_line_numbers();
        app.highlighted_lines = app.highlighter.highlight(app.buffer.rope());
        app.text_changed = false;
    }

    let available_height = size.height.saturating_sub(2) as usize;

    app.viewport_height = available_height;
    app.viewport_width = size.width.saturating_sub(5) as usize;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(size);

    let status = build_status_bar(app);
    f.render_widget(status, chunks[0]);

    let editor_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(5), Constraint::Min(1)])
        .split(chunks[1]);

    let line_numbers = render_line_numbers(app);
    f.render_widget(line_numbers, editor_chunks[0]);

    let text = build_highlighted_text(app);
    let paragraph = Paragraph::new(text).block(Block::default().borders(Borders::NONE));
    f.render_widget(paragraph, editor_chunks[1]);

    let command = if app.mode == EditorMode::Command || !app.command_buffer.is_empty() {
        let prefix = if app.mode == EditorMode::Command {
            ":"
        } else {
            ""
        };
        Paragraph::new(format!("{}{}", prefix, app.command_buffer))
            .style(Style::default().fg(Color::Yellow))
    } else {
        Paragraph::new("")
    };
    f.render_widget(command, chunks[2]);

    match app.mode {
        EditorMode::Command => {
            let cmd_x = 1 + app.command_buffer.len() as u16;
            f.set_cursor(cmd_x, chunks[2].y);
        }
        _ => {
            let cursor_x = (app.cursor_position.0 as usize).saturating_sub(app.horizontal_offset);
            let cursor_y = (app.cursor_position.1 as usize).saturating_sub(app.scroll_offset);

            if cursor_x < app.viewport_width && cursor_y < app.viewport_height {
                f.set_cursor(
                    editor_chunks[1].x + cursor_x as u16,
                    editor_chunks[1].y + cursor_y as u16,
                );
            }
        }
    }
}

fn render_line_numbers(app: &CimEditor) -> Paragraph {
    let start_line = app.scroll_offset;
    let end_line =
        (start_line + app.viewport_height).min(app.buffer.rope().measure::<xi_rope::LinesMetric>());

    let line_numbers = (start_line..end_line)
        .map(|line_idx| format!("{:4} ", line_idx + 1))
        .collect::<Vec<String>>()
        .join("\n");

    Paragraph::new(line_numbers)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(tui::layout::Alignment::Right)
}

fn build_status_bar(app: &CimEditor) -> Paragraph {
    let mode = match app.mode {
        EditorMode::Insert => "INSERT",
        EditorMode::Normal => "NORMAL",
        EditorMode::Command => "COMMAND",
    };

    let filename = app
        .file_path
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("[No Name]");

    let modified_indicator = if app.buffer.is_modified() { "[+]" } else { "" };

    let total_lines = app.buffer.rope().measure::<xi_rope::LinesMetric>();
    let position_info = format!(
        "Ln {}/{}, Col {}",
        app.cursor_position.1 + 1,
        total_lines,
        app.cursor_position.0 + 1
    );

    let status_text = format!(
        " {}: {}{} | {} ",
        mode, filename, modified_indicator, position_info
    );

    Paragraph::new(Spans::from(vec![Span::styled(
        status_text,
        Style::default()
            .fg(Color::Black)
            .bg(Color::LightBlue)
            .add_modifier(Modifier::BOLD),
    )]))
}

fn build_highlighted_text(app: &CimEditor) -> Text {
    let mut text = Text::default();
    let rope = app.buffer.rope();
    let start_line = app.scroll_offset;
    let end_line = (start_line + app.viewport_height).min(rope.measure::<xi_rope::LinesMetric>());

    if rope.len() == 0 {
        for _ in 0..app.viewport_height {
            text.lines.push(Spans::from(vec![Span::styled(
                " ".repeat(app.viewport_width),
                Style::default(),
            )]));
        }
        return text;
    }

    for line_num in start_line..end_line {
        if line_num >= rope.measure::<xi_rope::LinesMetric>() {
            text.lines.push(Spans::from(vec![Span::styled(
                " ".repeat(app.viewport_width),
                Style::default(),
            )]));
            continue;
        }

        let line_start = rope.offset_of_line(line_num);
        let line_end = rope.offset_of_line(line_num + 1);
        let line = rope.slice(line_start..line_end).to_string();
        let line_length = line.trim_end_matches(&['\r', '\n'][..]).chars().count();

        // Cap horizontal offset to avoid rendering issues
        let line_with_tabs_expanded = line.replace('\t', "    ");
        let effective_visual_offset = app.horizontal_offset.min(line_with_tabs_expanded.len());
        if app.highlighted_lines.is_empty() || line_num >= app.highlighted_lines.len() {
            let visible_part: String = line_with_tabs_expanded
    .chars()
    .skip(effective_visual_offset)
    .take(app.viewport_width)
    .collect();

            let visible_length = visible_part.chars().count();
            let visible_visual_width = visible_part.chars().count();
let padding = " ".repeat(app.viewport_width.saturating_sub(visible_visual_width));
            text.lines.push(Spans::from(vec![
                Span::styled(visible_part, Style::default()),
                Span::styled(padding, Style::default()),
            ]));
            continue;
        }

        let mut spans = Vec::new();
        let mut current_column = 0;
        let mut visible_width = 0;

        for (style, segment) in &app.highlighted_lines[line_num] {
            let expanded_segment = segment.replace('\t', "    ");
let segment_chars: Vec<char> = expanded_segment.chars().collect();
            let segment_len = segment_chars.len();

            if current_column + segment_len <= effective_visual_offset {
                current_column += segment_len;
                continue;
            }

            let offset_within_segment = if current_column < effective_visual_offset {
                effective_visual_offset - current_column
            } else {
                0
            };

            let chars_to_take = segment_len
                .saturating_sub(offset_within_segment)
                .min(app.viewport_width.saturating_sub(visible_width));

            if chars_to_take == 0 {
                current_column += segment_len;
                continue;
            }

            let visible_text: String = segment_chars
                .iter()
                .skip(offset_within_segment)
                .take(chars_to_take)
                .collect();

            spans.push(Span::styled(
                visible_text,
                Style::default().fg(Color::Rgb(
                    style.foreground.r,
                    style.foreground.g,
                    style.foreground.b,
                )),
            ));

            visible_width += chars_to_take;
            current_column += segment_len;

            if visible_width >= app.viewport_width {
                break;
            }
            
        }

        if visible_width < app.viewport_width {
            spans.push(Span::styled(
                " ".repeat(app.viewport_width - visible_width),
                Style::default(),
            ));
        }

        text.lines.push(Spans::from(spans));
    }

    let lines_added = (end_line - start_line) as usize;
    for _ in lines_added..app.viewport_height {
        text.lines.push(Spans::from(vec![Span::styled(
            " ".repeat(app.viewport_width),
            Style::default(),
        )]));
    }

    text
}