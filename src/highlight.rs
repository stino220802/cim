use std::path::Path;
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
    util::LinesWithEndings,
};

pub struct Highlighter {
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
    pub syntax: SyntaxReference,
    pub current_theme_name: String,
}

impl Highlighter {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();

        let syntax = syntax_set
            .find_syntax_by_extension("cpp")
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
            .clone();

        let current_theme_name = "base16-ocean.dark".to_string();

        Self {
            syntax_set,
            theme_set,
            syntax,
            current_theme_name,
        }
    }

    pub fn set_syntax_for_file(&mut self, file_path: Option<&Path>) {
        
        if let Some(path) = file_path {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if let Some(syntax) = self.syntax_set.find_syntax_by_extension(ext) {
                    self.syntax = syntax.clone();
                    return;
                }
            }

            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(syntax) = self.syntax_set.find_syntax_by_name(filename) {
                    self.syntax = syntax.clone();
                    return;
                }
            }

            if let Ok(content) = std::fs::read_to_string(path) {
                if let Some(first_line) = content.lines().next() {
                    if let Some(syntax) = self.syntax_set.find_syntax_by_first_line(first_line) {
                        self.syntax = syntax.clone();
                        return;
                    }
                }
            }
        }

        self.syntax = self.syntax_set.find_syntax_plain_text().clone();
    }

    pub fn highlight(&mut self, rope: &xi_rope::Rope) -> Vec<Vec<(SyntectStyle, String)>> {
        let mut result = Vec::new();
        let theme = &self.theme_set.themes[&self.current_theme_name];

        let content = rope.to_string();
        let mut highlighter = HighlightLines::new(&self.syntax, theme);

        for line in LinesWithEndings::from(&content) {
            match highlighter.highlight_line(line, &self.syntax_set) {
                Ok(ranges) => {
                    let processed_ranges: Vec<(SyntectStyle, String)> = ranges
                        .into_iter()
                        .map(|(style, text)| (style, text.to_string()))
                        .collect::<Vec<_>>();

                    result.push(processed_ranges);
                }
                Err(_) => {
                    result.push(vec![(SyntectStyle::default(), line.to_string())]);
                }
            }
        }

        if result.is_empty() {
            result.push(vec![(SyntectStyle::default(), "".to_string())]);
        }

        result
    }
}
