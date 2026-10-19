//! Drawing the interactive screen. Pure: state and size in, frame out. The screen is a grid of
//! styled cells, so the picker and the help can be drawn over the sections.
//!
//! ```text
//! ┌ Units ────────────────────────────────────────────────────────────┐
//! │  A  Area   D  Data   L  Length   M  Mass   T  Temperature   V  Volume
//! └───────────────────────────────────────────────────────────────────┘
//! ┌ History ──────────────────────────────────────────────────────────┐
//! │   10 km mi            = 6.2137 mi                                 │
//! │   (10+12-3)/4Km Mi    = 2.9515 mi                                 │
//! └───────────────────────────────────────────────────────────────────┘
//! ┌ Expression ───────────────────────────────────────────────────────┐
//! │ > 100 + 20 sqft m                                                 │
//! │   = …   Tab: mm2 m2 mi2                                           │
//! └───────────────────────────────────────────────────────────────────┘
//!  INPUT  note: evaluated as (100 + 20) + (50 / 10)    Esc command mode
//! ```

use super::picker::{format_rows, Picker};
use super::{shortcuts, App, Mode, Overlay, StatusKind};
use aequalis::config::System;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Style {
    Plain,
    Bold,
    Dim,
    Inverse,
    Red,
    Green,
    Yellow,
    /// The mode badge.
    Badge,
    /// A lit key: the one typed last, or the selected unit.
    Lit,
}

