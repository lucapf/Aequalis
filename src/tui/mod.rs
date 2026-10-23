//! Interactive mode: `aequalis` with no arguments in a terminal.
//!
//! Sections, top to bottom: the toolbar (F2 Measures, F3 Currencies, F4 Sizes, F5 History, F6 Trivia,
//! F10 Settings), the history of expressions and results as a scrollback, the request line with
//! its live result and suggestions, the band (the keypad when asked for, or the panel of a
//! window, which takes the rows it needs), and a status line. There are no modes: typing always edits
//! the request. Every toolbar entry has an Alt letter too (Alt+M, C, Z, H, T, B, S), for terminals that
//! keep the function keys; F1 / Alt+? is the help, Ctrl+K searches the units and the currencies, F9 shows the
//! keypad (Basic, then Sci) and hides it, Alt+Y copies the result, Alt+N writes `now`, Ctrl+Q quits. ↑ ↓ recall earlier requests
//! as in a shell; F5 puts the history in focus (↑ ↓ choose, Enter recalls, Alt+Y copies, Del
//! removes). Esc steps back: the open window, the history focus, the Tab cycle, the status
//! message, then it clears the line (↑ brings the cleared text back). Ctrl+C does the same and,
//! with nothing left to close or clear, quits.
//!
//! The state machine (`App`), key decoding and rendering are free of I/O; only `term` and `run`
//! touch the terminal.

mod action;
mod board;
mod clipboard;
mod currencies;
mod editor;
mod history_panel;
mod keys;
mod measures;
mod picker;
mod render;
mod search;
mod settings;
mod sizes;
mod term;
mod trivia;

use aequalis::config::{Config, System};
use aequalis::favourites::{self, Favourites};
use aequalis::numbers;
use aequalis::{convert_request, Options};
use editor::{Entry, History, Line};
use keys::Key;
use picker::{Direction, Picker};
use std::cell::RefCell;
use std::io::Write;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusKind {
    Note,
    Warning,
    Error,
}

/// An error as the TUI shows it: with the results it already has under it (`42 eu`: the shoe
/// sizes, while the currency to convert to is missing).
fn shown_error(e: &aequalis::CliError) -> String {
    match e {
        aequalis::CliError::Incomplete(hint, more) if !more.is_empty() => {
            std::iter::once(hint.clone()).chain(more.iter().map(|m| format!("- {}", m.line()))).collect::<Vec<_>>().join("\n")
        }
        e => e.to_string(),
    }
}

/// Drawn over the sections.
pub enum Overlay {
    /// `M`: the unit classes (Length, Temperature, …).
    Measures(measures::View),
    /// A class's units, on the keypad (Esc: back to the classes).
    Picker(Picker),
    /// `U`: searching the currencies.
    Currencies(currencies::View),
    /// F4: the Sizes class, its groups and system keys.
    Sizes(sizes::View),
    /// The help, scrolled down by this many lines.
    Help(usize),
    /// An ambiguous date (`12/04/2026`): which reading is meant; `selected` into its choices.
    Choice(aequalis::time::Ambiguity, usize),
    /// The Trivia (`R`).
    Trivia(trivia::View),
    /// `/`: searching the units.
    Search(search::Search),
    /// F5: the requests evaluated, to recall, copy or remove.
    History(history_panel::View),
    /// `A`: the logo, the version, memory, the last online sync.
    About,
    /// `O`: the settings.
    Settings(settings::View),
}

impl Overlay {
    /// A panel: it takes the band under the line (or the pane at the right), where the keyboard
    /// would be; the other windows are drawn over the screen.
    pub fn is_panel(&self) -> bool {
        matches!(
            self,
            Overlay::Measures(_)
                | Overlay::Picker(_)
                | Overlay::Currencies(_)
                | Overlay::Sizes(_)
                | Overlay::Trivia(_)
                | Overlay::Search(_)
                | Overlay::History(_)
        )
    }
}

/// The keyboard of the band: the calculator keys, with the functions beside them in Sci.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Keypad {
    Hidden,
    Basic,
    Sci,
}

/// A part of the keyboard Tab goes to: the two layouts, then the columns of shortcuts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Part {
    Basic,
    Sci,
    Units,
    Money,
}

/// Tab cycling through the suggestions for one word.
struct Cycle {
    start: usize,
    names: Vec<String>,
    labels: Vec<String>,
    index: usize,
}

pub struct App<'a> {
    config: &'a Config,
    opts: &'a Options<'a>,
    overlay: Option<Overlay>,
    line: Line,
    history: History,
    /// Notes, warnings or the error of the last evaluation.
    status: Option<(StatusKind, String)>,
    /// History lines scrolled up from the bottom.
    scroll: usize,
    cycle: Option<Cycle>,
    /// The keypad in the band under the request line (F9: Basic, Sci, hidden).
    keypad: Keypad,
    /// The measurement system the unit keys open on: the device's (`locale`).
    system: System,
    /// Decimal and thousands separators for the preview: the device's (`numbers`).
    numbers: numbers::NumberFormat,
    /// The keypad key to light up: the last character typed (`=` for Enter).
    pressed: Option<char>,
    /// The units and the currencies of the keypad's shortcuts (Alt+1…5, Alt+6…0) with their hits;
    /// `lit` is the shortcut used last (0-4 a unit, 5-9 a currency).
    units: Favourites,
    currencies: Favourites,
    lit: Option<usize>,
    /// How often each unit, currency and city was used (`usage.json`): the board's order.
    usage: aequalis::usage::Usage,
    /// The part of the keyboard that has the focus (letters and digits still go to the line), and
    /// the entry of the Units or Money column chosen.
    focus: Option<Part>,
    slot: usize,
    /// The terminal's rows and columns.
    screen: (usize, usize),
    /// What a click does, as the last frame drew it; whether the mouse is reported to the program
    /// (`tui.mouse`, Alt+O), and whether the terminal was last told so.
    hits: Vec<render::Hit>,
    mouse_on: bool,
    /// The board's list of measures: the first entry shown, and a press on the board waiting to
    /// be a click or the start of a drag.
    board_scroll: usize,
    pressed_on: Option<Press>,
    /// Whether the hits are kept in the settings (only in a real session: never in the tests).
    persist: bool,
    redraw: bool,
    quit: bool,
    /// The spacecraft's distances in the Trivia, retrieved when it first opens if `fetch_live`
    /// (only in a real session: never in the tests).
    /// The online data (`online.rs`), reloaded when its file changes; `online_stamp` is the file's
    /// stamp when read (`files::stamp`).
    online: aequalis::online::Data,
    online_stamp: Option<aequalis::files::Stamp>,
    /// The background sync started from here (to reap), and when expiry was last checked.
    sync: Option<std::process::Child>,
    sync_checked: Option<Instant>,
    /// The exchange rates and the local currency, for the search's currencies (read when it opens,
    /// and only in a real session: `live`, never in the tests).
    rates: std::sync::Arc<aequalis::online::Data>,
    local_currency: Option<&'static str>,
    live: bool,
    /// The `offline` setting (read once).
    forced_offline: bool,
    /// The entry of the history recalled into the line by ↑ ↓ (as in a shell), and the text the
    /// line had before ↑ (or before Esc cleared it), which ↓ past the newest brings back.
    recall: Option<usize>,
    draft: Option<String>,
    /// The last result copied from the history (`C`); to the system clipboard too when
    /// `system_clipboard` (only in a real session: never in the tests).
    copied: Option<String>,
    system_clipboard: bool,
    /// The device's country (to rank the cities of the search), read when the search opens, and the
    /// last search made (query, how many requests were in the history) and its matches.
    country: Option<String>,
    clocks: RefCell<Option<(i64, Rc<Vec<board::Clock>>)>>,
    found: RefCell<Option<LastSearch>>,
    /// The last request evaluated and its result: the preview, the currencies line and Enter
    /// share it (one evaluation per change of the bar, not one per use).
    evaluation: RefCell<Option<(String, Evaluated)>>,
}

/// The last search of Ctrl+K: its query, how many requests the history had, and its matches.
type LastSearch = (String, usize, Rc<Vec<aequalis::find::Found>>);

/// A press of the left button on the board: a click when released in place, a drag of the list
/// when the pointer moved (a finger on a touch screen).
#[derive(Debug, Clone, Copy)]
struct Press {
    hit: Option<render::Hit>,
    row: usize,
    col: usize,
    scroll: usize,
    dragged: bool,
}

/// A request's result, shared by its uses.
type Evaluated = Rc<Result<aequalis::Output, aequalis::CliError>>;

const SCROLL_STEP: usize = 5;

impl<'a> App<'a> {
    pub fn new(config: &'a Config, opts: &'a Options<'a>) -> Self {
        // The tests link the library as the program does, without its `cfg(test)` plain format:
        // they must not depend on the machine's locale either (first call wins: every test makes
        // an App before formatting a number).
        #[cfg(test)]
        numbers::set_device(numbers::NumberFormat::default());
        App {
            config,
            opts,
            overlay: None,
            line: Line::default(),
            history: History::new(config.max_history),
            status: None,
            scroll: 0,
            cycle: None,
            keypad: Keypad::Hidden,
            system: System::Metric,
            numbers: numbers::NumberFormat::default(),
            pressed: None,
            units: Favourites::of("", &favourites::METRIC),
            currencies: Favourites::of("", &favourites::CURRENCIES),
            lit: None,
            usage: aequalis::usage::Usage::default(),
            focus: None,
            slot: 0,
            screen: (24, 80),
            hits: Vec::new(),
            mouse_on: true,
            board_scroll: 0,
            pressed_on: None,
            persist: false,
            redraw: false,
            quit: false,
            online: aequalis::online::Data::default(),
            online_stamp: None,
            sync: None,
            sync_checked: None,
            rates: std::sync::Arc::default(),
            local_currency: None,
            live: false,
            forced_offline: aequalis::network::mode() == aequalis::network::Mode::Offline,
            recall: None,
            draft: None,
            copied: None,
            system_clipboard: false,
            country: None,
            clocks: RefCell::new(None),
            found: RefCell::new(None),
            evaluation: RefCell::new(None),
        }
    }

    /// Whether the keyboard is on screen (not hidden, a terminal tall enough, no window over it).
    fn keyboard_shown(&self) -> bool {
        self.keypad != Keypad::Hidden && self.screen.0 >= render::KEYPAD_FROM && !self.overlay.as_ref().is_some_and(Overlay::is_panel)
    }

    /// The columns the keyboard has: its column beside the board in the Wide layout (big keys),
    /// else the pane's, else the terminal's.
    fn keyboard_cols(&self) -> usize {
        match render::wide_split(self.screen.0, self.screen.1) {
            Some((_, keyboard)) => keyboard,
            None => render::pane_width(self.screen.0, self.screen.1).unwrap_or(self.screen.1),
        }
    }

