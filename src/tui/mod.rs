//! Interactive mode: `tally` with no arguments in a terminal.
//!
//! Sections, top to bottom: the unit categories with their shortcuts, the history of expressions
//! and results, the expression bar, and a status line. Two modes, Vim style:
//! - **input** (the start): typing edits the expression; Tab completes units; Esc → command mode.
//! - **command**: single keys. A category's initial turns the keypad into that category's unit
//!   buttons (type to search, arrows move, Enter writes the unit and a space and stays there for
//!   the next one, Esc back to the digits); `I`/`i` → input mode; `C` shows/hides the keypad;
//!   `E` → help; `j`/`k` browse the history; ← → / `h` `l` move the top bar's highlight and Enter
//!   opens it; `q` quits; `/` searches the units; `R` opens the Trivia (useful values with a
//!   description).
//!
//! The state machine (`App`), key decoding and rendering are free of I/O; only `term` and `run`
//! touch the terminal.

mod clipboard;
mod editor;
mod keys;
mod locale;
mod picker;
mod render;
mod search;
mod suggest;
mod term;
mod trivia;

use crate::config::{Config, System};
use crate::numbers;
use crate::{convert_request, Options};
use editor::{Entry, History, Line};
use keys::Key;
use picker::{Direction, Picker};
use std::io::{Read, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Input,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusKind {
    Note,
    Warning,
    Error,
}

/// Drawn over the sections.
pub enum Overlay {
    /// The unit buttons, on the keypad.
    Picker(Picker),
    /// The help, scrolled down by this many lines.
    Help(usize),
    /// An ambiguous date (`12/04/2026`): which reading is meant; `selected` into its choices.
    Choice(crate::time::Ambiguity, usize),
    /// The Trivia (`R`).
    Trivia(trivia::View),
    /// `/`: searching the units.
    Search(search::Search),
    /// `A`: the logo, the version, memory, the last online sync.
    About,
}

/// Tab cycling through the suggestions for one word.
struct Cycle {
    start: usize,
    names: Vec<&'static str>,
    index: usize,
}


pub struct App<'a> {
    config: &'a Config,
    opts: &'a Options<'a>,
    mode: Mode,
    overlay: Option<Overlay>,
    line: Line,
    history: History,
    /// Notes, warnings or the error of the last evaluation.
    status: Option<(StatusKind, String)>,
    /// History lines scrolled up from the bottom.
    scroll: usize,
    cycle: Option<Cycle>,
    /// The keypad drawn over the history (`C` in command mode).
    keypad: bool,
    /// The measurement system the unit keys open on: the device's (`locale`).
    system: System,
    /// Decimal and thousands separators for the preview: the device's (`numbers`).
    numbers: numbers::NumberFormat,
    /// The keypad key to light up: the last character typed (`=` for Enter).
    pressed: Option<char>,
    redraw: bool,
    quit: bool,
    /// The spacecraft's distances in the Trivia, retrieved when it first opens if `fetch_live`
    /// (only in a real session: never in the tests).
    /// The online data (`online.rs`), reloaded when its file changes; `online_stamp` is the file's
    /// modification time when read.
    online: crate::online::Data,
    online_stamp: Option<std::time::SystemTime>,
    /// The background sync started from here (to reap), and when expiry was last checked.
    sync: Option<std::process::Child>,
    sync_checked: Option<Instant>,
    /// `TALLY_MODE=offline` (read once).
    forced_offline: bool,
    /// The top bar's highlighted shortcut (command mode): ← → / `h` `l` move it, Enter opens it.
    menu: usize,
    /// The last result copied from the history (`C`); to the system clipboard too when
    /// `system_clipboard` (only in a real session: never in the tests).
    copied: Option<String>,
    system_clipboard: bool,
}

const SCROLL_STEP: usize = 5;

/// What a shortcut opens: one category, or a group of them (`Config::groups`, Size: Length, Area,
/// Volume), as indexes into `Config::categories`; none for the search and the Trivia.
pub struct Shortcut {
    pub key: char,
    pub name: &'static str,
    pub categories: Vec<usize>,
}

impl Shortcut {
    /// What it opens: the unit keys, or the Trivia.
    pub fn open(&self, config: &Config, device: System) -> Overlay {
        match self.categories[..] {
            [] if self.name == crate::config::SEARCH.0 => Overlay::Search(search::Search::default()),
            [] => Overlay::Trivia(trivia::View::new()),
            [one] => Overlay::Picker(Picker::new(config, one, device)),
            _ => Overlay::Picker(Picker::group(config, self.name, self.categories.clone(), device)),
        }
    }
}

/// The groups and the categories outside them, in the order of `Config::menu`, with their
/// shortcut (`Group::key`, `Category::key`).
pub fn shortcuts(config: &Config) -> Vec<Shortcut> {
    let index = |name: &str| config.categories.iter().position(|c| c.name == name).expect("a group member is a category");
    config
        .menu
        .iter()
        .map(|&name| match config.groups.iter().find(|g| g.name == name) {
            None if name == crate::config::TRIVIA.0 => Shortcut { key: crate::config::TRIVIA.1, name, categories: Vec::new() },
            None if name == crate::config::SEARCH.0 => Shortcut { key: crate::config::SEARCH.1, name, categories: Vec::new() },
            Some(g) => Shortcut { key: g.key, name: g.name, categories: g.members.iter().map(|m| index(m)).collect() },
            None => {
                let i = index(name);
                Shortcut { key: config.categories[i].key, name, categories: vec![i] }
            }
        })
        .collect()
}

impl<'a> App<'a> {
    pub fn new(config: &'a Config, opts: &'a Options<'a>) -> Self {
        App {
            config,
            opts,
            mode: Mode::Input,
            overlay: None,
            line: Line::default(),
            history: History::new(config.max_history),
            status: None,
            scroll: 0,
            cycle: None,
            keypad: true,
            system: System::Metric,
            numbers: numbers::NumberFormat::default(),
            pressed: None,
            redraw: false,
            quit: false,
            online: crate::online::Data::default(),
            online_stamp: None,
            sync: None,
            sync_checked: None,
            forced_offline: crate::network::mode() == Ok(crate::network::Mode::Offline),
            menu: 0,
            copied: None,
            system_clipboard: false,

        }
    }

    /// Online or offline: the last sync's result (`network::status_of`).
    pub fn network_status(&self) -> crate::network::Status {
        crate::network::status_of(&self.online, self.forced_offline)
    }

    /// Reloads the online data when its file changed (the background sync wrote it): `true` then,
    /// for a redraw.
    pub fn refresh_online(&mut self) -> bool {
        let Some(path) = crate::online::path() else { return false };
        let stamp = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if stamp == self.online_stamp {
            return false;
        }
        self.online_stamp = stamp;
        self.online = crate::online::load_from(&path);
        true
    }