impl Style {
    fn code(self) -> &'static str {
        match self {
            Style::Plain => "",
            Style::Bold => "\x1b[1m",
            Style::Dim => "\x1b[2m",
            Style::Inverse => "\x1b[7m",
            Style::Red => "\x1b[31m",
            Style::Green => "\x1b[32m",
            Style::Yellow => "\x1b[33m",
            Style::Badge => "\x1b[1;7m",
            Style::Lit => "\x1b[1;30;43m",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CursorShape {
    /// Input mode: a thin bar.
    Bar,
    /// Command mode: a block, as in Vim.
    Block,
}

pub struct Frame {
    pub lines: Vec<String>,
    /// Row and column of the cursor, from 0, and its shape.
    pub cursor: Option<(usize, usize, CursorShape)>,
}

impl Frame {
    /// The escapes that draw the frame; `clear` first erases the whole screen.
    pub fn to_ansi(&self, clear: bool) -> String {
        let mut s = String::from("\x1b[?25l");
        if clear {
            s.push_str("\x1b[2J");
        }
        // Every line fills the whole width, so nothing is erased after it: after the last column
        // the cursor stays on it (pending wrap), and an erase-to-end-of-line there would wipe the
        // right border (bug 003).
        for (i, line) in self.lines.iter().enumerate() {
            s.push_str(&format!("\x1b[{};1H{line}", i + 1));
        }
        if let Some((row, col, shape)) = self.cursor {
            let shape = if shape == CursorShape::Bar { 6 } else { 2 };
            s.push_str(&format!("\x1b[{};{}H\x1b[{shape} q\x1b[?25h", row + 1, col + 1));
        }
        s
    }
}

struct Canvas {
    cols: usize,
    cells: Vec<Vec<(char, Style)>>,
}

impl Canvas {
    fn new(rows: usize, cols: usize) -> Self {
        Canvas { cols, cells: vec![vec![(' ', Style::Plain); cols]; rows] }
    }

    /// Writes `text` from `col`, cut before column `limit`; returns the column after it.
    fn put_within(&mut self, row: usize, col: usize, limit: usize, text: &str, style: Style) -> usize {
        let limit = limit.min(self.cols);
        let mut c = col;
        for ch in text.chars() {
            if c >= limit {
                break;
            }
            self.cells[row][c] = (ch, style);
            c += 1;
        }
        c
    }

    fn put(&mut self, row: usize, col: usize, text: &str, style: Style) -> usize {
        self.put_within(row, col, self.cols, text, style)
    }

    /// A frame with a title; its inside is cleared.
    fn boxed(&mut self, top: usize, left: usize, height: usize, width: usize, title: &str) {
        let right = left + width - 1;
        let bottom = top + height - 1;
        for row in top..=bottom {
            self.cells[row][left..=right].fill((' ', Style::Plain));
            self.cells[row][left] = ('│', Style::Dim);
            self.cells[row][right] = ('│', Style::Dim);
        }
        self.cells[top][left + 1..right].fill(('─', Style::Dim));
        self.cells[bottom][left + 1..right].fill(('─', Style::Dim));
        self.cells[top][left] = ('┌', Style::Dim);
        self.cells[top][right] = ('┐', Style::Dim);
        self.cells[bottom][left] = ('└', Style::Dim);
        self.cells[bottom][right] = ('┘', Style::Dim);
        if !title.is_empty() {
            self.put_within(top, left + 2, right - 1, &format!(" {title} "), Style::Bold);
        }
    }

    fn lines(&self) -> Vec<String> {
        self.cells
            .iter()
            .map(|row| {
                let mut out = String::new();
                let mut current = Style::Plain;
                for &(ch, style) in row {
                    if style != current {
                        out.push_str("\x1b[0m");
                        out.push_str(style.code());
                        current = style;
                    }
                    out.push(ch);
                }
                if current != Style::Plain {
                    out.push_str("\x1b[0m");
                }
                out
            })
            .collect()
    }
}

pub const MIN_ROWS: usize = 12;
pub const MIN_COLS: usize = 40;

pub fn render(app: &App, rows: usize, cols: usize) -> Frame {
    let mut canvas = Canvas::new(rows, cols);
    if rows < MIN_ROWS || cols < MIN_COLS {
        canvas.put(0, 0, &format!("terminal too small (needs {MIN_COLS}x{MIN_ROWS})"), Style::Plain);
        return Frame { lines: canvas.lines(), cursor: None };
    }
    // The top bar: one line of shortcuts in a box.
    let head = 3;
    top_bar(app, &mut canvas, head);
    history(app, &mut canvas, head, rows - 5 - head);
    let mut cursor = expression(app, &mut canvas, rows - 5);
    status(app, &mut canvas, rows - 1);
    // The keypad, top-right in the history: digits, or a category's units while picking one.
    match &app.overlay {
        Some(Overlay::Measures(v)) => measures(app, v, &mut canvas, rows, head),
        Some(Overlay::Picker(p)) => unit_keys(app, p, &mut canvas, rows, head),
        Some(Overlay::Currencies(v)) => {
            currencies(v, &mut canvas, rows, head);
            cursor = None;
        }
        Some(Overlay::Trivia(v)) => trivia(app, v, &mut canvas, rows, head),
        Some(Overlay::Search(s)) => {
            search(app, s, &mut canvas, rows, head);
            cursor = None;
        }
        _ if app.keypad => keypad(app, &mut canvas, head + 1, rows - 7 - head),
        _ => {}
    }
    if let Some(Overlay::Trivia(v @ super::trivia::View { popup: Some(scroll), .. })) = &app.overlay {
        trivia_popup(app, v, &mut canvas, rows, *scroll);
        cursor = None;
    }
    if let Some(Overlay::About) = &app.overlay {
        about(&mut canvas, rows);
        cursor = None;
    }
    if let Some(Overlay::Settings(view)) = &app.overlay {
        settings_window(&mut canvas, rows, view);
        cursor = None;
    }
    if let Some(Overlay::Help(scroll)) = &app.overlay {
        help(&mut canvas, rows, *scroll);
        cursor = None;
    }
    if let Some(Overlay::Choice(a, selected)) = &app.overlay {
        choice(&mut canvas, rows, a, *selected);
        cursor = None;
    }
    Frame { lines: canvas.lines(), cursor }
}

/// The shortcuts shown in the top bar (`room` columns): all of them when they fit, else the
/// window of them that starts at the first one, or scrolled so that `active` (the shortcut of the
/// unit keys shown) is the last; and whether shortcuts are hidden before / after it.
fn visible_shortcuts(config: &aequalis::config::Config, room: usize, active: Option<usize>) -> (std::ops::Range<usize>, bool, bool) {
    // ` K `, ` Name ` and a space between them.
    let widths: Vec<usize> = shortcuts(config).iter().map(|s| s.name.chars().count() + 6).collect();
    if widths.iter().sum::<usize>() <= room {
        return (0..widths.len(), false, false);
    }
    // `‹ ` and ` ›` take two columns each.
    let room = room.saturating_sub(4);
    let fits = |r: std::ops::Range<usize>| widths[r].iter().sum::<usize>() <= room;
    let mut start = 0;
    if let Some(a) = active {
        while start < a && !fits(start..a + 1) {
            start += 1;
        }
    }
    let mut end = start + 1;
    while end < widths.len() && fits(start..end + 1) {
        end += 1;
    }
    (start..end, start > 0, end < widths.len())
}

/// The top bar, `height` rows. In input mode the units are hidden and the bar says how to switch
/// modes; in command mode it shows the categories in alphabetical order with their shortcuts.
fn top_bar(app: &App, canvas: &mut Canvas, height: usize) {
    let cols = canvas.cols;
    let right = cols - 2;
    // Online or offline, at the right of the top border, in both modes.
    let indicator = |canvas: &mut Canvas| {
        use aequalis::network::Status;
        let status = app.network_status();
        let (dot, style) = match status {
            Status::Online => ("●", Style::Green),
            Status::Offline | Status::Forced => ("●", Style::Yellow),
            Status::Checking => ("○", Style::Dim),
        };
        // The label, else a short one where it does not fit after the title (`offline (forced)`
        // → `offline`), else nothing.
        let short = match status {
            Status::Forced => "offline",
            Status::Checking => "…",
            _ => status.label(),
        };
        for label in [status.label(), short] {
            let text = format!(" {dot} {label} ");
            let start = cols.saturating_sub(3 + text.chars().count());
            if start > 18 {
                canvas.put_within(0, start, cols - 2, &text, style);
                break;
            }
        }
    };
    if app.mode == Mode::Input {
        canvas.boxed(0, 0, height, cols, "Input mode");
        indicator(canvas);
        let mut col = 2;
        for (text, style) in [
            ("Press ", Style::Plain),
            ("<Esc>", Style::Yellow),
            (" for command mode, and ", Style::Plain),
            ("I", Style::Yellow),
            (" to be back to input mode", Style::Plain),
        ] {
            col = canvas.put_within(1, col, right, text, style);
        }
        return;
    }
    canvas.boxed(0, 0, height, cols, "Units");
    indicator(canvas);
    // One line: when the shortcuts don't fit, it scrolls to the one of the unit keys shown.
    let list = shortcuts(app.config);
    // What is open, else the highlighted shortcut (← → / h l; the first at the start).
    let active = match &app.overlay {
        Some(Overlay::Measures(_) | Overlay::Picker(_)) => list.iter().position(|s| s.name == super::MEASURES.0),
        Some(Overlay::Currencies(_)) => list.iter().position(|s| s.name == super::CURRENCIES.0),
        Some(Overlay::Trivia(_)) => list.iter().position(|s| s.name == super::TRIVIA.0),
        Some(Overlay::Search(_)) => list.iter().position(|s| s.name == super::SEARCH.0),
        Some(Overlay::Settings(_)) => list.iter().position(|s| s.name == super::SETTINGS.0),
        _ => Some(app.menu.min(list.len().saturating_sub(1))),
    };
    let (shown, before, after) = visible_shortcuts(app.config, right - 2, active);
    let mut col = 2;
    if before {
        col = canvas.put_within(1, col, right, "‹ ", Style::Dim);
    }
    for s in &list[shown] {
        // The highlighted one (or the one open) lit whole: its key and its name.
        let lit = active.is_some_and(|a| list[a].key == s.key);
        col = canvas.put_within(1, col, right, &format!(" {} ", s.key), if lit { Style::Lit } else { Style::Inverse });
        col = canvas.put_within(1, col, right, &format!(" {} ", s.name), if lit { Style::Lit } else { Style::Plain });
        // A space between the items, so a highlighted one stands apart.
        col = canvas.put_within(1, col, right, " ", Style::Plain);
    }
    if after {
        canvas.put_within(1, col, right, "›", Style::Dim);
    }
}

/// The last entries (minus the scroll), newest at the bottom, results aligned.
fn history(app: &App, canvas: &mut Canvas, top: usize, height: usize) {
    let cols = canvas.cols;
    canvas.boxed(top, 0, height, cols, "History");
    let inner = height - 2;
    let right = cols - 2;
    let entries = app.history.entries();
    if entries.is_empty() {
        // Not while picking a unit: the unit keys cover that row.
        if !matches!(app.overlay, Some(Overlay::Picker(_))) {
            canvas.put_within(top + 1, 2, right, "Type an expression, e.g. 10 km mi or (100+20)/120", Style::Dim);
        }
        return;
    }
    let scroll = app.scroll.min(entries.len().saturating_sub(1));
    let end = entries.len() - scroll;
    let start = end.saturating_sub(inner);
    let width = entries.range(start..end).map(|e| e.input.chars().count()).max().unwrap_or(0).min(cols / 2);
    let first_row = top + 1 + inner - (end - start);
    for (i, entry) in entries.range(start..end).enumerate() {
        let row = first_row + i;
        let selected = app.history.selected() == Some(start + i);
        let mut col = canvas.put_within(row, 2, right, if selected { "› " } else { "  " }, Style::Bold);
        let input_style = if selected { Style::Inverse } else { Style::Plain };
        col = canvas.put_within(row, col, right, &format!("{:<width$}", entry.input), input_style);
        match &entry.result {
            Ok(result) => {
                col = canvas.put_within(row, col, right, "  = ", Style::Dim);
                canvas.put_within(row, col, right, result, Style::Bold);
            }
            Err(msg) => {
                col = canvas.put_within(row, col, right, "  ✗ ", Style::Red);
                canvas.put_within(row, col, right, msg, Style::Red);
            }
        }
    }
    if scroll > 0 {
        let label = format!(" ↓ {scroll} newer ");
        canvas.put_within(top + height - 1, cols - 2 - label.chars().count(), cols - 2, &label, Style::Dim);
    }
}

/// The keypad, row by row. `=` stands for Enter, `e` for the exponent (`1e3`).
const KEYPAD: [[char; 4]; 5] = [['(', ')', '^', '/'], ['7', '8', '9', '*'], ['4', '5', '6', '-'], ['1', '2', '3', '+'], ['0', '.', 'e', '=']];

/// A numeric keypad in the top-right corner of the history, in front of its lines. Full size with a
/// grid when there is room (11 rows), compact (7 rows) otherwise, not drawn when even that doesn't fit.
/// The key typed last is lit.
fn keypad(app: &App, canvas: &mut Canvas, top: usize, room: usize) {
    let full = room >= 11;
    if !full && room < 7 {
        return;
    }
    let (cell, width, height) = if full { (4, 17, 11) } else { (3, 15, 7) };
    let left = canvas.cols - 2 - width;
    canvas.boxed(top, left, height, width, "");
    // Clear the empty title the box drew.
    canvas.cells[top][left + 1..left + width - 1].fill(('─', Style::Dim));
    let key_row = |r: usize| if full { top + 1 + 2 * r } else { top + 1 + r };
    if full {
        for r in 0..KEYPAD.len() {
            for c in 1..4 {
                canvas.cells[key_row(r)][left + 4 * c] = ('│', Style::Dim);
                canvas.cells[top][left + 4 * c] = ('┬', Style::Dim);
                canvas.cells[top + height - 1][left + 4 * c] = ('┴', Style::Dim);
            }
            if r + 1 < KEYPAD.len() {
                let sep = key_row(r) + 1;
                canvas.cells[sep][left + 1..left + width - 1].fill(('─', Style::Dim));
                canvas.cells[sep][left] = ('├', Style::Dim);
                canvas.cells[sep][left + width - 1] = ('┤', Style::Dim);
                for c in 1..4 {
                    canvas.cells[sep][left + 4 * c] = ('┼', Style::Dim);
                }
            }
        }
    }
    for (r, keys) in KEYPAD.iter().enumerate() {
        for (c, &key) in keys.iter().enumerate() {
            let x = left + 1 + cell * c;
            let style = match key {
                k if Some(k) == app.pressed => Style::Lit,
                '0'..='9' | '.' => Style::Bold,
                _ => Style::Yellow,
            };
            canvas.put(key_row(r), x, &format!(" {key} "), style);
        }
    }
}

const PROMPT: &str = "> ";

/// The expression bar and, below it, the live result and the suggestions. Returns the cursor.
fn expression(app: &App, canvas: &mut Canvas, top: usize) -> Option<(usize, usize, CursorShape)> {
    let cols = canvas.cols;
    canvas.boxed(top, 0, 4, cols, "Expression");
    let right = cols - 2;
    // Scrolled sideways so the cursor stays visible.
    let start = 2 + PROMPT.len();
    let room = right - start - 1;
    let offset = app.line.cursor().saturating_sub(room);
    let visible: String = app.line.chars().iter().skip(offset).collect();
    canvas.put_within(top + 1, 2, right, PROMPT, Style::Bold);
    canvas.put_within(top + 1, start, right, &visible, Style::Plain);

    // The grouping applied (when there are operations), then the result or the error.
    let row = top + 2;
    let mut col = 4;
    if let Some(grouping) = app.grouping() {
        col = canvas.put_within(row, col, right, &grouping, Style::Bold);
        col = canvas.put_within(row, col, right, " ", Style::Plain);
    }
    match app.preview() {
        Some(Ok(result)) => {
            col = canvas.put_within(row, col, right, &format!("= {result}"), Style::Green);
            if let Some(currencies) = app.currencies() {
                col = canvas.put_within(row, col, right, &format!("   {currencies}"), Style::Dim);
            }
        }
        Some(Err(msg)) => col = canvas.put_within(row, col, right, &format!("· {msg}"), Style::Dim),
        None => {}
    }
    if let Some((names, selected, cities)) = app.suggestions() {
        col = canvas.put_within(row, col, right, "   ", Style::Plain);
        if selected.is_none() {
            col = canvas.put_within(row, col, right, "Tab:", Style::Dim);
        }
        for (i, name) in names.iter().enumerate() {
            // Cities have spaces in their labels: `·` between them.
            match (cities, i) {
                (true, 1..) => col = canvas.put_within(row, col, right, " · ", Style::Dim),
                _ => col = canvas.put_within(row, col, right, " ", Style::Plain),
            }
            let style = if selected == Some(i) { Style::Inverse } else { Style::Plain };
            col = canvas.put_within(row, col, right, name, style);
        }
    }
    let shape = if app.mode == Mode::Input { CursorShape::Bar } else { CursorShape::Block };
    Some((top + 1, start + app.line.cursor() - offset, shape))
}

/// `<key> what · <key> what`, right-aligned to end before column `end`; returns its first column.
/// On a box border (`framed`), it is padded with spaces so the line does not run into it.
fn hints(canvas: &mut Canvas, row: usize, end: usize, items: &[(&str, &str)], framed: bool) -> usize {
    let pad = if framed { " " } else { "" };
    let text_len: usize = items.iter().map(|(k, w)| k.chars().count() + 1 + w.chars().count()).sum::<usize>() + 3 * (items.len() - 1) + 2 * pad.len();
    let start = end.saturating_sub(text_len);
    let mut col = canvas.put_within(row, start, end, pad, Style::Plain);
    for (i, (key, what)) in items.iter().enumerate() {
        if i > 0 {
            col = canvas.put_within(row, col, end, " · ", Style::Dim);
        }
        col = canvas.put_within(row, col, end, key, Style::Yellow);
        col = canvas.put_within(row, col, end, &format!(" {what}"), Style::Dim);
    }
    canvas.put_within(row, col, end, pad, Style::Plain);
    start
}

/// The mode badge, the last note / warning / error, and the mode's keys on the right.
fn status(app: &App, canvas: &mut Canvas, row: usize) {
    let cols = canvas.cols;
    let (badge, keys): (&str, &[(&str, &str)]) = match (app.mode, &app.overlay) {
        // The unit keys: how to move around them.
        // Browsing the history: its keys.
        (_, None) if app.history.selected().is_some() => (" HISTORY ", &[("↑↓", "browse"), ("Enter", "edit"), ("C", "copy"), ("<Esc>", "back")]),
        (_, Some(Overlay::Choice(..))) => (" CHOOSE ", &[("↑↓ j k 1-9", "reading"), ("Enter", "confirm"), ("<Esc>", "back")]),
        (_, Some(Overlay::Trivia(v))) if v.popup.is_some() => (" TRIVIA ", &[("↑↓", "scroll"), ("Enter", "write the value"), ("<Esc>", "back")]),
        (_, Some(Overlay::Trivia(_))) => (" TRIVIA ", &[("Tab ←→", "section"), ("↑↓ j k", "entry"), ("Enter", "details"), ("<Esc>", "back")]),
        (_, Some(Overlay::About)) => (" ABOUT ", &[("<Esc>", "close")]),
        (_, Some(Overlay::Settings(v))) if v.edit.is_some() => (" SETTINGS ", &[("type", "value"), ("Enter", "save"), ("<Esc>", "cancel")]),
        (_, Some(Overlay::Settings(_))) => (" SETTINGS ", &[("↑↓", "choose"), ("Enter ←→", "change"), ("d", "default"), ("<Esc>", "close")]),
        (_, Some(Overlay::Search(_))) => (" SEARCH ", &[("type", "search"), ("↑↓", "choose"), ("Enter", "write"), ("<Esc>", "back")]),
        (_, Some(Overlay::Currencies(_))) => (" CURRENCIES ", &[("type", "search"), ("↑↓", "choose"), ("Enter", "write"), ("<Esc>", "back")]),
        (_, Some(Overlay::Measures(_))) => (" MEASURES ", &[("↑↓ j k", "choose"), ("letter Enter", "open"), ("<Esc>", "back")]),
        (_, Some(Overlay::Picker(_))) => (" COMMAND ", &[("←↓↑→ h j k l", "move"), ("Tab", "system"), ("Enter", "write"), ("<Esc>", "measures")]),
        (Mode::Input, _) => (" INPUT ", &[("<Esc>", "command mode"), ("exit", "quit")]),
        (Mode::Command, _) => (" COMMAND ", &[("←→ Enter", "menu"), ("<I>", "input"), ("<C>", "keypad"), ("<E>", "help"), ("<A>", "about"), ("<q>", "quit")]),
    };
    // Too narrow for every key: the first ones go (the top bar shows the menu anyway).
    let width = |keys: &[(&str, &str)]| keys.iter().map(|(k, w)| k.chars().count() + 1 + w.chars().count() + 3).sum::<usize>() - 3;
    let mut keys = keys;
    while keys.len() > 1 && badge.chars().count() + 1 + width(keys) > cols - 1 {
        keys = &keys[1..];
    }
    let hint_col = hints(canvas, row, cols - 1, keys, false);
    let col = canvas.put(row, 0, badge, Style::Badge) + 1;
    let limit = hint_col.saturating_sub(1);
    match &app.status {
        Some((StatusKind::Note, msg)) => canvas.put_within(row, col, limit, &format!("note: {msg}"), Style::Dim),
        Some((StatusKind::Warning, msg)) => canvas.put_within(row, col, limit, &format!("warning: {msg}"), Style::Yellow),
        Some((StatusKind::Error, msg)) => canvas.put_within(row, col, limit, &format!("error: {msg}"), Style::Red),
        None => col,
    };
}

/// A centred window of at most `width` × `height`; returns (top, left, width, height).
fn centred(canvas: &mut Canvas, rows: usize, width: usize, height: usize, title: &str) -> (usize, usize, usize, usize) {
    let (width, height) = (width.min(canvas.cols - 4), height.min(rows - 2));
    let (top, left) = ((rows - height) / 2, (canvas.cols - width) / 2);
    canvas.boxed(top, left, height, width, title);
    (top, left, width, height)
}

/// The keypad while picking a unit: the two tabs (metric / imperial, or decimal / binary), then
/// the units of the tab shown as buttons, the category's `columns` per row
/// (5 for Area and Data, 4 for the others), the selected one lit.
/// Top-right in the history like the digits; it grows upwards over the Units bar when the history
/// is too short.
fn unit_keys(app: &App, p: &Picker, canvas: &mut Canvas, rows: usize, head: usize) {
    let category = p.category(app.config);
    let items = p.items(app.config);
    let button = category.units.iter().map(|u| u.symbol.chars().count()).max().unwrap_or(1) + 2;
    let title = format!("{} units", p.title(app.config));
    let columns = super::measures::columns(category);
    // Wide enough for the buttons, the title and the tabs.
    let tabs = p.tabs(app.config);
    let tabs_len: usize = tabs.iter().map(|(n, _)| n.chars().count() + 3).sum();
    // Time's second tab is the table of date formats.
    let formats = (category.name == aequalis::config::TIME).then(format_rows);
    let table_width = 2 + FORMAT_LABEL + 1 + 5 * (FORMAT_CELL + 1) + 1;
    let width =
        (columns * (button + 1) + 1).max(title.chars().count() + 6).max(tabs_len + 4).max(if formats.is_some() { table_width } else { 0 }).min(canvas.cols - 3);
    // Sized for the larger tab, so the window does not jump while switching.
    let most = [System::Metric, System::Imperial].map(|tab| p.units_of(app.config, tab).count()).into_iter().max().unwrap_or(1);
    // The Formats tab: the table and a footer line for the selected format.
    let dates_rows = formats.as_ref().map_or(0, |t| t.len() + 1);
    // Below the top bar when there is room; small terminals let it cover the top bar.
    let table_rows = most.div_ceil(columns);
    let height = (table_rows.max(dates_rows) + 4).min((rows - 7 - head).max(7));
    let top = (head + 1).min(rows - 5 - height);
    let left = canvas.cols - 2 - width;
    let right = left + width - 1;
    canvas.boxed(top, left, height, width, &title);

    // The tabs: the one shown is lit.
    let mut col = left + 2;
    for (name, shown) in tabs {
        col = canvas.put_within(top + 1, col, right, &format!(" {name} "), if shown { Style::Lit } else { Style::Dim });
        col = canvas.put_within(top + 1, col, right, " ", Style::Plain);
    }
    canvas.cells[top + 2][left + 1..right].fill(('─', Style::Dim));
    canvas.cells[top + 2][left] = ('├', Style::Dim);
    canvas.cells[top + 2][right] = ('┤', Style::Dim);
    let first = top + 3;
    let visible = height - 4;

    if let (true, Some(table)) = (p.shows_formats(app.config), &formats) {
        let visible = visible.saturating_sub(1).max(1);
        let offset = p.cell.0.saturating_sub(visible.saturating_sub(1));
        let mut chosen = None;
        for (r, row) in table.iter().enumerate().skip(offset).take(visible) {
            let y = first + r - offset;
            canvas.put_within(y, left + 2, right, row.label, Style::Bold);
            let x0 = left + 2 + FORMAT_LABEL + 1;
            for (c, (short, word)) in row.cells.iter().enumerate() {
                let x = x0 + c * (FORMAT_CELL + 1);
                let selected = (r, c) == p.cell;
                let end = canvas.put_within(y, x, right, &format!("{short:^FORMAT_CELL$}"), if selected { Style::Lit } else { Style::Plain });
                if c + 1 < row.cells.len() && end < right {
                    canvas.cells[y][end] = ('│', Style::Dim);
                }
                if selected {
                    chosen = Some((word.clone(), x));
                }
            }
        }
        // The footer: which format, now in it, and (when it fits) what it is.
        let footer = top + height - 2;
        if let (Some((word, _)), true) = (chosen, height >= 6) {
            let [name, what, example] = format_card_lines(&word);
            let mut col = canvas.put_within(footer, left + 2, right, &name, Style::Bold);
            col = canvas.put_within(footer, col, right, &format!(" · {}", example.trim_start_matches("now: ")), Style::Green);
            if col + 3 + what.chars().count() < right {
                canvas.put_within(footer, col, right, &format!(" · {what}"), Style::Dim);
            }
        }
        return;
    }

    let table = p.rows(app.config);
    let offset = Picker::position(&table, p.selected).0.saturating_sub(visible.saturating_sub(1));
    let mut chosen = None;
    let mut x = left + 1;
    for (i, unit) in items.iter().enumerate() {
        let (r, c) = Picker::position(&table, i);
        if c == 0 {
            x = left + 1;
        }
        // All as wide as the category's longest.
        let width = button;
        let at = x;
        x += width + 1;
        if r < offset || r >= offset + visible {
            continue;
        }
        let row = first + r - offset;
        let style = if i == p.selected { Style::Lit } else { Style::Bold };
        let end = canvas.put_within(row, at, right, &format!("{:^width$}", unit.symbol), style);
        if c + 1 < table[r] && end < right {
            canvas.cells[row][end] = ('│', Style::Dim);
        }
        if i == p.selected {
            chosen = Some((*unit, at, width));
        }
    }
    if let Some((unit, x, width)) = chosen {
        card(canvas, &unit_card_lines(app, category, unit), (rows, head), (top, top + height - 1), (x, x + width));
    }
}

/// `text` in lines of at most `width` characters, broken between words.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

/// The Trivia: the sections as tabs, the entries of the one shown (name, value), and the selected
/// entry's description at the bottom. Top-right in the history, like the unit keys.
fn trivia(app: &App, v: &super::trivia::View, canvas: &mut Canvas, rows: usize, head: usize) {
    let width = 78.min(canvas.cols - 3);
    let height = (rows - 7 - head).max(8).min(rows - 5);
    let top = (head + 1).min(rows - 5 - height);
    let left = canvas.cols - 2 - width;
    let right = left + width - 1;
    canvas.boxed(top, left, height, width, super::TRIVIA.0);
    let mut col = left + 2;
    for (i, name) in aequalis::trivia::sections().iter().enumerate() {
        col = canvas.put_within(top + 1, col, right, &format!(" {name} "), if i == v.section { Style::Lit } else { Style::Dim });
        col = canvas.put_within(top + 1, col, right, " ", Style::Plain);
    }
    let rule = |canvas: &mut Canvas, row: usize| {
        canvas.cells[row][left + 1..right].fill(('─', Style::Dim));
        canvas.cells[row][left] = ('├', Style::Dim);
        canvas.cells[row][right] = ('┤', Style::Dim);
    };
    rule(canvas, top + 2);
    // The description: up to 3 lines and one about the value's source, when there is room.
    let footer = if height >= 13 { 4 } else { 0 };
    let visible = height - 4 - if footer > 0 { footer + 1 } else { 0 };
    let entries = v.entries();
    let now = aequalis::time::unix_now();
    // A live value: from the online data (synced when, from where), or its estimate.
    let live = |e: &aequalis::trivia::Entry| {
        let synced = app.online.get(&e.name);
        let origin = aequalis::trivia::value(app.config, e, synced, now).map(|(_, o)| o).unwrap_or(aequalis::trivia::Origin::Fixed);
        (synced, aequalis::trivia::origin_text(e, origin).unwrap_or_default())
    };
    // Columns: the name, the value (in the device's number format), the short description.
    let values: Vec<String> = entries
        .iter()
        .map(|e| match &e.source {
            aequalis::trivia::Source::Tip(example) => example.clone(),
            _ => aequalis::trivia::value(app.config, e, live(e).0, now)
                .map(|(x, _)| aequalis::trivia::shown(app.config, e, x, app.opts.precision, &|n| app.numbers.apply(&n)))
                .unwrap_or_else(|err| err),
        })
        .collect();
    let name_width = entries.iter().map(|e| e.name.chars().count()).max().unwrap_or(0).min(width / 2);
    let value_width = values.iter().map(|v| v.chars().count()).max().unwrap_or(0);
    let offset = v.selected.saturating_sub(visible.saturating_sub(1));
    for (i, e) in entries.iter().enumerate().skip(offset).take(visible) {
        let row = top + 3 + i - offset;
        let selected = i == v.selected;
        if selected {
            canvas.cells[row][left + 1..right].fill((' ', Style::Lit));
        }
        let text = format!(" {:<name_width$}  {:>value_width$}  ", e.name, values[i]);
        let col = canvas.put_within(row, left + 1, right, &text, if selected { Style::Lit } else { Style::Plain });
        // The short description, cut with … when the window is too narrow.
        let room = right.saturating_sub(col + 1);
        let full = aequalis::trivia::short(e, aequalis::time::unix_now());
        let short: String = match full.chars().count() > room {
            true => full.chars().take(room.saturating_sub(1)).chain(['…']).collect(),
            false => full,
        };
        canvas.put_within(row, col, right, &short, if selected { Style::Lit } else { Style::Dim });
    }
    if footer == 0 {
        return;
    }
    let base = top + height - 1 - footer;
    rule(canvas, base - 1);
    if let Some(e) = v.entry() {
        for (i, line) in wrap(&e.description, width - 4).iter().take(3).enumerate() {
            canvas.put_within(base + i, left + 2, right, line, Style::Plain);
        }
        let source = match &e.source {
            aequalis::trivia::Source::Spacecraft { .. } | aequalis::trivia::Source::Satellites { .. } => live(e).1,
            aequalis::trivia::Source::Between(..) => "computed from the coordinates".to_string(),
            aequalis::trivia::Source::Tip(_) => "Enter writes the example".to_string(),
            aequalis::trivia::Source::Value(_) => String::new(),
        };
        let launched = e.launched.as_ref().map(|l| format!("Launched {l}")).unwrap_or_default();
        let info = [launched, source].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
        canvas.put_within(base + 3, left + 2, right, &info, Style::Green);
    }
}

/// The Trivia's popup, titled with the entry's name: its description; its Wikipedia link; then
/// one line per fact, `Label: text` (the label in bold, the text wrapped under itself):
///
/// ```text
/// Space probe launched by NASA in the Voyager program …
///
/// Wikipedia: https://en.wikipedia.org/wiki/Voyager_1
///
/// Current distance: 25,794,004,533 km - 172.4223 au (updated now)
/// Launched: 1977-09-05 12:56 UTC
/// ```
///
/// Scrolled down by `scroll` lines.
fn trivia_popup(app: &App, v: &super::trivia::View, canvas: &mut Canvas, rows: usize, scroll: usize) {
    let Some(entry) = v.entry() else { return };
    let now = aequalis::time::unix_now();
    let live = app.online.get(&entry.name);
    let width = 76.min(canvas.cols - 4);
    let text_width = width - 4;
    // Each line: its pieces and their styles.
    let mut lines: Vec<Vec<(String, Style)>> = wrap(&entry.description, text_width).into_iter().map(|l| vec![(l, Style::Plain)]).collect();
    let facts = aequalis::trivia::popup(app.config, entry, live, now, app.opts.precision, &|n| app.numbers.apply(&n));
    for (i, (label, text)) in facts.iter().enumerate() {
        // A blank line after the description, and around the link.
        if i == 0 || (i == 1 && facts[0].0 == "Wikipedia") {
            lines.push(Vec::new());
        }
        let head = format!("{label}: ");
        let indent = 2;
        let first = text_width.saturating_sub(head.chars().count()).max(10);
        let mut words = wrap(text, first).into_iter();
        let first_line = words.next().unwrap_or_default();
        let link = if label == "Wikipedia" { Style::Green } else { Style::Plain };
        lines.push(vec![(head, Style::Bold), (first_line, link)]);
        let rest: String = words.collect::<Vec<_>>().join(" ");
        for l in wrap(&rest, text_width - indent) {
            lines.push(vec![(format!("{}{l}", " ".repeat(indent)), link)]);
        }
    }
    let height = (lines.len() + 2).min(rows - 2);
    let (top, left, width, height) = centred(canvas, rows, width, height, &entry.name);
    let right = left + width - 1;
    let visible = height - 2;
    let scroll = scroll.min(lines.len().saturating_sub(visible));
    for (i, pieces) in lines.iter().skip(scroll).take(visible).enumerate() {
        let mut col = left + 2;
        for (text, style) in pieces {
            col = canvas.put_within(top + 1 + i, col, right - 1, text, *style);
        }
    }
    if scroll + visible < lines.len() {
        canvas.put_within(top + height - 1, right.saturating_sub(10), right - 1, " ↓ more ", Style::Dim);
    }
}

/// `/`: the query, and the units matching it (symbol, name, category), the chosen one lit.
fn search(app: &App, s: &super::search::Search, canvas: &mut Canvas, rows: usize, head: usize) {
    let found = super::search::matches(app.config, &s.query);
    let width = 60.min(canvas.cols - 3);
    let height = (found.len().max(1) + 4).min((rows - 7 - head).max(6)).min(rows - 5);
    let top = (head + 1).min(rows - 5 - height);
    let left = canvas.cols - 2 - width;
    let right = left + width - 1;
    canvas.boxed(top, left, height, width, "Search units");
    let col = canvas.put_within(top + 1, left + 2, right, "/ ", Style::Yellow);
    let col = canvas.put_within(top + 1, col, right, &s.query, Style::Bold);
    canvas.put_within(top + 1, col, right, "▏", Style::Yellow);
    canvas.cells[top + 2][left + 1..right].fill(('─', Style::Dim));
    canvas.cells[top + 2][left] = ('├', Style::Dim);
    canvas.cells[top + 2][right] = ('┤', Style::Dim);
    let visible = height - 4;
    if found.is_empty() {
        canvas.put_within(top + 3, left + 2, right, "no unit matches", Style::Dim);
        return;
    }
    let symbol_width = found.iter().map(|(_, u)| u.symbol.chars().count()).max().unwrap_or(0);
    let offset = s.selected.saturating_sub(visible.saturating_sub(1));
    for (i, (c, u)) in found.iter().enumerate().skip(offset).take(visible) {
        let style = if i == s.selected { Style::Lit } else { Style::Plain };
        let text = format!(" {:<symbol_width$}  {} · {} ", u.symbol, u.name, c.name);
        canvas.put_within(top + 3 + i - offset, left + 1, right, &text, style);
    }
}

/// The Measures: the unit classes, one per row with its letter and a few of its units. Top-right
/// in the history, like the unit keys.
fn measures(app: &App, v: &super::measures::View, canvas: &mut Canvas, rows: usize, head: usize) {
    use super::measures::{items, label, Item};
    let items = items(app.config);
    let width = 60.min(canvas.cols - 3);
    let height = (items.len() + 2).min((rows - 7 - head).max(6)).min(rows - 5);
    let top = (head + 1).min(rows - 5 - height);
    let left = canvas.cols - 2 - width;
    let right = left + width - 1;
    canvas.boxed(top, left, height, width, super::MEASURES.0);
    let names = items.iter().map(|&i| label(app.config, i).0.chars().count()).max().unwrap_or(0);
    let visible = height - 2;
    let offset = v.selected.saturating_sub(visible.saturating_sub(1));
    for (i, &item) in items.iter().enumerate().skip(offset).take(visible) {
        let row = top + 1 + i - offset;
        let (name, key) = label(app.config, item);
        let lit = i == v.selected;
        let col = canvas.put_within(row, left + 1, right, &format!(" {key} "), if lit { Style::Lit } else { Style::Inverse });
        let col = canvas.put_within(row, col, right, &format!(" {name:<names$} "), if lit { Style::Lit } else { Style::Bold });
        // Its units, the whole ones that fit, then `…`.
        let room = (right - 1).saturating_sub(col + 1);
        let sample = match item {
            Item::Category(c) => {
                let mut sample = String::new();
                let units = &app.config.categories[c].units;
                for (n, u) in units.iter().enumerate() {
                    let more = if n + 1 < units.len() { 2 } else { 0 };
                    if sample.chars().count() + 1 + u.symbol.chars().count() + more > room {
                        sample.push_str(" …");
                        break;
                    }
                    sample.push(' ');
                    sample.push_str(u.symbol);
                }
                sample.trim_start().to_string()
            }
            Item::EarthDistance => "Rome to Paris, 41.9,12.5 to Tokyo".to_string(),
        };
        canvas.put_within(row, col + 1, right - 1, &sample, Style::Dim);
    }
}

/// The Currencies: the search line, then the currencies matching it (code, symbol, name).
fn currencies(v: &super::currencies::View, canvas: &mut Canvas, rows: usize, head: usize) {
    let found = aequalis::currency::search(&v.query);
    let width = 60.min(canvas.cols - 3);
    let height = (found.len().max(1) + 4).min((rows - 7 - head).max(6)).min(rows - 5);
    let top = (head + 1).min(rows - 5 - height);
    let left = canvas.cols - 2 - width;
    let right = left + width - 1;
    canvas.boxed(top, left, height, width, super::CURRENCIES.0);
    let col = canvas.put_within(top + 1, left + 2, right, "/ ", Style::Yellow);
    let col = canvas.put_within(top + 1, col, right, &v.query, Style::Bold);
    canvas.put_within(top + 1, col, right, "▏", Style::Yellow);
    canvas.cells[top + 2][left + 1..right].fill(('─', Style::Dim));
    canvas.cells[top + 2][left] = ('├', Style::Dim);
    canvas.cells[top + 2][right] = ('┤', Style::Dim);
    let visible = height - 4;
    if found.is_empty() {
        canvas.put_within(top + 3, left + 2, right, "no currency matches", Style::Dim);
        return;
    }
    let offset = v.selected.saturating_sub(visible.saturating_sub(1));
    for (i, c) in found.iter().enumerate().skip(offset).take(visible) {
        let style = if i == v.selected { Style::Lit } else { Style::Plain };
        let symbol = c.symbols.first().map_or("", String::as_str);
        let text = format!(" {}  {:<4} {} ", c.code, symbol, c.name);
        canvas.put_within(top + 3 + i - offset, left + 1, right, &text, style);
    }
}

/// Width of the Formats tab's row labels (`YYYY/MM/DD`) and of its cells (`ordinal`, `24h tz`).
const FORMAT_LABEL: usize = 10;
const FORMAT_CELL: usize = 9;

/// The card of a date format: its name, what it is, and the current time in it.
fn format_card_lines(word: &str) -> [String; 3] {
    // Whole seconds: an example, not a clock.
    let now = aequalis::time::now();
    let now = aequalis::time::Instant { ns: now.ns - now.ns.rem_euclid(1_000_000_000), ..now };
    let example = aequalis::time::format(&now, word).unwrap_or_else(|_| "not available here".into());
    [word.to_string(), aequalis::time::describe_format(word), format!("now: {example}")]
}

/// What the card under the selected unit says: full name, aliases, value in the base unit.
fn unit_card_lines(app: &App, category: &aequalis::config::Category, unit: &aequalis::config::Unit) -> [String; 3] {
    let number = |v: f64| app.numbers.apply(&aequalis::convert::format_value(v, 10));
    let aliases = match unit.aliases {
        [] => "aliases: none".to_string(),
        list => format!("aliases: {}", list.join(", ")),
    };
    let value = if unit.symbol == category.base {
        format!("base unit of {}", category.name)
    } else if unit.offset != 0.0 {
        format!("0 {} = {} {}", unit.symbol, number(unit.offset), category.base)
    } else {
        format!("1 {} = {} {}", unit.symbol, number(unit.factor), category.base)
    };
    [unit.name.to_string(), aliases, value]
}

/// A card below the unit keys window, starting under the selected button, with a `▲` pointing up at
/// it. All the buttons stay visible. It moves left to stay on screen, and goes above the window
/// (with a `▼`) when there is no room below the history.
fn card(
    canvas: &mut Canvas,
    lines: &[String; 3],
    (rows, head): (usize, usize),
    (window_top, window_bottom): (usize, usize),
    (button_left, button_right): (usize, usize),
) {
    // Inside the history's borders.
    let width = (lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) + 4).min(canvas.cols - 2);
    let height = lines.len() + 2;
    let left = button_left.min(canvas.cols - 1 - width);
    // Below the window, inside the history (which ends 5 rows above the bottom); else above it;
    // else, without covering the window's title, inside the window at its bottom. Never over the
    // top bar (`head` rows).
    let below = window_bottom + 1 + height <= rows - 5;
    let above = window_top >= head + height;
    let top = match (below, above) {
        (true, _) => window_bottom + 1,
        (false, true) => window_top - height,
        (false, false) => window_bottom.saturating_sub(height).max(window_top + 1),
    };
    canvas.boxed(top, left, height, width, "");
    let pointer = (button_left + button_right) / 2;
    if (below || above) && (left + 1..left + width - 1).contains(&pointer) {
        let edge = if below { top } else { top + height - 1 };
        canvas.cells[edge][pointer] = (if below { '▲' } else { '▼' }, Style::Yellow);
    }
    let right = left + width - 1;
    for (i, (text, style)) in lines.iter().zip([Style::Bold, Style::Plain, Style::Green]).enumerate() {
        canvas.put_within(top + 1 + i, left + 2, right, text, style);
    }
}

/// The readings of an ambiguous date, to choose from: `1  DD/MM/YYYY  2026-04-12`.
fn choice(canvas: &mut Canvas, rows: usize, a: &aequalis::time::Ambiguity, selected: usize) {
    let title = match a.kind {
        aequalis::time::AmbiguityKind::City => format!("{}: which one?", a.note),
        aequalis::time::AmbiguityKind::Country => format!("`{}` has several time zones: which one?", a.text),
        aequalis::time::AmbiguityKind::Date => format!("`{}` is ambiguous: which date?", a.text),
        aequalis::time::AmbiguityKind::Currency | aequalis::time::AmbiguityKind::Place => format!("{}: which one?", a.note),
    };
    // The patterns in a column as wide as the longest (at least 12), the dates after them.
    let column = a.choices.iter().map(|c| c.pattern.chars().count()).max().unwrap_or(0).max(12);
    let width = a.choices.iter().map(|c| column + c.date.chars().count() + 9).max().unwrap_or(0).max(title.chars().count() + 6);
    let (top, left, width, _) = centred(canvas, rows, width, a.choices.len() + 2, &title);
    let right = left + width - 1;
    for (i, c) in a.choices.iter().enumerate() {
        let row = top + 1 + i;
        let text = format!(" {}  {:<column$} {} ", i + 1, c.pattern, c.date);
        let style = if i == selected { Style::Lit } else { Style::Plain };
        canvas.put_within(row, left + 2, right - 1, &text, style);
    }
}

/// `O`: the settings, their values (changed ones bold), what the chosen one does, the download.
fn settings_window(canvas: &mut Canvas, rows: usize, view: &super::settings::View) {
    use aequalis::settings::KEYS;
    let current = &view.settings;
    let key_width = KEYS.iter().map(|k| k.key.len()).max().unwrap_or(0);
    let lines = super::settings::rows() + 5;
    let (top, left, width, height) = centred(canvas, rows, 78, lines, super::SETTINGS.0);
    let right = left + width - 2;
    let visible = height.saturating_sub(5);
    let offset = view.selected.saturating_sub(visible.saturating_sub(1));
    for (row, i) in (top + 1..).zip((0..super::settings::rows()).skip(offset).take(visible)) {
        let chosen = i == view.selected;
        let text = match KEYS.get(i) {
            Some(k) => {
                let value = match (&view.edit, chosen) {
                    (Some(typed), true) => format!("{typed}▏"),
                    _ => current.get(k.key).to_string(),
                };
                format!(" {:<key_width$}  {value}", k.key)
            }
            // Whether the city archive is there, how big and recent.
            None => match aequalis::cities::meta() {
                Some(m) => format!(" {}  (city archive: {} cities of {})", super::settings::DOWNLOAD, m.count, aequalis::time::human_day(m.built_at)),
                None => format!(" {}  (city archive: not downloaded)", super::settings::DOWNLOAD),
            },
        };
        let style = match (chosen, KEYS.get(i)) {
            (true, _) => Style::Lit,
            (false, Some(k)) if current.get(k.key) != k.default => Style::Bold,
            (false, Some(_)) => Style::Plain,
            (false, None) => Style::Yellow,
        };
        canvas.put_within(row, left + 1, right, &text, style);
    }
    let sep = top + height - 4;
    canvas.cells[sep][left + 1..left + width - 1].fill(('─', Style::Dim));
    let about = KEYS
        .get(view.selected)
        .map_or("the city archive, the exchange rates, the live values (aequalis --sync)".to_string(), |k| format!("{} (default: {})", k.about, k.default));
    canvas.put_within(sep + 1, left + 2, right, &about, Style::Dim);
    if let Some(msg) = &view.message {
        canvas.put_within(sep + 2, left + 2, right, msg, Style::Green);
    }
}

/// `text` cut into runs of characters that `is` holds for and runs of those it does not, each with
/// which kind it is.
fn runs(text: &str, is: fn(char) -> bool) -> Vec<(bool, &str)> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut kind = None;
    for (i, c) in text.char_indices() {
        match kind {
            Some(k) if k != is(c) => {
                out.push((k, &text[start..i]));
                start = i;
                kind = Some(is(c));
            }
            None => kind = Some(is(c)),
            _ => {}
        }
    }
    if let Some(k) = kind {
        out.push((k, &text[start..]));
    }
    out
}