    /// The layout the keyboard opens on: Sci when it fits (with the shortcuts, or in its column
    /// beside the board), else Basic.
    fn best_layout(&self) -> Keypad {
        let fits = match render::wide_split(self.screen.0, self.screen.1) {
            Some((_, keyboard)) => render::big_sci_fits(keyboard),
            None => render::shortcuts_fit(self.keyboard_cols(), true),
        };
        if fits {
            Keypad::Sci
        } else {
            Keypad::Basic
        }
    }

    /// The parts Tab goes through: the layouts, and the shortcuts when there is room for them (the
    /// big keyboard beside the board has none: the board has the units and the currencies).
    fn parts(&self) -> Vec<Part> {
        let mut parts = vec![Part::Basic, Part::Sci];
        let beside_the_board = render::wide_split(self.screen.0, self.screen.1).is_some();
        if !beside_the_board && render::shortcuts_fit(self.keyboard_cols(), self.keypad == Keypad::Sci) {
            parts.extend([Part::Units, Part::Money]);
        }
        parts
    }

    /// Tab (`forward`) or Shift+Tab: the focus to the next part, round; from the line, to the
    /// layout shown (to the last part going back). A layout is shown as soon as it has the focus.
    fn move_focus(&mut self, forward: bool) {
        let parts = self.parts();
        let n = parts.len();
        let at = self.focus.and_then(|f| parts.iter().position(|p| *p == f));
        let to = match (at, forward) {
            (Some(i), true) => (i + 1) % n,
            (Some(i), false) => (i + n - 1) % n,
            (None, true) => usize::from(self.keypad == Keypad::Sci),
            (None, false) => n - 1,
        };
        self.focus = Some(parts[to]);
        match parts[to] {
            Part::Basic => self.keypad = Keypad::Basic,
            Part::Sci => self.keypad = Keypad::Sci,
            Part::Units | Part::Money => {}
        }
    }

    /// The keys the keyboard takes while it is on screen: Tab / Shift+Tab across its parts, Esc
    /// back to the line, and in the Units and Money columns ↑ ↓ to choose, ← → to change column,
    /// Enter to write. Everything else goes on to the line (`false`).
    fn keyboard_key(&mut self, key: Key) -> bool {
        if !self.keyboard_shown() {
            self.focus = None;
            return false;
        }
        let column = |part: Option<Part>| matches!(part, Some(Part::Units | Part::Money));
        match key {
            Key::Tab => self.move_focus(true),
            Key::BackTab => self.move_focus(false),
            Key::Esc if self.focus.is_some() => self.focus = None,
            Key::Up if column(self.focus) => self.slot = (self.slot + favourites::SLOTS - 1) % favourites::SLOTS,
            Key::Down if column(self.focus) => self.slot = (self.slot + 1) % favourites::SLOTS,
            Key::Left if column(self.focus) => self.focus = Some(Part::Units),
            Key::Right if column(self.focus) => self.focus = Some(Part::Money),
            Key::Enter if column(self.focus) => {
                let base = if self.focus == Some(Part::Money) { 5 } else { 0 };
                self.apply(action::Action::Shortcut(base + self.slot));
            }
            _ => return false,
        }
        true
    }

    /// A unit (or a currency) written into the request where the cursor is, and counted.
    fn write_unit(&mut self, name: &str, currency: bool) {
        let line = picker::place_unit(&self.line.text(), self.line.cursor(), name);
        self.line.set(&line);
        self.used(name, currency);
    }

    /// The currency the board values the others in: the setting, else the local currency.
    pub(super) fn base_currency(&self) -> Option<String> {
        aequalis::settings::current().currency().map(str::to_string).or_else(|| self.local_currency.map(str::to_string))
    }

    /// The board's currencies against the base.
    pub(super) fn board_money(&self) -> Vec<board::Money> {
        board::currencies(&self.usage, &self.rates, self.base_currency().as_deref())
    }

    /// The clocks of the board: our own zone, then the common cities; made once a minute.
    pub(super) fn clocks(&self) -> Rc<Vec<board::Clock>> {
        let minute = aequalis::time::unix_now() / 60;
        if let Some((at, clocks)) = &*self.clocks.borrow() {
            if *at == minute {
                return Rc::clone(clocks);
            }
        }
        // Plainly, whatever the notes the session shows: only the time is wanted.
        let plain = Options { positional: Vec::new(), precision: self.opts.precision, silent: true, verbose: false };
        let print = |request: &str| {
            let tokens: Vec<&str> = request.split_whitespace().collect();
            match convert_request(self.config, &plain, &tokens) {
                Ok(out) => Some(out.stdout.trim().to_string()),
                Err(_) => None,
            }
        };
        let here = print("now").and_then(|printed| board::parse_time(&printed)).map(|(_, day, utc)| (day, utc));
        let mut clocks = Vec::new();
        if let Some(here) = here {
            for city in board::COMMON_CITIES {
                if let Some(clock) = print(&format!("now {city}")).and_then(|printed| board::clock(city, &printed, here)) {
                    clocks.push(clock);
                }
            }
        }
        let clocks = Rc::new(clocks);
        *self.clocks.borrow_mut() = Some((minute, Rc::clone(&clocks)));
        clocks
    }

    /// The shortcuts as the settings keep them (the units' defaults follow the measurement system).
    fn load_shortcuts(&mut self) {
        let stored = aequalis::settings::current();
        self.units = Favourites::of(stored.favourites(false), &favourites::default_units(self.system));
        self.currencies = Favourites::of(stored.favourites(true), &favourites::CURRENCIES);
    }

    /// What the shortcut `index` writes (0-4 a unit's symbol, 5-9 a currency's code).
    fn shortcut(&self, index: usize) -> Option<&str> {
        match index {
            0..=4 => self.units.names().nth(index),
            _ => self.currencies.names().nth(index - 5),
        }
    }

    /// A unit (`currency` false) or a currency was used: a hit for it, kept in the settings.
    fn used(&mut self, name: &str, currency: bool) {
        let (list, key) = match currency {
            true => (&mut self.currencies, "tui.favourite_currencies"),
            false => (&mut self.units, "tui.favourite_units"),
        };
        list.hit(name);
        self.usage.hit(if currency { aequalis::usage::Kind::Currency } else { aequalis::usage::Kind::Unit }, name);
        if self.persist {
            let _ = aequalis::settings::change(key, Some(&list.to_text()));
            aequalis::usage::save(&self.usage);
        }
    }

    /// The units and currencies a request that worked used: a hit for each, once.
    fn used_in(&mut self, request: &str) {
        let mut seen: Vec<(String, bool)> = Vec::new();
        for word in request.split(|c: char| c.is_whitespace() || "(),".contains(c)).filter(|w| !w.is_empty()) {
            let found = match aequalis::convert::find_unit(self.config, word) {
                Some((_, unit)) => Some((unit.symbol.to_string(), false)),
                None if word.len() == 3 && word.chars().all(|c| c.is_ascii_alphabetic()) => match aequalis::currency::exact(word).as_deref() {
                    Some(&[code]) => Some((code.to_string(), true)),
                    _ => None,
                },
                None => None,
            };
            if let Some(found) = found.filter(|f| !seen.contains(f)) {
                seen.push(found);
            }
        }
        for (name, currency) in seen {
            self.used(&name, currency);
        }
    }

    /// Online or offline: the last sync's result (`network::status_of`).
    pub fn network_status(&self) -> aequalis::network::Status {
        aequalis::network::status_of(&self.online, self.forced_offline)
    }

    /// How long ago the online data was last synced (`2h`; `None`: never).
    pub fn synced_ago(&self) -> Option<String> {
        aequalis::network::since_sync(&self.online)
    }

    /// Reloads the online data when its file changed (the background sync wrote it): `true` then,
    /// for a redraw.
    pub fn refresh_online(&mut self) -> bool {
        let Some(path) = aequalis::online::path() else { return false };
        let stamp = aequalis::files::stamp(&path);
        if stamp == self.online_stamp {
            return false;
        }
        self.online_stamp = stamp;
        self.online = aequalis::online::load_from(&path);
        // The exchange rates came with it.
        self.refresh_money();
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
        self.sync = aequalis::online::ensure_fresh();
    }

    /// Opens a unit class from the Measures: its units, or for the Earth distance the request
    /// written in the bar (the places are typed).
    fn open_measure(&mut self, item: measures::Item) {
        match item {
            measures::Item::Category(i) => self.overlay = Some(Overlay::Picker(Picker::new(self.config, i, self.system))),
            measures::Item::EarthDistance => {
                self.line.set("distance ");
                self.overlay = None;
            }
            measures::Item::Sizes => self.overlay = Some(Overlay::Sizes(sizes::View::default())),
        }
    }

    /// The Measures' keys: ↑ ↓ (j k) choose, a class's letter or Enter opens it, Esc closes.
    fn measures_key(&mut self, key: Key) {
        let Some(Overlay::Measures(v)) = &mut self.overlay else { return };
        match key {
            Key::Esc | Key::Char('q') => self.overlay = None,
            Key::Down | Key::Char('j') | Key::Tab => v.step(self.config, true),
            Key::Up | Key::Char('k') | Key::BackTab => v.step(self.config, false),
            Key::Enter | Key::Right | Key::Char('l') => {
                let item = v.item(self.config);
                self.open_measure(item);
            }
            Key::Char(c) => {
                if let Some(item) = measures::by_key(self.config, c) {
                    self.open_measure(item);
                }
            }
            _ => {}
        }
    }

    /// The currencies' keys: typing edits the query, ↑ ↓ choose, Enter writes the code.
    fn currencies_key(&mut self, key: Key) {
        let Some(Overlay::Currencies(v)) = &mut self.overlay else { return };
        let found = aequalis::currency::search(&v.query).len();
        match key {
            Key::Esc => self.overlay = None,
            Key::Char(c) => {
                v.query.push(c);
                v.selected = 0;
            }
            Key::Backspace => {
                v.query.pop();
                v.selected = 0;
            }
            Key::Down | Key::Tab if found > 0 => v.selected = (v.selected + 1) % found,
            Key::Up | Key::BackTab if found > 0 => v.selected = (v.selected + found - 1) % found,
            Key::Enter => {
                if let Some(code) = aequalis::currency::search(&v.query).get(v.selected).map(|c| c.code) {
                    let line = picker::place_unit(&self.line.text(), self.line.cursor(), code);
                    self.line.set(&line);
                    self.overlay = None;
                    self.used(code, true);
                }
            }
            _ => {}
        }
    }

    /// A key of the Sizes (`shoes`, `eu`) after what the line holds, `shoes` at its start when it has
    /// no item word yet (`42` + `eu` → `shoes 42 eu `): sizes, not a currency.
    fn write_size_key(&mut self, key: &str) {
        self.line.set(&aequalis::sizes::write_key(&self.line.text(), key));
    }

