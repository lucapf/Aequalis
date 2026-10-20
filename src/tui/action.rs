//! What a key or a click asks for, apart from which of them asked: `keymap` turns a key into an
//! `Action` and `apply` carries it out, so a click on the screen can ask for the same.

use super::keys::{Key, Mouse, MouseKind};
use super::{shortcuts, App, Mode, Overlay, SCROLL_STEP};

/// An act on the request line, the history or the windows, when no window is open.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Quit,
    Redraw,
    /// A character typed at the cursor.
    Insert(char),
    Backspace,
    Delete,
    Left,
    Right,
    WordLeft,
    WordRight,
    Home,
    End,
    KillToStart,
    KillToEnd,
    KillWord,
    Submit,
    /// Tab (`true`) or Shift+Tab: the next or the previous suggestion.
    Complete(bool),
    HistoryUp,
    HistoryDown,
    /// The history scrolled up or down.
    ScrollUp,
    ScrollDown,
    /// The window of the top bar with this shortcut key.
    Open(char),
    /// The top bar's highlight moved, and the highlighted entry opened.
    MenuLeft,
    MenuRight,
    OpenMenu,
    Help,
    About,
    ToggleKeypad,
    /// `now` at the cursor, for date math.
    Now,
    EnterInput,
    EnterCommand,
    /// Stop browsing the history and drop the status message.
    Cancel,
}

impl<'a> App<'a> {
    /// What a key does with no window open, in the current mode (`None`: nothing).
    pub(super) fn keymap(&self, key: Key) -> Option<Action> {
        match self.mode {
            Mode::Input => self.input_keymap(key),
            Mode::Command => self.command_keymap(key),
        }
    }

    fn input_keymap(&self, key: Key) -> Option<Action> {
        Some(match key {
            Key::Tab => Action::Complete(true),
            Key::BackTab => Action::Complete(false),
            Key::Esc => Action::EnterCommand,
            Key::Char(c) => Action::Insert(c),
            Key::Backspace => Action::Backspace,
            Key::Delete => Action::Delete,
            Key::Ctrl('b') => Action::Left,
            Key::Ctrl('f') => Action::Right,
            Key::Ctrl('a') => Action::Home,
            Key::Ctrl('e') => Action::End,
            Key::Ctrl('u') => Action::KillToStart,
            Key::Ctrl('k') => Action::KillToEnd,
            Key::Ctrl('w') => Action::KillWord,
            Key::Ctrl('p') => Action::HistoryUp,
            Key::Ctrl('n') => Action::HistoryDown,
            Key::Ctrl('d') if self.line.is_empty() => Action::Quit,
            Key::Ctrl('d') => Action::Delete,
            other => return Self::common_keymap(other),
        })
    }

    fn command_keymap(&self, key: Key) -> Option<Action> {
        Some(match key {
            Key::Char('i' | 'I') => Action::EnterInput,
            Key::Char('e' | 'E' | '?') => Action::Help,
            Key::Char('a' | 'A') => Action::About,
            Key::Left | Key::Char('h') => Action::MenuLeft,
            Key::Right | Key::Char('l') => Action::MenuRight,
            Key::Enter => Action::OpenMenu,
            Key::Home | Key::Char('0') => Action::Home,
            Key::End | Key::Char('$') => Action::End,
            Key::Char('k') => Action::HistoryUp,
            Key::Char('j') => Action::HistoryDown,
            Key::Char('w') => Action::WordRight,
            Key::Char('b') => Action::WordLeft,
            Key::Char('x') | Key::Delete => Action::Delete,
            Key::Char('c' | 'C') => Action::ToggleKeypad,
            Key::Char('n' | 'N') => Action::Now,
            Key::Char('q') | Key::Ctrl('d') => Action::Quit,
            Key::Backspace => Action::Left,
            Key::Esc => Action::Cancel,
            Key::Char(c) => {
                let c = c.to_ascii_uppercase();
                return shortcuts(self.config).iter().any(|s| s.key == c).then_some(Action::Open(c));
            }
            other => return Self::common_keymap(other),
        })
    }

    /// The keys that work the same in both modes.
    fn common_keymap(key: Key) -> Option<Action> {
        Some(match key {
            Key::Enter => Action::Submit,
            Key::Left => Action::Left,
            Key::Right => Action::Right,
            Key::WordLeft => Action::WordLeft,
            Key::WordRight => Action::WordRight,
            Key::Home => Action::Home,
            Key::End => Action::End,
            Key::Up => Action::HistoryUp,
            Key::Down => Action::HistoryDown,
            Key::PageUp => Action::ScrollUp,
            Key::PageDown => Action::ScrollDown,
            _ => return None,
        })
    }

    /// A mouse report: the wheel scrolls the history under the sections; the rest has no target yet.
    pub(super) fn mouse(&mut self, mouse: Mouse) {
        if self.overlay.is_some() {
            return;
        }
        match mouse.kind {
            MouseKind::ScrollUp => self.apply(Action::ScrollUp),
            MouseKind::ScrollDown => self.apply(Action::ScrollDown),
            MouseKind::Down(_) | MouseKind::Up | MouseKind::Drag(_) => {}
        }
    }

    pub(super) fn apply(&mut self, action: Action) {
        // Only Tab goes on with a cycle through the suggestions.
        if !matches!(action, Action::Complete(_)) {
            self.cycle = None;
        }
        match action {
            Action::Quit => self.quit = true,
            Action::Redraw => self.redraw = true,
            Action::Insert(c) => self.line.insert(c),
            Action::Backspace => self.line.backspace(),
            Action::Delete => self.line.delete(),
            Action::Left => self.line.left(),
            Action::Right => self.line.right(),
            Action::WordLeft => self.line.word_back(),
            Action::WordRight => self.line.word_forward(),
            Action::Home => self.line.home(),
            Action::End => self.line.end(),
            Action::KillToStart => self.line.kill_to_start(),
            Action::KillToEnd => self.line.kill_to_end(),
            Action::KillWord => self.line.kill_word(),
            Action::Submit => self.submit(),
            Action::Complete(forward) => self.complete(forward),
            Action::HistoryUp => self.history_up(),
            Action::HistoryDown => self.history_down(),
            Action::ScrollUp => self.scroll = (self.scroll + SCROLL_STEP).min(self.history.entries().len().saturating_sub(1)),
            Action::ScrollDown => self.scroll = self.scroll.saturating_sub(SCROLL_STEP),
            Action::Open(key) => {
                if let Some(s) = shortcuts(self.config).into_iter().find(|s| s.key == key) {
                    self.open(&s);
                }
            }
            Action::MenuLeft => {
                let n = shortcuts(self.config).len();
                self.menu = (self.menu + n - 1) % n;
            }
            Action::MenuRight => self.menu = (self.menu + 1) % shortcuts(self.config).len(),
            Action::OpenMenu => {
                if let Some(s) = shortcuts(self.config).into_iter().nth(self.menu) {
                    self.open(&s);
                }
            }
            Action::Help => self.overlay = Some(Overlay::Help(0)),
            Action::About => self.overlay = Some(Overlay::About),
            Action::ToggleKeypad => self.keypad = !self.keypad,
            Action::Now => {
                let at_start = self.line.cursor() == 0 || self.line.chars()[self.line.cursor() - 1] == ' ';
                for c in if at_start { "now " } else { " now " }.chars() {
                    self.line.insert(c);
                }
            }
            Action::EnterInput => self.mode = Mode::Input,
            Action::EnterCommand => self.mode = Mode::Command,
            Action::Cancel => {
                self.history.stop_browsing();
                self.status = None;
            }
        }
    }
}