/// `A`: the logo, the name and tagline, the facts (`about.rs`), centred.
fn about(canvas: &mut Canvas, rows: usize) {
    let facts = aequalis::about::facts(&aequalis::about::memory());
    let title = format!("{} — {}", aequalis::about::NAME, aequalis::about::TAGLINE);
    let mut lines: Vec<(String, Style)> = aequalis::about::ART.lines().map(|l| (l.to_string(), Style::Plain)).collect();
    let logo_rows = lines.len();
    lines.push((String::new(), Style::Plain));
    lines.push((title.clone(), Style::Bold));
    lines.push((String::new(), Style::Plain));
    let width = lines.iter().map(|(l, _)| l.chars().count()).chain(facts.iter().map(|(l, v)| l.chars().count() + v.chars().count() + 2)).max().unwrap_or(0) + 4;
    let height = lines.len() + facts.len() + 2;
    let (top, left, width, _) = centred(canvas, rows, width, height, "About");
    let right = left + width - 2;
    for (i, (line, style)) in lines.iter().enumerate() {
        if i < logo_rows {
            // The logo: the q as drawn, its arrows in yellow.
            let mut col = left + 2;
            for (arrow, run) in runs(line, aequalis::about::is_arrow) {
                col = canvas.put_within(top + 1 + i, col, right, run, if arrow { Style::Yellow } else { *style });
            }
        } else {
            canvas.put_within(top + 1 + i, left + 2, right, line, *style);
        }
    }
    for (i, (label, value)) in facts.iter().enumerate() {
        let row = top + 1 + lines.len() + i;
        let col = canvas.put_within(row, left + 2, right, &format!("{label}: "), Style::Bold);
        canvas.put_within(row, col, right, value, Style::Plain);
    }
}