    /// The Sizes' keys: ↑ ↓ (j k) choose, Enter writes the group's word or the system's code; the
    /// size's digits (and ⌫) go on into the request, the window open (`12`, ↓ to US, Enter:
    /// `shoes 12 us`).
    fn sizes_key(&mut self, key: Key) {
        let Some(Overlay::Sizes(v)) = &mut self.overlay else { return };
        match key {
            Key::Esc => self.overlay = None,
            Key::Char(c) if c.is_ascii_digit() || matches!(c, '.' | ',' | '½' | '⅓' | '⅔') => self.line.insert(c),
            Key::Backspace => self.line.backspace(),
            Key::Down | Key::Tab | Key::Char('j') => v.step(true),
            Key::Up | Key::BackTab | Key::Char('k') => v.step(false),
            Key::Enter => {
                if let Some(word) = v.word() {
                    self.write_size_key(word);
                    self.overlay = None;
                }
            }
            _ => {}
        }
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
            _ => {}
        }
    }

    /// The selected entry's value written into the bar (a tip: its example).
    fn write_trivia(&mut self) {
        let Some(Overlay::Trivia(v)) = &self.overlay else { return };
        let Some(entry) = v.entry() else { return };
        // A shoe table: the chosen row's request.
        if let aequalis::trivia::Source::Table(table) = entry.source {
            let rows = aequalis::trivia::table_rows(table);
            let chosen = v.popup.unwrap_or(0).min(rows.len().saturating_sub(1));
            if let Some(row) = rows.get(chosen) {
                self.line.set(&format!("{} ", row.request));
                self.overlay = None;
            }
            return;
        }
        if self.write_entry(entry) {
            self.overlay = None;
        }
    }

    /// `entry`'s value into the line (a tip: its example request); `false` when it has none.
    fn write_entry(&mut self, entry: &aequalis::trivia::Entry) -> bool {
        if let aequalis::trivia::Source::Tip(example) = &entry.source {
            self.line.set(example);
            return true;
        }
        let now = aequalis::time::unix_now();
        match aequalis::trivia::value(self.config, entry, self.online.get(&entry.name), now) {
            Ok((value, _)) => {
                self.line.set(&format!("{} ", aequalis::trivia::written(self.config, entry, value)));
                true
            }
            Err(_) => false,
        }
    }

    /// What Ctrl+K finds for `query` (made once while the query and the history stay the same).
    pub(super) fn found(&self, query: &str) -> Rc<Vec<aequalis::find::Found>> {
        let past = self.history.entries().len();
        if let Some((last, n, found)) = &*self.found.borrow() {
            if last == query && *n == past {
                return Rc::clone(found);
            }
        }
        let history: Vec<(String, String)> = self.history.entries().iter().map(|e| (e.input.clone(), e.result.clone().unwrap_or_else(|m| m))).collect();
        let sources = aequalis::find::Sources { history: &history, country: self.country.as_deref() };
        let found = Rc::new(aequalis::find::search(self.config, query, &sources));
        *self.found.borrow_mut() = Some((query.to_string(), past, Rc::clone(&found)));
        found
    }

    /// The rates and the local currency as they are now, for the currencies the search shows.
    pub(super) fn refresh_money(&mut self) {
        if self.live {
            self.rates = aequalis::currency::load();
            self.local_currency = aequalis::currency::local().ok().flatten().map(|local| local.code);
            self.country = aequalis::geo::local_country();
        }
    }

    /// The search's keys: typing edits the query, ↑ ↓ choose, Enter uses the match.
    fn search_key(&mut self, key: Key) {
        let query = match &self.overlay {
            Some(Overlay::Search(s)) => s.query.clone(),
            _ => return,
        };
        let found = self.found(&query);
        let Some(Overlay::Search(s)) = &mut self.overlay else { return };
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
            Key::Down | Key::Tab if !found.is_empty() => s.selected = (s.selected + 1) % found.len(),
            Key::Up | Key::BackTab if !found.is_empty() => s.selected = (s.selected + found.len() - 1) % found.len(),
            Key::Enter => {
                if let Some(chosen) = found.get(s.selected).cloned() {
                    self.use_found(&chosen);
                }
            }
            _ => {}
        }
    }

    /// Choosing a match of the search: a unit or a currency is written into the request; a city
    /// too; a trivia entry writes its value; a setting opens the Settings on it; an earlier
    /// request comes back into the line.
    fn use_found(&mut self, found: &aequalis::find::Found) {
        use aequalis::find::Kind;
        match found.kind {
            Kind::Unit | Kind::Currency => {
                let line = picker::place_unit(&self.line.text(), self.line.cursor(), &found.written);
                self.line.set(&line);
                self.overlay = None;
                self.used(&found.written, found.kind == Kind::Currency);
            }
            Kind::City => {
                let mut text = self.line.text();
                if !text.is_empty() && !text.ends_with(' ') {
                    text.push(' ');
                }
                text.push_str(&found.written);
                text.push(' ');
                self.line.set(&text);
                self.overlay = None;
            }
            Kind::Trivia => {
                if let Some(entry) = aequalis::trivia::entries().iter().find(|e| e.name == found.written) {
                    self.write_entry(entry);
                }
                self.overlay = None;
            }
            Kind::Setting => {
                self.overlay = Some(Overlay::Settings(settings::View::at(&found.written)));
            }
            Kind::History => {
                self.recall = None;
                self.line.set(&found.written);
                self.overlay = None;
            }
        }
    }

    pub fn handle(&mut self, key: Key) {
        let key = match key {
            Key::Mouse(mouse) => return self.mouse(mouse),
            // The windows move by item, not by word.
            Key::WordLeft if self.overlay.is_some() => Key::Left,
            Key::WordRight if self.overlay.is_some() => Key::Right,
            key => key,
        };
        // The toolbar and the keys that work in every window.
        (self.pressed, self.lit) = (None, None);
        if let Some(action) = self.global_keymap(key) {
            return self.apply(action);
        }
        match key {
            // No other function key is bound.
            Key::F(_) => return,
            // Alt + a key that nothing is bound to: Esc, then the key (as in Vim).
            Key::Alt(c) => {
                self.handle(Key::Esc);
                return self.handle(Key::Char(c));
            }
            _ => {}
        }
        self.pressed = match (key, &self.overlay) {
            (Key::Char('x'), None) => Some('*'),
            (Key::Char(c), None) => Some(c),
            (Key::Enter, None) => Some('='),
            (Key::Backspace, None) => Some('⌫'),
            _ => None,
        };
        if self.overlay.is_none() && self.keyboard_key(key) {
            return;
        }
        match self.overlay {
            Some(Overlay::Measures(_)) => self.measures_key(key),
            Some(Overlay::Picker(_)) => self.picker_key(key),
            Some(Overlay::Currencies(_)) => self.currencies_key(key),
            Some(Overlay::Sizes(_)) => self.sizes_key(key),
            Some(Overlay::Trivia(_)) => self.trivia_key(key),
            Some(Overlay::Search(_)) => self.search_key(key),
            Some(Overlay::History(_)) => self.history_key(key),
            Some(Overlay::Settings(_)) => self.settings_key(key),
            Some(Overlay::About) => {
                if matches!(key, Key::Esc | Key::Enter | Key::Char('a' | 'A' | 'q')) {
                    self.overlay = None;
                }
            }
            Some(Overlay::Choice(..)) => self.choice_key(key),
            Some(Overlay::Help(_)) => self.help_key(key),
            None => {
                if let Some(action) = self.keymap(key) {
                    self.apply(action);
                }
            }
        }
    }

    /// The Settings window's keys (`settings::key`); Esc closes it. The shortcuts and the offline
    /// indicator follow what was changed.
    fn settings_key(&mut self, key: Key) {
        let Some(Overlay::Settings(view)) = &mut self.overlay else { return };
        if !settings::key(view, key) {
            self.overlay = None;
        }
        if self.live {
            self.load_shortcuts();
        }
        self.forced_offline = aequalis::network::mode() == aequalis::network::Mode::Offline;
    }

    /// The help's keys: ↑ ↓ and the pages scroll it; Esc, Enter or its letters close it.
    fn help_key(&mut self, key: Key) {
        let Some(Overlay::Help(scroll)) = &mut self.overlay else { return };
        match key {
            Key::Esc | Key::Enter | Key::Char('e' | 'E' | 'q' | '?') => self.overlay = None,
            Key::Down | Key::Char('j') => *scroll += 1,
            Key::Up | Key::Char('k') => *scroll = scroll.saturating_sub(1),
            Key::PageDown => *scroll += SCROLL_STEP,
            Key::PageUp => *scroll = scroll.saturating_sub(SCROLL_STEP),
            _ => {}
        }
    }

    /// A question's keys: ↑ ↓ (circular) or a digit choose; Enter writes the chosen request in
    /// the line and evaluates it; Esc closes it.
    fn choice_key(&mut self, key: Key) {
        let Some(Overlay::Choice(a, selected)) = &mut self.overlay else { return };
        let n = a.choices.len();
        match key {
            Key::Up | Key::Char('k') => *selected = (*selected + n - 1) % n,
            Key::Down | Key::Char('j') | Key::Tab => *selected = (*selected + 1) % n,
            Key::Char(c @ '1'..='9') if (c as usize - '1' as usize) < n => *selected = c as usize - '1' as usize,
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

    /// The unit keys: arrows and `h j k l` move (circular), Tab switches tab, a class's letter
    /// shows that class, Enter writes the unit, Esc goes back to the Measures.
    fn picker_key(&mut self, key: Key) {
        let Some(Overlay::Picker(p)) = &mut self.overlay else { return };
        match key {
            Key::Esc => self.overlay = Some(Overlay::Measures(measures::View::at(self.config, measures::Item::Category(p.category)))),
            // Write the unit (and a space). With the target unit set the expression is complete:
            // back to input mode; otherwise stay on the buttons for the next unit.
            // Formats tab: the format becomes the target, which completes the expression.
            Key::Enter if p.shows_formats(self.config) => {
                if let Some(word) = p.selected_format() {
                    self.line.set(&picker::place_format(&self.line.text(), &word));
                    self.overlay = None;
                }
            }
            Key::Enter => {
                let chosen = p.items(self.config).get(p.selected).map(|u| u.symbol);
                if let Some(unit) = chosen {
                    let line = picker::place_unit(&self.line.text(), self.line.cursor(), unit);
                    self.line.set(&line);
                    if picker::has_target(&line) {
                        self.overlay = None;
                    }
                    self.used(unit, false);
                }
            }
            Key::Tab | Key::BackTab => p.switch_tab(),
            Key::Left | Key::Char('h') => p.step(self.config, Direction::Left),
            Key::Right | Key::Char('l') => p.step(self.config, Direction::Right),
            Key::Up | Key::Char('k') => p.step(self.config, Direction::Up),
            Key::Down | Key::Char('j') => p.step(self.config, Direction::Down),
            // Another class's units (`l` and `k` move, so Length is `L` and Temperature `K`).
            Key::Char(c) => {
                if let Some(item) = measures::by_key(self.config, c) {
                    self.open_measure(item);
                }
            }
            _ => {}
        }
    }

    /// The History panel's keys: typing filters, ↑ ↓ choose (Ctrl+P, Ctrl+N too), Enter writes the
    /// request into the line, Del removes it, Esc closes.
    fn history_key(&mut self, key: Key) {
        let Some(Overlay::History(v)) = &mut self.overlay else { return };
        let found = history_panel::matches(&self.history, &v.query);
        match key {
            Key::Esc => self.overlay = None,
            Key::Char(c) => {
                v.query.push(c);
                v.selected = 0;
            }
            Key::Backspace => {
                v.query.pop();
                v.selected = 0;
            }
            Key::Down | Key::Tab | Key::Ctrl('n') if !found.is_empty() => v.selected = (v.selected + 1) % found.len(),
            Key::Up | Key::BackTab | Key::Ctrl('p') if !found.is_empty() => v.selected = (v.selected + found.len() - 1) % found.len(),
            Key::Enter => {
                if let Some(&i) = found.get(v.selected) {
                    self.recall_entry(i);
                }
            }
            Key::Delete => {
                if let Some(&i) = found.get(v.selected) {
                    self.history.remove(i);
                    v.selected = v.selected.min(found.len().saturating_sub(2));
                }
            }
            _ => {}
        }
    }

    /// Entry `i` of the history (oldest first): its request into the line, the panel closed.
    fn recall_entry(&mut self, i: usize) {
        if let Some(input) = self.history.entries().get(i).map(|e| e.input.clone()) {
            self.recall = None;
            self.line.set(&input);
            self.overlay = None;
        }
    }

    /// The entry the History panel has chosen (oldest first).
    fn chosen_entry(&self) -> Option<usize> {
        match &self.overlay {
            Some(Overlay::History(v)) => history_panel::matches(&self.history, &v.query).get(v.selected).copied(),
            _ => None,
        }
    }

    /// Alt+Y: the chosen entry's result (in the History panel), else the answer of the line, else the last one in
    /// the history, to the clipboard.
    fn copy_answer(&mut self) {
        // The result, or what it stands for when it shows more (the currency beside the sizes).
        let copied = |e: &Entry| e.result.clone().map(|r| e.copy.clone().unwrap_or(r));
        let result = match self.chosen_entry().and_then(|i| self.history.entries().get(i)).map(copied) {
            Some(Ok(result)) => result,
            Some(Err(_)) => return self.status = Some((StatusKind::Warning, "nothing to copy: that entry is an error".into())),
            None => match self.preview() {
                Some(Ok(result)) => {
                    self.evaluated(&self.line.text()).as_ref().as_ref().ok().and_then(|out| out.copy.clone().or_else(|| out.result.clone())).unwrap_or(result)
                }
                _ => match self.history.entries().back().map(copied) {
                    Some(Ok(result)) => result,
                    _ => return self.status = Some((StatusKind::Warning, "nothing to copy yet".into())),
                },
            },
        };
        self.copy_text(result);
    }

    /// `result` to the clipboard (the system's too in a real session).
    fn copy_text(&mut self, result: String) {
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
                self.line.replace(c.start, c.start + old, &c.names[c.index]);
                c
            }
            None => {
                let Some(s) = aequalis::suggest::suggest(self.config, &self.line.text(), self.line.cursor(), 0) else {
                    return;
                };
                let index = if forward { 0 } else { s.names.len() - 1 };
                self.line.replace(s.start, self.line.cursor(), &s.names[index]);
                Cycle { start: s.start, names: s.names, labels: s.labels, index }
            }
        };
        self.cycle = Some(cycle);
    }

    /// A click on the n-th suggestion: the word under the cursor becomes it.
    fn complete_with(&mut self, n: usize) {
        let Some(s) = aequalis::suggest::suggest(self.config, &self.line.text(), self.line.cursor(), 0) else { return };
        if let Some(name) = s.names.get(n) {
            self.line.replace(s.start, self.line.cursor(), name);
        }
    }

    /// The result of `input` (trimmed), evaluated once while it stays the same.
    fn evaluated(&self, input: &str) -> Evaluated {
        let input = input.trim();
        if let Some((last, result)) = &*self.evaluation.borrow() {
            if last == input {
                return Rc::clone(result);
            }
        }
        let tokens: Vec<&str> = input.split_whitespace().collect();
        let result = Rc::new(convert_request(self.config, self.opts, &tokens));
        *self.evaluation.borrow_mut() = Some((input.to_string(), Rc::clone(&result)));
        result
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
        let (result, status) = match &*self.evaluated(&input) {
            // An ambiguous request: ask which reading is meant (the popup), nothing is added yet.
            Err(aequalis::CliError::Ambiguous(a)) => {
                self.overlay = Some(Overlay::Choice(a.clone(), 0));
                return;
            }
            Ok(out) => {
                let status = match (out.warnings.is_empty(), out.notes.is_empty()) {
                    (false, _) => Some((StatusKind::Warning, out.warnings.join(" · "))),
                    (true, false) => Some((StatusKind::Note, out.notes.join(" · "))),
                    (true, true) => None,
                };
                (Ok(out.stdout.clone()), status)
            }
            Err(e) => (Err(shown_error(e)), Some((StatusKind::Error, e.to_string()))),
        };
        if result.is_ok() {
            self.used_in(&input);
        }
        let copy = self.evaluated(&input).as_ref().as_ref().ok().and_then(|out| out.copy.clone().or_else(|| out.result.clone()));
        self.history.push(Entry { input, result, copy });
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
        // A written date with no format yet (`12/04/26 3:20PM`): say what comes next, with the keys
        // that write one (alone, the core reads `12/04/2026` as a division).
        if !aequalis::time::is_date_request(&input) && aequalis::time::is_date_value(&input) {
            return Some(Err("now the target format (T, then Tab: Formats)".into()));
        }
        let preview = match &*self.evaluated(&input) {
            // An ambiguous request: Enter asks which reading is meant.
            Err(aequalis::CliError::Ambiguous(a)) => {
                let patterns: Vec<&str> = a.choices.iter().map(|c| c.pattern.as_str()).collect();
                Err(match a.kind {
                    aequalis::time::AmbiguityKind::City => format!("did you mean {}? · Enter to choose", patterns.join(" or ")),
                    aequalis::time::AmbiguityKind::Country => format!("several time zones: {} · Enter to choose", patterns.join(", ")),
                    aequalis::time::AmbiguityKind::Currency | aequalis::time::AmbiguityKind::Place => {
                        format!("{}: {}? · Enter to choose", a.note, patterns.join(" or "))
                    }
                    aequalis::time::AmbiguityKind::Date => format!("ambiguous: {} · Enter to choose", patterns.join(" or ")),
                })
            }
            // Dates and distances come formatted; other numbers get the device's separators here.
            Ok(out) if out.formatted => Ok(out.stdout.clone()),
            Ok(out) => Ok(self.numbers.apply_to_result(&out.stdout)),
            Err(e) => Err(shown_error(e)),
        };
        // A word being typed (no space after it yet): no error or question, which would say a
        // request on its way (`Rie` → `Rieti to Rome`) is wrong; a space or Enter shows them.
        if preview.is_err() && !input.ends_with(char::is_whitespace) {
            return None;
        }
        Some(preview)
    }

    /// When countries stand for currencies, the currencies used (`United States: US dollar (USD,
    /// US$) → …`), next to the result to check it.
    fn currencies(&self) -> Option<String> {
        self.evaluated(&self.line.text()).as_ref().as_ref().ok().and_then(|out| out.check.clone())
    }

    /// How the expression in the bar is grouped, with every operation in brackets
    /// (`12+12/3` → `(12 + (12 / 3))`); `None` without operations.
    fn grouping(&self) -> Option<String> {
        aequalis::grouping(&self.line.text())
    }

    /// The suggestions to show: their labels (cities: `Milano - Italy`, else
    /// what Tab writes), which one Tab selected, and whether they are cities (shown apart, `·`).
    fn suggestions(&self) -> Option<(Vec<String>, Option<usize>, bool)> {
        // With the keyboard shown Tab goes to it: nothing to complete with.
        if self.keyboard_shown() || aequalis::time::is_date_request(&self.line.text()) {
            return None;
        }
        let shown = |names: &[String], labels: &[String]| (if labels.is_empty() { names } else { labels }.to_vec(), !labels.is_empty());
        if let Some(c) = &self.cycle {
            let (list, cities) = shown(&c.names, &c.labels);
            return Some((list, Some(c.index), cities));
        }
        aequalis::suggest::suggest(self.config, &self.line.text(), self.line.cursor(), 0).map(|s| {
            let (list, cities) = shown(&s.names, &s.labels);
            (list, None, cities)
        })
    }
}