    /// Every 10 minutes: reaps a finished sync, and starts one if the data has expired.
    fn keep_online_fresh(&mut self) {
        if self.sync_checked.is_some_and(|t| t.elapsed() < Duration::from_secs(600)) {
            return;
        }
        self.sync_checked = Some(Instant::now());
        if let Some(child) = &mut self.sync {
            if !matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
        }
        self.sync = crate::online::ensure_fresh();
    }

    /// Opens what a shortcut opens; the Trivia starts retrieving the spacecraft's distances.
    fn open(&mut self, shortcut: &Shortcut) {
        // The highlight follows: the top bar shows what is open.
        if let Some(i) = shortcuts(self.config).iter().position(|s| s.key == shortcut.key) {
            self.menu = i;
        }
        self.overlay = Some(shortcut.open(self.config, self.system));
    }

    /// The Trivia's keys: Tab / ← → the section, ↑ ↓ the entry, Enter its popup; in the popup,
    /// Enter writes its value, Esc goes back, ↑ ↓ scroll.
    fn trivia_key(&mut self, key: Key) {
        let Some(Overlay::Trivia(v)) = &mut self.overlay else { return };
        if let Some(scroll) = &mut v.popup {
            match key {
                Key::Esc | Key::Backspace | Key::Char('q') => v.popup = None,
                Key::Down | Key::Char('j') => *scroll += 1,
                Key::Up | Key::Char('k') => *scroll = scroll.saturating_sub(1),
                Key::Enter => self.write_trivia(),
                _ => {}
            }
            return;
        }
        match key {
            Key::Esc => self.overlay = None,
            Key::Tab | Key::Right | Key::Char('l') => v.switch(true),
            Key::BackTab | Key::Left | Key::Char('h') => v.switch(false),
            Key::Down | Key::Char('j') => v.step(true),
            Key::Up | Key::Char('k') => v.step(false),
            Key::Enter => v.popup = Some(0),
            Key::Char(c) => {
                if let Some(s) = shortcuts(self.config).into_iter().find(|s| s.key == c.to_ascii_uppercase()) {
                    self.open(&s);
                }
            }
            _ => {}
        }
    }

    /// The selected entry's value written into the bar (a tip: its example), in input mode.
    fn write_trivia(&mut self) {
        let Some(Overlay::Trivia(v)) = &self.overlay else { return };
        let Some(entry) = v.entry() else { return };
        // A tip: its example request.
        if let crate::trivia::Source::Tip(example) = &entry.source {
            self.line.set(example);
            self.overlay = None;
            self.mode = Mode::Input;
            return;
        }
        let now = crate::time::now().ns.div_euclid(1_000_000_000) as i64;
        if let Ok((value, _)) = crate::trivia::value(self.config, entry, self.online.get(&entry.name), now) {
            self.line.set(&format!("{} ", crate::trivia::written(self.config, entry, value)));
            self.overlay = None;
            self.mode = Mode::Input;
        }
    }