/// Section headings have an empty description.
const HELP: &[(&str, &str)] = &[
    ("Input mode", ""),
    ("  type  ←/→  Backspace", "edit the expression"),
    ("  Tab / Shift-Tab", "complete the unit (cycles)"),
    ("  ↑/↓", "highlight an earlier entry (the bar is kept)"),
    ("  Enter  C  Esc", "while one is: edit it, copy its result, stop"),
    ("  Enter", "evaluate"),
    ("  Esc", "command mode"),
    ("  exit + Enter", "quit (quit + Enter works too)"),
    ("  about + Enter", "about: version, memory, last online sync"),
    ("Command mode", ""),
    ("  ←/→  h/l", "highlight a shortcut in the top bar (it scrolls)"),
    ("  Enter", "open the highlighted shortcut"),
    ("  / M U R O", "Search, Measures, Currencies, Trivia, Settings"),
    ("  N", "write `now` (dates: now + 3d, now unix, …)"),
    ("  I  i", "input mode (Enter there evaluates)"),
    ("  j/k  ↑/↓", "browse the history, as in input mode"),
    ("  w/b  0/$  Home/End", "move the cursor"),
    ("  x", "delete the character under the cursor"),
    ("  C", "show / hide the keypad over the history"),
    ("  PgUp/PgDn", "scroll the history"),
    ("  E  ?", "this help"),
    ("  A", "about: version, memory, last online sync"),
    ("  q", "quit"),
    ("Measures", ""),
    ("  ↑/↓  j/k", "choose a class (Length, Temperature, …)"),
    ("  Enter  or its letter", "its units (D: Earth distance, then the places)"),
    ("Currencies", ""),
    ("  type  ↑/↓  Enter", "search by code or name, choose, write the code"),
    ("Unit keys (on the keypad)", ""),
    ("  ←↓↑→  h j k l", "move; circular, wraps around at the edges"),
    ("  Tab / Shift-Tab", "the next / previous tab (Metric / Imperial:"),
    ("", "opens on the device's system; Bits & Bytes,"),
    ("", "Date & Time: the first)"),
    ("  a class's letter", "another class's units (L: Length, K: …)"),
    ("  Enter", "write the unit and a space; stay for the next,"),
    ("", "or, with the target set, back to input mode"),
    ("", "replaces the unit under the cursor, else fills"),
    ("", "the source, then the target"),
    ("  Esc", "back to the Measures, without choosing"),
    ("Help", ""),
    ("  j/k  ↑/↓  PgUp/PgDn", "scroll"),
    ("Anywhere", ""),
    ("  Ctrl-C  Ctrl-L", "quit, redraw"),
];