/// A read that returns 0 bytes this fast is end of input (the terminal is gone), not the
/// 0.2 s timeout of raw mode.
const HANGUP: Duration = Duration::from_millis(50);

pub fn run(config: &Config, opts: &Options) -> ExitCode {
    let guard = match term::enter() {
        Ok(guard) => guard,
        Err(msg) => {
            eprintln!("aequalis: error: {msg}");
            return ExitCode::from(1);
        }
    };
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        term::leave();
        default_hook(info);
    }));
    let mut app = session(config, opts);
    let mut screen = Screen { drawn: None, dirty: true, clear: true, mouse: false, synced: None };
    let mut input = Input { pending: Vec::new(), buf: [0; 256], hangups: 0 };
    let mut stdout = std::io::stdout();
    while !app.quit {
        screen.update(&mut app, &mut stdout);
        match input.read() {
            Typed::Keys(keys) => {
                for key in keys {
                    app.handle(key);
                }
                screen.clear = std::mem::take(&mut app.redraw);
                screen.dirty = true;
            }
            Typed::Nothing => {}
            Typed::Closed => break,
        }
    }
    drop(guard);
    ExitCode::SUCCESS
}

/// The app of a session at the terminal: the device's system and number format, the settings
/// (shortcuts, keyboard, mouse), the usage; the notices (the `notify` refresh policies) in the
/// status bar at the start.
fn session<'a>(config: &'a Config, opts: &'a Options<'a>) -> App<'a> {
    let mut app = App::new(config, opts);
    app.system_clipboard = true;
    app.live = true;
    app.refresh_online();
    let notices = aequalis::notify::current();
    if !notices.is_empty() {
        app.status = Some((StatusKind::Note, notices.join(" · ")));
    }
    app.system = aequalis::locale::device_system();
    app.numbers = numbers::device_format();
    app.persist = true;
    app.load_shortcuts();
    app.usage = aequalis::usage::load();
    app.refresh_money();
    // The keypad as the `tui.keyboard` setting says (F9 shows, changes and hides it): on screen
    // from the start when asked for, or (auto) when the screen has the room for the board and the
    // keyboard side by side.
    app.screen = term::size().unwrap_or((24, 80));
    let both = render::wide_split(app.screen.0, app.screen.1).is_some();
    app.keypad = if aequalis::settings::current().keyboard().unwrap_or(both) { app.best_layout() } else { Keypad::Hidden };
    // The mouse as the `tui.mouse` setting says (Alt+O switches it for the session).
    app.mouse_on = aequalis::settings::current().mouse();
    app
}