    /// The search's keys: typing edits the query, ↑ ↓ choose, Enter writes the unit.
    fn search_key(&mut self, key: Key) {
        let Some(Overlay::Search(s)) = &mut self.overlay else { return };
        let found = search::matches(self.config, &s.query).len();
        match key {
            Key::Esc => self.overlay = None,
            Key::Char(c) => {
                s.query.push(c);
                s.selected = 0;
            }
            Key::Backspace => {
                s.query.pop();
                s.selected = 0;
            }
            Key::Down | Key::Tab if found > 0 => s.selected = (s.selected + 1) % found,
            Key::Up | Key::BackTab if found > 0 => s.selected = (s.selected + found - 1) % found,
            Key::Enter => {
                let chosen = search::matches(self.config, &s.query).get(s.selected).map(|(_, u)| u.symbol);
                if let Some(unit) = chosen {
                    let line = picker::place_unit(&self.line.text(), self.line.cursor(), unit);
                    self.line.set(&line);
                    self.overlay = None;
                    if picker::has_target(&line) {
                        self.mode = Mode::Input;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn handle(&mut self, key: Key) {
        match key {
            Key::Ctrl('c') => return self.quit = true,
            Key::Ctrl('l') => return self.redraw = true,
            _ => {}
        }
        self.pressed = match (key, &self.overlay, self.mode) {
            (Key::Char('x'), None, Mode::Input) => Some('*'),
            (Key::Char(c), None, Mode::Input) => Some(c),
            (Key::Enter, None, Mode::Input) if self.history.selected().is_none() => Some('='),
            _ => None,
        };
        // Browsing the history (↑ ↓, or j k in command mode): its own keys first.
        if self.overlay.is_none() && self.history.selected().is_some() && self.browse_key(key) {
            return;
        }
        match self.overlay {
            Some(Overlay::Picker(_)) => self.picker_key(key),
            Some(Overlay::Trivia(_)) => self.trivia_key(key),
            Some(Overlay::Search(_)) => self.search_key(key),
            Some(Overlay::About) => {
                if matches!(key, Key::Esc | Key::Enter | Key::Char('a' | 'A' | 'q')) {
                    self.overlay = None;
                }
            }
            Some(Overlay::Choice(ref a, ref mut selected)) => {
                let n = a.choices.len();
                match key {
                    Key::Up | Key::Char('k') => *selected = (*selected + n - 1) % n,
                    Key::Down | Key::Char('j') | Key::Tab => *selected = (*selected + 1) % n,
                    Key::Char(c @ '1'..='9') if (c as usize - '1' as usize) < n => *selected = c as usize - '1' as usize,
                    // The chosen pattern is written into the expression, which is evaluated.
                    Key::Enter => {
                        let request = a.choices[*selected].request.clone();
                        self.overlay = None;
                        self.line.set(&request);
                        self.submit();
                    }
                    Key::Esc => self.overlay = None,
                    _ => {}
                }
            }
            Some(Overlay::Help(ref mut scroll)) => match key {
                Key::Esc | Key::Enter | Key::Char('e' | 'E' | 'q' | '?') => self.overlay = None,
                Key::Down | Key::Char('j') => *scroll += 1,
                Key::Up | Key::Char('k') => *scroll = scroll.saturating_sub(1),
                Key::PageDown => *scroll += SCROLL_STEP,
                Key::PageUp => *scroll = scroll.saturating_sub(SCROLL_STEP),
                _ => {}
            },
            None => match self.mode {
                Mode::Input => self.input_key(key),
                Mode::Command => self.command_key(key),
            },
        }
    }

    fn input_key(&mut self, key: Key) {
        match key {
            Key::Tab => return self.complete(true),
            Key::BackTab => return self.complete(false),
            _ => self.cycle = None,
        }
        match key {
            Key::Esc => self.mode = Mode::Command,
            Key::Char(c) => self.line.insert(c),
            Key::Backspace => self.line.backspace(),
            Key::Delete => self.line.delete(),
            Key::Ctrl('b') => self.line.left(),
            Key::Ctrl('f') => self.line.right(),
            Key::Ctrl('a') => self.line.home(),
            Key::Ctrl('e') => self.line.end(),
            Key::Ctrl('u') => self.line.kill_to_start(),
            Key::Ctrl('k') => self.line.kill_to_end(),
            Key::Ctrl('w') => self.line.kill_word(),
            Key::Ctrl('p') => self.history_up(),
            Key::Ctrl('n') => self.history_down(),
            Key::Ctrl('d') if self.line.is_empty() => self.quit = true,
            Key::Ctrl('d') => self.line.delete(),
            other => self.common_key(other),
        }
    }

    fn command_key(&mut self, key: Key) {
        match key {
            Key::Char('i' | 'I') => self.mode = Mode::Input,
            Key::Char('e' | 'E' | '?') => self.overlay = Some(Overlay::Help(0)),
            Key::Char('a' | 'A') => self.overlay = Some(Overlay::About),
            // The top bar: ← → / h l move the highlight (circular, scrolling the bar), Enter opens it.
            Key::Left | Key::Char('h') => {
                let n = shortcuts(self.config).len();
                self.menu = (self.menu + n - 1) % n;
            }
            Key::Right | Key::Char('l') => self.menu = (self.menu + 1) % shortcuts(self.config).len(),
            Key::Enter => {
                if let Some(s) = shortcuts(self.config).into_iter().nth(self.menu) {
                    self.open(&s);
                }
            }
            Key::Home => self.line.home(),
            Key::End => self.line.end(),
            Key::Char('k') => self.history_up(),
            Key::Char('j') => self.history_down(),
            Key::Char('w') => self.line.word_forward(),
            Key::Char('b') => self.line.word_back(),
            Key::Char('0') => self.line.home(),
            Key::Char('$') => self.line.end(),
            Key::Char('x') => self.line.delete(),
            Key::Char('c' | 'C') => self.keypad = !self.keypad,
            // `now` at the cursor, for date math: `now + 3d`.
            Key::Char('n' | 'N') => {
                let at_start = self.line.cursor() == 0 || self.line.chars()[self.line.cursor() - 1] == ' ';
                for c in if at_start { "now " } else { " now " }.chars() {
                    self.line.insert(c);
                }
            }
            Key::Char('q') | Key::Ctrl('d') => self.quit = true,
            Key::Backspace => self.line.left(),
            Key::Delete => self.line.delete(),
            Key::Esc => {
                self.history.stop_browsing();
                self.status = None;
            }
            Key::Char(c) => {
                if let Some(s) = shortcuts(self.config).into_iter().find(|s| s.key == c.to_ascii_uppercase()) {
                    self.open(&s);
                }
            }
            other => self.common_key(other),
        }
    }

    /// Keys that work the same in both modes.
    fn common_key(&mut self, key: Key) {
        match key {
            Key::Enter => self.submit(),
            Key::Left => self.line.left(),
            Key::Right => self.line.right(),
            Key::Home => self.line.home(),
            Key::End => self.line.end(),
            Key::Up => self.history_up(),
            Key::Down => self.history_down(),
            Key::PageUp => self.scroll = (self.scroll + SCROLL_STEP).min(self.history.entries().len().saturating_sub(1)),
            Key::PageDown => self.scroll = self.scroll.saturating_sub(SCROLL_STEP),
            _ => {}
        }
    }

    /// The unit keys: arrows and `h j k l` move (circular), Tab switches tab, a category letter
    /// shows that category, Enter writes the unit, Esc goes back to the digits.
    fn picker_key(&mut self, key: Key) {
        let Some(Overlay::Picker(p)) = &mut self.overlay else { return };
        match key {
            Key::Esc => self.overlay = None,
            // Write the unit (and a space). With the target unit set the expression is complete:
            // back to input mode; otherwise stay on the buttons for the next unit.
            // Formats tab: the format becomes the target, which completes the expression.
            // Earth distance: the request is written, the places are typed.
            Key::Enter if p.tool == Some(crate::config::EARTH_DISTANCE) => {
                self.line.set("distance ");
                self.overlay = None;
                self.mode = Mode::Input;
            }
            Key::Enter if p.shows_formats(self.config) => {
                if let Some(word) = p.selected_format() {
                    self.line.set(&picker::place_format(&self.line.text(), &word));
                    self.overlay = None;
                    self.mode = Mode::Input;
                }
            }
            Key::Enter => {
                let chosen = p.items(self.config).get(p.selected).map(|u| u.symbol);
                if let Some(unit) = chosen {
                    let line = picker::place_unit(&self.line.text(), self.line.cursor(), unit);
                    self.line.set(&line);
                    if picker::has_target(&line) {
                        self.overlay = None;
                        self.mode = Mode::Input;
                    }
                }
            }
            Key::Tab => p.switch_tab(self.config, true),
            Key::BackTab => p.switch_tab(self.config, false),
            Key::Left | Key::Char('h') => p.step(self.config, Direction::Left),
            Key::Right | Key::Char('l') => p.step(self.config, Direction::Right),
            Key::Up | Key::Char('k') => p.step(self.config, Direction::Up),
            Key::Down | Key::Char('j') => p.step(self.config, Direction::Down),
            // Another category's units (`l` and `k` move, so Length is `L` and Temperature `K`).
            Key::Char(c) => {
                if let Some(s) = shortcuts(self.config).into_iter().find(|s| s.key == c.to_ascii_uppercase()) {
                    self.open(&s);
                }
            }
            _ => {}
        }
    }

    /// Highlights an older entry of the history; the bar is not changed.
    fn history_up(&mut self) {
        self.history.up();
    }

    fn history_down(&mut self) {
        self.history.down();
    }

    /// The keys while an entry of the history is highlighted: ↑ ↓ (`k` `j` in command mode) move,
    /// Enter writes its expression into the bar, `C` copies its result, Esc stops browsing. Any
    /// other key stops browsing and does what it does (`false`).
    fn browse_key(&mut self, key: Key) -> bool {
        let command = self.mode == Mode::Command;
        match key {
            Key::Up | Key::Ctrl('p') => self.history.up(),
            Key::Down | Key::Ctrl('n') => self.history.down(),
            Key::Char('k') if command => self.history.up(),
            Key::Char('j') if command => self.history.down(),
            Key::Enter => {
                if let Some(input) = self.history.highlighted().map(|e| e.input.clone()) {
                    self.line.set(&input);
                }
                self.history.stop_browsing();
            }
            Key::Char('c' | 'C') => self.copy_highlighted(),
            Key::Esc => {
                self.history.stop_browsing();
                self.status = None;
            }
            _ => {
                self.history.stop_browsing();
                return false;
            }
        }
        true
    }

    /// `C` while browsing: the highlighted result to the clipboard.
    fn copy_highlighted(&mut self) {
        let result = match self.history.highlighted().map(|e| e.result.clone()) {
            Some(Ok(result)) => result,
            Some(Err(_)) => return self.status = Some((StatusKind::Warning, "nothing to copy: that entry is an error".into())),
            None => return,
        };
        let copied = if self.system_clipboard { clipboard::copy(&result) } else { Ok(()) };
        self.status = Some(match copied {
            Ok(()) => (StatusKind::Note, format!("copied {result}")),
            Err(e) => (StatusKind::Error, e),
        });
        self.copied = Some(result);
    }

    /// Tab: replace the word with the next suggestion (the first one on the first Tab).
    fn complete(&mut self, forward: bool) {
        let cycle = match self.cycle.take() {
            Some(mut c) => {
                let len = c.names.len();
                let old = c.names[c.index].chars().count();
                c.index = if forward { (c.index + 1) % len } else { (c.index + len - 1) % len };
                self.line.replace(c.start, c.start + old, c.names[c.index]);
                c
            }
            None => {
                let Some(s) = suggest::suggest(self.config, &self.line.text(), self.line.cursor()) else {
                    return;
                };
                let index = if forward { 0 } else { s.names.len() - 1 };
                self.line.replace(s.start, self.line.cursor(), s.names[index]);
                Cycle { start: s.start, names: s.names, index }
            }
        };
        self.cycle = Some(cycle);
    }

    fn evaluate(&self, input: &str) -> Result<crate::Output, String> {
        let tokens: Vec<&str> = input.split_whitespace().collect();
        convert_request(self.config, self.opts, &tokens).map_err(|e| e.to_string())
    }

    /// Enter: evaluate the bar, add it to the history and clear the bar.
    fn submit(&mut self) {
        let input = self.line.text().trim().to_string();
        if input.is_empty() {
            return;
        }
        if input == "exit" || input == "quit" {
            self.quit = true;
            return;
        }
        if input == "about" {
            self.line.set("");
            self.overlay = Some(Overlay::About);
            return;
        }
        // An ambiguous date: ask which reading is meant (the popup), nothing is added yet.
        let tokens: Vec<&str> = input.split_whitespace().collect();
        if let Err(crate::CliError::Ambiguous(a)) = convert_request(self.config, self.opts, &tokens) {
            self.overlay = Some(Overlay::Choice(a, 0));
            return;
        }
        let (result, status) = match self.evaluate(&input) {
            Ok(out) => {
                let status = match (out.warnings.is_empty(), out.notes.is_empty()) {
                    (false, _) => Some((StatusKind::Warning, out.warnings.join(" · "))),
                    (true, false) => Some((StatusKind::Note, out.notes.join(" · "))),
                    (true, true) => None,
                };
                (Ok(out.stdout), status)
            }
            Err(msg) => (Err(msg.clone()), Some((StatusKind::Error, msg))),
        };
        self.history.push(Entry { input, result });
        self.status = status;
        self.line.set("");
        self.scroll = 0;
    }

    /// The result of the bar as it is now, shown while typing.
    fn preview(&self) -> Option<Result<String, String>> {
        let input = self.line.text();
        if input.trim().is_empty() || ["exit", "quit", "about"].contains(&input.trim()) {
            return None;
        }
        // A written date with no format yet (`12/04/26 3:20PM`): say what comes next.
        if !crate::time::is_date_request(&input) && crate::time::is_date_value(&input) {
            return Some(Err("now the target format (T, then Tab: Formats)".into()));
        }
        // Only a known source unit so far: say what comes next instead of the usage error. Not for a
        // date (`2026/12/24 - 12d`: the `d` is date math, the request is complete; bug 007).
        let date = crate::time::is_date_request(&input);
        if let (false, Ok((_, rest))) = (date, crate::calc::parse_prefix(&input)) {
            if let [from] = rest.split_whitespace().collect::<Vec<_>>()[..] {
                if crate::convert::resolve(self.config, from).is_ok() {
                    return Some(Err("now the target unit".into()));
                }
            }
        }
        // An ambiguous date: Enter asks which reading is meant.
        let tokens: Vec<&str> = input.split_whitespace().collect();
        if let Err(crate::CliError::Ambiguous(a)) = convert_request(self.config, self.opts, &tokens) {
            let patterns: Vec<&str> = a.choices.iter().map(|c| c.pattern.as_str()).collect();
            match a.kind {
                crate::time::AmbiguityKind::City => return Some(Err(format!("did you mean {}? · Enter to choose", patterns.join(" or ")))),
                crate::time::AmbiguityKind::Country => return Some(Err(format!("several time zones: {} · Enter to choose", patterns.join(", ")))),
                crate::time::AmbiguityKind::Date => {}
            }
            return Some(Err(format!("ambiguous: {} · Enter to choose", patterns.join(" or "))));
        }
        // Dates and timestamps keep their own format, distances come with the device's separators
        // already; other numbers get them here.
        let date = crate::time::is_date_request(&input) || crate::geo::request(self.config, &input, 0).is_some_and(|r| r.is_ok());
        Some(self.evaluate(&input).map(|out| if date { out.stdout } else { self.numbers.apply_to_result(&out.stdout) }))
    }

    /// How the expression in the bar is grouped, with every operation in brackets
    /// (`12+12/3` → `(12 + (12 / 3))`); `None` without operations.
    fn grouping(&self) -> Option<String> {
        let input = self.line.text();
        // A date is not arithmetic: `2026-12-25` is not 2026 − 12 − 25.
        if crate::time::is_date_request(&input) {
            return None;
        }
        let (expr, _) = crate::calc::parse_prefix(&input).ok()?;
        (expr.operations() > 0).then(|| expr.bracketed())
    }

    /// The suggestions to show (input mode only), and which one Tab selected.
    fn suggestions(&self) -> Option<(Vec<&'static str>, Option<usize>)> {
        if self.mode != Mode::Input || crate::time::is_date_request(&self.line.text()) {
            return None;
        }
        if let Some(c) = &self.cycle {
            return Some((c.names.clone(), Some(c.index)));
        }
        suggest::suggest(self.config, &self.line.text(), self.line.cursor()).map(|s| (s.names, None))
    }
}

/// A read that returns 0 bytes this fast is end of input (the terminal is gone), not the
/// 0.2 s timeout of raw mode.
const HANGUP: Duration = Duration::from_millis(50);

pub fn run(config: &Config, opts: &Options) -> ExitCode {
    let guard = match term::enter() {
        Ok(guard) => guard,
        Err(msg) => {
            eprintln!("tally: error: {msg}");
            return ExitCode::from(1);
        }
    };
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        term::leave();
        default_hook(info);
    }));

    let mut app = App::new(config, opts);
    app.system_clipboard = true;
    app.refresh_online();
    app.system = locale::device_system();
    app.numbers = numbers::device_format();
    let (mut stdin, mut stdout) = (std::io::stdin(), std::io::stdout());
    let (mut pending, mut buf) = (Vec::new(), [0u8; 256]);
    let (mut drawn, mut dirty, mut clear, mut hangups) = (None, true, true, 0);
    while !app.quit {
        app.keep_online_fresh();
        dirty |= app.refresh_online();
        let size = term::size().unwrap_or((24, 80));
        if dirty || drawn != Some(size) {
            let frame = render::render(&app, size.0, size.1);
            let _ = stdout.write_all(frame.to_ansi(clear || drawn != Some(size)).as_bytes());
            let _ = stdout.flush();
            (drawn, dirty, clear) = (Some(size), false, false);
        }
        let started = Instant::now();
        let n = match stdin.read(&mut buf) {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        if n == 0 {
            hangups = if started.elapsed() < HANGUP { hangups + 1 } else { 0 };
            if hangups > 3 {
                break;
            }
            continue;
        }
        pending.extend_from_slice(&buf[..n]);
        let (keys, used) = keys::decode(&pending);
        pending.drain(..used);
        for key in keys {
            app.handle(key);
        }
        clear = std::mem::take(&mut app.redraw);
        dirty = true;
    }
    drop(guard);
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(config: &Config) -> Options<'static> {
        Options { positional: Vec::new(), precision: config.default_precision, silent: false, verbose: false }
    }

    fn typed(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle(Key::Char(c));
        }
    }

    #[test]
    fn shortcuts_in_menu_order_and_unique() {
        let config = Config::default();
        let list = shortcuts(&config);
        let names: Vec<_> = list.iter().map(|s| format!("{} {}", s.key, s.name)).collect();
        // Length, Area and Volume are in the Size group.
        assert_eq!(names, ["/ Search", "T Time", "D Data", "S Size", "P Other physical", "R Trivia"]);
        for (i, s) in list.iter().enumerate() {
            assert!(list[i + 1..].iter().all(|other| other.key != s.key), "shortcut {} used twice", s.key);
        }
    }

    /// In command mode, what a printable key does: the category it opens (a shortcut), or
    /// `None` when it is a command key or does nothing.
    fn opens(config: &Config, c: char) -> Option<String> {
        let opts = options(config);
        let mut app = App::new(config, &opts);
        app.line.set("10");
        app.handle(Key::Esc);
        app.handle(Key::Char(c));
        match &app.overlay {
            Some(Overlay::Picker(p)) => Some(p.title(config).to_string()),
            Some(Overlay::Trivia(_)) => Some(crate::config::TRIVIA.0.to_string()),
            Some(Overlay::Search(_)) => Some(crate::config::SEARCH.0.to_string()),
            _ => None,
        }
    }

    /// Every shortcut opens its own category: in upper case always, in lower case unless the
    /// lower case is a command key (taken from what the keys do, not from a list): `k` is
    /// history up… so Temperature is `K` only.
    #[test]
    fn shortcuts_open_their_category() {
        let config = Config::default();
        for s in shortcuts(&config) {
            let key = s.key;
            assert_eq!(opens(&config, key).as_deref(), Some(s.name), "{key}");
            let lower = key.to_ascii_lowercase();
            match opens(&config, lower) {
                Some(name) => assert_eq!(name, s.name, "{lower}"),
                None => assert!(['k'].contains(&lower), "{lower} is a command key: only {key} opens {}", s.name),
            }
        }
        // No other letter opens a category.
        let keys: Vec<char> = shortcuts(&config).iter().flat_map(|s| [s.key, s.key.to_ascii_lowercase()]).collect();
        for c in ('a'..='z').chain('A'..='Z').filter(|c| !keys.contains(c)) {
            assert_eq!(opens(&config, c), None, "{c}");
        }
    }
    #[test]
    fn copy_an_error_entry() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10 foo bar");
        app.handle(Key::Enter);
        app.handle(Key::Up);
        app.handle(Key::Char('c'));
        assert_eq!(app.copied, None);
        assert!(matches!(&app.status, Some((StatusKind::Warning, m)) if m.contains("nothing to copy")));
        // In command mode, j / k browse and C copies (not the keypad) while an entry is highlighted.
        typed(&mut app, "x");
        app.handle(Key::Ctrl('u'));
        typed(&mut app, "2^8");
        app.handle(Key::Enter);
        app.handle(Key::Esc);
        app.handle(Key::Char('k'));
        let keypad = app.keypad;
        app.handle(Key::Char('C'));
        assert_eq!((app.copied.as_deref(), app.keypad), (Some("256"), keypad));
        app.handle(Key::Esc);
        app.handle(Key::Char('C'));
        assert_eq!(app.keypad, !keypad, "not browsing: C is the keypad again");
    }