/// The help window; scrolls (j/k) when the terminal is shorter than the help.
fn help(canvas: &mut Canvas, rows: usize, scroll: usize) {
    let (top, left, width, height) = centred(canvas, rows, 80, HELP.len() + 2, "Help · Esc close");
    let right = left + width - 2;
    let visible = height - 2;
    let scroll = scroll.min(HELP.len() - visible);
    for (i, (keys, what)) in HELP.iter().skip(scroll).take(visible).enumerate() {
        let row = top + 1 + i;
        let style = if what.is_empty() { Style::Bold } else { Style::Plain };
        canvas.put_within(row, left + 2, right, keys, style);
        canvas.put_within(row, left + 26, right, what, Style::Dim);
    }
    let bottom = top + height - 1;
    // aequalis's memory, on the bottom border, always shown: now (where known) and the peak.
    let memory = match aequalis::json::current_kb() {
        Some(now) => format!(" Memory: {} now · peak {} ", aequalis::json::size(now), aequalis::json::size(aequalis::json::peak_kb())),
        None => format!(" Memory: peak {} ", aequalis::json::size(aequalis::json::peak_kb())),
    };
    canvas.put_within(bottom, left + 2, right.saturating_sub(9), &memory, Style::Dim);
    if scroll > 0 {
        canvas.put_within(top, right - 7, right, " ↑ more ", Style::Dim);
    }
    if scroll + visible < HELP.len() {
        canvas.put_within(bottom, right - 7, right, " ↓ more ", Style::Dim);
    }
}

#[cfg(test)]
mod tests {
    use super::super::keys::{self, Key};
    use super::*;
    use aequalis::config::Config;
    use aequalis::Options;

