use super::editor::EditorAction;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle_key_event(key: KeyEvent) -> Option<EditorAction> {
    match key {
        KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => Some(EditorAction::Exit),

        KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::Exit),

        KeyEvent {
            code: KeyCode::Char('w'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::Save),

        KeyEvent {
            code: KeyCode::Char('i'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::ChangeMode(true)),

        KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::ChangeMode(false)),

        KeyEvent {
            code: KeyCode::Char(':'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::StartCommand),

        KeyEvent {
            code: KeyCode::Up | KeyCode::Char('k'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::MoveCursor((0, -1))),

        KeyEvent {
            code: KeyCode::Down | KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::MoveCursor((0, 1))),

        KeyEvent {
            code: KeyCode::Left | KeyCode::Char('h'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::MoveCursor((-1, 0))),

        KeyEvent {
            code: KeyCode::Right | KeyCode::Char('l'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::MoveCursor((1, 0))),

        KeyEvent {
            code: KeyCode::Char('w'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => Some(EditorAction::MoveWord(1)),

        KeyEvent {
            code: KeyCode::Char('b'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => Some(EditorAction::MoveWord(-1)),

        KeyEvent {
            code: KeyCode::Home | KeyCode::Char('0'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::LineStart),

        KeyEvent {
            code: KeyCode::End | KeyCode::Char('$'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::LineEnd),

        KeyEvent {
            code: KeyCode::PageUp,
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::PageUp),

        KeyEvent {
            code: KeyCode::PageDown,
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::PageDown),

        KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            ..
        } => Some(EditorAction::InsertChar(c)),

        KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::Tab),

        KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::InsertChar('\n')),

        KeyEvent {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditorAction::DeleteChar),

        _ => None,
    }
}