    #[test]
    fn evaluate_and_recall() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        assert_eq!(app.mode, Mode::Input);
        typed(&mut app, "10 km mi");
        assert_eq!(app.preview(), Some(Ok("6.2137 mi".into())));
        app.handle(Key::Enter);
        assert!(app.line.is_empty());
        assert_eq!(app.history.entries()[0], Entry { input: "10 km mi".into(), result: Ok("6.2137 mi".into()) });

        // ↑ highlights the entry; the bar is not changed.
        app.handle(Key::Up);
        assert_eq!((app.history.selected(), app.line.text()), (Some(0), String::new()));
        // C copies its result (to the clipboard in a real session); browsing goes on.
        app.handle(Key::Char('C'));
        assert_eq!(app.copied.as_deref(), Some("6.2137 mi"));
        assert_eq!(app.status, Some((StatusKind::Note, "copied 6.2137 mi".into())));
        assert_eq!((app.history.selected(), app.line.text()), (Some(0), String::new()), "C is not typed");
        // Esc stops browsing, still in input mode; ↓ past the newest too.
        app.handle(Key::Esc);
        assert_eq!((app.history.selected(), app.mode), (None, Mode::Input));
        app.handle(Key::Up);
        app.handle(Key::Down);
        assert_eq!(app.history.selected(), None);
        // A key that is not a browsing key ends it and does what it does: here, typing.
        app.handle(Key::Up);
        typed(&mut app, "5");
        assert_eq!((app.history.selected(), app.line.text()), (None, "5".to_string()));
        app.handle(Key::Ctrl('u'));
        // Enter writes the highlighted expression into the bar, without evaluating it.
        app.handle(Key::Up);
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "10 km mi");
        assert_eq!((app.history.selected(), app.history.entries().len()), (None, 1));
        // Edit and evaluate again: a new entry, the old one is unchanged.
        app.handle(Key::Backspace);
        app.handle(Key::Backspace);
        typed(&mut app, "ft");
        app.handle(Key::Enter);
        let results: Vec<_> = app.history.entries().iter().map(|e| e.result.clone()).collect();
        assert_eq!(results, [Ok("6.2137 mi".to_string()), Ok("32808.399 ft".to_string())]);
    }

    #[test]
    fn modes() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        // In input mode, letters are text.
        typed(&mut app, "1 L");
        assert_eq!(app.line.text(), "1 L");
        assert!(app.overlay.is_none());
        app.handle(Key::Esc);
        assert_eq!(app.mode, Mode::Command);
        // In command mode, letters are commands: x deletes, 0 and $ move.
        app.handle(Key::Char('0'));
        app.handle(Key::Char('x'));
        app.handle(Key::Char('$'));
        assert_eq!((app.line.text(), app.line.cursor()), (" L".into(), 2));
        app.handle(Key::Char('i'));
        assert_eq!(app.mode, Mode::Input);
        // `I` works as well, and keeps the cursor where it is.
        app.handle(Key::Esc);
        app.handle(Key::Backspace);
        app.handle(Key::Char('I'));
        assert_eq!((app.mode, app.line.cursor()), (Mode::Input, 1));
        typed(&mut app, "x");
        assert_eq!(app.line.text(), " xL");
    }

    #[test]
    fn pick_source_and_target_in_command_mode() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10");
        app.handle(Key::Esc);
        // Z turns the keypad into the Size units, on the Length tab (mm cm m km / in ft yd mi);
        // l l l → km, Enter writes it and a space.
        app.handle(Key::Char('S'));
        assert!(matches!(&app.overlay, Some(Overlay::Picker(p)) if p.category(&config).name == "Length"));
        typed(&mut app, "lll");
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "10 km ");
        assert_eq!(app.mode, Mode::Command);
        // The buttons stay for the next unit. j: the imperial row, same column → mi.
        assert!(matches!(app.overlay, Some(Overlay::Picker(_))));
        typed(&mut app, "j");
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "10 km mi ");
        // The target is set: the expression is complete, so the buttons close and input mode is back.
        assert!(app.overlay.is_none());
        assert_eq!((app.mode, app.line.cursor()), (Mode::Input, 9));
        app.handle(Key::Enter);
        assert_eq!(app.history.entries()[0].result, Ok("6.2137 mi".into()));
    }
    #[test]
    fn vim_keys_in_the_unit_keys() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::Esc);
        app.handle(Key::Char('D'));
        let state = |app: &App| match &app.overlay {
            Some(Overlay::Picker(p)) => (p.category(&config).name, p.tab, p.items(&config)[p.selected].symbol),
            _ => panic!("no unit keys"),
        };
        // Data, Decimal: B kB MB GB TB / b kb Mb Gb Tb.
        assert_eq!(state(&app), ("Data", System::Metric, "B"));
        // h / l: left / right, circular.
        typed(&mut app, "ll");
        assert_eq!(state(&app).2, "MB");
        typed(&mut app, "hhh");
        assert_eq!(state(&app).2, "Tb", "left of the first unit: the last");
        typed(&mut app, "l");
        assert_eq!(state(&app).2, "B", "right of the last: the first");
        // j / k: down / up, circular.
        typed(&mut app, "j");
        assert_eq!(state(&app).2, "b");
        typed(&mut app, "j");
        assert_eq!(state(&app).2, "B", "below the last row: the first");
        typed(&mut app, "k");
        assert_eq!(state(&app).2, "b", "above the first row: the last");
        // Tab: the other tab, circular.
        app.handle(Key::Tab);
        assert_eq!(state(&app).1, System::Imperial);
        app.handle(Key::Tab);
        assert_eq!(state(&app).1, System::Metric);
        app.handle(Key::BackTab);
        assert_eq!(state(&app).1, System::Imperial);
        // A shortcut's letter shows its units: P, Other physical, on Mass; S, Size, on Length.
        typed(&mut app, "p");
        assert_eq!(state(&app).0, "Mass");
        typed(&mut app, "s");
        assert_eq!(state(&app).0, "Length");
        typed(&mut app, "t");
        assert_eq!((state(&app).0, state(&app).1), ("Time", System::Metric), "Time opens on Short");
        typed(&mut app, "x");
        assert_eq!(state(&app).0, "Time", "other letters do nothing");
    }    #[test]
    fn unit_buttons_move_by_row() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.system = System::Imperial;
        app.handle(Key::Esc);
        app.handle(Key::Char('s'));
        // Size; Tab Tab: the Volume tab, on the imperial row (the app's system): ml cl l m3 /
        // floz-us pt-us qt-us gal-us cup floz-uk pt-uk qt-uk gal-uk. ↑ ↓ one row; ← → one button.
        app.handle(Key::Tab);
        app.handle(Key::Tab);
        let selected = |app: &App| match &app.overlay {
            Some(Overlay::Picker(p)) => p.items(&config)[p.selected].symbol,
            _ => panic!("no picker"),
        };
        assert_eq!(selected(&app), "floz-us");
        app.handle(Key::Right);
        assert_eq!(selected(&app), "pt-us");
        app.handle(Key::Up);
        assert_eq!(selected(&app), "cl");
        app.handle(Key::Down);
        assert_eq!(selected(&app), "pt-us");
        app.handle(Key::Left);
        app.handle(Key::Left);
        assert_eq!(selected(&app), "m3", "left of the row's first: the row above's last");
        // Tab: the next tab (Angle), on the imperial row.
        app.handle(Key::Tab);
        assert_eq!(selected(&app), "deg");
        app.handle(Key::BackTab);
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "1 floz-us ");
        // Only the source: the buttons stay, still in command mode.
        assert!(matches!(app.overlay, Some(Overlay::Picker(_))));
        assert_eq!(app.mode, Mode::Command);
    }

    #[test]
    fn top_bar_highlight() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::Esc);
        // The first is highlighted; → / l move right, ← / h left, circular.
        assert_eq!(app.menu, 0);
        app.handle(Key::Right);
        app.handle(Key::Char('l'));
        assert_eq!(app.menu, 2);
        app.handle(Key::Char('h'));
        app.handle(Key::Left);
        app.handle(Key::Left);
        assert_eq!(app.menu, shortcuts(&config).len() - 1, "left of the first: the last");
        // Enter opens it: the Trivia.
        app.handle(Key::Enter);
        assert!(matches!(app.overlay, Some(Overlay::Trivia(_))));
        // A shortcut's letter moves the highlight to it.
        app.handle(Key::Esc);
        app.handle(Key::Char('S'));
        app.handle(Key::Esc);
        assert_eq!(shortcuts(&config)[app.menu].name, "Size");
        app.handle(Key::Enter);
        assert!(matches!(&app.overlay, Some(Overlay::Picker(p)) if p.title(&config) == "Size"));
    }

    #[test]
    fn trivia_writes_the_value() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::Esc);
        app.handle(Key::Char('R'));
        assert!(matches!(app.overlay, Some(Overlay::Trivia(_))));
        // Math, π: Enter shows its popup, Enter again writes it, ready for the rest of the expression.
        app.handle(Key::Tab);
        app.handle(Key::Tab);
        app.handle(Key::Tab);
        app.handle(Key::Enter);
        assert!(matches!(&app.overlay, Some(Overlay::Trivia(v)) if v.popup == Some(0)));
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "3.1415926536 ");
        assert!(app.overlay.is_none());
        assert_eq!(app.mode, Mode::Input);
        // Earth's first: a tip, writing its example request.
        app.handle(Key::Esc);
        app.handle(Key::Char('r'));
        app.handle(Key::Enter);
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "London to Paris km");
        assert_eq!(app.preview(), Some(Ok("344 km".into())));
        // Earth, Equator: with its unit, so a target converts it.
        app.handle(Key::Esc);
        app.handle(Key::Char('r'));
        app.handle(Key::Char('j'));
        app.handle(Key::Enter);
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "40075.017 km ");
        typed(&mut app, "mi");
        assert_eq!(app.preview(), Some(Ok("24901.4611 mi".into())));
        // Another shortcut from the Trivia.
        app.handle(Key::Esc);
        app.handle(Key::Char('R'));
        app.handle(Key::Char('D'));
        assert!(matches!(&app.overlay, Some(Overlay::Picker(p)) if p.title(&config) == "Data"));
    }

    #[test]
    fn search_writes_the_unit() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10");
        app.handle(Key::Esc);
        app.handle(Key::Char('/'));
        // Letters are the query, not commands (h, j, k, q…).
        typed(&mut app, "kilometre");
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "10 km ");
        assert!(app.overlay.is_none());
        app.handle(Key::Char('/'));
        typed(&mut app, "mile");
        app.handle(Key::Down);
        app.handle(Key::Up);
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "10 km mi ");
        assert_eq!(app.mode, Mode::Input, "source and target: complete");
        // Esc closes it without writing.
        app.handle(Key::Esc);
        app.handle(Key::Char('/'));
        typed(&mut app, "kg");
        app.handle(Key::Esc);
        assert_eq!((app.line.text(), app.overlay.is_none()), ("10 km mi ".to_string(), true));
    }

    #[test]
    fn earth_distance_writes_the_request() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::Esc);
        app.handle(Key::Char('S'));
        for _ in 0..4 {
            app.handle(Key::Tab);
        }
        app.handle(Key::Enter);
        assert_eq!((app.line.text(), app.mode), ("distance ".to_string(), Mode::Input));
        typed(&mut app, "Rome to Paris");
        assert_eq!(app.preview(), Some(Ok("1107 km".into())));
    }

    #[test]
    fn n_writes_now() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::Esc);
        app.handle(Key::Char('N'));
        assert_eq!(app.line.text(), "now ");
        app.handle(Key::Char('i'));
        typed(&mut app, "+ 3d");
        assert!(app.preview().is_some_and(|r| r.is_ok_and(|v| v.contains('T'))), "a date: {:?}", app.preview());
        app.handle(Key::Esc);
        app.handle(Key::Char('n'));
        assert_eq!(app.line.text(), "now + 3d now ");
    }
    #[test]
    fn data_buttons_five_per_row() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::Esc);
        app.handle(Key::Char('D'));
        // Decimal: B kB MB GB TB / b kb Mb Gb Tb, so ↓ goes from bytes to the same size in bits.
        app.handle(Key::Right);
        app.handle(Key::Right);
        app.handle(Key::Down);
        assert!(matches!(&app.overlay, Some(Overlay::Picker(p)) if p.items(&config)[p.selected].symbol == "Mb"));
    }

    #[test]
    fn unit_keys_open_on_the_device_system() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.system = System::Imperial;
        app.handle(Key::Esc);
        app.handle(Key::Char('P'));
        assert!(matches!(&app.overlay, Some(Overlay::Picker(p)) if p.items(&config)[p.selected].symbol == "oz"), "Mass: the imperial row");
        app.handle(Key::Esc);
        app.handle(Key::Char('S'));
        assert!(matches!(&app.overlay, Some(Overlay::Picker(p)) if p.items(&config)[p.selected].symbol == "in"), "Size: the imperial row");
        app.handle(Key::Esc);
        app.handle(Key::Char('D'));
        assert!(matches!(&app.overlay, Some(Overlay::Picker(p)) if p.tab == System::Metric), "Data: decimal");
    }

    #[test]
    fn picker_esc_cancels() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10 km");
        app.handle(Key::Esc);
        app.handle(Key::Char('M'));
        typed(&mut app, "kk");
        app.handle(Key::Esc);
        assert!(app.overlay.is_none());
        assert_eq!(app.line.text(), "10 km");
        assert_eq!(app.mode, Mode::Command);
    }
    #[test]
    fn pick_replaces_the_unit_under_the_cursor() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10 km mi");
        app.handle(Key::Esc);
        app.handle(Key::Char('0'));
        app.handle(Key::Char('w'));
        // Size, the imperial row (j), ft (l).
        app.handle(Key::Char('S'));
        typed(&mut app, "jl");
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "10 ft mi", "replaced in place: no space added");
        assert_eq!(app.mode, Mode::Input, "source and target set: complete");
    }    #[test]
    fn help() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "e");
        assert!(app.overlay.is_none(), "e is text in input mode");
        app.handle(Key::Esc);
        app.handle(Key::Char('E'));
        assert!(matches!(app.overlay, Some(Overlay::Help(0))));
        app.handle(Key::Char('j'));
        app.handle(Key::Down);
        app.handle(Key::Char('k'));
        assert!(matches!(app.overlay, Some(Overlay::Help(1))), "j/k scroll and keep the help open");
        app.handle(Key::Char('S'));
        assert!(matches!(app.overlay, Some(Overlay::Help(1))), "shortcuts do nothing under the help");
        app.handle(Key::Esc);
        assert!(app.overlay.is_none());
    }

    #[test]
    fn preview_asks_for_the_target() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "25 km ");
        assert_eq!(app.preview(), Some(Err("now the target unit".into())));
        app.handle(Key::Ctrl('u'));
        typed(&mut app, "25 foo");
        assert!(matches!(app.preview(), Some(Err(m)) if m.contains("expected")), "unknown unit: the real error");
        app.handle(Key::Ctrl('u'));
        typed(&mut app, "12/04/2026");
        assert_eq!(app.preview(), Some(Err("now the target format (T, then Tab: Formats)".into())));
        // A written date with a time is complete: its ISO reading, without the unwritten seconds.
        app.handle(Key::Ctrl('u'));
        typed(&mut app, "2026/12/04 10:00 PM");
        assert!(matches!(app.preview(), Some(Ok(p)) if p.starts_with("04 Dec 26 10:00 PM GMT")), "for people: {:?}", app.preview());
        app.handle(Key::Ctrl('u'));
        typed(&mut app, "12/04/26 3:20PM");
        assert!(matches!(app.preview(), Some(Err(m)) if m.starts_with("ambiguous")), "{:?}", app.preview());
        app.handle(Key::Ctrl('u'));
        typed(&mut app, "12/4");
        assert_eq!(app.preview(), Some(Ok("3".into())), "a division stays a division");
    }

    #[test]
    fn preview_uses_the_number_format() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "1 GiB MB");
        assert_eq!(app.preview(), Some(Ok("1073.7418 MB".into())), "plain by default");
        app.numbers = numbers::NumberFormat { decimal: ",".into(), thousands: ".".into(), grouping: vec![3, 3] };
        assert_eq!(app.preview(), Some(Ok("1.073,7418 MB".into())));
        // Only the preview: the history keeps the plain result.
        app.handle(Key::Enter);
        assert_eq!(app.history.entries()[0].result, Ok("1073.7418 MB".into()));
    }

    #[test]
    fn preview_grouping() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "12+12/3");
        assert_eq!(app.grouping().as_deref(), Some("(12 + (12 / 3))"));
        assert_eq!(app.preview(), Some(Ok("16".into())));
        app.handle(Key::Ctrl('u'));
        typed(&mut app, "100 + 20 sqft m2");
        assert_eq!(app.grouping().as_deref(), Some("(100 + 20)"));
        app.handle(Key::Ctrl('u'));
        for plain in ["10 km mi", "-40 C F", "", "abc", "2026-12-25 - now d", "now + 3d"] {
            typed(&mut app, plain);
            assert_eq!(app.grouping(), None, "{plain}");
            app.handle(Key::Ctrl('u'));
        }
    }

    #[test]
    fn keypad_toggle_and_pressed_key() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        assert!(app.keypad, "shown at the start");
        typed(&mut app, "7");
        assert_eq!(app.pressed, Some('7'));
        typed(&mut app, "x");
        assert_eq!(app.pressed, Some('*'), "x is multiply");
        // In input mode C is text; in command mode it toggles the keypad.
        typed(&mut app, "C");
        assert!(app.keypad);
        app.handle(Key::Esc);
        assert_eq!(app.pressed, None);
        app.handle(Key::Char('C'));
        assert!(!app.keypad);
        app.handle(Key::Char('c'));
        assert!(app.keypad);
        // Enter lights = in input mode (in command mode it opens a shortcut).
        app.handle(Key::Char('i'));
        app.handle(Key::Enter);
        assert_eq!(app.pressed, Some('='));
    }

    /// Bug 007: a written date with date math is complete: its result previews, no "target unit"
    /// prompt and no unit suggestions.
    #[test]
    fn written_date_math_is_complete() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "2026/12/24 - 12d");
        let preview = app.preview().unwrap().unwrap();
        assert_eq!(preview, "2026/12/12 YYYY/MM/DD", "only the result, in the input's format");
        assert!(app.suggestions().is_none());
        app.handle(Key::Enter);
        assert!(app.history.entries()[0].result.as_ref().is_ok_and(|r| r.ends_with("2026/12/12 YYYY/MM/DD")));
        // Picking the duration's unit on the unit keys completes the expression: input mode.
        typed(&mut app, "2026/12/24 - 12");
        app.handle(Key::Esc);
        app.handle(Key::Char('T'));
        for _ in 0..6 {
            app.handle(Key::Char('l'));
        }
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "2026/12/24 - 12 d ");
        assert!(app.overlay.is_none());
        assert_eq!(app.mode, Mode::Input);
        assert!(app.preview().unwrap().unwrap().ends_with("2026/12/12 YYYY/MM/DD"));
    }

    #[test]
    fn ambiguous_date_popup() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "12/04/2026 unix");
        assert!(matches!(app.preview(), Some(Err(m)) if m.starts_with("ambiguous: ") && m.contains("DD/MM/YYYY")));
        app.handle(Key::Enter);
        let Some(Overlay::Choice(a, 0)) = &app.overlay else { panic!("no popup") };
        assert_eq!(a.choices.len(), 2);
        assert!(app.history.entries().is_empty(), "nothing evaluated yet");
        // Choose the DD/MM/YYYY reading, wherever it is, then Enter.
        let dmy = a.choices.iter().position(|c| c.pattern == "DD/MM/YYYY").unwrap();
        for _ in 0..dmy {
            app.handle(Key::Char('j'));
        }
        app.handle(Key::Enter);
        assert!(app.overlay.is_none());
        let entry = &app.history.entries()[0];
        assert_eq!(entry.input, "12/04/2026 DD/MM/YYYY unix", "the pattern is written in");
        // Midnight of 12 April in the local zone, whatever it is (Rome here, UTC on CI): the same
        // as the unambiguous ISO date.
        let iso = crate::time::request(&config, "2026-04-12T00:00 unix", 4, false).unwrap().unwrap().0;
        assert_eq!(entry.result, Ok(iso), "only the result (no --verbose)");
        // Esc: back to the expression, unchanged.
        typed(&mut app, "12/04/2026 unix");
        app.handle(Key::Enter);
        app.handle(Key::Char('2'));
        assert!(matches!(&app.overlay, Some(Overlay::Choice(_, 1))), "digits choose too");
        app.handle(Key::Esc);
        assert!(app.overlay.is_none());
        assert_eq!(app.line.text(), "12/04/2026 unix");
        assert_eq!(app.history.entries().len(), 1);
    }

    #[test]
    fn errors_notes_and_warnings() {
        let config = Config::default();
        // Notes only with --verbose.
        let opts = Options { verbose: true, ..options(&config) };
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10 km kg");
        app.handle(Key::Enter);
        assert!(matches!(&app.history.entries()[0].result, Err(m) if m.contains("different kinds")));
        assert_eq!(app.status.as_ref().unwrap().0, StatusKind::Error);
        typed(&mut app, "100 + 20 + 50 / 10");
        app.handle(Key::Enter);
        assert_eq!(app.status, Some((StatusKind::Note, "evaluated as (100 + 20) + (50 / 10)".into())));
        typed(&mut app, "1 GB GiB");
        app.handle(Key::Enter);
        assert_eq!(app.status.as_ref().unwrap().0, StatusKind::Warning);
    }

    #[test]
    fn tab_cycles_suggestions() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10 km m");
        app.handle(Key::Tab);
        assert_eq!(app.line.text(), "10 km mm");
        app.handle(Key::Tab);
        assert_eq!(app.line.text(), "10 km m");
        app.handle(Key::Tab);
        assert_eq!(app.line.text(), "10 km mi");
        app.handle(Key::BackTab);
        assert_eq!(app.line.text(), "10 km m");
        assert_eq!(app.suggestions().unwrap().1, Some(1));
        app.handle(Key::Char('2'));
        assert!(app.suggestions().is_none());
    }

    #[test]
    fn draft_and_quit_keys() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "1+1");
        app.handle(Key::Enter);
        typed(&mut app, "draft");
        app.handle(Key::Up);
        app.handle(Key::Down);
        assert_eq!(app.line.text(), "draft");
        app.handle(Key::Ctrl('u'));
        app.handle(Key::Ctrl('d'));
        assert!(app.quit);
        let mut app = App::new(&config, &opts);
        app.handle(Key::Esc);
        app.handle(Key::Char('q'));
        assert!(app.quit);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "quit");
        app.handle(Key::Enter);
        assert!(app.quit);
    }

    #[test]
    fn about_from_both_modes() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "about");
        app.handle(Key::Enter);
        assert!(matches!(app.overlay, Some(Overlay::About)) && app.line.is_empty() && app.history.entries().is_empty());
        app.handle(Key::Esc);
        assert!(app.overlay.is_none());
        app.handle(Key::Esc);
        app.handle(Key::Char('A'));
        assert!(matches!(app.overlay, Some(Overlay::About)));
    }

}
