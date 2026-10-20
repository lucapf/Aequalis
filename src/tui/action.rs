//! What a key or a click asks for, apart from which of them asked: `keymap` turns a key into an
//! `Action` and `apply` carries it out, so a click on the screen can ask for the same.

use super::keys::{Button, Key, Mouse, MouseKind};
use super::render::Hit;
use super::{currencies, history_panel, measures, search, settings, trivia, App, Keypad, Overlay, Part, StatusKind, SCROLL_STEP};

/// A window of the toolbar (and Ctrl+K).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Panel {
    Measures,
    Currencies,
    Trivia,
    Settings,
    /// Ctrl+K: searching the units.
    Search,
    /// F5: the requests evaluated.
    History,
}

impl Panel {
    fn overlay(self) -> Overlay {
        match self {
            Panel::Measures => Overlay::Measures(measures::View::default()),
            Panel::Currencies => Overlay::Currencies(currencies::View::default()),
            Panel::Trivia => Overlay::Trivia(trivia::View::new()),
            Panel::Settings => Overlay::Settings(settings::View::default()),
            Panel::Search => Overlay::Search(search::Search::default()),
            Panel::History => Overlay::History(super::history_panel::View::default()),
        }
    }

    /// Whether `overlay` is this window (the unit keys are part of the Measures).
    pub fn is(self, overlay: &Option<Overlay>) -> bool {
        matches!(
            (self, overlay),
            (Panel::Measures, Some(Overlay::Measures(_) | Overlay::Picker(_)))
                | (Panel::Currencies, Some(Overlay::Currencies(_)))
                | (Panel::Trivia, Some(Overlay::Trivia(_)))
                | (Panel::Settings, Some(Overlay::Settings(_)))
                | (Panel::Search, Some(Overlay::Search(_)))
                | (Panel::History, Some(Overlay::History(_)))
        )
    }
}

/// An entry of the toolbar: the function key, the Alt letter for terminals that keep the function
/// keys, its label, a shorter one for a narrow terminal, and what it asks for.
pub struct Tool {
    pub fkey: u8,
    pub alt: char,
    pub label: &'static str,
    pub short: &'static str,
    pub action: Action,
}

/// The toolbar, left to right.
pub const TOOLBAR: [Tool; 6] = [
    Tool { fkey: 2, alt: 'm', label: "Measures", short: "Meas", action: Action::Open(Panel::Measures) },
    Tool { fkey: 3, alt: 'c', label: "Currencies", short: "Curr", action: Action::Open(Panel::Currencies) },
    Tool { fkey: 4, alt: 't', label: "Trivia", short: "Triv", action: Action::Open(Panel::Trivia) },
    Tool { fkey: 5, alt: 'h', label: "History", short: "Hist", action: Action::Open(Panel::History) },
    Tool { fkey: 9, alt: 'b', label: "Keys", short: "Keys", action: Action::ToggleKeypad },
    Tool { fkey: 10, alt: 's', label: "⚙", short: "⚙", action: Action::Open(Panel::Settings) },
];

/// What a click on a region of the screen asks for (`render::Hit`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    /// What a key or the toolbar asks for.
    Act(Action),
    /// A part of the keyboard: Basic, Sci, Units, Money.
    Focus(Part),
    /// A history entry's request: back into the line.
    Entry(usize),
    /// A history entry's result: copied.
    EntryResult(usize),
    /// A history entry, removed (in the History panel).
    Remove(usize),
    /// The request line, whose first column shows the character at this offset.
    Line(usize),
    /// A suggestion under the line (the n-th): the word completed with it.
    Suggest(usize),
    /// The n-th item of the window open: chosen and used (Enter).
    Pick(usize),
    /// The n-th tab of the window open (the unit keys' systems, the Trivia's sections).
    Tab(usize),
    /// The unit keys' date formats: the cell at (row, column), chosen and used.
    Cell(usize, usize),
    /// The board: the n-th unit button of the n-th class, the n-th currency, the n-th clock, the
    /// name of the n-th class (its units open).
    BoardUnit(usize, usize),
    BoardMoney(usize),
    BoardClock(usize),
    BoardClass(usize),
    /// A window over the screen: a click outside it closes it (Esc); inside it, nothing.
    Dismiss,
    Inside,
}