/// What the terminal shows: the size last drawn, whether the app changed since, whether the next
/// frame clears the terminal first; the mouse reporting set, and the sync age shown.
struct Screen {
    drawn: Option<(usize, usize)>,
    dirty: bool,
    clear: bool,
    mouse: bool,
    synced: Option<String>,
}

impl Screen {
    /// The online data read again if a sync changed it, the mouse reporting as the app wants it,
    /// and the frame drawn again when something changed: the app, the size, the toolbar's age of
    /// the last sync (`2h`, which changes with the clock too).
    fn update(&mut self, app: &mut App, stdout: &mut std::io::Stdout) {
        app.keep_online_fresh();
        self.dirty |= app.refresh_online();
        let synced = app.synced_ago();
        if synced != self.synced {
            (self.dirty, self.synced) = (true, synced);
        }
        if app.mouse_on != self.mouse {
            term::mouse(app.mouse_on);
            self.mouse = app.mouse_on;
        }
        let size = term::size().unwrap_or((24, 80));
        app.screen = size;
        if self.dirty || self.drawn != Some(size) {
            let frame = render::render(app, size.0, size.1);
            let _ = stdout.write_all(frame.to_ansi(self.clear || self.drawn != Some(size)).as_bytes());
            let _ = stdout.flush();
            app.hits = frame.hits;
            (self.drawn, self.dirty, self.clear) = (Some(size), false, false);
        }
    }
}

/// The terminal's input: the bytes of keys not complete yet, and how many reads in a row found
/// it closed.
struct Input {
    pending: Vec<u8>,
    buf: [u8; 256],
    hangups: usize,
}

/// What a read of the terminal gave.
enum Typed {
    /// The keys completed by the bytes read (maybe none: half an escape sequence).
    Keys(Vec<Key>),
    /// Nothing: raw mode's 0.2 s timeout, or a signal.
    Nothing,
    /// The terminal is gone.
    Closed,
}

impl Input {
    fn read(&mut self) -> Typed {
        let started = Instant::now();
        let n = match term::read(&mut self.buf) {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => return Typed::Nothing,
            Err(_) => return Typed::Closed,
        };
        if n == 0 {
            self.hangups = if started.elapsed() < HANGUP { self.hangups + 1 } else { 0 };
            return if self.hangups > 3 { Typed::Closed } else { Typed::Nothing };
        }
        self.pending.extend_from_slice(&self.buf[..n]);
        let (keys, used) = keys::decode(&self.pending);
        self.pending.drain(..used);
        Typed::Keys(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::action::{Action, Panel, TOOLBAR};
    use super::*;

    /// The same machine on every laptop, for the test's thread: Italy's (metric, Rome) with the
    /// plain number format the tests were written for.
    pub(crate) fn test_machine() {
        if aequalis::machine::mocked().is_none() {
            aequalis::machine::Mock::italy().with_format(".", "").keep();
        }
    }

    fn options(config: &Config) -> Options<'static> {
        test_machine();
        Options { positional: Vec::new(), precision: config.default_precision, silent: false, verbose: false }
    }

    fn typed(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle(Key::Char(c));
        }
    }