    fn plain(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                chars.by_ref().find(|c| c.is_ascii_alphabetic());
            } else {
                out.push(c);
            }
        }
        out.trim_end().to_string()
    }

    /// Types `keys` (`\n` = Enter, `\x1b` = Esc) and renders.
    const OPTS: Options<'static> = Options { positional: Vec::new(), precision: 4, silent: false, verbose: false };

    fn screen(keys: &str, rows: usize, cols: usize) -> (Vec<String>, Option<(usize, usize, CursorShape)>) {
        let config = Config::default();
        // Verbose: the status line shows the notes.
        let opts = Options { positional: Vec::new(), precision: 4, silent: false, verbose: true };
        let mut app = App::new(&config, &opts);
        for c in keys.chars() {
            app.handle(match c {
                '\n' => Key::Enter,
                '\x1b' => Key::Esc,
                '\t' => Key::Tab,
                _ => Key::Char(c),
            });
        }
        let frame = render(&app, rows, cols);
        (frame.lines.iter().map(|l| plain(l)).collect(), frame.cursor)
    }

    #[test]
    fn sections() {
        let (lines, cursor) = screen("10 km mi\n(100+20)/120\n100 + 20 sqft m", 14, 100);
        assert_eq!(lines.len(), 14);
        // Input mode: the units are hidden, the bar explains the modes.
        assert!(lines[0].starts_with("┌─ Input mode ─"));
        assert!(lines[1].starts_with("│ Press <Esc> for command mode, and I to be back to input mode"), "{}", lines[1]);
        assert!(!lines[1].contains("Length"));
        assert!(lines[3].starts_with("┌─ History ─"));
        assert_eq!(&lines[4..6], ["│".to_string() + &" ".repeat(98) + "│", "│".to_string() + &" ".repeat(98) + "│"]);
        assert!(lines[6].starts_with("│   10 km mi      = 6.2137 mi"), "{}", lines[6]);
        assert!(lines[7].starts_with("│   (100+20)/120  = 1"));
        assert!(lines[8].starts_with("└"));
        assert!(lines[9].starts_with("┌─ Expression ─"));
        assert!(lines[10].starts_with("│ > 100 + 20 sqft m "));
        assert!(lines[11].contains("Tab: mm2 m2 mi2"), "{}", lines[11]);
        assert!(lines[13].starts_with(" INPUT  note: evaluated as (100 + 20) / 120"), "{}", lines[13]);
        assert!(lines[13].ends_with("<Esc> command mode · exit quit"));
        assert_eq!(cursor, Some((10, 19, CursorShape::Bar)));
    }

    #[test]
    fn command_mode_status_and_cursor() {
        let (lines, cursor) = screen("10 km\x1b", 12, 80);
        assert!(lines[11].starts_with(" COMMAND "));
        // 80 columns: the menu keys go, the top bar shows the menu.
        assert!(lines[11].ends_with("<I> input · <C> keypad · <E> help · <A> about · <q> quit"), "{}", lines[11]);
        let (wide, _) = screen("10 km\x1b", 12, 100);
        assert!(wide[11].ends_with("←→ Enter menu · <I> input · <C> keypad · <E> help · <A> about · <q> quit"), "{}", wide[11]);
        // Command mode: the units, and how to go back to input mode.
        assert!(lines[0].starts_with("┌─ Units ─"));
        // No hints on the border: the status line has them.
        assert!(lines[0].ends_with("──┐") && !lines[0].contains("input mode"), "{}", lines[0]);
        // In the menu's order, on one line; a space between the items.
        assert!(lines[1].starts_with("│  /  Search   M  Measures   U  Currencies   R  Trivia   O  ⚙ Settings "), "{}", lines[1]);
        assert!(lines[2].starts_with("└─"), "{}", lines[2]);
        // Moving the highlight scrolls the bar: at 40 columns, → × 4 shows the Settings.
        let (config, opts) = (Config::default(), OPTS);
        let mut app = App::new(&config, &opts);
        app.handle(Key::Esc);
        for _ in 0..4 {
            app.handle(Key::Right);
        }
        let narrow: Vec<String> = render(&app, 12, 40).lines.iter().map(|l| plain(l)).collect();
        assert!(narrow[1].starts_with("│ ‹  R  Trivia   O  ⚙ Settings "), "{}", narrow[1]);
        // In a narrow terminal, still one line: the shortcuts that fit, and › for the hidden ones.
        let (narrow, _) = screen("10 km\x1b", 12, 40);
        assert!(narrow[1].starts_with("│  /  Search   M  Measures  ›"), "{}", narrow[1]);
        assert!(narrow[2].starts_with("└─") && narrow[3].starts_with("┌─ History"), "{}", narrow.join("\n"));
        // With a hidden shortcut's window open, it scrolls to show it; ‹ for the ones before.
        let (narrow, _) = screen("10 km\x1bR", 24, 40);
        assert!(narrow[1].starts_with("│ ‹  U  Currencies   R  Trivia  ›"), "{}", narrow[1]);
        let (narrow, _) = screen("10 km\x1bR\x1b/", 24, 40);
        assert!(narrow[1].starts_with("│  /  Search   M  Measures  ›"), "back to the start: {}", narrow[1]);
        assert_eq!(cursor.unwrap().2, CursorShape::Block);
        // No Tab suggestions in command mode.
        assert!(!lines.join("\n").contains("Tab:"));
    }

    #[test]
    fn unit_keys_replace_the_digits() {
        // Esc M B: the keypad shows the bits and bytes, top-right in the history: the Decimal tab
        // first; Tab switches to Binary. No search row.
        let (lines, _) = screen("10\x1bMB", 24, 80);
        let text = lines.join("\n");
        assert!(!text.contains("│ 7 │"), "digits replaced");
        assert!(lines[4].contains("┌─ Bits & Bytes units ─") && lines[4].ends_with("┐ │"), "{}", lines[4]);
        assert!(lines[5].contains("│  Decimal   Binary "), "{}", lines[5]);
        assert!(lines[6].contains("├──"), "{}", lines[6]);
        assert!(lines[7].contains("│  B  │ kB  │ MB  │ GB  │ TB  "), "{}", lines[7]);
        assert!(lines[9].contains("└──") && !lines[9].contains("Enter"), "no keys on the border: {}", lines[9]);
        assert!(!text.contains(" KiB "), "binary units on the other tab");
        assert!(!lines[4..10].join("\n").contains("/ "), "no search row");
        assert!(!text.contains("Type an expression"), "hint hidden while picking");
        // The navigation keys are on the status line.
        assert!(lines[23].starts_with(" COMMAND "));
        assert!(lines[23].ends_with("←↓↑→ h j k l move · Tab system · Enter write · <Esc> measures"), "{}", lines[23]);
        let (lines, _) = screen("10\x1bMB\t", 24, 80);
        assert!(lines[7].contains("│  B  │ KiB │ MiB │ GiB │ TiB "), "Tab: {}", lines[7]);
        // Shown even when the digit keypad is hidden.
        let (lines, _) = screen("\x1bCMB", 24, 80);
        assert!(lines.join("\n").contains("Bits & Bytes units"));
        // Another class's letter: its units.
        let (lines, _) = screen("\x1bMBT", 24, 80);
        assert!(lines.join("\n").contains("Date & Time units"));
    }

    #[test]
    fn trivia_window() {
        let (lines, _) = screen("\x1bR", 24, 80);
        assert!(lines[1].contains("  R  Trivia "), "{}", lines[1]);
        assert!(lines[4].contains("┌─ Trivia ─"), "{}", lines[4]);
        assert!(lines[5].contains("│  Earth   Space   Satellites   Math   Physics "), "{}", lines[5]);
        // Name, value, short description; the first is a tip.
        assert!(lines[7].contains("│ Distance between places  London to Paris km  Just type the cities"), "{}", lines[7]);
        assert!(lines[8].contains("│ Equator                        40075.017 km  Earth's circumference"), "{}", lines[8]);
        assert!(lines[13].contains("Just type the city from and to: London [to] Paris [mi/km/…]."), "the description: {}", lines[13]);
        // Space: km, and the astronomical unit in brackets.
        let (lines, _) = screen("\x1bR\t", 24, 80);
        assert!(lines[7].contains("│ Voyager 1 ") && lines[8].contains("│ Voyager 2 "), "the Voyagers first: {}", lines[7]);
        // A planet by its name, its distance from the Earth now (changing every day: its shape).
        assert!(lines[10].contains("│ Mercury ") && lines[10].contains(" au)  Distance from the Earth"), "{}", lines[10]);
        assert!(lines[23].starts_with(" TRIVIA ") && lines[23].ends_with("Enter details · <Esc> back"), "{}", lines[23]);
        // Space: the Voyagers first, estimated (no retrieval in the tests), with their launch.
        let (lines, _) = screen("\x1bR\t", 24, 80);
        let text = lines.join("\n");
        assert!(text.contains("│ Voyager 1 ") && text.contains("Launched 1977-09-05 12:56 UTC · estimated"), "{text}");
        // Satellites: counts, as of their date offline.
        let (lines, _) = screen("\x1bR\t\t", 24, 80);
        let text = lines.join("\n");
        assert!(text.contains("│ Active satellites ") && text.contains("16701") && text.contains("as of 2026-10-11"), "{text}");
        // Math: pi with 10 decimals.
        let (lines, _) = screen("\x1bR\t\t\t", 24, 80);
        assert!(lines.join("\n").contains("3.1415926536"));
    }

    #[test]
    fn trivia_popup_window() {
        // Enter: the popup, with the description and the details; Esc back to the list.
        let (lines, cursor) = screen("\x1bR\t\t\t\n", 30, 80);
        let text = lines.join("\n");
        assert!(cursor.is_none());
        assert!(text.contains("┌─ π (pi) ─") && text.contains("Mathematician") && text.contains("Archimedes"), "{text}");
        assert!(text.contains("Definition") && text.contains("π = C / d"), "{text}");
        assert!(lines[29].ends_with("Enter write the value · <Esc> back"), "{}", lines[29]);
        // A planet: its distances from the Sun and the Earth, with the next dates.
        let (lines, _) = screen("\x1bR\tjjjjjj\n", 30, 80);
        let text = lines.join("\n");
        assert!(text.contains("Wikipedia: https://en.wikipedia.org/wiki/Mars"), "{text}");
        assert!(text.contains("Mean distance from the Sun: 227943822 km - 1.5237 au"), "{text}");
        assert!(text.contains("Closest to the Sun (perihelion): ") && text.contains(", next on "), "{text}");
        assert!(text.contains("Closest to the Earth: "), "{text}");
        // Voyager 1, as in the request: description, link, current distance, launch.
        let (lines, _) = screen("\x1bR\t\n", 30, 80);
        let text = lines.join("\n");
        assert!(text.contains("┌─ Voyager 1 ─") && text.contains("Space probe launched by NASA"), "{text}");
        assert!(text.contains("Wikipedia: https://en.wikipedia.org/wiki/Voyager_1"), "{text}");
        // The estimate grows with time: the format, not today's figure.
        assert!(text.contains("Current distance: ") && text.contains(" km - 1") && text.contains(" au - ") && text.contains("e-3 ly"), "{text}");
        assert!(text.contains("(estimated,"), "{text}");
        assert!(text.contains("Launched: 1977-09-05 12:56 UTC"), "{text}");
        let (lines, _) = screen("\x1bR\t\t\t\n\x1b", 30, 80);
        assert!(!lines.join("\n").contains("Mathematician"), "Esc: back to the list");
    }

    #[test]
    fn search_window() {
        let (lines, cursor) = screen("\x1b/kilo", 24, 80);
        assert!(cursor.is_none(), "the search has its own caret");
        let text = lines.join("\n");
        assert!(text.contains("┌─ Search units ─") && text.contains("│ / kilo▏"), "{text}");
        assert!(text.contains("│ km       kilometre · Length "), "{text}");
        assert!(lines[23].starts_with(" SEARCH "), "{}", lines[23]);
        let (lines, _) = screen("\x1b/zzz", 24, 80);
        assert!(lines.join("\n").contains("no unit matches"));
    }

    #[test]
    fn measures_window() {
        let (lines, cursor) = screen("\x1bM", 24, 80);
        let text = lines.join("\n");
        assert!(lines[1].contains("  M  Measures "), "lit in the top bar: {}", lines[1]);
        assert!(text.contains("┌─ Measures ─"), "{text}");
        // Each class with its letter and its units; the Earth distance after Length.
        assert!(text.contains(" T  Date & Time   ") && text.contains(" B  Bits & Bytes  "), "{text}");
        assert!(text.contains(" L  Length ") && text.contains(" mm cm m km "), "{text}");
        // Whole units, then `…` when they don't fit.
        assert!(text.contains(" B kB MB GB TB KiB MiB GiB TiB b kb …  │"), "{text}");
        let length = lines.iter().position(|l| l.contains(" L  Length ")).unwrap();
        assert!(lines[length + 1].contains(" D  Earth distance ") && lines[length + 1].contains("Rome to Paris"), "{text}");
        assert!(text.contains(" K  Temperature ") && !text.contains(" W  Power "), "{text}");
        // ↑ from the first: the last, the list scrolled to it.
        let (lines, _) = screen("\x1bMk", 24, 80);
        assert!(lines.join("\n").contains(" W  Power "), "{}", lines.join("\n"));
        assert!(lines[23].starts_with(" MEASURES ") && lines[23].ends_with("<Esc> back"), "{}", lines[23]);
        assert!(cursor.is_some());
        // Every row inside 40 columns.
        let (lines, _) = screen("\x1bMjj", 12, 40);
        assert!(lines.iter().all(|l| l.chars().count() <= 40));
        assert!(lines.join("\n").contains(" L  Length"), "scrolled to the row chosen: {}", lines.join("\n"));
    }

    #[test]
    fn currencies_window() {
        let (lines, cursor) = screen("\x1bUyen", 24, 80);
        assert!(cursor.is_none(), "the search has its own caret");
        let text = lines.join("\n");
        assert!(lines[1].contains("  U  Currencies "), "{}", lines[1]);
        assert!(text.contains("┌─ Currencies ─") && text.contains("│ / yen▏"), "{text}");
        assert!(text.contains("│ JPY  ¥    Japanese yen "), "{text}");
        assert!(lines[23].starts_with(" CURRENCIES "), "{}", lines[23]);
        let (lines, _) = screen("\x1bUzzzz", 24, 80);
        assert!(lines.join("\n").contains("no currency matches"));
    }

    #[test]
    fn card_under_the_selected_unit() {
        // Length, Imperial: in selected first; → → → mi.
        let config = Config::default();
        let opts = Options { positional: Vec::new(), precision: 4, silent: false, verbose: false };
        let mut app = App::new(&config, &opts);
        for key in [Key::Esc, Key::Char('M'), Key::Char('L'), Key::Tab, Key::Right, Key::Right, Key::Right] {
            app.handle(key);
        }
        let lines: Vec<String> = render(&app, 24, 80).lines.iter().map(|l| plain(l)).collect();
        let button = lines.iter().position(|l| l.contains("│ in │ ft │ yd │ mi ")).unwrap();
        let km = lines[button].chars().collect::<Vec<_>>().windows(4).position(|w| w == [' ', 'm', 'i', ' ']).unwrap();
        // The window's bottom border follows (sized for the metric tab's two rows); the card
        // starts under it, under mi, pointing up at it.
        let button = (button + 1..).find(|&i| lines[i].contains("└─")).unwrap() - 2;
        assert!(lines[button + 2].contains("└─"));
        let card_top: Vec<char> = lines[button + 3].chars().collect();
        // It starts under mi, or further left when it would not fit; ▲ is always under mi.
        assert!(card_top[..=km].contains(&'┌'), "{}", lines[button + 3]);
        assert_eq!(card_top[km + 2], '▲', "{}", lines[button + 3]);
        assert!(lines[button + 4].contains("│ mile "));
        assert!(lines[button + 5].contains("│ aliases: mile, miles "));
        assert!(lines[button + 6].contains("│ 1 mi = 1609.344 m "));
        assert!(lines[button + 7].contains('└'));
        // The history's right border is not covered.
        assert!(lines[button + 3..button + 8].iter().all(|l| l.ends_with("│") || l.ends_with("┐") || l.ends_with("┘")));
    }

    #[test]
    fn card_contents() {
        // Base unit, no aliases, data, temperature, imperial.
        let config = Config::default();
        let opts = Options { positional: Vec::new(), precision: 4, silent: false, verbose: false };
        let card = |category: &str, symbol: &str| {
            let app = App::new(&config, &opts);
            let cat = config.categories.iter().find(|c| c.name == category).unwrap();
            let unit = cat.units.iter().find(|u| u.symbol == symbol).unwrap();
            unit_card_lines(&app, cat, unit)
        };
        assert_eq!(card("Length", "m"), ["metre", "aliases: meter, meters, metre, metres", "base unit of Length"]);
        assert_eq!(card(aequalis::config::BITS, "MB"), ["megabyte (10⁶ bytes)", "aliases: none", "1 MB = 8000000 b"]);
        assert_eq!(card("Temperature", "C"), ["degree Celsius", "aliases: °C, celsius", "0 C = 273.15 K"]);
        assert_eq!(card("Volume", "floz-uk")[1..], ["aliases: none".to_string(), "1 floz-uk = 0.0284130625 l".to_string()]);
        assert_eq!(card("Mass", "lb")[2], "1 lb = 0.45359237 kg");
        // Too small for 10 decimals: scientific notation.
        assert_eq!(card("Energy", "eV")[2], "1 eV = 1.602176634e-19 J");
    }

    #[test]
    fn card_stays_on_screen() {
        // The last unit of the row is near the right edge: the card moves left. In a short terminal
        // it goes above the button.
        for (rows, cols) in [(24, 80), (12, 40), (14, 60)] {
            for keys in ["\x1bML\x1b[C", "\x1bMV\t\x1b[B", "\x1bMB\t"] {
                let config = Config::default();
                let opts = Options { positional: Vec::new(), precision: 4, silent: false, verbose: false };
                let mut app = App::new(&config, &opts);
                for k in keys::decode(keys.as_bytes()).0 {
                    app.handle(k);
                }
                for _ in 0..4 {
                    app.handle(Key::Right);
                }
                let lines: Vec<String> = render(&app, rows, cols).lines.iter().map(|l| plain(l)).collect();
                assert_eq!(lines.len(), rows);
                assert!(lines.iter().all(|l| l.chars().count() <= cols), "{keys:?} {rows}x{cols}");
                assert!(lines.join("\n").contains("aliases:"), "{keys:?} {rows}x{cols}: card shown");
            }
        }
    }

    #[test]
    fn area_and_data_have_five_columns() {
        let (lines, _) = screen("\x1bMB", 24, 80);
        // Bytes on the first row, bits below.
        let bytes = lines.iter().position(|l| l.contains("│  B  │ kB  │ MB  │ GB  │ TB  ")).expect("bytes row");
        assert!(lines[bytes + 1].contains("│  b  │ kb  │ Mb  │ Gb  │ Tb  "), "{}", lines[bytes + 1]);
        assert!(lines[bytes - 1].starts_with("│") && lines[bytes - 1].contains("├─"), "first row of buttons");
        let (lines, _) = screen("\x1bMB\t", 24, 80);
        let bytes = lines.iter().position(|l| l.contains("│  B  │ KiB │ MiB │ GiB │ TiB ")).expect("bytes row");
        assert!(lines[bytes + 1].contains("│  b  │ Kib │ Mib │ Gib │ Tib "));
    }

    #[test]
    fn time_window_durations_and_dates() {
        // Durations: every time unit on the first tab.
        let (lines, _) = screen("\x1bMT", 24, 80);
        let text = lines.join("\n");
        assert!(lines[4].contains("─ Date & Time units ─"), "{}", lines[4]);
        assert!(text.contains(" Durations   Formats "));
        assert!(text.contains("│ ns  │ us  │ ms  │  s  │ min ") && text.contains("│  h  │  d  │ wk  │ mo  │ yr "), "{text}");
        assert!(text.contains("nanosecond"), "the card, inside the window: {text}");
        // Formats: the table of formats, and the footer for the selected one (25 rows: with the
        // two-line units bar, 24 rows scroll the table).
        let (lines, _) = screen("\x1bMT\t", 25, 80);
        let text = lines.join("\n");
        assert!(lines[4].contains("─ Date & Time units ─"), "the title is not covered: {}", lines[4]);
        for row in [
            "ISO           iso   │   utc   │  basic  │  date   │  time",
            "Unix/local     s    │   ms    │   µs    │   ns    │  local",
            "YYYY/MM/DD   date   │   24h   │ 24h tz  │   12h   │ 12h tz",
            "MM/DD/YY     date   │   24h   │ 24h tz  │   12h   │ 12h tz",
        ] {
            assert!(text.contains(row), "{row}\n{text}");
        }
        // The footer: the selected format and now in it; the window stays inside the history.
        assert!(text.contains("│ iso · 20"), "{text}");
        assert!(lines[18].starts_with("│ ") && lines[19].starts_with("└"), "inside the history:\n{text}");
        // l l: basic.
        let (lines, _) = screen("\x1bMT\tll", 24, 80);
        assert!(lines.join("\n").contains("iso-basic · 20"));
        // A pattern: its name and now in it, e.g. `DD/MM/YYYY-12h-tz · 10/10/2026 06:45:54 PM +02:00`.
        let (lines, _) = screen("\x1bMT\tjjjjjjllll", 24, 80);
        assert!(lines.join("\n").contains("DD/MM/YYYY-12h-tz · "), "{}", lines.join("\n"));
    }

    #[test]
    fn data_tabs_are_decimal_and_binary() {
        let (lines, _) = screen("\x1bMB", 24, 80);
        let text = lines.join("\n");
        assert!(text.contains(" Decimal   Binary "));
        assert!(text.contains(" kB ") && !text.contains("KiB"));
        let (lines, _) = screen("\x1bMB\t", 24, 80);
        let text = lines.join("\n");
        assert!(text.contains(" KiB ") && !text.contains(" kB "));
    }

    #[test]
    fn unit_keys_fit_every_category() {
        let config = Config::default();
        for category in &config.categories {
            let key = crate::tui::measures::key(category);
            for (rows, cols) in [(12, 40), (24, 80)] {
                // Both tabs.
                let screens: Vec<Vec<String>> = (0..2).map(|t| screen(&format!("\x1bM{key}{}", "\t".repeat(t)), rows, cols).0).collect();
                let first = &screens[0];
                assert!(first.join("\n").contains(&format!("{} units", category.name)) || cols < 50, "{} {rows}x{cols}", category.name);
                assert!(screens.iter().flatten().all(|l| l.chars().count() <= cols));
                // At 80 columns every unit is on one of the tabs, and visible there.
                if cols == 80 {
                    assert!(!first.join("\n").contains("Time units") || first[4].contains("Time units"), "the title is not covered");
                    let all = screens.iter().map(|s| s.join("\n")).collect::<Vec<_>>().join("\n");
                    for u in &category.units {
                        assert!(all.contains(&format!(" {} ", u.symbol)), "{} missing", u.symbol);
                    }
                }
            }
        }
    }

    #[test]
    fn help_window() {
        let (lines, cursor) = screen("\x1bE", 48, 90);
        let text = lines.join("\n");
        assert!(text.contains("Help · Esc close"));
        assert!(text.contains("/ M U R O") && text.contains("this help") && text.contains("Ctrl-C"));
        assert!(text.contains("exit + Enter"));
        assert!(!text.contains("more"), "everything fits");
        assert!(cursor.is_none());
    }

    #[test]
    fn online_indicator() {
        use aequalis::network::Status;
        use aequalis::online::Data;
        let config = Config::default();
        let opts = Options { positional: Vec::new(), precision: 4, silent: false, verbose: false };
        // The status comes from the last sync (no probing); forced by the offline setting.
        let top = |status: Status, cols: usize, command: bool| {
            let mut app = App::new(&config, &opts);
            app.forced_offline = status == Status::Forced;
            app.online = match status {
                Status::Checking | Status::Forced => Data::default(),
                Status::Online => Data { attempted_at: Some(1), ok: true, ..Data::default() },
                Status::Offline => Data { attempted_at: Some(1), ok: false, ..Data::default() },
            };
            if command {
                app.handle(Key::Esc);
            }
            plain(&render(&app, 12, cols).lines[0])
        };
        // Top right, in both modes; "not synced" before the first sync.
        assert!(top(Status::Checking, 80, false).ends_with(" ○ not synced ──┐"), "{}", top(Status::Checking, 80, false));
        assert!(top(Status::Online, 80, false).ends_with(" ● online ──┐"));
        assert!(top(Status::Online, 80, true).starts_with("┌─ Units ─") && top(Status::Online, 80, true).ends_with(" ● online ──┐"));
        assert!(top(Status::Offline, 80, true).ends_with(" ● offline ──┐"));
        assert!(top(Status::Forced, 80, false).ends_with(" ● offline (forced) ──┐"));
        // Narrow: the short label.
        assert!(top(Status::Forced, 40, false).ends_with(" ● offline ──┐"), "{}", top(Status::Forced, 40, false));
    }

    #[test]
    fn about_window() {
        let (lines, cursor) = screen("\x1bA", 24, 80);
        let text = lines.join("\n");
        assert!(cursor.is_none());
        assert!(text.contains("┌─ About ─") && text.contains("█▛ ----> ███▌") && text.contains("▐█▖<----▗███▌"), "{text}");
        assert!(text.contains("Aequalis — Ultralight calculator and converter made for humans"), "{text}");
        assert!(text.contains("Memory: ") && text.contains("Last online sync: ") && text.contains("Version: "), "{text}");
        assert!(lines[23].starts_with(" ABOUT "), "{}", lines[23]);
        let (lines, _) = screen("\x1bA\x1b", 24, 80);
        assert!(!lines.join("\n").contains("┌─ About ─"), "Esc closes it");
    }

    #[test]
    fn help_shows_memory() {
        // Short or tall, scrolled or not: the memory is on the bottom border.
        for rows in [14, 48] {
            let (lines, _) = screen("\x1bE", rows, 90);
            let text = lines.join("\n");
            assert!(text.contains(" Memory: ") && text.contains(" peak "), "{text}");
            if cfg!(target_os = "linux") {
                assert!(text.contains(" now · peak "), "{text}");
            }
        }
    }

    #[test]
    fn help_scrolls_when_short() {
        let (lines, _) = screen("\x1bE", 14, 80);
        let text = lines.join("\n");
        assert!(text.contains("Input mode") && text.contains("↓ more") && !text.contains("Ctrl-C"));
        // Scrolled past the end: stops at the last line.
        let (lines, _) = screen(&format!("\x1bE{}", "j".repeat(50)), 14, 80);
        let text = lines.join("\n");
        assert!(text.contains("Ctrl-C") && text.contains("↑ more") && !text.contains("↓ more"));
    }

    #[test]
    fn help_fits_80_columns() {
        let (lines, _) = screen("\x1bE", 48, 80);
        for (keys, what) in HELP {
            let text = keys.trim().to_string();
            assert!(text.is_empty() || lines.iter().any(|l| l.contains(&text)), "{text}");
            assert!(what.is_empty() || lines.iter().any(|l| l.contains(what)), "cut: {what}");
        }
    }

    #[test]
    fn preview_shows_the_grouping() {
        let (lines, _) = screen("12+12/3", 12, 80);
        assert!(lines[9].starts_with("│   (12 + (12 / 3)) = 16"), "{}", lines[9]);
        let (lines, _) = screen("100 + 20 + 50 / 10 sqft m2", 12, 80);
        assert!(lines[9].starts_with("│   ((100 + 20) + (50 / 10)) = 11.6129 m2"), "{}", lines[9]);
        // A plain conversion: just the result.
        let (lines, _) = screen("10 km mi", 12, 80);
        assert!(lines[9].starts_with("│   = 6.2137 mi"), "{}", lines[9]);
        // Only the source unit so far: what comes next follows the grouping, once the unit is typed
        // (a space after it: `km` may still become `km/h`, and Tab offers those meanwhile).
        let (lines, _) = screen("1+2 km", 12, 100);
        assert!(lines[9].starts_with("│   (1 + 2)    Tab: km · km2 · km/h"), "{}", lines[9]);
        let (lines, _) = screen("1+2 km ", 12, 100);
        assert!(lines[9].starts_with("│   (1 + 2) · now the length unit to convert to: in, ft, yd…"), "{}", lines[9]);
        // A real error follows the grouping too, once the word is typed (a space after it).
        let (lines, _) = screen("1+2 km mi m", 12, 100);
        assert!(lines[9].starts_with("│   (1 + 2)") && !lines[9].contains("expected"), "{}", lines[9]);
        let (lines, _) = screen("1+2 km mi m ", 12, 100);
        assert!(lines[9].starts_with("│   (1 + 2) · expected"), "{}", lines[9]);
    }

    #[test]
    fn keypad_in_front_of_the_history() {
        // Full size, top-right: 24 rows leave 14 history rows.
        let (lines, _) = screen("", 24, 80);
        assert!(lines[4].starts_with("│ Type an expression"), "{}", lines[4]);
        assert!(lines[4].ends_with("┌───┬───┬───┬───┐ │"), "{}", lines[4]);
        assert!(lines[5].ends_with("│ ( │ ) │ ^ │ / │ │"));
        assert!(lines[6].ends_with("├───┼───┼───┼───┤ │"));
        assert!(lines[7].ends_with("│ 7 │ 8 │ 9 │ * │ │"));
        assert!(lines[13].ends_with("│ 0 │ . │ e │ = │ │"));
        assert!(lines[14].ends_with("└───┴───┴───┴───┘ │"));
        // Compact on a short terminal.
        let (lines, _) = screen("", 17, 80);
        let compact =
            ["┌─────────────┐ │", "│ (  )  ^  /  │ │", "│ 7  8  9  *  │ │", "│ 4  5  6  -  │ │", "│ 1  2  3  +  │ │", "│ 0  .  e  =  │ │", "└─────────────┘ │"];
        for (line, want) in lines[4..11].iter().zip(compact) {
            assert!(line.ends_with(want), "{line}");
        }
        // It covers the history lines under it; the newest lines (bottom) stay visible.
        let wide: String = (1..=20).map(|i| format!("{}+{i}\n", "1".repeat(70))).collect();
        let (lines, _) = screen(&wide, 24, 80);
        assert!(lines[5].ends_with("│ ( │ ) │ ^ │ / │ │"));
        assert!(lines[17].contains("+20"), "{}", lines[17]);
        // C hides it and the lines show.
        let (lines, _) = screen(&format!("{wide}\x1bC"), 24, 80);
        assert!(!lines[5].contains("│ ( │"), "{}", lines[5]);
        // Too short for even the compact one: not drawn.
        let (lines, _) = screen("", 16, 80);
        assert!(!lines.join("").contains("┌───"));
    }

    /// How long a keystroke takes in the interactive mode (the key, then the whole screen drawn
    /// again, the preview evaluated): `cargo test --release -- --ignored keystroke_cost --nocapture`.
    /// Each request is typed once first (the tables built once per process are not a keystroke's
    /// cost), then the best of 5 runs is kept (the others met the machine's other work).
    #[test]
    #[ignore = "a measure, not a check"]
    fn keystroke_cost() {
        let config = Config::default();
        let opts = Options { positional: Vec::new(), precision: 4, silent: false, verbose: false };
        let typed = |input: &str| {
            let mut app = App::new(&config, &opts);
            let started = std::time::Instant::now();
            for c in input.chars() {
                app.handle(Key::Char(c));
                std::hint::black_box(render(&app, 40, 120));
            }
            started.elapsed() / input.chars().count() as u32
        };
        let inputs = ["10 km mi", "12+4/2*sqrt 16", "Rome Paris", "10:00am jappan romme", "12 USA UAE", "now Tokyo"];
        for input in inputs {
            typed(input);
        }
        for input in inputs {
            let each = (0..5).map(|_| typed(input)).min().unwrap();
            println!("KEYSTROKE {input:<24} {each:?} per key (handle + render)");
        }
    }

    #[test]
    fn keypad_lights_the_key_typed() {
        let config = Config::default();
        let opts = Options { positional: Vec::new(), precision: 4, silent: false, verbose: false };
        let mut app = App::new(&config, &opts);
        for digit in "0123456789".chars() {
            app.handle(Key::Char(digit));
            let frame = render(&app, 24, 80);
            let lit: Vec<_> = frame.lines.iter().filter(|l| l.contains(&format!("\x1b[1;30;43m {digit} "))).collect();
            assert_eq!(lit.len(), 1, "{digit} lit");
            assert_eq!(frame.lines.iter().map(|l| l.matches("\x1b[1;30;43m").count()).sum::<usize>(), 1, "only {digit} lit");
        }
    }

    #[test]
    fn errors_and_preview() {
        let (lines, _) = screen("10 km kg\n10 km m", 12, 100);
        let text = lines.join("\n");
        assert!(text.contains("10 km kg  ✗ cannot convert `km` to `kg`: different kinds of unit"));
        assert!(lines[9].contains("= 10000 m"), "{}", lines[9]);
        assert!(lines[11].starts_with(" INPUT  error: "));
    }

    #[test]
    fn everything_fits() {
        let long = "1+".repeat(60) + "1";
        for (rows, cols) in [(12, 40), (24, 80), (50, 200)] {
            for keys in [long.as_str(), "10 km mi\n\x1bML", "\x1bMLkilometre", "\x1bE", "1\n2\n3\n4\n5\n6\n7\n8\n9\n"] {
                let (lines, cursor) = screen(keys, rows, cols);
                assert_eq!(lines.len(), rows);
                assert!(lines.iter().all(|l| l.chars().count() <= cols), "{keys:?} at {rows}x{cols}");
                if let Some((r, c, _)) = cursor {
                    assert!(r < rows && c < cols, "{keys:?} at {rows}x{cols}: cursor {r},{c}");
                }
            }
        }
    }

    /// Bug 003: no erase after a full-width line, and every line is exactly the screen width.
    #[test]
    fn frames_fill_the_width_without_erasing() {
        let config = Config::default();
        let opts = Options { positional: Vec::new(), precision: 4, silent: false, verbose: false };
        let app = App::new(&config, &opts);
        for (rows, cols) in [(24, 80), (12, 40), (8, 40)] {
            let frame = render(&app, rows, cols);
            let ansi = frame.to_ansi(true);
            assert!(!ansi.contains("\x1b[K"), "{rows}x{cols}");
            assert!(frame.lines.iter().all(|l| plain(l).chars().count() <= cols));
            if rows >= MIN_ROWS {
                let full = |l: &String| {
                    let mut out = String::new();
                    let mut chars = l.chars();
                    while let Some(c) = chars.next() {
                        if c == '\x1b' {
                            chars.by_ref().find(|c| c.is_ascii_alphabetic());
                        } else {
                            out.push(c);
                        }
                    }
                    out.chars().count()
                };
                assert!(frame.lines.iter().all(|l| full(l) == cols), "{rows}x{cols}: every line is full width");
            }
        }
    }

    #[test]
    fn ambiguity_popup() {
        let (lines, cursor) = screen("12/04/2026 unix\n", 24, 80);
        let text = lines.join("\n");
        assert!(text.contains("`12/04/2026` is ambiguous: which date?"), "{text}");
        assert!(text.contains(" 1  ") && text.contains(" 2  ") && text.contains("DD/MM/YYYY   2026-04-12"), "{text}");
        assert!(lines[23].starts_with(" CHOOSE ") && lines[23].ends_with("Enter confirm · <Esc> back"), "{}", lines[23]);
        assert!(cursor.is_none());
    }

    #[test]
    fn too_small() {
        let (lines, cursor) = screen("", 8, 40);
        assert_eq!(lines[0], "terminal too small (needs 40x12)");
        assert!(cursor.is_none());
    }
}
