use std::io::{self, Write};
use xi_rope::Rope;

#[derive(Debug)]
pub struct RopeTextBuffer {
    rope: Rope,
    modified: bool,
}

impl RopeTextBuffer {
    pub fn new(rope: Rope) -> Self {
        Self {
            rope,
            modified: false,
        }
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn rope_mut(&mut self) -> &mut Rope {
        &mut self.rope
    }

    pub fn set_modified(&mut self, modified: bool) {
        self.modified = modified;
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn insert_char(&mut self, pos: usize, c: char) {
        self.rope.edit(pos..pos, &c.to_string());
        self.modified = true;
    }

    pub fn remove_char(&mut self, pos: usize) {
        if pos < self.rope.len() {
            self.rope.edit(pos..pos + 1, "");
            self.modified = true;
        }
    }

    pub fn save_to_file(&self, path: &std::path::Path) -> io::Result<()> {
        std::fs::write(path, self.rope.to_string())
    }
}