/// An act on the request line, the history or the windows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Quit,
    Redraw,
    /// A character typed at the cursor.
    Insert(char),
    /// Text written at the cursor (a function and its bracket).
    Write(&'static str),
    /// The request line cleared (what is cleared comes back with ↑).
    Clear,
    /// Alt+O: the mouse reported to the program, or left to the terminal for selecting text.
    ToggleMouse,
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
    /// ↑ ↓ on the line: an earlier or a later request, as in a shell.
    HistoryUp,
    HistoryDown,
    /// The history scrolled up or down.
    ScrollUp,
    ScrollDown,
    /// A window opened, or closed when it is the one open.
    Open(Panel),
    Help,
    About,
    ToggleKeypad,
    /// A shortcut of the keypad's right side: 0-4 its units, 5-9 its currencies (Alt+1…5, Alt+6…0).
    Shortcut(usize),
    /// `now` at the cursor, for date math.
    Now,
    /// The answer, or the history entry in focus, to the clipboard.
    Copy,
    /// Esc: one step back.
    Cancel,
    /// Ctrl+C: Esc's steps, then clearing the line, and with nothing to clear, quitting.
    Interrupt,
}

impl<'a> App<'a> {
    /// What a key does with no window open (`None`: nothing).
    pub(super) fn keymap(&self, key: Key) -> Option<Action> {
        Some(match key {
            Key::Tab => Action::Complete(true),
            Key::BackTab => Action::Complete(false),
            Key::Esc => Action::Cancel,
            Key::Char(c) => Action::Insert(c),
            Key::Backspace => Action::Backspace,
            Key::Delete => Action::Delete,
            Key::Ctrl('b') => Action::Left,
            Key::Ctrl('f') => Action::Right,
            Key::Ctrl('a') => Action::Home,
            Key::Ctrl('e') => Action::End,
            Key::Ctrl('u') => Action::KillToStart,
            Key::Ctrl('w') => Action::KillWord,
            Key::Ctrl('p') => Action::HistoryUp,
            Key::Ctrl('n') => Action::HistoryDown,
            Key::Ctrl('d') if self.line.is_empty() => Action::Quit,
            Key::Ctrl('d') => Action::Delete,
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

    /// The keys that work with a window open too: the toolbar's function keys and Alt letters,
    /// Ctrl+K, Ctrl+C, Ctrl+Q. Not while a question is asked (the ambiguous date), except Ctrl+C.
    pub(super) fn global_keymap(&self, key: Key) -> Option<Action> {
        match key {
            Key::Ctrl('c') => return Some(Action::Interrupt),
            Key::Ctrl('l') => return Some(Action::Redraw),
            Key::Ctrl('q') => return Some(Action::Quit),
            _ => {}
        }
        if matches!(self.overlay, Some(Overlay::Choice(..))) {
            return None;
        }
        match key {
            Key::F(1) => Some(Action::Help),
            Key::F(n) => TOOLBAR.iter().find(|t| t.fkey == n).map(|t| t.action),
            Key::Ctrl('k') => Some(Action::Open(Panel::Search)),
            Key::Alt(c) => match c.to_ascii_lowercase() {
                '?' => Some(Action::Help),
                'a' => Some(Action::About),
                'n' => Some(Action::Now),
                'y' => Some(Action::Copy),
                'k' => Some(Action::KillToEnd),
                'o' => Some(Action::ToggleMouse),
                'q' => Some(Action::Quit),
                // Alt+1…5 the units, Alt+6…9 and Alt+0 the currencies.
                '1'..='9' => Some(Action::Shortcut(c as usize - '1' as usize)),
                '0' => Some(Action::Shortcut(9)),
                c => TOOLBAR.iter().find(|t| t.alt == c).map(|t| t.action),
            },
            _ => None,
        }
    }

    /// A mouse report: the wheel scrolls the history; a left click asks for what the screen drew
    /// under it (`hits`, from the last frame).
    pub(super) fn mouse(&mut self, mouse: Mouse) {
        (self.pressed, self.lit) = (None, None);
        match mouse.kind {
            // The wheel moves through the window open, as ↑ ↓ do (not while typing a setting).
            MouseKind::ScrollUp | MouseKind::ScrollDown if self.overlay.is_some() => {
                if !matches!(&self.overlay, Some(Overlay::Settings(v)) if v.edit.is_some()) {
                    self.handle(if mouse.kind == MouseKind::ScrollUp { Key::Up } else { Key::Down });
                }
            }
            MouseKind::ScrollUp => self.apply(Action::ScrollUp),
            MouseKind::ScrollDown => self.apply(Action::ScrollDown),
            MouseKind::Down(Button::Left) => {
                let hit = self.hits.iter().rev().find(|h| h.contains(mouse.row, mouse.col)).copied();
                if let Some(hit) = hit {
                    self.click(hit, mouse.col);
                }
            }
            MouseKind::Down(_) | MouseKind::Up | MouseKind::Drag(_) => {}
        }
    }

    /// A click on the n-th item of the window open: it is chosen, then used as Enter would.
    fn pick(&mut self, n: usize) {
        match &mut self.overlay {
            Some(Overlay::Measures(v)) => v.selected = n,
            Some(Overlay::Picker(p)) => p.selected = n,
            Some(Overlay::Currencies(v)) => v.selected = n,
            Some(Overlay::Search(s)) => s.selected = n,
            Some(Overlay::History(v)) => v.selected = n,
            Some(Overlay::Trivia(v)) if v.popup.is_none() => v.selected = n,
            Some(Overlay::Choice(_, selected)) => *selected = n,
            // The settings: a row being typed in is left as it is.
            Some(Overlay::Settings(v)) if v.edit.is_none() => v.selected = n,
            _ => return,
        }
        self.handle(Key::Enter);
    }

    /// A click on the n-th tab of the window open.
    fn pick_tab(&mut self, n: usize) {
        match &mut self.overlay {
            Some(Overlay::Picker(p)) => {
                if p.tabs(self.config).get(n).is_some_and(|&(_, shown)| !shown) {
                    p.switch_tab();
                }
            }
            Some(Overlay::Trivia(v)) if v.popup.is_none() => {
                v.section = n;
                v.selected = 0;
            }
            _ => {}
        }
    }

    /// A left click at column `col` on `hit`.
    fn click(&mut self, hit: Hit, col: usize) {
        self.cycle = None;
        match hit.target {
            Target::Act(action) => {
                // The keypad's keys light up as the typed ones do.
                self.pressed = match action {
                    Action::Insert(c) => Some(c),
                    Action::Backspace => Some('⌫'),
                    Action::Submit => Some('='),
                    _ => None,
                };
                self.apply(action);
            }
            Target::Focus(part) => {
                self.focus = Some(part);
                match part {
                    Part::Basic => self.keypad = Keypad::Basic,
                    Part::Sci => self.keypad = Keypad::Sci,
                    Part::Units | Part::Money => {}
                }
            }
            Target::Entry(i) => self.recall_entry(i),
            Target::Remove(i) => {
                self.history.remove(i);
                if let Some(Overlay::History(v)) = &mut self.overlay {
                    v.selected = v.selected.min(history_panel::matches(&self.history, &v.query).len().saturating_sub(1));
                }
            }
            Target::EntryResult(i) => match self.history.entries().get(i).map(|e| e.result.clone()) {
                Some(Ok(result)) => self.copy_text(result),
                Some(Err(_)) => self.status = Some((StatusKind::Warning, "nothing to copy: that entry is an error".into())),
                None => {}
            },
            Target::Line(offset) => self.line.move_to(offset + col - hit.col),
            Target::Suggest(i) => self.complete_with(i),
            Target::BoardUnit(class, button) => {
                let name = super::board::classes(self.config, &self.usage).get(class).and_then(|c| c.buttons.get(button)).map(|b| b.written.clone());
                if let Some(name) = name {
                    self.write_unit(&name, false);
                }
            }
            Target::BoardMoney(i) => {
                let code = self.board_money().get(i).map(|m| m.code.clone());
                if let Some(code) = code {
                    self.write_unit(&code, true);
                }
            }
            Target::BoardClock(i) => {
                if let Some(city) = self.clocks().get(i).map(|c| c.city.clone()) {
                    let mut text = self.line.text();
                    text = if text.trim().is_empty() { "now ".to_string() } else if text.ends_with(' ') { text } else { format!("{text} ") };
                    self.line.set(&format!("{text}{city} "));
                }
            }
            Target::BoardClass(i) => {
                if let Some(index) = super::board::classes(self.config, &self.usage).get(i).and_then(|c| self.config.categories.iter().position(|k| k.name == c.name)) {
                    self.overlay = Some(Overlay::Picker(super::picker::Picker::new(self.config, index, self.system)));
                }
            }
            Target::Dismiss => self.handle(Key::Esc),
            Target::Inside => {}
            Target::Pick(n) => self.pick(n),
            Target::Tab(n) => self.pick_tab(n),
            Target::Cell(r, c) => {
                if let Some(Overlay::Picker(p)) = &mut self.overlay {
                    p.cell = (r, c);
                    self.handle(Key::Enter);
                }
            }
        }
    }

    pub(super) fn apply(&mut self, action: Action) {
        // Only Tab goes on with a cycle through the suggestions; Esc ends one.
        let cycling = !matches!(action, Action::Complete(_)) && self.cycle.take().is_some();
        // Whatever edits the line ends the recall of an earlier request (and drops the draft).
        if matches!(
            action,
            Action::Insert(_)
                | Action::Write(_)
                | Action::Backspace
                | Action::Delete
                | Action::KillToStart
                | Action::KillToEnd
                | Action::KillWord
                | Action::Complete(_)
                | Action::Submit
                | Action::Shortcut(_)
                | Action::Now
        ) {
            (self.recall, self.draft) = (None, None);
        }
        // The edits of the line are for the line, not for a window drawn over it.
        if self.overlay.is_some() && matches!(action, Action::Now | Action::KillToEnd) {
            return;
        }
        match action {
            Action::Quit => self.quit = true,
            Action::Redraw => self.redraw = true,
            Action::Insert(c) => self.line.insert(c),
            Action::Write(text) => text.chars().for_each(|c| self.line.insert(c)),
            Action::Clear => self.clear_line(),
            Action::ToggleMouse => {
                self.mouse_on = !self.mouse_on;
                self.status = Some((
                    StatusKind::Note,
                    if self.mouse_on { "mouse on: Shift+drag selects text".into() } else { "mouse off: the terminal selects text (Alt+O: on)".into() },
                ));
            }
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
            Action::HistoryUp => self.recall_older(),
            Action::HistoryDown => self.recall_newer(),
            Action::ScrollUp => self.scroll = (self.scroll + SCROLL_STEP).min(self.history.entries().len().saturating_sub(1)),
            Action::ScrollDown => self.scroll = self.scroll.saturating_sub(SCROLL_STEP),
            Action::Open(panel) => {
                self.focus = None;
                self.overlay = if panel.is(&self.overlay) { None } else { Some(panel.overlay()) };
                if panel == Panel::Search && self.overlay.is_some() {
                    self.refresh_money();
                }
            }
            Action::Help => self.overlay = if matches!(self.overlay, Some(Overlay::Help(_))) { None } else { Some(Overlay::Help(0)) },
            Action::About => self.overlay = if matches!(self.overlay, Some(Overlay::About)) { None } else { Some(Overlay::About) },
            Action::ToggleKeypad => {
                // A panel in the band gives way to the keyboard; with the keyboard shown, F9 hides it.
                let was_panel = matches!(self.overlay, Some(Overlay::Measures(_) | Overlay::Picker(_) | Overlay::Currencies(_) | Overlay::Trivia(_) | Overlay::Search(_) | Overlay::History(_)));
                self.overlay = if was_panel { None } else { self.overlay.take() };
                if self.keypad != Keypad::Hidden && !was_panel {
                    (self.keypad, self.focus) = (Keypad::Hidden, None);
                } else {
                    if self.keypad == Keypad::Hidden {
                        self.keypad = self.best_layout();
                    }
                    // The focus goes to the keyboard: Tab moves it across, Esc gives it back.
                    self.focus = Some(if self.keypad == Keypad::Sci { Part::Sci } else { Part::Basic });
                }
            }
            Action::Shortcut(index) => {
                if self.overlay.is_some() {
                    return;
                }
                let Some(name) = self.shortcut(index).map(str::to_string) else { return };
                let line = super::picker::place_unit(&self.line.text(), self.line.cursor(), &name);
                self.line.set(&line);
                self.lit = Some(index);
                self.used(&name, index >= 5);
            }
            Action::Now => {
                let at_start = self.line.cursor() == 0 || self.line.chars()[self.line.cursor() - 1] == ' ';
                for c in if at_start { "now " } else { " now " }.chars() {
                    self.line.insert(c);
                }
            }
            Action::Copy => self.copy_answer(),
            Action::Cancel => {
                if cycling {
                    return;
                }
                if self.status.take().is_none() {
                    self.clear_line();
                }
            }
            Action::Interrupt => {
                if self.overlay.take().is_some() || cycling || self.focus.take().is_some() {
                    return;
                }
                if !self.line.is_empty() {
                    self.clear_line();
                } else {
                    self.quit = true;
                }
            }
        }
    }

    /// Esc or Ctrl+C on the line: its text goes aside, where ↑ finds it again.
    fn clear_line(&mut self) {
        let text = self.line.text();
        if !text.is_empty() {
            self.draft = Some(text);
        }
        self.recall = None;
        self.line.set("");
    }

    /// ↑: the request before the one recalled, the newest first; the line's text is kept aside.
    /// On an empty line, what Esc cleared comes back instead.
    fn recall_older(&mut self) {
        let entries = self.history.entries();
        let index = match self.recall {
            None => {
                if let (true, Some(draft)) = (self.line.is_empty(), self.draft.take()) {
                    return self.line.set(&draft);
                }
                if entries.is_empty() {
                    return;
                }
                self.draft = Some(self.line.text());
                entries.len() - 1
            }
            Some(i) => i.saturating_sub(1),
        };
        let input = entries[index].input.clone();
        self.recall = Some(index);
        self.line.set(&input);
    }

    /// ↓: the request after the one recalled; after the newest, the text that was kept aside.
    fn recall_newer(&mut self) {
        let Some(i) = self.recall else { return };
        match self.history.entries().get(i + 1).map(|e| e.input.clone()) {
            Some(input) => {
                self.recall = Some(i + 1);
                self.line.set(&input);
            }
            None => {
                self.recall = None;
                let draft = self.draft.take().unwrap_or_default();
                self.line.set(&draft);
            }
        }
    }
}