    #[test]
    fn alt_digits_write_the_shortcuts_and_count_them() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        // Alt+7 is the second currency (EUR), Alt+1 the first unit (km): `1 EUR`, then a target.
        app.handle(Key::Alt('7'));
        assert_eq!((app.line.text().as_str(), app.lit), ("1 EUR ", Some(6)));
        app.handle(Key::Alt('6'));
        assert_eq!(app.line.text(), "1 EUR USD ", "the target");
        assert_eq!(app.currencies.to_text(), "USD:1,EUR:1,JPY:0,CNY:0,AED:0");
        // Alt+0 is the fifth currency; Alt+5 the fifth unit.
        app.handle(Key::Alt('0'));
        assert_eq!(app.currencies.hits("AED"), Some(1));
        app.handle(Key::Alt('5'));
        assert_eq!(app.units.hits("m"), Some(1));
        // Typing anything else puts the light out.
        app.handle(Key::Char('x'));
        assert_eq!(app.lit, None);
        // A window open: the shortcuts wait.
        app.handle(Key::F(2));
        let before = app.line.text();
        app.handle(Key::Alt('1'));
        assert_eq!((app.line.text(), app.units.hits("km")), (before, Some(0)));
    }

    #[test]
    fn the_search_uses_each_kind_of_match() {
        let config = Config::default();
        let opts = options(&config);
        // A setting: the Settings window opens on it.
        let mut app = App::new(&config, &opts);
        app.handle(Key::Ctrl('k'));
        typed(&mut app, "precision");
        app.handle(Key::Enter);
        assert!(matches!(&app.overlay, Some(Overlay::Settings(v)) if matches!(v.row(), Some(settings::Row::Setting(k)) if k.key == "precision")));
        // An earlier request comes back into the line.
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10 km mi");
        app.handle(Key::Enter);
        app.handle(Key::Ctrl('k'));
        typed(&mut app, "10 km");
        app.handle(Key::Enter);
        assert_eq!((app.line.text().as_str(), app.overlay.is_none()), ("10 km mi", true));
        // A trivia entry writes its value (a tip, its example); the window closes.
        let mut app = App::new(&config, &opts);
        app.handle(Key::Ctrl('k'));
        typed(&mut app, "speed of light");
        app.handle(Key::Enter);
        assert!(app.overlay.is_none() && !app.line.is_empty(), "{:?}", app.line.text());
        // A unit is placed as before.
        let mut app = App::new(&config, &opts);
        typed(&mut app, "5 ");
        app.handle(Key::Ctrl('k'));
        typed(&mut app, "kilomet");
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "5 km ");
    }

    #[test]
    fn what_is_used_is_counted() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        // A request that works counts each unit once, on both sides (a currency request needs the
        // rates, which a test has not).
        typed(&mut app, "5 km to mi");
        app.handle(Key::Enter);
        assert_eq!(app.units.to_text(), "km:1,kg:0,l:0,C:0,mi:1", "mi takes m's place");
        // Every unit counts in the usage, listed or not (the board's order).
        assert_eq!((app.usage.hits(aequalis::usage::Kind::Unit, "km"), app.usage.hits(aequalis::usage::Kind::Unit, "mi")), (1, 1));
        // One that fails counts nothing.
        typed(&mut app, "5 km to eur");
        app.handle(Key::Enter);
        assert_eq!((app.units.hits("km"), app.currencies.hits("EUR")), (Some(1), Some(0)));
        // The Ctrl+K search and the Currencies panel count the pick.
        app.handle(Key::Ctrl('k'));
        typed(&mut app, "pound sterling");
        app.handle(Key::Enter);
        assert_eq!(app.currencies.to_text(), "USD:0,EUR:0,JPY:0,CNY:0,GBP:1", "GBP takes AED's place: {:?}", app.currencies);
        // A pick that is not saved in tests: the settings file is untouched.
        assert!(!app.persist);
    }

    #[test]
    fn the_units_default_follows_the_measurement_system() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.system = System::Imperial;
        app.units = Favourites::of("", &favourites::default_units(app.system));
        assert_eq!(app.shortcut(0), Some("mi"));
        assert_eq!(app.shortcut(9), Some("AED"));
        assert_eq!(app.shortcut(10), None);
    }

    #[test]
    fn alt_key_without_a_binding_is_esc_then_the_key() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        // Alt+u: Esc (nothing to step back from), then `u`, which is text.
        app.handle(Key::Alt('u'));
        assert!(app.overlay.is_none());
        assert_eq!(app.line.text(), "u");
        // With a line, the Esc clears it first, as when typed apart.
        app.handle(Key::Alt('x'));
        assert_eq!(app.line.text(), "x");
        // Alt+q is bound: it quits.
        let mut app = App::new(&config, &opts);
        app.handle(Key::Alt('q'));
        assert!(app.quit);
    }

    #[test]
    fn toolbar_keys_are_unique_and_open_their_window() {
        let config = Config::default();
        let opts = options(&config);
        for (i, t) in TOOLBAR.iter().enumerate() {
            let app = App::new(&config, &opts);
            assert_eq!(app.global_keymap(Key::F(t.fkey)), Some(t.action), "{}", t.label);
            assert_eq!(app.global_keymap(Key::Alt(t.alt)), Some(t.action), "{}", t.label);
            assert_eq!(app.global_keymap(Key::Alt(t.alt.to_ascii_uppercase())), Some(t.action), "Alt+Shift works too");
            assert!(TOOLBAR[i + 1..].iter().all(|o| o.fkey != t.fkey && o.alt != t.alt), "{} shares a key", t.label);
            // Not F1 (help) or an Alt letter of another command.
            assert!(![1].contains(&t.fkey) && !['a', 'n', 'y', 'k', 'q'].contains(&t.alt), "{}", t.label);
        }
        for (key, panel, is) in [
            (Key::F(2), Panel::Measures, (|o: &Option<Overlay>| matches!(o, Some(Overlay::Measures(_)))) as fn(&Option<Overlay>) -> bool),
            (Key::F(3), Panel::Currencies, |o| matches!(o, Some(Overlay::Currencies(_)))),
            (Key::F(4), Panel::Sizes, |o| matches!(o, Some(Overlay::Sizes(_)))),
            (Key::F(6), Panel::Trivia, |o| matches!(o, Some(Overlay::Trivia(_)))),
            (Key::F(10), Panel::Settings, |o| matches!(o, Some(Overlay::Settings(_)))),
            (Key::Ctrl('k'), Panel::Search, |o| matches!(o, Some(Overlay::Search(_)))),
        ] {
            let mut app = App::new(&config, &opts);
            app.handle(key);
            assert!(is(&app.overlay) && panel.is(&app.overlay), "{key:?}");
            // The same key again closes it.
            app.handle(key);
            assert!(app.overlay.is_none(), "{key:?} again");
        }
        let mut app = App::new(&config, &opts);
        // Another window replaces the one open, from inside it.
        app.handle(Key::F(4));
        app.handle(Key::F(2));
        assert!(matches!(app.overlay, Some(Overlay::Measures(_))));
        app.handle(Key::Alt('c'));
        assert!(matches!(app.overlay, Some(Overlay::Currencies(_))));
        // Unbound function keys do nothing.
        app.handle(Key::F(7));
        assert!(matches!(app.overlay, Some(Overlay::Currencies(_))));
    }

    #[test]
    fn clicks_do_nothing_yet_and_the_wheel_scrolls() {
        use keys::{Button, Mouse, MouseKind};
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "1+2");
        app.handle(Key::Mouse(Mouse { kind: MouseKind::Down(Button::Left), row: 1, col: 1 }));
        assert!(app.overlay.is_none() && app.line.text() == "1+2");
        for n in 0..8 {
            app.line.set(&format!("{n}+1"));
            app.handle(Key::Enter);
        }
        app.handle(Key::Mouse(Mouse { kind: MouseKind::ScrollUp, row: 5, col: 5 }));
        assert_eq!(app.scroll, SCROLL_STEP);
        app.handle(Key::Mouse(Mouse { kind: MouseKind::ScrollDown, row: 5, col: 5 }));
        assert_eq!(app.scroll, 0);
    }

    #[test]
    fn words_by_alt_or_ctrl_arrows() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10 km");
        app.handle(Key::WordLeft);
        assert_eq!(app.line.cursor(), 3);
        app.handle(Key::WordLeft);
        assert_eq!(app.line.cursor(), 0);
        app.handle(Key::WordRight);
        assert_eq!(app.line.cursor(), 3);
    }

    #[test]
    fn letters_are_always_text() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "1 L eqmcthsakn?/");
        assert_eq!(app.line.text(), "1 L eqmcthsakn?/");
        assert!(app.overlay.is_none() && !app.quit);
    }

    #[test]
    fn copy_an_error_entry() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10 foo bar");
        app.handle(Key::Enter);
        app.handle(Key::F(5));
        app.handle(Key::Alt('y'));
        assert_eq!(app.copied, None);
        assert!(matches!(&app.status, Some((StatusKind::Warning, m)) if m.contains("nothing to copy")));
        // In the History panel Alt+Y copies the chosen entry's result.
        app.handle(Key::Esc);
        typed(&mut app, "2^8");
        app.handle(Key::Enter);
        app.handle(Key::F(5));
        app.handle(Key::Alt('y'));
        assert_eq!(app.copied.as_deref(), Some("256"));
        assert_eq!(app.status, Some((StatusKind::Note, "copied 256".into())));
        // In the panel typing filters: `c` is not a command.
        app.handle(Key::Char('c'));
        assert!(matches!(&app.overlay, Some(Overlay::History(v)) if v.query == "c"));
    }

    #[test]
    fn copy_the_answer() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::Alt('y'));
        assert!(matches!(&app.status, Some((StatusKind::Warning, m)) if m.contains("nothing to copy yet")));
        typed(&mut app, "10 km mi");
        app.handle(Key::Alt('y'));
        assert_eq!(app.copied.as_deref(), Some("6.2137 mi"), "the answer being typed");
        app.handle(Key::Enter);
        app.status = None;
        app.handle(Key::Alt('y'));
        assert_eq!(app.copied.as_deref(), Some("6.2137 mi"), "the last one in the history");
        // The line is untouched by the copy.
        assert!(app.line.is_empty());
    }

    #[test]
    fn evaluate_and_the_history_panel() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10 km mi");
        assert_eq!(app.preview(), Some(Ok("6.2137 mi".into())));
        app.handle(Key::Enter);
        assert!(app.line.is_empty());
        assert_eq!(app.history.entries()[0], Entry { input: "10 km mi".into(), result: Ok("6.2137 mi".into()), copy: None });

        // F5 opens the History panel; the line is not changed. Alt+Y copies the result of the entry
        // chosen (the newest first), to the clipboard in a real session; the panel stays.
        app.handle(Key::F(5));
        assert!(matches!(app.overlay, Some(Overlay::History(_))) && app.line.text().is_empty());
        app.handle(Key::Alt('y'));
        assert_eq!(app.copied.as_deref(), Some("6.2137 mi"));
        assert_eq!(app.status, Some((StatusKind::Note, "copied 6.2137 mi".into())));
        assert!(app.overlay.is_some());
        // F5 again, or Esc, closes it.
        app.handle(Key::F(5));
        assert!(app.overlay.is_none());
        app.handle(Key::F(5));
        app.handle(Key::Esc);
        assert!(app.overlay.is_none());
        // Typing filters (it is not the line's text); Enter writes the request into the line,
        // without evaluating it, and closes the panel.
        app.handle(Key::F(5));
        typed(&mut app, "zzz");
        app.handle(Key::Enter);
        assert!(app.line.is_empty() && app.overlay.is_some(), "nothing matches: nothing to recall");
        for _ in 0..3 {
            app.handle(Key::Backspace);
        }
        typed(&mut app, "km");
        app.handle(Key::Enter);
        assert_eq!((app.line.text().as_str(), app.overlay.is_none(), app.history.entries().len()), ("10 km mi", true, 1));
        // Edit and evaluate again: a new entry, the old one is unchanged.
        app.handle(Key::Backspace);
        app.handle(Key::Backspace);
        typed(&mut app, "ft");
        app.handle(Key::Enter);
        let results: Vec<_> = app.history.entries().iter().map(|e| e.result.clone()).collect();
        assert_eq!(results, [Ok("6.2137 mi".to_string()), Ok("32808.399 ft".to_string())]);
        // ↑ ↓ choose (the newest first); Del removes the entry chosen.
        app.handle(Key::F(5));
        app.handle(Key::Down);
        app.handle(Key::Delete);
        assert_eq!(app.history.entries().len(), 1);
        assert_eq!(app.history.entries()[0].result, Ok("32808.399 ft".to_string()), "the older one went");
        app.handle(Key::Delete);
        assert!(app.history.entries().is_empty());
        // Nothing to show: the panel says so, and Enter does nothing.
        app.handle(Key::Enter);
        assert!(app.overlay.is_some() && app.line.is_empty());
    }

    #[test]
    fn up_and_down_recall_as_in_a_shell() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::Up);
        assert!(app.line.is_empty(), "nothing to recall");
        for request in ["1+1", "10 km mi", "2^8"] {
            typed(&mut app, request);
            app.handle(Key::Enter);
        }
        typed(&mut app, "draft");
        // ↑ replaces the line with the newest request, then older ones; the oldest stays.
        app.handle(Key::Up);
        assert_eq!(app.line.text(), "2^8");
        app.handle(Key::Up);
        assert_eq!(app.line.text(), "10 km mi");
        app.handle(Key::Up);
        app.handle(Key::Up);
        assert_eq!(app.line.text(), "1+1", "stays on the oldest");
        assert_eq!(app.line.cursor(), 3, "the cursor at the end");
        // ↓ goes newer; past the newest, what was being typed comes back.
        app.handle(Key::Down);
        assert_eq!(app.line.text(), "10 km mi");
        app.handle(Key::Down);
        app.handle(Key::Down);
        assert_eq!(app.line.text(), "draft");
        app.handle(Key::Down);
        assert_eq!(app.line.text(), "draft", "↓ with nothing recalled does nothing");
        // Ctrl+P / Ctrl+N are the same.
        app.handle(Key::Ctrl('p'));
        assert_eq!(app.line.text(), "2^8");
        app.handle(Key::Ctrl('n'));
        assert_eq!(app.line.text(), "draft");
        // Editing a recalled request ends the recall: ↑ starts again from the newest, and the
        // edited text is what ↓ brings back.
        app.handle(Key::Up);
        app.handle(Key::Up);
        typed(&mut app, "!");
        assert_eq!(app.line.text(), "10 km mi!");
        app.handle(Key::Up);
        assert_eq!(app.line.text(), "2^8");
        app.handle(Key::Down);
        assert_eq!(app.line.text(), "10 km mi!");
        // Enter evaluates the recalled request; a new one starts clean.
        app.handle(Key::Ctrl('u'));
        app.handle(Key::Up);
        app.handle(Key::Enter);
        assert!(app.line.is_empty() && app.recall.is_none() && app.draft.is_none());
        app.handle(Key::Down);
        assert!(app.line.is_empty());
    }

    #[test]
    fn esc_steps_back_one_at_a_time() {
        let config = Config::default();
        let opts = options(&config);
        let opts = Options { verbose: true, ..opts };
        let mut app = App::new(&config, &opts);
        typed(&mut app, "100 + 20 + 50 / 10");
        app.handle(Key::Enter);
        assert!(app.status.is_some());
        typed(&mut app, "10 km m");
        // A window open: Esc closes it, nothing else.
        app.handle(Key::F(2));
        app.handle(Key::Esc);
        assert!(app.overlay.is_none() && app.line.text() == "10 km m" && app.status.is_some());
        // The History panel.
        app.handle(Key::F(5));
        app.handle(Key::Esc);
        assert!(app.overlay.is_none() && app.line.text() == "10 km m");
        // The Tab cycle: Esc ends it and keeps the word it chose.
        app.handle(Key::Tab);
        assert_eq!(app.line.text(), "10 km mm");
        app.handle(Key::Esc);
        assert!(app.cycle.is_none() && app.line.text() == "10 km mm" && app.status.is_some());
        // The status message, then the line.
        app.handle(Key::Esc);
        assert!(app.status.is_none() && app.line.text() == "10 km mm");
        app.handle(Key::Esc);
        assert!(app.line.is_empty() && !app.quit);
        // ↑ brings the cleared text back (not the history's newest).
        app.handle(Key::Up);
        assert_eq!(app.line.text(), "10 km mm");
        // Esc on an empty line, with nothing else, does nothing.
        app.handle(Key::Ctrl('u'));
        app.handle(Key::Esc);
        app.handle(Key::Esc);
        assert!(app.line.is_empty() && !app.quit && app.overlay.is_none());
    }

    #[test]
    fn pick_source_and_target() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10");
        // F2: the Measures; L: the Length units, metric (mm cm m km); l l l → km, Enter writes it
        // and a space.
        app.handle(Key::F(2));
        assert!(matches!(app.overlay, Some(Overlay::Measures(_))));
        app.handle(Key::Char('L'));
        assert!(matches!(&app.overlay, Some(Overlay::Picker(p)) if p.category(&config).name == "Length"));
        typed(&mut app, "lll");
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "10 km ");
        // The buttons stay for the next unit. Tab: the imperial tab (in ft yd mi), l l l → mi.
        assert!(matches!(app.overlay, Some(Overlay::Picker(_))));
        app.handle(Key::Tab);
        typed(&mut app, "lll");
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "10 km mi ");
        // The target is set: the expression is complete, so the buttons close.
        assert!(app.overlay.is_none());
        assert_eq!(app.line.cursor(), 9);
        app.handle(Key::Enter);
        assert_eq!(app.history.entries()[0].result, Ok("6.2137 mi".into()));
    }

    #[test]
    fn vim_keys_in_the_unit_keys() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::F(2));
        typed(&mut app, "B");
        let state = |app: &App| match &app.overlay {
            Some(Overlay::Picker(p)) => (p.category(&config).name, p.tab, p.items(&config)[p.selected].symbol),
            _ => panic!("no unit keys"),
        };
        // Data, Decimal: B kB MB GB TB / b kb Mb Gb Tb.
        assert_eq!(state(&app), (aequalis::config::BITS, System::Metric, "B"));
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
        // A class's letter shows its units: M Mass, s Speed, T Date & Time.
        typed(&mut app, "M");
        assert_eq!(state(&app).0, "Mass");
        typed(&mut app, "s");
        assert_eq!(state(&app).0, "Speed");
        typed(&mut app, "t");
        assert_eq!((state(&app).0, state(&app).1), (aequalis::config::TIME, System::Metric), "Date & Time opens on the durations");
        typed(&mut app, "x");
        assert_eq!(state(&app).0, aequalis::config::TIME, "other letters do nothing");
        // Esc: back to the Measures, on the class shown.
        app.handle(Key::Esc);
        assert!(matches!(&app.overlay, Some(Overlay::Measures(v)) if v.item(&config) == measures::Item::Category(0)));
        app.handle(Key::Esc);
        assert!(app.overlay.is_none());
    }

    #[test]
    fn measures_list() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::F(2));
        // ↓ ↓: Length; Enter opens its units, Esc comes back to it.
        app.handle(Key::Down);
        app.handle(Key::Char('j'));
        app.handle(Key::Enter);
        assert!(matches!(&app.overlay, Some(Overlay::Picker(p)) if p.title(&config) == "Length"));
        app.handle(Key::Esc);
        assert!(matches!(&app.overlay, Some(Overlay::Measures(v)) if v.selected == 2));
        app.handle(Key::Up);
        app.handle(Key::Enter);
        assert!(matches!(&app.overlay, Some(Overlay::Picker(p)) if p.title(&config) == aequalis::config::BITS));
    }

    #[test]
    fn sizes_write_their_words() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::F(4));
        assert!(matches!(app.overlay, Some(Overlay::Sizes(_))));
        // Shoes first: its word, ready for the size.
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "shoes ");
        assert!(app.overlay.is_none());
        // The size typed with the window open: its digits go into the line.
        app.handle(Key::Esc);
        app.line.set("");
        app.handle(Key::F(4));
        app.handle(Key::Char('1'));
        app.handle(Key::Char('2'));
        assert!(matches!(app.overlay, Some(Overlay::Sizes(_))), "still open");
        for _ in 0..7 {
            app.handle(Key::Down);
        }
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "12 us ", "the size, then US: its code alone");
        app.line.set("shoes ");
        // A person: Alt+Z, down to Men; then another in its place.
        app.handle(Key::Char('4'));
        app.handle(Key::Char('2'));
        app.handle(Key::Alt('z'));
        app.handle(Key::Down);
        app.handle(Key::Down);
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "shoes 42 men ");
        app.handle(Key::Alt('z'));
        for _ in 0..3 {
            app.handle(Key::Down);
        }
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "shoes 42 women ");
        // Then a system: down past the people to EU.
        app.handle(Key::Alt('z'));
        for _ in 0..5 {
            app.handle(Key::Down);
        }
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "shoes 42 women eu ");
        // Clothes in place of shoes.
        app.handle(Key::Alt('z'));
        app.handle(Key::Down);
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "clothes 42 women eu ");
    }

    #[test]
    fn a_shoe_table_writes_the_rows_request() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        // The section before the last, Shoes: Men's shoes, its table; down to its 6th row (US 8.5), Enter.
        app.handle(Key::F(6));
        app.handle(Key::BackTab);
        app.handle(Key::BackTab);
        app.handle(Key::Enter);
        for _ in 0..5 {
            app.handle(Key::Down);
        }
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "42 eu men shoes ");
        assert!(app.overlay.is_none());
        // The last, Clothes: Women's clothes; its 5th row (US 8).
        app.line.set("");
        app.handle(Key::F(6));
        app.handle(Key::BackTab);
        app.handle(Key::Enter);
        for _ in 0..4 {
            app.handle(Key::Down);
        }
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "44 it dress ");
    }

    #[test]
    fn trivia_writes_the_value() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::F(6));
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
        // Earth's first: a tip, writing its example request.
        app.handle(Key::F(6));
        app.handle(Key::Enter);
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "London to Paris km");
        assert_eq!(app.preview(), Some(Ok("344 km".into())));
        // Earth, Equator: with its unit, so a target converts it.
        app.handle(Key::F(6));
        app.handle(Key::Char('j'));
        app.handle(Key::Enter);
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "40075.017 km ");
        typed(&mut app, "mi");
        assert_eq!(app.preview(), Some(Ok("24901.4611 mi".into())));
        // Another window from the Trivia.
        app.handle(Key::F(6));
        app.handle(Key::F(2));
        assert!(matches!(app.overlay, Some(Overlay::Measures(_))));
    }

    #[test]
    fn search_writes_the_unit() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10");
        app.handle(Key::Ctrl('k'));
        // Letters are the query, not commands (h, j, k, q…).
        typed(&mut app, "kilometre");
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "10 km ");
        assert!(app.overlay.is_none());
        app.handle(Key::Ctrl('k'));
        typed(&mut app, "mile");
        app.handle(Key::Down);
        app.handle(Key::Up);
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "10 km mi ");
        // Esc closes it without writing.
        app.handle(Key::Ctrl('k'));
        typed(&mut app, "kg");
        app.handle(Key::Esc);
        assert_eq!((app.line.text(), app.overlay.is_none()), ("10 km mi ".to_string(), true));
    }

    #[test]
    fn earth_distance_writes_the_request() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::F(2));
        typed(&mut app, "D");
        assert_eq!(app.line.text(), "distance ");
        assert!(app.overlay.is_none());
        typed(&mut app, "Rome to Paris");
        assert_eq!(app.preview(), Some(Ok("1107 km".into())));
    }

    #[test]
    fn currencies_write_the_code() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10");
        app.handle(Key::F(3));
        // Letters are the query, not commands.
        typed(&mut app, "euro");
        app.handle(Key::Enter);
        assert_eq!((app.line.text(), app.overlay.is_none()), ("10 EUR ".to_string(), true));
        app.handle(Key::F(3));
        typed(&mut app, "yen");
        app.handle(Key::Down);
        app.handle(Key::Up);
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "10 EUR JPY ");
        // Esc closes it without writing.
        app.handle(Key::F(3));
        typed(&mut app, "x");
        app.handle(Key::Backspace);
        app.handle(Key::Esc);
        assert_eq!((app.line.text(), app.overlay.is_none()), ("10 EUR JPY ".to_string(), true));
    }

    #[test]
    fn alt_n_writes_now() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::Alt('n'));
        assert_eq!(app.line.text(), "now ");
        typed(&mut app, "+ 3d");
        assert!(app.preview().is_some_and(|r| r.is_ok_and(|v| v.contains('T'))), "a date: {:?}", app.preview());
        app.handle(Key::Alt('n'));
        assert_eq!(app.line.text(), "now + 3d now ");
        // Not into the line behind a window.
        app.handle(Key::F(3));
        app.handle(Key::Alt('n'));
        assert_eq!(app.line.text(), "now + 3d now ");
    }

    #[test]
    fn alt_k_kills_to_the_end() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "100 km mi");
        for _ in 0..6 {
            app.handle(Key::Left);
        }
        app.handle(Key::Alt('k'));
        assert_eq!(app.line.text(), "100");
        assert!(app.overlay.is_none(), "Ctrl+K is the unit search, Alt+K kills");
    }

    #[test]
    fn data_buttons_five_per_row() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        app.handle(Key::F(2));
        typed(&mut app, "B");
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
        app.handle(Key::F(2));
        typed(&mut app, "M");
        assert!(matches!(&app.overlay, Some(Overlay::Picker(p)) if p.tab == System::Imperial), "Mass: imperial");
        typed(&mut app, "L");
        assert!(matches!(&app.overlay, Some(Overlay::Picker(p)) if p.items(&config)[p.selected].symbol == "in"), "Length: imperial");
        typed(&mut app, "B");
        assert!(matches!(&app.overlay, Some(Overlay::Picker(p)) if p.tab == System::Metric), "bits and bytes: decimal");
    }

    #[test]
    fn picker_esc_cancels() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10 km");
        app.handle(Key::F(2));
        typed(&mut app, "M");
        typed(&mut app, "kk");
        app.handle(Key::Esc);
        app.handle(Key::Esc);
        assert!(app.overlay.is_none());
        assert_eq!(app.line.text(), "10 km");
    }

    #[test]
    fn pick_replaces_the_unit_under_the_cursor() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "10 km mi");
        app.handle(Key::Home);
        app.handle(Key::WordRight);
        // Length, the imperial tab, ft (l).
        app.handle(Key::F(2));
        typed(&mut app, "L");
        app.handle(Key::Tab);
        typed(&mut app, "l");
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "10 ft mi", "replaced in place: no space added");
        assert!(app.overlay.is_none(), "source and target set: complete");
    }

    #[test]
    fn help() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "e?");
        assert!(app.overlay.is_none(), "e and ? are text");
        app.handle(Key::F(1));
        assert!(matches!(app.overlay, Some(Overlay::Help(0))));
        app.handle(Key::Char('j'));
        app.handle(Key::Down);
        app.handle(Key::Char('k'));
        assert!(matches!(app.overlay, Some(Overlay::Help(1))), "j/k scroll and keep the help open");
        app.handle(Key::Char('S'));
        assert!(matches!(app.overlay, Some(Overlay::Help(1))), "letters do nothing under the help");
        app.handle(Key::Esc);
        assert!(app.overlay.is_none());
        // Alt+? opens it too, and closes it.
        app.handle(Key::Alt('?'));
        assert!(matches!(app.overlay, Some(Overlay::Help(0))));
        app.handle(Key::F(1));
        assert!(app.overlay.is_none());
    }

    #[test]
    fn preview_asks_for_the_target() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "25 km ");
        assert_eq!(app.preview(), Some(Err("now the length unit to convert to: in, ft, yd…".into())), "what comes next, as in every UI");
        app.handle(Key::Ctrl('u'));
        typed(&mut app, "25 foo");
        assert_eq!(app.preview(), None, "a word being typed: no error yet");
        typed(&mut app, " ");
        assert!(matches!(app.preview(), Some(Err(m)) if m.contains("expected")), "unknown unit after the space: the real error");
        app.handle(Key::Ctrl('u'));
        typed(&mut app, "12/04/2026");
        assert_eq!(app.preview(), Some(Err("now the target format (T, then Tab: Formats)".into())));
        // A written date with a time is complete: its ISO reading, without the unwritten seconds.
        app.handle(Key::Ctrl('u'));
        typed(&mut app, "2026/12/04 10:00 PM");
        assert!(matches!(app.preview(), Some(Ok(p)) if p.starts_with("04 Dec 26 10:00 PM GMT")), "for people: {:?}", app.preview());
        app.handle(Key::Ctrl('u'));
        typed(&mut app, "12/04/26 3:20PM ");
        assert!(matches!(app.preview(), Some(Err(m)) if m.starts_with("ambiguous")), "{:?}", app.preview());
        app.handle(Key::Ctrl('u'));
        typed(&mut app, "12/4");
        assert_eq!(app.preview(), Some(Ok("3".into())), "a division stays a division");
    }

    /// Typing `Rieti to Roma`: no error on the way (`Rie`, `Rieti`, `Rieti to Ro`), only once a
    /// word is typed (a space after it) or on Enter.
    #[test]
    fn no_error_while_a_word_is_typed() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        for c in "Rieti to Roma".chars() {
            typed(&mut app, &c.to_string());
            let text = app.line.text();
            if !text.ends_with(' ') {
                assert_eq!(app.preview(), None, "{text:?}");
            }
        }
        // A result shows at once: `12+4` mid-word too.
        app.handle(Key::Ctrl('u'));
        typed(&mut app, "12+4");
        assert_eq!(app.preview(), Some(Ok("16".into())));
        // After a space the error shows (no city archive in the tests: `Rieti` is unknown).
        app.handle(Key::Ctrl('u'));
        typed(&mut app, "Rieti ");
        assert!(matches!(app.preview(), Some(Err(_))), "{:?}", app.preview());
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
    fn keypad_cycles_and_lights_the_key_typed() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        assert_eq!(app.keypad, Keypad::Hidden, "asked for, not shown at the start");
        typed(&mut app, "7");
        assert_eq!(app.pressed, Some('7'));
        typed(&mut app, "x");
        assert_eq!(app.pressed, Some('*'), "x is multiply");
        app.handle(Key::Backspace);
        assert_eq!(app.pressed, Some('⌫'));
        // C is text; F9 shows the keyboard (Sci, with room for the shortcuts), then hides it; no key
        // is lit after it.
        typed(&mut app, "C");
        assert_eq!(app.keypad, Keypad::Hidden);
        for want in [Keypad::Sci, Keypad::Hidden, Keypad::Sci] {
            app.handle(Key::F(9));
            assert_eq!((app.keypad, app.pressed), (want, None));
        }
        // On a narrow terminal it opens on Basic.
        let mut narrow = App::new(&config, &opts);
        narrow.screen = (24, 58);
        narrow.handle(Key::F(9));
        assert_eq!(narrow.keypad, Keypad::Basic);
        // Enter lights =.
        app.handle(Key::Enter);
        assert_eq!(app.pressed, Some('='));
        // A panel in the band gives way to the keypad, which then shows.
        app.handle(Key::F(2));
        app.handle(Key::F(9));
        assert!(app.overlay.is_none() && app.keypad == Keypad::Sci);
        app.keypad = Keypad::Hidden;
        app.handle(Key::F(3));
        app.handle(Key::F(9));
        assert!(app.overlay.is_none());
        assert_eq!(app.keypad, Keypad::Sci, "hidden: the first F9 shows it");
    }

    #[test]
    fn tab_moves_the_focus_across_the_keyboard() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        // Hidden: Tab completes as before.
        typed(&mut app, "5 k");
        app.handle(Key::Tab);
        assert!(app.focus.is_none() && app.line.text() != "5 k", "{}", app.line.text());
        app.handle(Key::Esc);
        // F9 opens the keyboard with the focus on its layout.
        app.handle(Key::F(9));
        assert_eq!((app.keypad, app.focus), (Keypad::Sci, Some(Part::Sci)));
        // Tab: Units, Money, round to Basic (shown at once), Sci again.
        for (want, layout) in [(Part::Units, Keypad::Sci), (Part::Money, Keypad::Sci), (Part::Basic, Keypad::Basic), (Part::Sci, Keypad::Sci)] {
            app.handle(Key::Tab);
            assert_eq!((app.focus, app.keypad), (Some(want), layout));
        }
        app.handle(Key::BackTab);
        assert_eq!((app.focus, app.keypad), (Some(Part::Basic), Keypad::Basic));
        app.handle(Key::BackTab);
        assert_eq!(app.focus, Some(Part::Money), "back round: the last part");
        // Letters and digits go to the line, wherever the focus is.
        app.line.set("");
        typed(&mut app, "12 e");
        assert_eq!((app.line.text().as_str(), app.focus), ("12 e", Some(Part::Money)));
        // In Money: ↑ ↓ choose, ← → change the column, Enter writes and stays for the next.
        app.line.set("");
        app.handle(Key::Down);
        app.handle(Key::Down);
        app.handle(Key::Enter);
        assert_eq!((app.line.text().as_str(), app.focus, app.slot), ("1 JPY ", Some(Part::Money), 2));
        app.handle(Key::Left);
        app.handle(Key::Up);
        app.handle(Key::Enter);
        assert_eq!((app.line.text().as_str(), app.focus), ("1 JPY kg ", Some(Part::Units)), "the second entry, replacing the unit");
        // Esc gives the focus back to the line (the keyboard stays); Esc again clears the line.
        app.handle(Key::Esc);
        assert_eq!((app.focus, app.keypad, app.line.text().as_str()), (None, Keypad::Basic, "1 JPY kg "));
        // The arrows are the line's again; Tab returns to the keyboard, on the layout shown.
        app.handle(Key::Up);
        app.handle(Key::Tab);
        assert_eq!(app.focus, Some(Part::Basic));
        // A window takes the focus; F9 hides everything.
        app.handle(Key::F(3));
        assert!(app.focus.is_none());
        app.handle(Key::F(9));
        app.handle(Key::F(9));
        assert_eq!((app.keypad, app.focus), (Keypad::Hidden, None));
        // On a narrow terminal there are no columns to go to.
        let mut narrow = App::new(&config, &opts);
        narrow.screen = (24, 50);
        narrow.handle(Key::F(9));
        narrow.handle(Key::Tab);
        narrow.handle(Key::Tab);
        assert_eq!(narrow.focus, Some(Part::Basic));
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
        // Picking the duration's unit on the unit keys completes the expression.
        typed(&mut app, "2026/12/24 - 12");
        app.handle(Key::F(2));
        typed(&mut app, "T");
        for _ in 0..6 {
            app.handle(Key::Char('l'));
        }
        app.handle(Key::Enter);
        assert_eq!(app.line.text(), "2026/12/24 - 12 d ");
        assert!(app.overlay.is_none());
        assert!(app.preview().unwrap().unwrap().ends_with("2026/12/12 YYYY/MM/DD"));
    }

    #[test]
    fn ambiguous_date_popup() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "12/04/2026 unix");
        assert_eq!(app.preview(), None, "being typed: the question waits");
        typed(&mut app, " ");
        assert!(matches!(app.preview(), Some(Err(m)) if m.starts_with("ambiguous: ") && m.contains("DD/MM/YYYY")));
        app.handle(Key::Enter);
        let Some(Overlay::Choice(a, 0)) = &app.overlay else { panic!("no popup") };
        assert_eq!(a.choices.len(), 2);
        assert!(app.history.entries().is_empty(), "nothing evaluated yet");
        // The toolbar keeps out of the question.
        app.handle(Key::F(2));
        assert!(matches!(app.overlay, Some(Overlay::Choice(..))));
        let Some(Overlay::Choice(a, 0)) = &app.overlay else { panic!("no popup") };
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
        let iso = aequalis::time::request(&config, "2026-04-12T00:00 unix", 4, false).unwrap().unwrap().0;
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
    fn quit_keys() {
        let config = Config::default();
        let opts = options(&config);
        // Ctrl+D quits on an empty line, deletes otherwise.
        let mut app = App::new(&config, &opts);
        typed(&mut app, "ab");
        app.handle(Key::Left);
        app.handle(Key::Ctrl('d'));
        assert_eq!((app.line.text(), app.quit), ("a".to_string(), false));
        app.handle(Key::Ctrl('u'));
        app.handle(Key::Ctrl('d'));
        assert!(app.quit);
        for key in [Key::Ctrl('q'), Key::Alt('q'), Key::Alt('Q')] {
            let mut app = App::new(&config, &opts);
            typed(&mut app, "text");
            app.handle(key);
            assert!(app.quit, "{key:?}");
        }
        let mut app = App::new(&config, &opts);
        typed(&mut app, "quit");
        app.handle(Key::Enter);
        assert!(app.quit);
        // Esc never quits.
        let mut app = App::new(&config, &opts);
        for _ in 0..3 {
            app.handle(Key::Esc);
        }
        assert!(!app.quit);
    }

    #[test]
    fn ctrl_c_closes_clears_then_quits() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "1+1");
        app.handle(Key::Enter);
        typed(&mut app, "draft");
        // A window first (even the question of an ambiguous date), then the history focus, the
        // Tab cycle, the line; then it quits.
        app.handle(Key::F(3));
        app.handle(Key::Ctrl('c'));
        assert!(app.overlay.is_none() && app.line.text() == "draft" && !app.quit);
        app.handle(Key::F(5));
        app.handle(Key::Ctrl('c'));
        assert!(app.overlay.is_none() && app.line.text() == "draft" && !app.quit);
        app.handle(Key::Ctrl('c'));
        assert!(app.line.is_empty() && !app.quit);
        app.handle(Key::Up);
        assert_eq!(app.line.text(), "draft", "↑ brings back what Ctrl+C cleared");
        app.handle(Key::Ctrl('u'));
        app.handle(Key::Ctrl('c'));
        assert!(app.quit);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "12/04/2026 unix ");
        app.handle(Key::Enter);
        assert!(matches!(app.overlay, Some(Overlay::Choice(..))));
        app.handle(Key::Ctrl('c'));
        assert!(app.overlay.is_none() && !app.quit);
    }

    #[test]
    fn about_by_command_or_key() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "about");
        app.handle(Key::Enter);
        assert!(matches!(app.overlay, Some(Overlay::About)) && app.line.is_empty() && app.history.entries().is_empty());
        app.handle(Key::Esc);
        assert!(app.overlay.is_none());
        app.handle(Key::Alt('a'));
        assert!(matches!(app.overlay, Some(Overlay::About)));
        app.handle(Key::Alt('a'));
        assert!(app.overlay.is_none());
    }

    #[test]
    fn actions_do_not_edit_behind_a_window() {
        let config = Config::default();
        let opts = options(&config);
        let mut app = App::new(&config, &opts);
        typed(&mut app, "abc");
        app.handle(Key::F(1));
        app.apply(Action::KillToEnd);
        app.apply(Action::Now);
        assert_eq!(app.line.text(), "abc");
    }
}
