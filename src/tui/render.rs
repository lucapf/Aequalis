//! Drawing the interactive screen. Pure: state and size in, frame out. The screen is a grid of
//! styled cells, so the picker and the help can be drawn over the sections.
//!
//! ```text
//!  F2 Measures  F3 Currencies  F4 👕👟 Sizes  F5 History  F6 Trivia  F9 Keys  F10 ⚙  ● online
//! ──────────────────────────────────────────────────────────────────────────────────
//!    10 km mi            = 6.2137 mi
//!    (10+12-3)/4Km Mi    = 2.9515 mi
//! ──────────────────────────────────────────────────────────────────────────────────
//!  > 100 + 20 sqft m
//!    = …   Tab: mm2 m2 mi2
//! ─── Basic │ Sci ────────────────────────────────────────────────────────────────────
//!    sin  cos │   (    )    %    ÷                       the band: the keypad (F9), or the
//!    tan   √  │   7    8    9    ×                       panel of F2…F6 and Ctrl+K,   
//!    ln   log │   4    5    6    −                       which takes its rows
//!     π    e  │   1    2    3    +
//!     ^    °  │   0    .    ⌫    ⏎
//!  note: evaluated as (100 + 20) + (50 / 10)      F1 help · Ctrl+K search · Ctrl+Q quit
//! ```

use super::action::{Action, Target, TOOLBAR};
use super::picker::{format_rows, Picker};
use super::{App, Keypad, Overlay, Part, StatusKind};
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
    /// The badge of a window.
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

pub struct Frame {
    pub lines: Vec<String>,
    /// Row and column of the cursor, from 0.
    pub cursor: Option<(usize, usize)>,
    /// What a click on a cell does, as drawn.
    pub hits: Vec<Hit>,
}

/// A rectangle of the screen and what a click in it asks for: registered by what draws it, so
/// clickable regions cannot drift from what is on screen. The last registered under a cell wins
/// (what is drawn over another covers it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub row: usize,
    pub col: usize,
    pub width: usize,
    pub height: usize,
    pub target: Target,
}

impl Hit {
    pub fn contains(&self, row: usize, col: usize) -> bool {
        (self.row..self.row + self.height).contains(&row) && (self.col..self.col + self.width).contains(&col)
    }
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
        if let Some((row, col)) = self.cursor {
            // A bar, steady.
            s.push_str(&format!("\x1b[{};{}H\x1b[6 q\x1b[?25h", row + 1, col + 1));
        }
        s
    }
}

struct Canvas {
    cols: usize,
    cells: Vec<Vec<(char, Style)>>,
    hits: Vec<Hit>,
}

impl Canvas {
    fn new(rows: usize, cols: usize) -> Self {
        Canvas { cols, cells: vec![vec![(' ', Style::Plain); cols]; rows], hits: Vec::new() }
    }

    /// A click on `width` cells of `row` from `col` asks for `target`.
    fn hit(&mut self, row: usize, col: usize, width: usize, target: Target) {
        self.hit_rect(row, col, width, 1, target);
    }

    fn hit_rect(&mut self, row: usize, col: usize, width: usize, height: usize, target: Target) {
        if width > 0 {
            self.hits.push(Hit { row, col, width, height, target });
        }
    }

    /// Writes `text` from `col`, cut before column `limit`; returns the column after it. A wide
    /// character (👕) takes two columns: the second cell holds `WIDE` and is not written out.
    fn put_within(&mut self, row: usize, col: usize, limit: usize, text: &str, style: Style) -> usize {
        let limit = limit.min(self.cols);
        let mut c = col;
        for ch in text.chars() {
            let w = width(ch);
            if c + w > limit {
                break;
            }
            self.cells[row][c] = (ch, style);
            if w == 2 {
                self.cells[row][c + 1] = (WIDE, style);
            }
            c += w;
        }
        c
    }

    fn put(&mut self, row: usize, col: usize, text: &str, style: Style) -> usize {
        self.put_within(row, col, self.cols, text, style)
    }

    /// A line across the whole width.
    fn rule(&mut self, row: usize) {
        self.cells[row].fill(('─', Style::Dim));
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
                for &(ch, style) in row.iter().filter(|(ch, _)| *ch != WIDE) {
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

/// The second cell of a wide character: the terminal draws the character over both.
const WIDE: char = '\0';

/// The columns a character takes in a terminal: two for emoji (👕 👟) and East Asian wide letters.
fn width(c: char) -> usize {
    match c as u32 {
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F
        | 0x1F900..=0x1F9FF
        | 0x20000..=0x3FFFD => 2,
        _ => 1,
    }
}

/// The columns `text` takes.
fn columns(text: &str) -> usize {
    text.chars().map(width).sum()
}

pub const MIN_ROWS: usize = 12;
pub const MIN_COLS: usize = 40;

/// From this many columns the keyboard and the panels open in a pane at the right of the request
/// and the history (the Wide layout), which stay as they are; below it, in the band under the line.
const WIDE_FROM: usize = 120;

/// The width of the pane at the right in the Wide layout, for a terminal of `rows` and `cols`: 40%
/// of the width, and from 150 columns 60%, so that the board and the keyboard are side by side.
pub(super) fn pane_width(rows: usize, cols: usize) -> Option<usize> {
    if cols < WIDE_FROM || rows < MIN_ROWS + 2 {
        return None;
    }
    Some(if cols >= BOTH_FROM { (cols * 3 / 5).clamp(100, 130) } else { (cols * 2 / 5).clamp(52, 90) })
}

/// From this many columns the pane holds the board and the keyboard side by side.
const BOTH_FROM: usize = 150;

/// The columns of the board, which needs this many for a class's name and five buttons.
const BOARD_WIDTH: usize = 52;

/// In the Wide layout with room for both: the columns of the board and of the keyboard beside it.
pub(super) fn wide_split(rows: usize, cols: usize) -> Option<(usize, usize)> {
    let pane = pane_width(rows, cols).filter(|w| *w >= BOARD_WIDTH + 1 + BIG_BASIC_WIDTH)?;
    Some((BOARD_WIDTH, pane - BOARD_WIDTH - 1))
}

/// The columns of the big keyboard: Basic is 5 keys, Sci 2 more; a key is `BIG_KEY` wide and a
/// column of the keyboard `BIG_KEY + 1` with the gap.
const BIG_KEY: usize = 7;
const BIG_BASIC_WIDTH: usize = 5 * (BIG_KEY + 1);
const BIG_SCI_WIDTH: usize = 2 * (BIG_KEY + 1) + 2;
/// The rows of the big keyboard: its tabs, then five rows of keys three high.
const BIG_ROWS: usize = 1 + 5 * 3;

/// Whether the big keyboard has the room for the Sci keys beside the Basic ones.
pub(super) fn big_sci_fits(columns: usize) -> bool {
    columns >= BIG_SCI_WIDTH + BIG_BASIC_WIDTH
}

/// Whether a panel (Measures, Currencies, …) is open: it takes the pane.
fn panel_open(app: &App) -> bool {
    app.overlay.as_ref().is_some_and(Overlay::is_panel)
}

/// The Basic keys of the big keyboard, five rows of five, and the Sci keys beside them, five of two.
const BIG_BASIC: [[&str; 5]; 5] =
    [["(", ")", "%", "÷", "C"], ["7", "8", "9", "×", "⌫"], ["4", "5", "6", "−", "←"], ["1", "2", "3", "+", "→"], ["0", ".", "to", "␣", "⏎"]];
const BIG_SCI: [[&str; 2]; 5] = [["sin", "cos"], ["tan", "√"], ["ln", "log"], ["π", "e"], ["^", "°"]];

/// The big keyboard, for a finger: keys three rows high, its tabs on the first row. Every key is
/// a button; the one typed last is lit.
fn big_keyboard(app: &App, canvas: &mut Canvas, area: Area) {
    let Area { top, left, .. } = area;
    let sci = app.keypad == Keypad::Sci && big_sci_fits(area.width);
    let tab = |part: Part, shown: bool| match (app.focus == Some(part), shown) {
        (true, _) => Style::Inverse,
        (false, true) => Style::Lit,
        (false, false) => Style::Dim,
    };
    let mut col = canvas.put(top, left + 1, " Basic ", tab(Part::Basic, !sci));
    canvas.hit(top, left + 1, col - left - 1, Target::Focus(Part::Basic));
    col = canvas.put(top, col, "│", Style::Dim);
    let from = col;
    col = canvas.put(top, col, " Sci ", tab(Part::Sci, sci));
    canvas.hit(top, from, col - from, Target::Focus(Part::Sci));
    let key = |canvas: &mut Canvas, row: usize, x: usize, label: &str| {
        let typed = label.chars().next().filter(|_| label.chars().count() == 1).map(|c| match c {
            '÷' => '/',
            '×' => '*',
            '−' => '-',
            '⏎' => '=',
            c => c,
        });
        let lit = typed.is_some_and(|c| Some(c) == app.pressed);
        let style = match typed {
            _ if lit => Style::Lit,
            Some('0'..='9' | '.') => Style::Bold,
            _ => Style::Yellow,
        };
        big_button(canvas, row, x, BIG_KEY, "");
        canvas.put(row + 1, x + 1, &format!("{label:^width$}", width = BIG_KEY - 2), style);
        canvas.hit_rect(row, x, BIG_KEY, 3, Target::Act(key_action(label)));
    };
    for r in 0..5 {
        let row = top + 1 + r * 3;
        let mut x = left + 1;
        if sci {
            for (c, label) in BIG_SCI[r].iter().enumerate() {
                key(canvas, row, x + c * (BIG_KEY + 1), label);
            }
            x += 2 * (BIG_KEY + 1) + 2;
            canvas.cells[row + 1][x - 2] = ('│', Style::Dim);
        }
        for (c, label) in BIG_BASIC[r].iter().enumerate() {
            key(canvas, row, x + c * (BIG_KEY + 1), label);
        }
    }
}

/// The width a clock takes on the board, and the first column of a class's buttons.
const CLOCK_WIDTH: usize = 30;
const CLASS_NAME: usize = 16;

/// A box three rows high at (`y`, `x`) of `width` columns, with `title` on its top border
/// (`┌─USD────┐`); its middle row is for the caller.
fn big_button(canvas: &mut Canvas, y: usize, x: usize, width: usize, title: &str) {
    let inner = width - 2;
    let top = if title.is_empty() {
        format!("┌{}┐", "─".repeat(inner))
    } else {
        format!("┌─{title}{}┐", "─".repeat(inner.saturating_sub(title.chars().count() + 1)))
    };
    canvas.put(y, x, &top, Style::Dim);
    canvas.put(y + 1, x, "│", Style::Dim);
    canvas.put(y + 1, x + width - 1, "│", Style::Dim);
    canvas.put(y + 2, x, &format!("└{}┘", "─".repeat(inner)), Style::Dim);
}

/// The width of a currency tile and the gap after it.
const TILE: usize = 11;

/// The rows an entry of the board's list takes: a class one row (brackets) or three (boxes); the
/// currencies a heading and rows of tiles (three high), or one row.
fn item_rows(item: super::board::Item, tall: bool, width: usize) -> usize {
    match (item, tall) {
        (super::board::Item::Class(_), true) => 3,
        (super::board::Item::Class(_), false) => 1,
        (super::board::Item::Currencies, true) => 1 + 3 * super::board::CURRENCIES.div_ceil(((width.saturating_sub(1)) / (TILE + 1)).max(1)),
        (super::board::Item::Currencies, false) => 1,
    }
}

/// Whether the board is what the pane shows (not a panel, nor the compact keyboard in its place).
pub(super) fn board_shown(app: &App) -> bool {
    let (rows, cols) = app.screen;
    match pane_width(rows, cols) {
        Some(pane_w) => !panel_open(app) && !(app.keyboard_shown() && wide_split(rows, cols).is_none() && pane_w < BOTH_FROM),
        None => false,
    }
}

/// The board: the clocks, then the list of measures that scrolls (the wheel, a drag of the finger):
/// the currencies and every class, the one used most first, each with its most used units, used
/// ones lit. Every button writes what it shows.
fn board_view(app: &App, canvas: &mut Canvas, area: Area) {
    use super::board::Item;
    let Area { top, left, width, height } = area;
    let right = left + width;
    let mut y = top + board_clocks(app, canvas, area);
    // The list: a heading, then the entries from the first one scrolled to as far as they fit.
    let classes = super::board::classes(app.config, &app.usage);
    let items = super::board::items(&classes, &app.usage);
    let first = app.board_scroll.min(items.len() - 1);
    let end = top + height;
    if end <= y + 2 {
        return;
    }
    let heading = y;
    jump_buttons(canvas, heading, left, right, matches!(items[first], Item::Currencies));
    y += 1;
    let tall = end - y >= 15;
    // How many entries fit; the last row says how many are below when not all do.
    let fit = |limit: usize| fitting(&items[first..], tall, width, limit);
    let mut drawn = fit(end - y);
    if first + drawn < items.len() {
        drawn = fit(end - y - 1);
    }
    for item in &items[first..first + drawn] {
        let h = item_rows(*item, tall, width);
        let row = Area { top: y, left, width, height: h };
        match *item {
            Item::Class(ci) => board_class(canvas, row, tall, ci, &classes[ci]),
            Item::Currencies => board_currencies(app, canvas, row, tall),
        }
        y += h;
    }
    // What is above and below: a click moves there.
    if first > 0 {
        let text = format!("▲ {first}");
        canvas.put_within(heading, right.saturating_sub(text.chars().count() + 1), right, &text, Style::Yellow);
        canvas.hit(heading, right.saturating_sub(text.chars().count() + 1), text.chars().count(), Target::BoardScroll(-(drawn as isize)));
    }
    if first + drawn < items.len() {
        let text = format!("▼ {} more", items.len() - first - drawn);
        canvas.put_within(end - 1, left + 1, right, &text, Style::Yellow);
        canvas.hit(end - 1, left + 1, text.chars().count(), Target::BoardScroll(drawn as isize));
    }
}

/// The board's clocks, as many as fit, in rows under a heading: the rows they take.
fn board_clocks(app: &App, canvas: &mut Canvas, area: Area) -> usize {
    let Area { top, left, width, height } = area;
    let right = left + width;
    let clocks = app.clocks();
    let per = ((width.saturating_sub(1)) / CLOCK_WIDTH).max(1);
    let allowed_rows = height.saturating_sub(4 + 20 + 1).min(6);
    let shown = clocks.len().min(per * allowed_rows);
    if shown == 0 {
        return 0;
    }
    canvas.put_within(top, left + 1, right, "Clocks", Style::Bold);
    for (i, c) in clocks.iter().take(shown).enumerate() {
        let x = left + 1 + (i % per) * CLOCK_WIDTH;
        let row = top + 1 + i / per;
        let text = format!("{:<12}{:>8} {:>4}{:>4}", c.city, c.time, c.difference, c.day);
        let end = canvas.put_within(row, x, right, &text, Style::Plain);
        canvas.hit(row, x, end - x, Target::BoardClock(i));
    }
    1 + shown.div_ceil(per)
}

/// Two buttons that jump to the currencies or to the measures; the one the list is at is lit.
fn jump_buttons(canvas: &mut Canvas, row: usize, left: usize, right: usize, on_money: bool) {
    let mut x = left + 1;
    for (label, money) in [("Currencies", true), ("Measures", false)] {
        let text = format!("[ {label} ]");
        let w = text.chars().count();
        if x + w > right.saturating_sub(8) {
            break;
        }
        canvas.put(row, x, &text, if on_money == money { Style::Lit } else { Style::Plain });
        canvas.hit(row, x, w, Target::BoardJump(money));
        x += w + 1;
    }
}

/// How many of `items` fit in `limit` rows.
fn fitting(items: &[super::board::Item], tall: bool, width: usize, limit: usize) -> usize {
    let (mut used, mut n) = (0, 0);
    for item in items {
        let h = item_rows(*item, tall, width);
        if used + h > limit {
            break;
        }
        used += h;
        n += 1;
    }
    n
}

/// A class of the board, in `row`: its name, then its units as buttons (boxes when `tall`).
fn board_class(canvas: &mut Canvas, row: Area, tall: bool, ci: usize, class: &super::board::Class) {
    let Area { top: y, left, width, height: h } = row;
    let right = left + width;
    let name_row = if tall { y + 1 } else { y };
    canvas.put_within(name_row, left + 1, left + CLASS_NAME, &format!("{} {}", class.letter, class.name), Style::Bold);
    canvas.hit_rect(y, left + 1, CLASS_NAME - 1, h, Target::BoardClass(ci));
    let mut x = left + 1 + CLASS_NAME;
    for (bi, b) in class.buttons.iter().enumerate() {
        let w = (b.label.chars().count() + 4).max(6);
        if x + w > right {
            break;
        }
        let style = if b.used { Style::Lit } else { Style::Plain };
        if tall {
            big_button(canvas, y, x, w, "");
            canvas.put(y + 1, x + 1, &format!("{:^width$}", b.label, width = w - 2), style);
        } else {
            canvas.put(y, x, "[", Style::Dim);
            canvas.put(y, x + 1, &format!(" {} ", b.label), style);
            canvas.put(y, x + w - 1, "]", Style::Dim);
        }
        canvas.hit_rect(y, x, w, h, Target::BoardUnit(ci, bi));
        x += w + 1;
    }
}

/// The currencies of the board, in `row`: valued in the local currency, as tiles (when `tall`) or
/// on one row.
fn board_currencies(app: &App, canvas: &mut Canvas, row: Area, tall: bool) {
    let Area { top: y, left, width, .. } = row;
    let right = left + width;
    let money = app.board_money();
    let title = match app.base_currency() {
        Some(base) => format!("Currencies in {base}"),
        None => "Currencies  (set currency.local for values)".to_string(),
    };
    canvas.put_within(y, left + 1, right, &title, Style::Bold);
    if tall {
        let tiles_per_row = ((width.saturating_sub(1)) / (TILE + 1)).max(1);
        for (i, m) in money.iter().enumerate() {
            let (row, x) = (y + 1 + 3 * (i / tiles_per_row), left + 1 + (i % tiles_per_row) * (TILE + 1));
            big_button(canvas, row, x, TILE, &m.code);
            let value = m.value.clone().unwrap_or_else(|| "–".to_string());
            canvas.put(row + 1, x + 1, &format!("{value:^9}"), if m.used { Style::Lit } else { Style::Plain });
            canvas.hit_rect(row, x, TILE, 3, Target::BoardMoney(i));
        }
        return;
    }
    let mut x = left + 1 + CLASS_NAME;
    for (i, m) in money.iter().enumerate() {
        let text = format!("[ {} {} ]", m.code, m.value.clone().unwrap_or_default());
        let w = text.chars().count();
        if x + w > right {
            break;
        }
        canvas.put(y, x, &text, if m.used { Style::Lit } else { Style::Plain });
        canvas.hit(y, x, w, Target::BoardMoney(i));
        x += w + 1;
    }
}

/// What the band holds, in `area`: a panel, else the keyboard (from `keypad_top`) when `keypad_on`.
/// The cursor goes when a panel has its own caret.
fn band_content(app: &App, canvas: &mut Canvas, area: Area, keypad_on: bool, cursor: &mut Option<(usize, usize)>) {
    match &app.overlay {
        Some(Overlay::Measures(v)) => measures(app, v, canvas, area),
        Some(Overlay::Picker(p)) => unit_keys(app, p, canvas, area),
        Some(Overlay::Currencies(v)) => {
            currencies(v, canvas, area);
            *cursor = None;
        }
        Some(Overlay::Sizes(v)) => sizes(v, canvas, area),
        Some(Overlay::Trivia(v)) => trivia(app, v, canvas, area),
        Some(Overlay::History(v)) => {
            history_panel(app, v, canvas, area);
            *cursor = None;
        }
        Some(Overlay::Search(s)) => {
            search(app, s, canvas, area);
            *cursor = None;
        }
        _ if keypad_on => keypad(app, canvas, area.top),
        _ => {}
    }
}

pub fn render(app: &App, rows: usize, cols: usize) -> Frame {
    let mut canvas = Canvas::new(rows, cols);
    if rows < MIN_ROWS || cols < MIN_COLS {
        canvas.put(0, 0, &format!("terminal too small (needs {MIN_COLS}x{MIN_ROWS})"), Style::Plain);
        return Frame { lines: canvas.lines(), cursor: None, hits: Vec::new() };
    }
    toolbar(app, &mut canvas);
    status(app, &mut canvas, rows - 1);
    // From the bottom: the status line; the band (its top rule or border first); the request
    // block above it (a rule, the line, its result and suggestions); the history takes the rest,
    // under a rule.
    let mut cursor = match pane_width(rows, cols) {
        Some(pane_w) => wide(app, &mut canvas, pane_w),
        None => {
            let band = band_rows(app, rows);
            let band_top = rows - 1 - band;
            history(app, &mut canvas, HEAD, band_top - 3 - HEAD);
            let mut cursor = expression(app, &mut canvas, band_top - 3);
            // What the band holds: a panel, else the keypad; with nothing open, only the rule.
            let width = cols.min(PANEL_WIDTH);
            let area = Area { top: band_top, left: (cols - width) / 2, width, height: band };
            band_content(app, &mut canvas, area, band > 1, &mut cursor);
            cursor
        }
    };
    if window(app, &mut canvas, rows) {
        cursor = None;
    }
    Frame { lines: canvas.lines(), cursor, hits: canvas.hits }
}

/// The rows above the history: the toolbar.
const HEAD: usize = 1;

/// The Wide layout: the history and the request keep the left; what the band holds is in a pane
/// at the right, `pane_w` wide, as high as the screen between the toolbar and the status line.
/// Where the cursor is.
fn wide(app: &App, canvas: &mut Canvas, pane_w: usize) -> Option<(usize, usize)> {
    let (rows, cols) = (canvas.cells.len(), canvas.cols);
    let left_w = cols - pane_w - 1;
    let band_top = rows - 2;
    let mut left = Canvas::new(rows, left_w);
    history(app, &mut left, HEAD, band_top - 3 - HEAD);
    let mut cursor = expression(app, &mut left, band_top - 3);
    left.rule(band_top);
    let mut pane = Canvas::new(rows, pane_w);
    let area = Area { top: 1, left: 0, width: pane_w, height: rows - 2 };
    if panel_open(app) {
        band_content(app, &mut pane, area, false, &mut cursor);
    } else if let (Some((board_w, keyboard_w)), true) = (wide_split(rows, cols), app.keyboard_shown()) {
        // The big keyboard (the compact one when the screen is not tall enough for it) at the
        // left and, beside it, the board.
        let keyboard = Area { width: keyboard_w, ..area };
        if area.height > BIG_ROWS {
            big_keyboard(app, &mut pane, keyboard);
        } else {
            keypad(app, &mut pane, 1);
        }
        for row in 1..rows - 1 {
            pane.cells[row][keyboard_w] = ('│', Style::Dim);
        }
        board_view(app, &mut pane, Area { left: keyboard_w + 1, width: board_w, ..area });
    } else if !board_shown(app) {
        keypad(app, &mut pane, 1);
    } else {
        board_view(app, &mut pane, area);
    }
    for row in 1..rows - 1 {
        canvas.cells[row][..left_w].copy_from_slice(&left.cells[row]);
        // A divider for the keyboard; a panel has its own border.
        canvas.cells[row][left_w] = if panel_open(app) { (' ', Style::Plain) } else { ('│', Style::Dim) };
        canvas.cells[row][left_w + 1..].copy_from_slice(&pane.cells[row]);
    }
    canvas.hits.extend(left.hits);
    canvas.hits.extend(pane.hits.into_iter().map(|h| Hit { col: h.col + left_w + 1, ..h }));
    cursor
}

/// The window open over the screen, if any (a Trivia popup, About, Settings, the help, a
/// question): `true` when one is drawn (it takes the cursor).
fn window(app: &App, canvas: &mut Canvas, rows: usize) -> bool {
    match &app.overlay {
        Some(Overlay::Trivia(v @ super::trivia::View { popup: Some(scroll), .. })) => trivia_popup(app, v, canvas, rows, *scroll),
        Some(Overlay::About) => about(canvas, rows),
        Some(Overlay::Settings(view)) => settings_window(canvas, rows, view),
        Some(Overlay::Help(scroll)) => help(canvas, rows, *scroll),
        Some(Overlay::Choice(a, selected)) => choice(canvas, rows, a, *selected),
        _ => return false,
    }
    true
}

/// The toolbar: the function key and the name of each window, the open one lit; shorter names,
/// then the keys alone, when the terminal is narrow. The online indicator follows when it fits.
fn toolbar(app: &App, canvas: &mut Canvas) {
    let cols = canvas.cols;
    // The key, then the name: in full, short, or none.
    let parts = |t: &super::action::Tool, tier: usize| {
        let name = match tier {
            0 => format!(" {} ", t.label),
            1 => format!(" {} ", t.short),
            _ => " ".to_string(),
        };
        (format!(" F{}", t.fkey), name)
    };
    let width = |tier: usize| TOOLBAR.iter().map(|t| parts(t, tier)).map(|(k, n)| columns(&k) + columns(&n)).sum::<usize>();
    let tier = (0..2).find(|&tier| width(tier) <= cols).unwrap_or(2);
    let mut col = 0;
    for tool in &TOOLBAR {
        let lit = match tool.action {
            Action::Open(panel) => panel.is(&app.overlay),
            // Lit while the keypad is what the band shows.
            Action::ToggleKeypad => app.keypad != Keypad::Hidden && !panel_open(app),
            _ => false,
        };
        let (key, name) = parts(tool, tier);
        let from = col;
        col = canvas.put_within(0, col, cols, &key, if lit { Style::Lit } else { Style::Yellow });
        col = canvas.put_within(0, col, cols, &name, if lit { Style::Lit } else { Style::Plain });
        canvas.hit(0, from, col - from, Target::Act(tool.action));
    }
    online_indicator(app, canvas, col);
}

/// Online or offline, at the right of the toolbar (its tools end at `col`), with the age of the
/// last sync when it fits.
fn online_indicator(app: &App, canvas: &mut Canvas, col: usize) {
    let cols = canvas.cols;
    use aequalis::network::Status;
    let status = app.network_status();
    let (dot, style) = match status {
        Status::Online => ("●", Style::Green),
        Status::Offline | Status::Forced => ("●", Style::Yellow),
        Status::Checking => ("○", Style::Dim),
    };
    // The label with the age of the last sync (`online (2h old)`), else a short one (`offline
    // (forced)` → `offline`), else nothing.
    let short = match status {
        Status::Forced => "offline",
        Status::Checking => "…",
        _ => status.label(),
    };
    let age = match app.synced_ago() {
        _ if status == Status::Checking => None,
        Some(age) if age == "now" => Some("just synced".to_string()),
        Some(age) => Some(format!("{age} old")),
        None => Some("never synced".to_string()),
    };
    let full = match (status, &age) {
        (Status::Forced, Some(age)) => format!("offline (forced, {age})"),
        (_, Some(age)) => format!("{} ({age})", status.label()),
        (_, None) => status.label().to_string(),
    };
    let with_age = age.map_or(short.to_string(), |age| format!("{short} ({age})"));
    let labels = [full, with_age, status.label().to_string(), short.to_string()];
    for label in labels {
        let text = format!(" {dot} {label} ");
        let start = cols.saturating_sub(1 + text.chars().count());
        if start > col {
            canvas.put_within(0, start, cols, &text, style);
            break;
        }
    }
}

/// The last entries (minus the scroll), newest at the bottom, results aligned: `height` rows, the
/// first of them a rule.
fn history(app: &App, canvas: &mut Canvas, top: usize, height: usize) {
    let cols = canvas.cols;
    canvas.rule(top);
    let inner = height - 1;
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
        let mut col = canvas.put_within(row, 2, right, "  ", Style::Bold);
        let input_style = Style::Plain;
        // The request recalls into the line; the result is copied.
        let from = col - 2;
        col = canvas.put_within(row, col, right, &format!("{:<width$}", entry.input), input_style);
        canvas.hit(row, 2, col - from, Target::Entry(start + i));
        let result_from = col;
        match &entry.result {
            Ok(result) => {
                col = canvas.put_within(row, col, right, "  = ", Style::Dim);
                col = canvas.put_within(row, col, right, result, Style::Bold);
            }
            Err(msg) => {
                col = canvas.put_within(row, col, right, "  ✗ ", Style::Red);
                col = canvas.put_within(row, col, right, msg, Style::Red);
            }
        }
        canvas.hit(row, result_from, col - result_from, Target::EntryResult(start + i));
    }
    if scroll > 0 {
        // On the rule under the history.
        let label = format!(" ↓ {scroll} newer ");
        canvas.put_within(top + height, cols - 2 - label.chars().count(), cols - 2, &label, Style::Dim);
    }
}

/// A key of the keypad: what it shows, and the character that lights it when typed.
type KeyCap = (&'static str, Option<char>);

/// The calculator keys, row by row: the same as the Android app's.
const BASIC: [[KeyCap; 4]; 5] = [
    [("(", Some('(')), (")", Some(')')), ("%", Some('%')), ("÷", Some('/'))],
    [("7", Some('7')), ("8", Some('8')), ("9", Some('9')), ("×", Some('*'))],
    [("4", Some('4')), ("5", Some('5')), ("6", Some('6')), ("−", Some('-'))],
    [("1", Some('1')), ("2", Some('2')), ("3", Some('3')), ("+", Some('+'))],
    [("0", Some('0')), (".", Some('.')), ("⌫", Some('⌫')), ("⏎", Some('='))],
];

/// What a click on a key of the keypad asks for: its label's character (`÷` is `/`, `×` is `*`),
/// ⌫ and ⏎, or for a function its name and bracket.
fn key_action(label: &str) -> Action {
    match label {
        "÷" => Action::Insert('/'),
        "×" => Action::Insert('*'),
        "−" => Action::Insert('-'),
        "⌫" => Action::Backspace,
        "⏎" => Action::Submit,
        "C" => Action::Clear,
        "←" => Action::Left,
        "→" => Action::Right,
        "to" => Action::Write(" to "),
        "␣" => Action::Insert(' '),
        "sin" => Action::Write("sin("),
        "cos" => Action::Write("cos("),
        "tan" => Action::Write("tan("),
        "ln" => Action::Write("ln("),
        "log" => Action::Write("log("),
        "√" => Action::Write("sqrt("),
        "π" => Action::Write("pi"),
        other => Action::Insert(other.chars().next().unwrap_or(' ')),
    }
}

/// The functions beside them in Sci (each writes its bracket: `sin(`; `π` writes `pi`).
const SCI: [[KeyCap; 2]; 5] = [
    [("sin", None), ("cos", None)],
    [("tan", None), ("√", None)],
    [("ln", None), ("log", None)],
    [("π", None), ("e", Some('e'))],
    [("^", Some('^')), ("°", Some('°'))],
];

/// Rows of the keypad in the band: its rule with the tabs, and five rows of keys.
const KEYPAD_ROWS: usize = 6;
/// The panels in the band are at most this wide, centred.
const PANEL_WIDTH: usize = 100;
/// The terminal's rows from which the keypad is shown: below them the history would be too short.
pub(super) const KEYPAD_FROM: usize = 15;

/// A rectangle of the screen.
#[derive(Clone, Copy)]
struct Area {
    top: usize,
    left: usize,
    width: usize,
    height: usize,
}

/// The rows the band takes under the request line, its top rule or border included: just that
/// rule (1) with nothing open. A panel takes the rows it needs, or what the terminal leaves.
fn band_rows(app: &App, rows: usize) -> usize {
    let wanted = match &app.overlay {
        Some(Overlay::Measures(_)) => measures_rows(app),
        Some(Overlay::Picker(p)) => unit_keys_rows(app, p),
        Some(Overlay::Currencies(v)) => list_rows(aequalis::currency::search(&v.query).len()),
        Some(Overlay::Sizes(_)) => super::sizes::rows().len() + 2,
        Some(Overlay::Search(s)) => list_rows(app.found(&s.query).len()),
        Some(Overlay::History(v)) => list_rows(super::history_panel::matches(&app.history, &v.query).len()),
        Some(Overlay::Trivia(v)) => trivia_rows(v),
        _ if app.keypad != Keypad::Hidden && rows >= KEYPAD_FROM => KEYPAD_ROWS,
        _ => 1,
    };
    // The toolbar, its rule, the history's rule and one line, the request's 4 rows and the status.
    wanted.clamp(1, rows - 7)
}

/// The keypad in the band: its rule carries the tabs (the one shown is lit), then the keys. The key
/// typed last is lit.
fn keypad(app: &App, canvas: &mut Canvas, top: usize) {
    let sci = app.keypad == Keypad::Sci;
    canvas.rule(top);
    // The layout shown is lit; the part with the focus is reversed.
    let tab = |part: Part, shown: bool| match (app.focus == Some(part), shown) {
        (true, _) => Style::Inverse,
        (false, true) => Style::Lit,
        (false, false) => Style::Dim,
    };
    let mut col = canvas.put(top, 3, " Basic ", tab(Part::Basic, !sci));
    canvas.hit(top, 3, col - 3, Target::Focus(Part::Basic));
    col = canvas.put(top, col, "│", Style::Dim);
    let from = col;
    col = canvas.put(top, col, " Sci ", tab(Part::Sci, sci));
    canvas.hit(top, from, col - from, Target::Focus(Part::Sci));
    let cap = |canvas: &mut Canvas, row: usize, x: usize, (label, typed): KeyCap| {
        let style = match typed {
            Some(c) if Some(c) == app.pressed => Style::Lit,
            Some('0'..='9' | '.') => Style::Bold,
            Some(_) => Style::Yellow,
            None => Style::Plain,
        };
        canvas.put(row, x, &format!("{label:^5}"), style);
        canvas.hit(row, x, 5, Target::Act(key_action(label)));
    };
    for r in 0..BASIC.len() {
        let row = top + 1 + r;
        let mut x = 2;
        if sci {
            for (c, &key) in SCI[r].iter().enumerate() {
                cap(canvas, row, x + 5 * c, key);
            }
            canvas.cells[row][x + 10] = ('│', Style::Dim);
            x += 12;
        }
        for (c, &key) in BASIC[r].iter().enumerate() {
            cap(canvas, row, x + 5 * c, key);
        }
    }
    shortcuts(app, canvas, top, sci);
}

/// The width of the shortcuts' two columns (units, currencies) and the gap between them.
const SHORTCUT_COLUMN: usize = 9;
const SHORTCUT_GAP: usize = 2;
/// The columns kept free between the keypad and the shortcuts.
const SHORTCUT_APART: usize = 3;

/// The column the shortcuts end at: the right edge of the width the panels use.
fn shortcuts_right(cols: usize) -> usize {
    let area = cols.min(PANEL_WIDTH);
    (cols - area) / 2 + area - 2
}

/// Whether the shortcuts fit at the right of the keypad (Basic, or Sci) in `cols` columns.
pub(super) fn shortcuts_fit(cols: usize, sci: bool) -> bool {
    let keypad_end = if sci { 34 } else { 22 };
    shortcuts_right(cols) >= keypad_end + SHORTCUT_APART + 2 * SHORTCUT_COLUMN + SHORTCUT_GAP
}

/// The shortcuts at the right of the keypad (Alt+1…5 units, Alt+6…0 currencies), when the keypad
/// ends at `keypad_end` and the width leaves room: the units and the currencies used most, each
/// with the digit that writes it. They end at the right edge of the panels' width.
fn shortcuts(app: &App, canvas: &mut Canvas, top: usize, sci: bool) {
    if !shortcuts_fit(canvas.cols, sci) {
        return;
    }
    let width = 2 * SHORTCUT_COLUMN + SHORTCUT_GAP;
    let left = shortcuts_right(canvas.cols) - width;
    let title = |part: Part| if app.focus == Some(part) { Style::Inverse } else { Style::Dim };
    canvas.put(top, left - 1, " Units ", title(Part::Units));
    canvas.put(top, left + SHORTCUT_COLUMN + SHORTCUT_GAP - 1, " Money ", title(Part::Money));
    canvas.hit(top, left - 1, 7, Target::Focus(Part::Units));
    canvas.hit(top, left + SHORTCUT_COLUMN + SHORTCUT_GAP - 1, 7, Target::Focus(Part::Money));
    for row in 0..BASIC.len() {
        for (column, base) in [(0, 0), (1, 5)] {
            let index = base + row;
            let Some(name) = app.shortcut(index) else { continue };
            let name = if column == 0 { aequalis::favourites::label(name) } else { name };
            let digit = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'][index];
            let x = left + column * (SHORTCUT_COLUMN + SHORTCUT_GAP);
            let chosen = (app.focus == Some(Part::Units) && column == 0 || app.focus == Some(Part::Money) && column == 1) && app.slot == row;
            let lit = app.lit == Some(index) || chosen;
            canvas.put(top + 1 + row, x, &digit.to_string(), if lit { Style::Lit } else { Style::Dim });
            canvas.put_within(top + 1 + row, x + 2, x + SHORTCUT_COLUMN, &format!("{name:<7}"), if lit { Style::Lit } else { Style::Plain });
            canvas.hit(top + 1 + row, x, SHORTCUT_COLUMN, Target::Act(Action::Shortcut(index)));
        }
    }
}

const PROMPT: &str = "> ";

/// The request line between two rules and, below it, the live result and the suggestions. Returns
/// the cursor.
fn expression(app: &App, canvas: &mut Canvas, top: usize) -> Option<(usize, usize)> {
    let cols = canvas.cols;
    canvas.rule(top);
    canvas.rule(top + 3);
    let right = cols - 2;
    // Scrolled sideways so the cursor stays visible.
    let start = 2 + PROMPT.len();
    let room = right - start - 1;
    let offset = app.line.cursor().saturating_sub(room);
    let visible: String = app.line.chars().iter().skip(offset).collect();
    canvas.put_within(top + 1, 2, right, PROMPT, Style::Bold);
    canvas.put_within(top + 1, start, right, &visible, Style::Plain);
    canvas.hit(top + 1, start, right - start, Target::Line(offset));

    // The result (or the error) row. At the right edge the answer, which a click copies (kept
    // whole); at the left what the line means, as far as the answer leaves room: the grouping
    // applied (when there are operations), the error, the currencies, the suggestions as buttons.
    let row = top + 2;
    let preview = app.preview();
    let answer = match &preview {
        Some(Ok(result)) => Some(format!("= {result} ⧉")),
        _ => None,
    };
    let answer_width = answer.as_ref().map_or(0, |a| a.chars().count());
    let limit = if answer.is_some() { (right + 1).saturating_sub(answer_width + 2).max(4) } else { right };
    if let Some(answer) = &answer {
        let from = (right + 1).saturating_sub(answer_width).max(limit + 1);
        let end = canvas.put_within(row, from, right + 1, answer, Style::Green);
        canvas.hit(row, from, end - from, Target::Act(Action::Copy));
    }
    let mut col = 4;
    if let Some(grouping) = app.grouping() {
        col = canvas.put_within(row, col, limit, &grouping, Style::Bold);
        col = canvas.put_within(row, col, limit, " ", Style::Plain);
    }
    if let Some(Err(msg)) = &preview {
        col = canvas.put_within(row, col, limit, &format!("· {msg}"), Style::Dim);
    }
    col = suggestions(app, canvas, row, col, limit);
    if let (Some(Ok(_)), Some(currencies)) = (&preview, app.currencies()) {
        canvas.put_within(row, col, limit, &format!("   {currencies}"), Style::Dim);
    }
    Some((top + 1, start + app.line.cursor() - offset))
}

/// The suggestions for the word at the cursor, from `col` of `row` up to `limit`: buttons
/// (`[ mi ]`) when there is room for the brackets, plain words in a narrow window. Returns where
/// they end.
fn suggestions(app: &App, canvas: &mut Canvas, row: usize, mut col: usize, limit: usize) -> usize {
    let Some((names, selected, cities)) = app.suggestions() else { return col };
    col = canvas.put_within(row, col, limit, "   ", Style::Plain);
    if selected.is_none() {
        col = canvas.put_within(row, col, limit, "Tab:", Style::Dim);
    }
    let buttons = canvas.cols >= 60;
    for (i, name) in names.iter().enumerate() {
        // Cities have spaces in their labels: `·` between them.
        match (cities, i, buttons) {
            (true, 1.., false) => col = canvas.put_within(row, col, limit, " · ", Style::Dim),
            _ => col = canvas.put_within(row, col, limit, " ", Style::Plain),
        }
        let style = if selected == Some(i) { Style::Inverse } else { Style::Plain };
        let from = col;
        if buttons {
            col = canvas.put_within(row, col, limit, "[", Style::Dim);
            col = canvas.put_within(row, col, limit, &format!(" {name} "), style);
            col = canvas.put_within(row, col, limit, "]", Style::Dim);
        } else {
            col = canvas.put_within(row, col, limit, name, style);
        }
        canvas.hit(row, from, col - from, Target::Suggest(i));
    }
    col
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

/// The badge of the window open (none on the line), the last note / warning / error, and the keys
/// of the moment on the right.
fn status(app: &App, canvas: &mut Canvas, row: usize) {
    let cols = canvas.cols;
    let (badge, keys): (&str, &[(&str, &str)]) = match &app.overlay {
        // The keyboard has the focus (F9, Tab).
        None if app.focus.is_some() && app.keyboard_shown() => match app.focus {
            Some(Part::Units | Part::Money) => {
                (" KEYS ", &[("Tab", "next"), ("Shift+Tab", "previous"), ("↑↓ ←→", "choose"), ("Enter", "write"), ("<Esc>", "back")])
            }
            _ => (" KEYS ", &[("Tab", "next"), ("Shift+Tab", "previous"), ("type", "as usual"), ("<Esc>", "back")]),
        },
        Some(Overlay::Choice(..)) => (" CHOOSE ", &[("↑↓ j k 1-9", "reading"), ("Enter", "confirm"), ("<Esc>", "back")]),
        Some(Overlay::Trivia(v)) if v.popup.is_some() => (" TRIVIA ", &[("↑↓", "scroll"), ("Enter", "write the value"), ("<Esc>", "back")]),
        Some(Overlay::Trivia(_)) => (" TRIVIA ", &[("Tab ←→", "section"), ("↑↓ j k", "entry"), ("Enter", "details"), ("<Esc>", "back")]),
        Some(Overlay::About) => (" ABOUT ", &[("<Esc>", "close")]),
        Some(Overlay::Help(_)) => (" HELP ", &[("↑↓", "scroll"), ("<Esc>", "close")]),
        Some(Overlay::Settings(v)) if v.edit.is_some() => (" SETTINGS ", &[("type", "value"), ("Enter", "save"), ("<Esc>", "cancel")]),
        Some(Overlay::Settings(v)) if v.list.is_some() => (" SETTINGS ", &[("type", "filter"), ("↑↓", "choose"), ("Enter", "save"), ("<Esc>", "cancel")]),
        Some(Overlay::Settings(_)) => {
            (" SETTINGS ", &[("↑↓", "choose"), ("Enter ←→", "change"), ("d", "default"), ("a", "advanced"), ("R", "reset"), ("<Esc>", "close")])
        }
        Some(Overlay::History(_)) => {
            (" HISTORY ", &[("type", "filter"), ("↑↓", "choose"), ("Enter", "recall"), ("Alt+Y", "copy result"), ("Del", "remove"), ("<Esc>", "back")])
        }
        Some(Overlay::Search(_)) => (" SEARCH ", &[("type", "search"), ("↑↓", "choose"), ("Enter", "write"), ("<Esc>", "back")]),
        Some(Overlay::Currencies(_)) => (" CURRENCIES ", &[("type", "search"), ("↑↓", "choose"), ("Enter", "write"), ("<Esc>", "back")]),
        Some(Overlay::Sizes(_)) => (" SIZES ", &[("↑↓ j k", "choose"), ("Enter", "write"), ("<Esc>", "back")]),
        Some(Overlay::Measures(_)) => (" MEASURES ", &[("↑↓ j k", "choose"), ("letter Enter", "open"), ("<Esc>", "back")]),
        Some(Overlay::Picker(_)) => (" UNITS ", &[("←↓↑→ h j k l", "move"), ("Tab", "system"), ("Enter", "write"), ("<Esc>", "measures")]),
        None if app.keyboard_shown() => ("", &[("F1", "help"), ("Tab", "keyboard"), ("Alt+Y", "copy result"), ("Ctrl+Q", "quit")]),
        None => ("", &[("F1", "help"), ("Ctrl+K", "search"), ("Alt+Y", "copy result"), ("Ctrl+Q", "quit")]),
    };
    // The message first: the first keys go until it fits next to them, and all of them when it
    // still does not.
    let message = app.status.as_ref().map(|(kind, msg)| match kind {
        StatusKind::Note => (format!("note: {msg}"), Style::Dim),
        StatusKind::Warning => (format!("warning: {msg}"), Style::Yellow),
        StatusKind::Error => (format!("error: {msg}"), Style::Red),
    });
    let width = |keys: &[(&str, &str)]| keys.iter().map(|(k, w)| k.chars().count() + 1 + w.chars().count() + 3).sum::<usize>() - 3;
    let taken = badge.chars().count() + 1 + message.as_ref().map_or(0, |(text, _)| text.chars().count() + 2);
    let (mut keys, kept) = (keys, if message.is_some() { 0 } else { 1 });
    while keys.len() > kept && taken + width(keys) > cols - 1 {
        keys = &keys[1..];
    }
    let hint_col = if keys.is_empty() { cols + 1 } else { hints(canvas, row, cols - 1, keys, false) };
    let col = canvas.put(row, 0, badge, Style::Badge) + 1;
    if let Some((text, style)) = message {
        canvas.put_within(row, col, hint_col.saturating_sub(1), &text, style);
    }
}

/// A window over the screen takes the clicks: those outside it ask to close it (Esc), those inside
/// it are the window's (and where it has nothing, nothing).
fn modal_hits(canvas: &mut Canvas, rows: usize, (top, left, width, height): (usize, usize, usize, usize)) {
    canvas.hit_rect(1, 0, canvas.cols, rows - 1, Target::Dismiss);
    canvas.hit_rect(top, left, width, height, Target::Inside);
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
/// The rows the unit keys need: the tabs, the buttons of the larger tab (or the table of date
/// formats and its footer), the line about the selected unit, and the borders.
fn unit_keys_rows(app: &App, p: &Picker) -> usize {
    let category = p.category(app.config);
    let columns = super::measures::columns(category);
    let most = [System::Metric, System::Imperial].map(|tab| p.units_of(app.config, tab).count()).into_iter().max().unwrap_or(1);
    let dates_rows = (category.name == aequalis::config::TIME).then(format_rows).map_or(0, |t| t.len() + 1);
    // Sized for the larger tab, so the panel does not jump while switching.
    (most.div_ceil(columns) + 1).max(dates_rows) + 4
}

fn unit_keys(app: &App, p: &Picker, canvas: &mut Canvas, area: Area) {
    let category = p.category(app.config);
    canvas.boxed(area.top, area.left, area.height, area.width, &format!("{} units", p.title(app.config)));
    panel_tabs(canvas, area, p.tabs(app.config));
    // Time's second tab is the table of date formats.
    match (category.name == aequalis::config::TIME).then(format_rows) {
        Some(table) if p.shows_formats(app.config) => format_table(p, canvas, area, &table),
        _ => unit_buttons(app, p, canvas, area),
    }
}

/// A panel's tabs on its second row (the one shown lit; a click shows it), and a rule under them.
fn panel_tabs<'a>(canvas: &mut Canvas, area: Area, tabs: impl IntoIterator<Item = (&'a str, bool)>) {
    let Area { top, left, width, .. } = area;
    let right = left + width - 1;
    let mut col = left + 2;
    for (k, (name, shown)) in tabs.into_iter().enumerate() {
        let from = col;
        col = canvas.put_within(top + 1, col, right, &format!(" {name} "), if shown { Style::Lit } else { Style::Dim });
        canvas.hit(top + 1, from, col - from, Target::Tab(k));
        col = canvas.put_within(top + 1, col, right, " ", Style::Plain);
    }
    panel_rule(canvas, top + 2, left, right);
}

/// A rule across a panel, joined to its frame (`├───┤`).
fn panel_rule(canvas: &mut Canvas, row: usize, left: usize, right: usize) {
    canvas.cells[row][left + 1..right].fill(('─', Style::Dim));
    canvas.cells[row][left] = ('├', Style::Dim);
    canvas.cells[row][right] = ('┤', Style::Dim);
}

/// Time's Formats tab: the table of date formats, the chosen cell lit, and a footer about it
/// (its name, now in it and, when it fits, what it is).
fn format_table(p: &Picker, canvas: &mut Canvas, area: Area, table: &[super::picker::FormatRow]) {
    let Area { top, left, width, height } = area;
    let right = left + width - 1;
    let visible = (height - 4).saturating_sub(1).max(1);
    let offset = p.cell.0.saturating_sub(visible.saturating_sub(1));
    let mut chosen = None;
    for (r, row) in table.iter().enumerate().skip(offset).take(visible) {
        let y = top + 3 + r - offset;
        canvas.put_within(y, left + 2, right, row.label, Style::Bold);
        let x0 = left + 2 + FORMAT_LABEL + 1;
        for (c, (short, word)) in row.cells.iter().enumerate() {
            let x = x0 + c * (FORMAT_CELL + 1);
            let selected = (r, c) == p.cell;
            let end = canvas.put_within(y, x, right, &format!("{short:^FORMAT_CELL$}"), if selected { Style::Lit } else { Style::Plain });
            canvas.hit(y, x, end - x, Target::Cell(r, c));
            if c + 1 < row.cells.len() && end < right {
                canvas.cells[y][end] = ('│', Style::Dim);
            }
            if selected {
                chosen = Some(word.clone());
            }
        }
    }
    let footer = top + height - 2;
    if let (Some(word), true) = (chosen, height >= 6) {
        let [name, what, example] = format_card_lines(&word);
        let mut col = canvas.put_within(footer, left + 2, right, &name, Style::Bold);
        col = canvas.put_within(footer, col, right, &format!(" · {}", example.trim_start_matches("now: ")), Style::Green);
        if col + 3 + what.chars().count() < right {
            canvas.put_within(footer, col, right, &format!(" · {what}"), Style::Dim);
        }
    }
}

/// The units of the tab shown as buttons, all as wide as the category's longest, and under them
/// one line about the selected unit when there is room.
fn unit_buttons(app: &App, p: &Picker, canvas: &mut Canvas, area: Area) {
    let Area { top, left, width, height } = area;
    let right = left + width - 1;
    let category = p.category(app.config);
    let items = p.items(app.config);
    let button = category.units.iter().map(|u| u.symbol.chars().count()).max().unwrap_or(1) + 2;
    let about = (height >= 7).then(|| top + height - 2);
    let visible = height - 4 - usize::from(about.is_some());
    let table = p.rows(app.config);
    let offset = Picker::position(&table, p.selected).0.saturating_sub(visible.saturating_sub(1));
    let mut x = left + 1;
    for (i, unit) in items.iter().enumerate() {
        let (r, c) = Picker::position(&table, i);
        if c == 0 {
            x = left + 1;
        }
        let at = x;
        x += button + 1;
        if r < offset || r >= offset + visible {
            continue;
        }
        let row = top + 3 + r - offset;
        let style = if i == p.selected { Style::Lit } else { Style::Bold };
        let end = canvas.put_within(row, at, right, &format!("{:^button$}", unit.symbol), style);
        canvas.hit(row, at, end - at, Target::Pick(i));
        if c + 1 < table[r] && end < right {
            canvas.cells[row][end] = ('│', Style::Dim);
        }
    }
    if let (Some(unit), Some(row)) = (items.get(p.selected), about) {
        let [name, aliases, value] = unit_card_lines(app, category, unit);
        // The aliases go first when the line is too long for the panel.
        let long = name.chars().count() + aliases.chars().count() + value.chars().count() + 6 > width.saturating_sub(4);
        let mut col = canvas.put_within(row, left + 2, right, &name, Style::Bold);
        if !long {
            col = canvas.put_within(row, col, right, &format!(" · {aliases}"), Style::Dim);
        }
        canvas.put_within(row, col, right, &format!(" · {value}"), Style::Green);
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

/// The rows of the Trivia: the sections, the entries, and the description of the selected one.
fn trivia_rows(v: &super::trivia::View) -> usize {
    (v.entries().len().max(1) + 9).min(18)
}

/// The Trivia: the sections as tabs, the entries of the one shown (name, value), and the selected
/// entry's description at the bottom. Top-right in the history, like the unit keys.
fn trivia(app: &App, v: &super::trivia::View, canvas: &mut Canvas, area: Area) {
    let Area { top, left, width, height } = area;
    canvas.boxed(top, left, height, width, "Trivia");
    let sections = aequalis::trivia::sections();
    panel_tabs(canvas, area, sections.iter().enumerate().map(|(i, name)| (*name, i == v.section)));
    // The description: up to 3 lines and one about the value's source, when there is room.
    let footer = if height >= 13 { 4 } else { 0 };
    let visible = height - 4 - if footer > 0 { footer + 1 } else { 0 };
    trivia_entries(app, v, canvas, Area { top: top + 3, left, width, height: visible });
    if footer > 0 {
        let base = top + height - 1 - footer;
        panel_rule(canvas, base - 1, left, left + width - 1);
        if let Some(e) = v.entry() {
            trivia_about(app, e, canvas, Area { top: base, left, width, height: footer });
        }
    }
}

/// A Trivia entry's value at `now`, and how it was obtained when it is live (from the online
/// data: synced when, from where; or its estimate).
fn trivia_live(app: &App, e: &aequalis::trivia::Entry, now: i64) -> (Option<aequalis::online::Stored>, String) {
    let synced = app.online.get(&e.name);
    let origin = aequalis::trivia::value(app.config, e, synced, now).map(|(_, o)| o).unwrap_or(aequalis::trivia::Origin::Fixed);
    (synced, aequalis::trivia::origin_text(e, origin).unwrap_or_default())
}

/// The entries of the Trivia's section, in `area`'s rows: the name, the value (in the device's
/// number format), the short description; the selected one lit.
fn trivia_entries(app: &App, v: &super::trivia::View, canvas: &mut Canvas, area: Area) {
    let Area { top, left, width, height: visible } = area;
    let right = left + width - 1;
    let entries = v.entries();
    let now = aequalis::time::unix_now();
    let values: Vec<String> = entries
        .iter()
        .map(|e| match &e.source {
            aequalis::trivia::Source::Tip(example) => example.clone(),
            _ => aequalis::trivia::value(app.config, e, trivia_live(app, e, now).0, now)
                .map(|(x, _)| aequalis::trivia::shown(app.config, e, x, app.opts.precision, &|n| app.numbers.apply(&n)))
                .unwrap_or_else(|err| err),
        })
        .collect();
    let name_width = entries.iter().map(|e| e.name.chars().count()).max().unwrap_or(0).min(width / 2);
    let value_width = values.iter().map(|v| v.chars().count()).max().unwrap_or(0);
    let offset = v.selected.saturating_sub(visible.saturating_sub(1));
    for (i, e) in entries.iter().enumerate().skip(offset).take(visible) {
        let row = top + i - offset;
        let selected = i == v.selected;
        canvas.hit(row, left + 1, width - 2, Target::Pick(i));
        if selected {
            canvas.cells[row][left + 1..right].fill((' ', Style::Lit));
        }
        let text = format!(" {:<name_width$}  {:>value_width$}  ", e.name, values[i]);
        let col = canvas.put_within(row, left + 1, right, &text, if selected { Style::Lit } else { Style::Plain });
        // The short description, cut with … when the window is too narrow.
        let room = right.saturating_sub(col + 1);
        let full = aequalis::trivia::short(e, now);
        let short: String = match full.chars().count() > room {
            true => full.chars().take(room.saturating_sub(1)).chain(['…']).collect(),
            false => full,
        };
        canvas.put_within(row, col, right, &short, if selected { Style::Lit } else { Style::Dim });
    }
}

/// The selected entry's description (3 lines at most), then its launch and where its value comes
/// from.
fn trivia_about(app: &App, e: &aequalis::trivia::Entry, canvas: &mut Canvas, area: Area) {
    let Area { top: base, left, width, .. } = area;
    let right = left + width - 1;
    for (i, line) in wrap(&e.description, width - 4).iter().take(3).enumerate() {
        canvas.put_within(base + i, left + 2, right, line, Style::Plain);
    }
    let source = match &e.source {
        aequalis::trivia::Source::Spacecraft { .. } | aequalis::trivia::Source::Satellites { .. } => trivia_live(app, e, aequalis::time::unix_now()).1,
        aequalis::trivia::Source::Between(..) => "computed from the coordinates".to_string(),
        aequalis::trivia::Source::Tip(_) => "Enter writes the example".to_string(),
        aequalis::trivia::Source::Value(_) => String::new(),
    };
    let launched = e.launched.as_ref().map(|l| format!("Launched {l}")).unwrap_or_default();
    let info = [launched, source].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
    canvas.put_within(base + 3, left + 2, right, &info, Style::Green);
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
    let facts = aequalis::trivia::facts(app.config, entry, live, now);
    let facts = aequalis::trivia::popup(app.config, entry, &facts, app.opts.precision, &|n| app.numbers.apply(&n));
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
    modal_hits(canvas, rows, (top, left, width, height));
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
fn search(app: &App, s: &super::search::Search, canvas: &mut Canvas, area: Area) {
    use aequalis::find::Kind;
    let found = app.found(&s.query);
    let Area { top, left, width, height } = area;
    let right = left + width - 1;
    canvas.boxed(top, left, height, width, "Search units, currencies, settings, trivia, cities, history");
    let col = canvas.put_within(top + 1, left + 2, right, "/ ", Style::Yellow);
    let col = canvas.put_within(top + 1, col, right, &s.query, Style::Bold);
    canvas.put_within(top + 1, col, right, "▏", Style::Yellow);
    canvas.cells[top + 2][left + 1..right].fill(('─', Style::Dim));
    canvas.cells[top + 2][left] = ('├', Style::Dim);
    canvas.cells[top + 2][right] = ('┤', Style::Dim);
    let visible = height - 4;
    if found.is_empty() {
        canvas.put_within(top + 3, left + 2, right, "nothing matches", Style::Dim);
        return;
    }
    let symbol_width = found.iter().map(|f| f.written.chars().count()).max().unwrap_or(0).min(24);
    let kind_width = 9;
    let offset = s.selected.saturating_sub(visible.saturating_sub(1));
    for (i, f) in found.iter().enumerate().skip(offset).take(visible) {
        let lit = i == s.selected;
        let what = match f.kind {
            Kind::Unit => format!("{} · {}", f.name, f.about),
            Kind::Currency => format!("{} · {}", f.name, conversion(app, &f.written)),
            Kind::Trivia => format!("{} · {}", f.name, f.about),
            Kind::Setting | Kind::City => f.name.clone(),
            Kind::History => format!("= {}", f.name),
        };
        let row = top + 3 + i - offset;
        if lit {
            canvas.cells[row][left + 1..right].fill((' ', Style::Lit));
        }
        let col = canvas.put_within(row, left + 1, right, &format!(" {:<kind_width$}", f.kind.label()), if lit { Style::Lit } else { Style::Dim });
        let text = format!("{:<symbol_width$}  {what} ", f.written);
        canvas.put_within(row, col, right, &text, if lit { Style::Lit } else { Style::Plain });
        canvas.hit(row, left + 1, width - 2, Target::Pick(i));
    }
}

/// How a currency stands against the local one: `1 EUR = 1.0850 USD`, `your currency` for the
/// local one itself, `currency` when there is no local currency or no rate to give.
fn conversion(app: &App, code: &str) -> String {
    match app.local_currency {
        Some(local) if local == code => "your currency".to_string(),
        Some(local) => match aequalis::currency::rate(&app.rates, code, local) {
            Some(rate) => format!("1 {code} = {} {local}", aequalis::convert::format_value(rate, 4)),
            None => "currency, no rate".to_string(),
        },
        None => "currency".to_string(),
    }
}

fn measures_rows(app: &App) -> usize {
    super::measures::items(app.config).len() + 2
}

fn measures(app: &App, v: &super::measures::View, canvas: &mut Canvas, area: Area) {
    use super::measures::{items, label, Item};
    let items = items(app.config);
    let Area { top, left, width, height } = area;
    let right = left + width - 1;
    canvas.boxed(top, left, height, width, "Measures");
    let names = items.iter().map(|&i| label(app.config, i).0.chars().count()).max().unwrap_or(0);
    let visible = height - 2;
    let offset = v.selected.saturating_sub(visible.saturating_sub(1));
    for (i, &item) in items.iter().enumerate().skip(offset).take(visible) {
        let row = top + 1 + i - offset;
        let (name, key) = label(app.config, item);
        let lit = i == v.selected;
        canvas.hit(row, left + 1, width - 2, Target::Pick(i));
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

/// The History panel: the filter line, then the requests, newest first, each with its result and
/// the buttons that recall it (↩), copy its result (⧉) and remove it (✕).
fn history_panel(app: &App, v: &super::history_panel::View, canvas: &mut Canvas, area: Area) {
    let found = super::history_panel::matches(&app.history, &v.query);
    let Area { top, left, width, height } = area;
    let right = left + width - 1;
    canvas.boxed(top, left, height, width, "History");
    let col = canvas.put_within(top + 1, left + 2, right, "/ ", Style::Yellow);
    let col = canvas.put_within(top + 1, col, right, &v.query, Style::Bold);
    canvas.put_within(top + 1, col, right, "▏", Style::Yellow);
    canvas.cells[top + 2][left + 1..right].fill(('─', Style::Dim));
    canvas.cells[top + 2][left] = ('├', Style::Dim);
    canvas.cells[top + 2][right] = ('┤', Style::Dim);
    let visible = height - 4;
    if found.is_empty() {
        let what = if app.history.entries().is_empty() { "nothing evaluated yet" } else { "no request matches" };
        canvas.put_within(top + 3, left + 2, right, what, Style::Dim);
        return;
    }
    let buttons = 9;
    let input_width = found.iter().map(|&i| app.history.entries()[i].input.chars().count()).max().unwrap_or(0).min(width / 2);
    let offset = v.selected.saturating_sub(visible.saturating_sub(1));
    for (n, &i) in found.iter().enumerate().skip(offset).take(visible) {
        let row = top + 3 + n - offset;
        let entry = &app.history.entries()[i];
        let lit = n == v.selected;
        // The whole row recalls; the result and the two buttons at the right are over it.
        canvas.hit(row, left + 1, width - 2, Target::Pick(n));
        if lit {
            canvas.cells[row][left + 1..right].fill((' ', Style::Lit));
        }
        let text_limit = right.saturating_sub(buttons + 1);
        let mut col = canvas.put_within(row, left + 1, text_limit, &format!(" {:<input_width$}  ", entry.input), if lit { Style::Lit } else { Style::Plain });
        let from = col;
        col = match &entry.result {
            Ok(result) => canvas.put_within(row, col, text_limit, &format!("= {result}"), if lit { Style::Lit } else { Style::Bold }),
            Err(msg) => canvas.put_within(row, col, text_limit, &format!("✗ {msg}"), if lit { Style::Lit } else { Style::Red }),
        };
        canvas.hit(row, from, col - from, Target::EntryResult(i));
        let at = right - buttons;
        for (k, (glyph, target)) in [("↩", Target::Entry(i)), ("⧉", Target::EntryResult(i)), ("✕", Target::Remove(i))].into_iter().enumerate() {
            canvas.put_within(row, at + 3 * k + 1, right, glyph, if lit { Style::Lit } else { Style::Yellow });
            canvas.hit(row, at + 3 * k, 3, target);
        }
    }
}

/// The Currencies: the search line, then the currencies matching it (code, symbol, name).
/// The rows of a search panel: the query, a rule, up to 8 matches, the borders.
fn list_rows(found: usize) -> usize {
    (found.max(1) + 4).min(12)
}

fn currencies(v: &super::currencies::View, canvas: &mut Canvas, area: Area) {
    let found = aequalis::currency::search(&v.query);
    let Area { top, left, width, height } = area;
    let right = left + width - 1;
    canvas.boxed(top, left, height, width, "Currencies");
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
        let symbol = c.symbols.first().copied().unwrap_or("");
        let text = format!(" {}  {:<4} {} ", c.code, symbol, c.name);
        canvas.put_within(top + 3 + i - offset, left + 1, right, &text, style);
        canvas.hit(top + 3 + i - offset, left + 1, width - 2, Target::Pick(i));
    }
}

/// The Sizes panel: the groups, which write their item word, then the system keys, their code.
fn sizes(v: &super::sizes::View, canvas: &mut Canvas, area: Area) {
    let rows = super::sizes::rows();
    let Area { top, left, width, height } = area;
    let right = left + width - 1;
    canvas.boxed(top, left, height, width, "👕👟 Sizes");
    let names = rows.iter().map(|(name, ..)| name.chars().count()).max().unwrap_or(0);
    let visible = height - 2;
    let offset = v.selected.saturating_sub(visible.saturating_sub(1));
    for (i, (name, sample, _)) in rows.iter().enumerate().skip(offset).take(visible) {
        let row = top + 1 + i - offset;
        let lit = i == v.selected;
        canvas.hit(row, left + 1, width - 2, Target::Pick(i));
        let col = canvas.put_within(row, left + 1, right, &format!(" {name:<names$} "), if lit { Style::Lit } else { Style::Bold });
        canvas.put_within(row, col + 1, right - 1, sample, Style::Dim);
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
    let (top, left, width, height) = centred(canvas, rows, width, a.choices.len() + 2, &title);
    modal_hits(canvas, rows, (top, left, width, height));
    let right = left + width - 1;
    for (i, c) in a.choices.iter().enumerate() {
        let row = top + 1 + i;
        canvas.hit(row, left + 1, width - 2, Target::Pick(i));
        let text = format!(" {}  {:<column$} {} ", i + 1, c.pattern, c.date);
        let style = if i == selected { Style::Lit } else { Style::Plain };
        canvas.put_within(row, left + 2, right - 1, &text, style);
    }
}

/// `O`: the settings, their values (changed ones bold), what the chosen one does, the download.
fn settings_window(canvas: &mut Canvas, rows: usize, view: &super::settings::View) {
    use super::settings::Row;
    use aequalis::settings::KEYS;
    let current = &view.settings;
    let key_width = KEYS.iter().map(|k| k.key.len()).max().unwrap_or(0);
    let list = view.rows();
    let lines = list.len() + 5;
    let (top, left, width, height) = centred(canvas, rows, 78, lines, "⚙ Settings");
    modal_hits(canvas, rows, (top, left, width, height));
    let right = left + width - 2;
    let visible = height.saturating_sub(5);
    let offset = view.selected.saturating_sub(visible.saturating_sub(1));
    for (row, i) in (top + 1..).zip((0..list.len()).skip(offset).take(visible)) {
        let chosen = i == view.selected;
        canvas.hit(row, left + 1, width - 2, Target::Pick(i));
        let style = match (chosen, list[i]) {
            (true, _) => Style::Lit,
            (false, Row::Setting(k)) if current.get(k.key) != k.default => Style::Bold,
            (false, Row::Setting(_)) => Style::Plain,
            (false, Row::Reset) => Style::Red,
            (false, _) => Style::Yellow,
        };
        canvas.put_within(row, left + 1, right, &settings_row(view, list[i], chosen, key_width), style);
    }
    let sep = top + height - 4;
    canvas.cells[sep][left + 1..left + width - 1].fill(('─', Style::Dim));
    let about = match list.get(view.selected) {
        Some(Row::Setting(k)) if k.default.is_empty() => format!("{} (d clears)", k.about),
        Some(Row::Setting(k)) => format!("{} (default: {})", k.about, k.default),
        Some(Row::Advanced) => "downloads, formats, mouse, shortcuts (a)".to_string(),
        Some(Row::Reset) => "every setting to its default (R, Enter twice)".to_string(),
        _ => "download rates, cities and live values now".to_string(),
    };
    canvas.put_within(sep + 1, left + 2, right, &about, Style::Dim);
    if let Some(msg) = &view.message {
        canvas.put_within(sep + 2, left + 2, right, msg, Style::Green);
    }
    if let Some(list) = &view.list {
        let row = top + 1 + view.selected.saturating_sub(offset);
        settings_list(canvas, rows, view, list, (row, left + key_width + 3));
    }
}

/// A row of the Settings window: a setting and its value (as typed, while it is), or one of the
/// rows after them.
fn settings_row(view: &super::settings::View, row: super::settings::Row, chosen: bool, key_width: usize) -> String {
    use super::settings::Row;
    match row {
        Row::Setting(k) => {
            let current = view.settings.get(k.key);
            let value = match (&view.edit, chosen) {
                (Some(typed), true) => format!("{typed}▏"),
                _ if k.kind == aequalis::settings::Kind::Favourites && current.is_empty() => "(none yet: the defaults)".to_string(),
                _ => aequalis::settings::shown(k.key, current),
            };
            format!(" {:<key_width$}  {value}", k.key)
        }
        Row::Advanced if view.advanced => " Fewer".to_string(),
        Row::Advanced => match view.hidden_changes() {
            0 => " Advanced".to_string(),
            n => format!(" Advanced ({n} changed)"),
        },
        Row::Reset => format!(" {}", super::settings::RESET),
        Row::Download => format!(" {}", super::settings::DOWNLOAD),
    }
}

/// The list of a setting's values, open under its row at (`row`, `col`) (above it when there is
/// no room): what is typed, then the values, the chosen one lit.
fn settings_list(canvas: &mut Canvas, rows: usize, view: &super::settings::View, list: &super::settings::List, (row, col): (usize, usize)) {
    let choices = view.choices();
    let shown = choices.len().clamp(1, 8);
    let height = shown + 3;
    let list_left = col.min(canvas.cols.saturating_sub(34));
    let list_width = 32.min(canvas.cols - list_left);
    let list_top = if row + 1 + height <= rows { row + 1 } else { row.saturating_sub(height) };
    canvas.boxed(list_top, list_left, height, list_width, "");
    canvas.hit_rect(list_top, list_left, list_width, height, Target::Inside);
    let inner = list_left + list_width - 2;
    canvas.put_within(list_top + 1, list_left + 2, inner, &format!("{}▏", list.query), Style::Plain);
    if choices.is_empty() {
        canvas.put_within(list_top + 2, list_left + 2, inner, "no match", Style::Dim);
    }
    let first = list.selected.saturating_sub(shown - 1);
    for (line, i) in (list_top + 2..).zip((first..choices.len()).take(shown)) {
        canvas.hit(line, list_left + 1, list_width - 2, Target::Pick(i));
        let style = if i == list.selected { Style::Lit } else { Style::Plain };
        canvas.put_within(line, list_left + 1, inner + 1, &format!(" {:<w$}", choices[i].1, w = list_width - 3), style);
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

/// `A`: the logo, the name and tagline, what it does, the facts (`about.rs`), centred.
fn about(canvas: &mut Canvas, rows: usize) {
    let facts = aequalis::about::facts(&aequalis::about::memory());
    let title = format!("{} — {}", aequalis::about::NAME, aequalis::about::TAGLINE);
    let mut lines: Vec<(String, Style)> = aequalis::about::ART.lines().map(|l| (l.to_string(), Style::Plain)).collect();
    let logo_rows = lines.len();
    lines.push((String::new(), Style::Plain));
    lines.push((title.clone(), Style::Bold));
    lines.push((aequalis::about::DESCRIPTOR.to_string(), Style::Plain));
    lines.push((String::new(), Style::Plain));
    let width = lines.iter().map(|(l, _)| l.chars().count()).chain(facts.iter().map(|(l, v)| l.chars().count() + v.chars().count() + 2)).max().unwrap_or(0) + 4;
    let height = lines.len() + facts.len() + 2;
    let (top, left, width, about_height) = centred(canvas, rows, width, height, "About");
    modal_hits(canvas, rows, (top, left, width, about_height));
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
    ("The request", ""),
    ("  type  ←/→  Backspace", "edit it; Enter evaluates"),
    ("  Home End  Ctrl-A/E", "start, end"),
    ("  Alt-←/→  Ctrl-←/→", "by word"),
    ("  Ctrl-W  Ctrl-U  Alt-K", "kill the word before, to the start, to the end"),
    ("  Tab / Shift-Tab", "complete the unit (cycles)"),
    ("  ↑/↓  Ctrl-P  Ctrl-N", "earlier / later requests, as in a shell;"),
    ("", "↓ past the newest: what you were typing"),
    ("  Alt-N", "write `now` (dates: now + 3d, now unix, …)"),
    ("  Alt-Y", "copy the result, not a selection"),
    ("  PgUp/PgDn", "scroll the history"),
    ("  exit + Enter", "quit (quit + Enter works too)"),
    ("  about + Enter", "about: version, memory, last online sync"),
    ("Windows (the toolbar; Alt letters where F-keys are taken)", ""),
    ("  F2  Alt-M", "Measures: the unit classes, then their units"),
    ("  F3  Alt-C", "Currencies: search by code, name or country"),
    ("  F4  Alt-Z", "Sizes: shoes, then the systems (EU, US…)"),
    ("  F5  Alt-H", "History: recall, copy, remove a request"),
    ("  F6  Alt-T", "Trivia: useful values with a description"),
    ("  F10  Alt-S", "Settings"),
    ("  F1  Alt-?", "this help"),
    ("  Alt-A", "about: version, memory, last online sync"),
    ("  Ctrl-K", "search units, currencies, settings, trivia,"),
    ("", "cities, earlier requests; Enter uses it"),
    ("  Alt-O", "mouse on/off (off: the terminal selects text)"),
    ("  Alt-L", "board: Currencies / Measures"),
    ("Wide screens (from 120 columns)", ""),
    ("  the board", "clocks, units per class, currencies;"),
    ("", "a button writes it; used ones are lit"),
    ("  150 columns", "the big keyboard beside the board"),
    ("  F9  Alt-B", "the keyboard under the request; again: hides it"),
    ("  Tab  Shift-Tab", "Basic, Sci, Units, Money; Esc back"),
    ("  ↑↓ ←→  Enter", "in Units / Money: choose, column, write"),
    ("  Alt-1…5  Alt-6…0", "write one of your units / currencies, as listed"),
    ("", "beside the keypad; the used ones stay (Settings)"),
    ("  the same key again", "closes the window"),
    ("History (F5)", ""),
    ("  type  ↑/↓", "filter, choose a request (newest first)"),
    ("  Enter  Alt-Y  Del", "recall it, copy its result, remove it"),
    ("  ↩  ⧉  ✕  (click)", "the same; a click on the result copies it"),
    ("Measures", ""),
    ("  ↑/↓  j/k", "choose a class (Length, Temperature, …)"),
    ("  Enter  or its letter", "its units (D: Earth distance, then the places)"),
    ("Currencies", ""),
    ("  type  ↑/↓  Enter", "search by code or name, choose, write the code"),
    ("Unit keys (in the band under the request)", ""),
    ("  ←↓↑→  h j k l", "move; circular, wraps around at the edges"),
    ("  Tab / Shift-Tab", "the next / previous tab (Metric / Imperial:"),
    ("", "opens on the device's system; Bits & Bytes,"),
    ("", "Date & Time: the first)"),
    ("  a class's letter", "another class's units (L: Length, K: …)"),
    ("  Enter", "write the unit and a space; stay for the next,"),
    ("", "or, with the target set, back to the request"),
    ("", "replaces the unit under the cursor, else fills"),
    ("", "the source, then the target"),
    ("  Esc", "back to the Measures, without choosing"),
    ("Help", ""),
    ("  j/k  ↑/↓  PgUp/PgDn", "scroll"),
    ("Esc and Ctrl-C", ""),
    ("  Esc", "one step back: the window, the history, the Tab"),
    ("", "cycle, the message, then the line (↑ restores)"),
    ("  Ctrl-C", "the same; with nothing to close or clear: quit"),
    ("  Ctrl-Q  Alt-Q  Ctrl-D", "quit (Ctrl-D on an empty line)"),
    ("  Ctrl-L", "redraw"),
];

/// The help window; scrolls (j/k) when the terminal is shorter than the help.
fn help(canvas: &mut Canvas, rows: usize, scroll: usize) {
    let (top, left, width, height) = centred(canvas, rows, 80, HELP.len() + 2, "Help · Esc close");
    modal_hits(canvas, rows, (top, left, width, height));
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
    let memory = match aequalis::memory::current_kb() {
        Some(now) => format!(" Memory: {} now · peak {} ", aequalis::memory::size(now), aequalis::memory::size(aequalis::memory::peak_kb())),
        None => format!(" Memory: peak {} ", aequalis::memory::size(aequalis::memory::peak_kb())),
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
    use super::super::keys::Key;
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

    /// Types `keys` (`\n` = Enter, `\t` = Tab, `\x1b` = Esc; `<F2>`, `<A-m>`, `<C-k>`, `<Esc>`,
    /// `<Up>`, `<Down>` for the other keys) and renders.
    const OPTS: Options<'static> = Options { positional: Vec::new(), precision: 4, silent: false, verbose: false };

    fn app_after<'a>(config: &'a Config, opts: &'a Options<'a>, keys: &str) -> App<'a> {
        app_after_in(config, opts, keys, (24, 80))
    }

    /// `app_after` on a terminal of `screen` rows and columns.
    fn app_after_in<'a>(config: &'a Config, opts: &'a Options<'a>, keys: &str, screen: (usize, usize)) -> App<'a> {
        super::super::tests::test_machine();
        let mut app = App::new(config, opts);
        app.screen = screen;
        let mut chars = keys.chars();
        while let Some(c) = chars.next() {
            let key = match c {
                '\n' => Key::Enter,
                '\x1b' => Key::Esc,
                '\t' => Key::Tab,
                '<' => {
                    let name: String = chars.by_ref().take_while(|&c| c != '>').collect();
                    match name.as_str() {
                        "Esc" => Key::Esc,
                        "Up" => Key::Up,
                        "Down" => Key::Down,
                        "Right" => Key::Right,
                        "Left" => Key::Left,
                        "S-Tab" => Key::BackTab,
                        n if n.starts_with('F') => Key::F(n[1..].parse().unwrap()),
                        n if n.starts_with("A-") => Key::Alt(n.chars().nth(2).unwrap()),
                        n if n.starts_with("C-") => Key::Ctrl(n.chars().nth(2).unwrap()),
                        n => panic!("no key <{n}>"),
                    }
                }
                _ => Key::Char(c),
            };
            app.handle(key);
        }
        app
    }

    fn screen(keys: &str, rows: usize, cols: usize) -> (Vec<String>, Option<(usize, usize)>) {
        let config = Config::default();
        // Verbose: the status line shows the notes.
        let opts = Options { positional: Vec::new(), precision: 4, silent: false, verbose: true };
        let app = app_after_in(&config, &opts, keys, (rows, cols));
        let frame = render(&app, rows, cols);
        (frame.lines.iter().map(|l| plain(l)).collect(), frame.cursor)
    }

    #[test]
    fn sections() {
        let (lines, cursor) = screen("10 km mi\n(100+20)/120\n100 + 20 sqft m", 14, 100);
        assert_eq!(lines.len(), 14);
        // The toolbar, then a rule, the history as a scrollback (newest at the bottom), a rule,
        // the request line, its result and suggestions, a rule, the status line.
        assert!(lines[0].starts_with(" F2 Measures  F3 Currencies  F4 👕👟 Sizes  F5 History  F6 Trivia  F9 Keys  F10 ⚙ "), "{}", lines[0]);
        assert!(lines[1].chars().all(|c| c == '─'), "{}", lines[1]);
        assert!(lines[2..7].iter().all(|l| !l.contains("km")), "{lines:?}");
        assert!(lines[7].starts_with("    10 km mi      = 6.2137 mi"), "{}", lines[7]);
        assert!(lines[8].starts_with("    (100+20)/120  = 1"), "{}", lines[8]);
        assert!(lines[9].chars().all(|c| c == '─'));
        assert!(lines[10].starts_with("  > 100 + 20 sqft m"), "{}", lines[10]);
        assert!(lines[11].contains("Tab: [ mm2 ] [ m2 ] [ mi2 ]"), "{}", lines[11]);
        assert!(lines[12].chars().all(|c| c == '─'));
        assert!(lines[13].starts_with(" note: evaluated as (100 + 20) / 120"), "{}", lines[13]);
        assert!(lines[13].ends_with("F1 help · Ctrl+K search · Alt+Y copy result · Ctrl+Q quit"), "{}", lines[13]);
        assert_eq!(cursor, Some((10, 19)));
        // No mode: nothing says "INPUT" or "COMMAND".
        assert!(!lines.join("\n").contains("INPUT") && !lines.join("\n").contains("COMMAND"));
    }

    #[test]
    fn toolbar() {
        // The online indicator at the right; the open window lit (the style, not the text).
        let (lines, _) = screen("", 12, 100);
        assert!(lines[0].starts_with(" F2 Measures  F3 Currencies  F4 👕👟 Sizes  F5 History  F6 Trivia  F9 Keys  F10 ⚙ "), "{}", lines[0]);
        assert!(lines[0].contains('●') || lines[0].contains('○'), "{}", lines[0]);
        let lit = |keys: &str, cols: usize| {
            let config = Config::default();
            let opts = OPTS;
            let app = app_after(&config, &opts, keys);
            let top = render(&app, 24, cols).lines.remove(0);
            top.split("\x1b[0m").filter(|part| part.starts_with("\x1b[1;30;43m")).map(plain).collect::<Vec<_>>()
        };
        assert!(lit("", 100).is_empty());
        assert_eq!(lit("<F2>", 100), [" F2 Measures"]);
        assert_eq!(lit("<A-c>", 100), [" F3 Currencies"]);
        assert_eq!(lit("<F4>", 100), [" F4 👕👟 Sizes"]);
        assert_eq!(lit("<A-z>", 100), [" F4 👕👟 Sizes"]);
        assert_eq!(lit("<F6>", 100), [" F6 Trivia"]);
        assert_eq!(lit("<F10>", 100), [" F10 ⚙"]);
        // The keypad's key is lit while the keypad is shown: it says how to bring it, and back.
        assert_eq!(lit("<F9>", 100), [" F9 Keys"]);
        assert_eq!(lit("<A-b>", 100), [" F9 Keys"]);
        assert!(lit("<F9><F9>", 100).is_empty(), "hidden again");
        assert!(lit("<F9><F2>", 100).iter().all(|l| l != " F9 Keys"), "a panel has taken the band");
        assert_eq!(lit("1\n<F5>", 100), [" F5 History"], "the history in focus");
        assert!(lit("<F1>", 100).is_empty(), "the help is not on the toolbar");
        // The Measures' unit keys are the Measures'.
        assert_eq!(lit("<F2>L", 100), [" F2 Measures"]);
        // From 82 columns the full names; at 80 the short ones and the online indicator.
        let (lines, _) = screen("", 12, 82);
        assert!(lines[0].starts_with(" F2 Measures  F3 Currencies  F4 👕👟 Sizes  F5 History  F6 Trivia  F9 Keys  F10 ⚙"), "{}", lines[0]);
        let (lines, _) = screen("", 12, 80);
        assert!(lines[0].starts_with(" F2 Meas  F3 Curr  F4 👕👟  F5 Hist  F6 Triv  F9 Keys  F10 ⚙"), "{}", lines[0]);
        assert!(lines[0].contains('●') || lines[0].contains('○'), "the indicator at 80 columns: {}", lines[0]);
        // Narrower: the short names, then the keys alone.
        let (lines, _) = screen("", 12, 61);
        assert!(lines[0].starts_with(" F2 Meas  F3 Curr  F4 👕👟  F5 Hist  F6 Triv  F9 Keys  F10 ⚙"), "{}", lines[0]);
        let (lines, _) = screen("", 12, 50);
        assert!(lines[0].starts_with(" F2  F3  F4  F5  F6  F9  F10"), "{}", lines[0]);
        let (lines, _) = screen("", 12, 40);
        assert!(lines[0].starts_with(" F2  F3  F4  F5  F6  F9  F10"), "{}", lines[0]);
    }

    #[test]
    fn status_line_and_cursor() {
        let (lines, cursor) = screen("10 km", 12, 80);
        assert!(lines[11].ends_with("F1 help · Ctrl+K search · Alt+Y copy result · Ctrl+Q quit"), "{}", lines[11]);
        assert_eq!(cursor, Some((8, 9)), "the cursor in the request, at the end");
        // Narrow: the first hints go.
        let (lines, _) = screen("10 km", 12, 40);
        assert!(lines[11].ends_with("Alt+Y copy result · Ctrl+Q quit") && !lines[11].contains("F1"), "{}", lines[11]);
        // A message keeps its room: the hints give way (the first ones, then all).
        let (lines, _) = screen("(10+12-3)/4Km Mi\n", 12, 80);
        assert!(
            lines[11].starts_with(" note: evaluated as ((10 + 12) - 3) / 4 Km Mi") || lines[11].starts_with(" note: evaluated as ((10 + 12) - 3) / 4"),
            "{}",
            lines[11]
        );
        assert!(lines[11].ends_with("Alt+Y copy result · Ctrl+Q quit") || lines[11].ends_with("Ctrl+Q quit"), "{}", lines[11]);
        let (lines, _) = screen("10 km kg\n", 12, 60);
        assert!(lines[11].starts_with(" error: cannot convert `km` to `kg`: different kinds of unit"), "{}", lines[11]);
        // The History panel: its own keys and badge.
        let (lines, _) = screen("1+1\n<F5>", 12, 100);
        assert!(lines[11].starts_with(" HISTORY ") && lines[11].ends_with("Enter recall · Alt+Y copy result · Del remove · <Esc> back"), "{}", lines[11]);
        // The Tab suggestions show while typing, whatever came before.
        let (lines, _) = screen("10 km m", 12, 80);
        assert!(lines.join("\n").contains("Tab:"));
    }

    #[test]
    fn unit_keys_in_the_band() {
        // F2 B: the bits and bytes in the band, under the request line: the Decimal tab first;
        // Tab switches to Binary. The keypad (when shown) gives way to them.
        let (lines, _) = screen("10<F2>B", 24, 80);
        let text = lines.join("\n");
        let top = lines.iter().position(|l| l.contains("┌─ Bits & Bytes units ─")).expect("the panel");
        assert!(lines[top].ends_with("┐"), "{}", lines[top]);
        assert!(lines[top + 1].contains("│  Decimal   Binary "), "{}", lines[top + 1]);
        assert!(lines[top + 2].contains("├──"), "{}", lines[top + 2]);
        assert!(lines[top + 3].contains("│  B  │ kB  │ MB  │ GB  │ TB  "), "{}", lines[top + 3]);
        assert!(lines[top + 5].contains("byte (8 bits) · aliases: byte, bytes · 1 B = 8 b"), "the selected unit: {}", lines[top + 5]);
        assert!(lines[top + 6].contains("└──") && !lines[top + 6].contains("Enter"), "no keys on the border: {}", lines[top + 6]);
        assert_eq!(top + 6, 22, "it ends right above the status line");
        assert!(!text.contains(" KiB "), "binary units on the other tab");
        assert!(!text.contains("Type an expression") || top > 3, "the request stays above it");
        // The navigation keys are on the status line.
        assert!(lines[23].starts_with(" UNITS "));
        assert!(lines[23].ends_with("←↓↑→ h j k l move · Tab system · Enter write · <Esc> measures"), "{}", lines[23]);
        let (lines, _) = screen("10<F2>B\t", 24, 80);
        assert!(lines[top + 3].contains("│  B  │ KiB │ MiB │ GiB │ TiB "), "Tab: {}", lines[top + 3]);
        // The keypad shown gives way to it, and comes back with Esc Esc.
        let (lines, _) = screen("<F9><F2>B", 24, 80);
        assert!(lines.join("\n").contains("Bits & Bytes units") && !lines.join("\n").contains("Basic │ Sci"));
        let (lines, _) = screen("<F9><F2>B\x1b\x1b", 24, 80);
        assert!(lines.join("\n").contains("Basic │ Sci") && !lines.join("\n").contains("units"));
        // Another class's letter: its units.
        let (lines, _) = screen("<F2>BT", 24, 80);
        assert!(lines.join("\n").contains("Date & Time units"));
    }

    #[test]
    fn trivia_window() {
        let (lines, _) = screen("<F6>", 24, 80);
        let top = lines.iter().position(|l| l.contains("┌─ Trivia ─")).expect("the panel");
        assert_eq!(top + 14, 23, "the panel ends right above the status line");
        assert!(lines[top + 1].contains("│  Earth   Space   Satellites   Math   Physics "), "{}", lines[top + 1]);
        // Name, value, short description; the first is a tip.
        assert!(lines[top + 3].contains("│ Distance between places  London to Paris km  Just type the cities"), "{}", lines[top + 3]);
        assert!(lines[top + 4].contains("│ Equator                        40075.017 km  Earth's circumference"), "{}", lines[top + 4]);
        assert!(lines.join("\n").contains("Just type the city from and to: London [to] Paris [mi/km/…]."), "the description");
        // Space: the Voyagers first, estimated (no retrieval in the tests), with their launch.
        let (lines, _) = screen("<F6>\t", 24, 80);
        let text = lines.join("\n");
        let voyager = lines.iter().position(|l| l.contains("│ Voyager 1 ")).expect("Voyager 1");
        assert!(lines[voyager + 1].contains("│ Voyager 2 "), "the Voyagers first: {}", lines[voyager]);
        // A planet by its name, its distance from the Earth now (changing every day: its shape).
        let mercury = lines.iter().find(|l| l.contains("│ Mercury ")).expect("Mercury");
        assert!(mercury.contains(" au)  Distance from the Earth"), "{mercury}");
        assert!(lines[23].starts_with(" TRIVIA ") && lines[23].ends_with("Enter details · <Esc> back"), "{}", lines[23]);
        assert!(text.contains("Launched 1977-09-05 12:56 UTC · estimated"), "{text}");
        // Satellites: counts, as of their date offline.
        let (lines, _) = screen("<F6>\t\t", 24, 80);
        let text = lines.join("\n");
        assert!(text.contains("│ Active satellites ") && text.contains("16701") && text.contains("as of 2026-10-11"), "{text}");
        // Math: pi with 10 decimals.
        let (lines, _) = screen("<F6>\t\t\t", 24, 80);
        assert!(lines.join("\n").contains("3.1415926536"));
    }

    #[test]
    fn trivia_popup_window() {
        // Enter: the popup, with the description and the details; Esc back to the list.
        let (lines, cursor) = screen("<F6>\t\t\t\n", 30, 80);
        let text = lines.join("\n");
        assert!(cursor.is_none());
        assert!(text.contains("┌─ π (pi) ─") && text.contains("Mathematician") && text.contains("Archimedes"), "{text}");
        assert!(text.contains("Definition") && text.contains("π = C / d"), "{text}");
        assert!(lines[29].ends_with("Enter write the value · <Esc> back"), "{}", lines[29]);
        // A planet: its distances from the Sun and the Earth, with the next dates.
        let (lines, _) = screen("<F6>\tjjjjjj\n", 30, 80);
        let text = lines.join("\n");
        assert!(text.contains("Wikipedia: https://en.wikipedia.org/wiki/Mars"), "{text}");
        assert!(text.contains("Mean distance from the Sun: 227943822 km - 1.5237 au"), "{text}");
        assert!(text.contains("Closest to the Sun (perihelion): ") && text.contains(", next on "), "{text}");
        assert!(text.contains("Closest to the Earth: "), "{text}");
        // Voyager 1, as in the request: description, link, current distance, launch.
        let (lines, _) = screen("<F6>\t\n", 30, 80);
        let text = lines.join("\n");
        assert!(text.contains("┌─ Voyager 1 ─") && text.contains("Space probe launched by NASA"), "{text}");
        assert!(text.contains("Wikipedia: https://en.wikipedia.org/wiki/Voyager_1"), "{text}");
        // The estimate grows with time: the format, not today's figure.
        assert!(text.contains("Current distance: ") && text.contains(" km - 1") && text.contains(" au - ") && text.contains("e-3 ly"), "{text}");
        assert!(text.contains("(estimated,"), "{text}");
        assert!(text.contains("Launched: 1977-09-05 12:56 UTC"), "{text}");
        let (lines, _) = screen("<F6>\t\t\t\n\x1b", 30, 80);
        assert!(!lines.join("\n").contains("Mathematician"), "Esc: back to the list");
    }

    #[test]
    fn search_window() {
        let (lines, cursor) = screen("<C-k>kilo", 24, 80);
        assert!(cursor.is_none(), "the search has its own caret");
        let text = lines.join("\n");
        assert!(text.contains("┌─ Search units, currencies, settings, trivia, cities, history ─") && text.contains("│ / kilo▏"), "{text}");
        assert!(text.contains("│ unit     km") && text.contains("kilometre · Length "), "{text}");
        assert!(lines[23].starts_with(" SEARCH "), "{}", lines[23]);
        let (lines, _) = screen("<C-k>zzz", 24, 80);
        assert!(lines.join("\n").contains("nothing matches"));
        // Other kinds: a setting by its words, a trivia entry, an earlier request.
        let (lines, _) = screen("<C-k>precision", 24, 100);
        assert!(lines.join("\n").contains("│ setting  precision"), "{}", lines.join("\n"));
        let (lines, _) = screen("<C-k>voyager", 24, 100);
        assert!(lines.join("\n").contains("│ trivia   Voyager 1"), "{}", lines.join("\n"));
        let (lines, _) = screen("10 km mi\n<C-k>10 km", 24, 100);
        assert!(lines.join("\n").contains("│ history  10 km mi  = 6.2137 mi"), "{}", lines.join("\n"));
    }

    /// The rows of the search for `query`, with rates (USD/EUR 0.9, USD/JPY 150, USD/GBP 0.8) and
    /// the local currency EUR (`local`) or none.
    fn search_rows(query: &str, local: Option<&'static str>) -> Vec<String> {
        use aequalis::online::{Data, Stored};
        let config = Config::default();
        let opts = OPTS;
        let mut app = app_after(&config, &opts, &format!("<C-k>{query}"));
        let rate = |name: &str, value: f64| (name.to_string(), Stored { value, at: 1 });
        app.rates = std::sync::Arc::new(Data { values: vec![rate("USD/EUR", 0.9), rate("USD/JPY", 150.0), rate("USD/GBP", 0.8)], ..Data::default() });
        app.local_currency = local;
        let lines: Vec<String> = render(&app, 24, 100).lines.iter().map(|l| plain(l)).collect();
        lines.iter().filter_map(|l| l.split_once("│ ").map(|(_, rest)| rest.trim_end_matches(['│', ' ']).to_string())).filter(|r| !r.starts_with('/')).collect()
    }

    #[test]
    fn search_shows_currencies_against_the_local_one() {
        // The code column is as wide as the widest symbol listed: compare without the padding.
        let squeeze = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
        let has = |rows: &[String], row: &str| rows.iter().any(|r| squeeze(r).starts_with(&format!("currency {}", squeeze(row))));
        // A currency: its code, its name and what one of it is in the local currency.
        let rows = search_rows("yen", Some("EUR"));
        assert!(has(&rows, "JPY  Japanese yen · 1 JPY = 0.006 EUR"), "{rows:?}");
        let rows = search_rows("gbp", Some("EUR"));
        assert!(has(&rows, "GBP  pound sterling · 1 GBP = 1.125 EUR"), "{rows:?}");
        let rows = search_rows("usd", Some("EUR"));
        assert!(has(&rows, "USD  US dollar · 1 USD = 0.9 EUR"), "{rows:?}");
        // The local one itself, one without a rate, and no local currency.
        let rows = search_rows("eur", Some("EUR"));
        assert!(has(&rows, "EUR  euro · your currency"), "{rows:?}");
        let rows = search_rows("kpw", Some("EUR"));
        assert!(has(&rows, "KPW  North Korean won · currency, no rate"), "{rows:?}");
        let rows = search_rows("yen", None);
        assert!(has(&rows, "JPY  Japanese yen · currency"), "{rows:?}");
        // Units and currencies together: the unit first, then the currencies by name.
        let rows = search_rows("pound", Some("EUR"));
        assert!(rows[0].starts_with("unit     lb "), "{rows:?}");
        assert!(has(&rows, "GBP  pound sterling · 1 GBP = 1.125 EUR"), "{rows:?}");
    }

    #[test]
    fn search_writes_a_currency() {
        let config = Config::default();
        let opts = OPTS;
        let app = app_after(&config, &opts, "10 <C-k>yen\n");
        assert_eq!(app.line.text(), "10 JPY ");
        assert!(app.overlay.is_none());
        let app = app_after(&config, &opts, "10 km <C-k>gbp\n");
        assert_eq!(app.line.text(), "10 km GBP ");
    }

    #[test]
    fn measures_window() {
        let (lines, cursor) = screen("<F2>", 24, 80);
        let text = lines.join("\n");
        assert!(text.contains("┌─ Measures ─"), "{text}");
        // Each class with its letter and its units; the Earth distance after Length.
        assert!(text.contains(" T  Date & Time   ") && text.contains(" B  Bits & Bytes  "), "{text}");
        assert!(text.contains(" L  Length ") && text.contains(" mm cm m km "), "{text}");
        // Whole units, then `…` when they don't fit.
        assert!(text.contains(" B kB MB GB TB KiB MiB GiB TiB b kb Mb Gb Tb Kib Mib …"), "{text}");
        let length = lines.iter().position(|l| l.contains(" L  Length ")).unwrap();
        assert!(lines[length + 1].contains(" D  Earth distance ") && lines[length + 1].contains("Rome to Paris"), "{text}");
        assert!(text.contains(" K  Temperature ") && text.contains(" W  Power "), "{text}");
        // A short terminal scrolls the list; ↑ from the first: the last, scrolled to it.
        let (lines, _) = screen("<F2>", 14, 80);
        assert!(!lines.join("\n").contains(" W  Power "), "{}", lines.join("\n"));
        let (lines, _) = screen("<F2>k", 14, 80);
        assert!(lines.join("\n").contains(" W  Power "), "{}", lines.join("\n"));
        assert!(lines[13].starts_with(" MEASURES ") && lines[13].ends_with("<Esc> back"), "{}", lines[13]);
        assert!(cursor.is_some());
        // Every row inside 40 columns.
        let (lines, _) = screen("<F2>jj", 12, 40);
        assert!(lines.iter().all(|l| l.chars().count() <= 40));
        assert!(lines.join("\n").contains(" L  Length"), "scrolled to the row chosen: {}", lines.join("\n"));
    }

    #[test]
    fn currencies_window() {
        let (lines, cursor) = screen("<F3>yen", 24, 80);
        assert!(cursor.is_none(), "the search has its own caret");
        let text = lines.join("\n");
        assert!(text.contains("┌─ Currencies ─") && text.contains("│ / yen▏"), "{text}");
        assert!(text.contains("│ JPY  ¥    Japanese yen "), "{text}");
        assert!(lines[23].starts_with(" CURRENCIES "), "{}", lines[23]);
        let (lines, _) = screen("<F3>zzzz", 24, 80);
        assert!(lines.join("\n").contains("no currency matches"));
    }

    #[test]
    fn selected_unit_line() {
        // Length, Imperial: in selected first; → → → mi. The line under the buttons says what it is.
        let (lines, _) = screen("<F2>L\t<Right><Right><Right>", 24, 80);
        let button = lines.iter().position(|l| l.contains("│ in │ ft │ yd │ mi ")).expect("the buttons");
        assert!(lines[button + 2].contains("mile · aliases: mile, miles · 1 mi = 1609.344 m"), "{}", lines[button + 2]);
        assert!(lines[button + 3].contains('└'), "the panel ends under it: {}", lines[button + 3]);
        // Too narrow for the aliases: they go first.
        let (lines, _) = screen("<F2>L\t<Right><Right><Right>", 24, 40);
        let line = lines.iter().find(|l| l.contains("1 mi = 1609.344 m")).expect("the line");
        assert!(line.contains("mile · 1 mi") && !line.contains("aliases"), "{line}");
        // A short terminal has no room for it.
        let (lines, _) = screen("<F2>L", 12, 80);
        assert!(!lines.join("\n").contains("aliases"));
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
    fn unit_keys_stay_on_screen() {
        for (rows, cols) in [(24, 80), (12, 40), (14, 60), (50, 200)] {
            for keys in ["<F2>L<Right>", "<F2>V\t<Down>", "<F2>B\t", "<F2>T\t"] {
                let (lines, _) = screen(keys, rows, cols);
                assert_eq!(lines.len(), rows);
                assert!(lines.iter().all(|l| l.chars().count() <= cols), "{keys:?} {rows}x{cols}");
                assert!(lines[rows - 1].starts_with(" UNITS "), "{keys:?} {rows}x{cols}: the status line");
                assert!(lines.iter().any(|l| l.starts_with("  >")), "{keys:?} {rows}x{cols}: the request line stays");
            }
        }
    }

    #[test]
    fn area_and_data_have_five_columns() {
        let (lines, _) = screen("<F2>B", 24, 80);
        // Bytes on the first row, bits below.
        let bytes = lines.iter().position(|l| l.contains("│  B  │ kB  │ MB  │ GB  │ TB  ")).expect("bytes row");
        assert!(lines[bytes + 1].contains("│  b  │ kb  │ Mb  │ Gb  │ Tb  "), "{}", lines[bytes + 1]);
        assert!(lines[bytes - 1].contains("├─"), "first row of buttons");
        let (lines, _) = screen("<F2>B\t", 24, 80);
        let bytes = lines.iter().position(|l| l.contains("│  B  │ KiB │ MiB │ GiB │ TiB ")).expect("bytes row");
        assert!(lines[bytes + 1].contains("│  b  │ Kib │ Mib │ Gib │ Tib "));
    }

    #[test]
    fn time_window_durations_and_dates() {
        // Durations: every time unit on the first tab.
        let (lines, _) = screen("<F2>T", 24, 80);
        let text = lines.join("\n");
        assert!(text.contains("─ Date & Time units ─"), "{text}");
        assert!(text.contains(" Durations   Formats "));
        assert!(text.contains("│ ns  │ us  │ ms  │  s  │ min ") && text.contains("│  h  │  d  │ wk  │ mo  │ yr "), "{text}");
        assert!(text.contains("nanosecond · aliases: nanosecond, nanoseconds · 1 ns = 0.000000001 s"), "the selected unit's line: {text}");
        // Formats: the table of formats, and the footer for the selected one.
        let (lines, _) = screen("<F2>T\t", 24, 80);
        let text = lines.join("\n");
        let top = lines.iter().position(|l| l.contains("─ Date & Time units ─")).expect("the title is not covered");
        for row in [
            "ISO           iso   │   utc   │  basic  │  date   │  time",
            "Unix/local     s    │   ms    │   µs    │   ns    │  local",
            "YYYY/MM/DD   date   │   24h   │ 24h tz  │   12h   │ 12h tz",
            "MM/DD/YY     date   │   24h   │ 24h tz  │   12h   │ 12h tz",
        ] {
            assert!(text.contains(row), "{row}\n{text}");
        }
        // The footer: the selected format and now in it; the panel ends above the status line.
        assert!(text.contains("│ iso · 20"), "{text}");
        let footer = lines.iter().position(|l| l.contains("│ iso · 20")).unwrap();
        assert!(top < footer && lines[footer + 1].contains('└') && footer + 1 == 22, "{text}");
        // l l: basic.
        let (lines, _) = screen("<F2>T\tll", 24, 80);
        assert!(lines.join("\n").contains("iso-basic · 20"));
        // A pattern: its name and now in it, e.g. `DD/MM/YYYY-12h-tz · 10/10/2026 06:45:54 PM +02:00`.
        let (lines, _) = screen("<F2>T\tjjjjjjllll", 24, 80);
        assert!(lines.join("\n").contains("DD/MM/YYYY-12h-tz · "), "{}", lines.join("\n"));
    }

    #[test]
    fn data_tabs_are_decimal_and_binary() {
        let (lines, _) = screen("<F2>B", 24, 80);
        let text = lines.join("\n");
        assert!(text.contains(" Decimal   Binary "));
        assert!(text.contains(" kB ") && !text.contains("KiB"));
        let (lines, _) = screen("<F2>B\t", 24, 80);
        let text = lines.join("\n");
        assert!(text.contains(" KiB ") && !text.contains(" kB "));
    }

    #[test]
    fn unit_keys_fit_every_category() {
        let config = Config::default();
        for category in config.categories {
            let key = crate::tui::measures::key(category);
            for (rows, cols) in [(12, 40), (24, 80)] {
                // Both tabs.
                let screens: Vec<Vec<String>> = (0..2).map(|t| screen(&format!("<F2>{key}{}", "\t".repeat(t)), rows, cols).0).collect();
                let first = &screens[0];
                assert!(first.join("\n").contains(&format!("{} units", category.name)) || cols < 50, "{} {rows}x{cols}", category.name);
                assert!(screens.iter().flatten().all(|l| l.chars().count() <= cols));
                // At 80 columns every unit is on one of the tabs, and visible there.
                if cols == 80 {
                    assert!(first.iter().any(|l| l.contains(&format!("{} units", category.name))), "{} has its title", category.name);
                    let all = screens.iter().map(|s| s.join("\n")).collect::<Vec<_>>().join("\n");
                    for u in category.units {
                        assert!(all.contains(&format!(" {} ", u.symbol)), "{} missing", u.symbol);
                    }
                }
            }
        }
    }

    #[test]
    fn help_window() {
        let (lines, cursor) = screen("<F1>", 100, 90);
        let text = lines.join("\n");
        assert!(text.contains("Help · Esc close"));
        assert!(text.contains("F2  Alt-M") && text.contains("this help") && text.contains("Ctrl-C"));
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
        let top = |status: Status, cols: usize| {
            let mut app = App::new(&config, &opts);
            app.forced_offline = status == Status::Forced;
            app.online = match status {
                Status::Checking | Status::Forced => Data::default(),
                Status::Online => Data { attempted_at: Some(1), ok: true, synced_at: Some(aequalis::time::unix_now() - 7_300), ..Data::default() },
                Status::Offline => Data { attempted_at: Some(1), ok: false, ..Data::default() },
            };
            plain(&render(&app, 12, cols).lines[0])
        };
        // Top right, on the toolbar's line; "not synced" before the first sync.
        assert!(top(Status::Checking, 100).ends_with(" ○ not synced"), "{}", top(Status::Checking, 100));
        // With how long ago the last sync was.
        assert!(top(Status::Online, 120).ends_with(" ● online (2h old)"), "{}", top(Status::Online, 120));
        assert!(top(Status::Offline, 120).ends_with(" ● offline (never synced)"), "{}", top(Status::Offline, 120));
        assert!(top(Status::Forced, 120).ends_with(" ● offline (forced, never synced)"), "{}", top(Status::Forced, 120));
        // At 80 columns the toolbar leaves room for the short ones.
        assert!(top(Status::Online, 80).ends_with(" ● online"), "{}", top(Status::Online, 80));
        assert!(top(Status::Forced, 80).ends_with(" ● offline"), "{}", top(Status::Forced, 80));
        // Narrow: the short label, else none.
        assert!(top(Status::Forced, 44).ends_with(" ● offline"), "{}", top(Status::Forced, 44));
        assert!(top(Status::Forced, 40).ends_with(" F10"), "{}", top(Status::Forced, 40));
    }

    #[test]
    fn about_window() {
        let (lines, cursor) = screen("<A-a>", 24, 80);
        let text = lines.join("\n");
        assert!(cursor.is_none());
        assert!(text.contains("┌─ About ─") && text.contains("█▛ ----> ███▌") && text.contains("▐█▖<----▗███▌"), "{text}");
        assert!(text.contains("Aequalis — Type it. Get it.") && text.contains("Maths, units, currencies and time, answered as you type."), "{text}");
        assert!(text.contains("Memory: ") && text.contains("Last online sync: ") && text.contains("Version: "), "{text}");
        assert!(lines[23].starts_with(" ABOUT "), "{}", lines[23]);
        let (lines, _) = screen("<A-a>\x1b", 24, 80);
        assert!(!lines.join("\n").contains("┌─ About ─"), "Esc closes it");
    }

    #[test]
    fn help_shows_memory() {
        // Short or tall, scrolled or not: the memory is on the bottom border.
        for rows in [14, 70] {
            let (lines, _) = screen("<F1>", rows, 90);
            let text = lines.join("\n");
            assert!(text.contains(" Memory: ") && text.contains(" peak "), "{text}");
            if cfg!(target_os = "linux") {
                assert!(text.contains(" now · peak "), "{text}");
            }
        }
    }

    #[test]
    fn help_scrolls_when_short() {
        let (lines, _) = screen("<F1>", 14, 80);
        let text = lines.join("\n");
        assert!(text.contains("The request") && text.contains("↓ more") && !text.contains("Ctrl-C"));
        // Scrolled past the end: stops at the last line.
        let (lines, _) = screen(&format!("<F1>{}", "j".repeat(80)), 14, 80);
        let text = lines.join("\n");
        assert!(text.contains("Ctrl-C") && text.contains("↑ more") && !text.contains("↓ more"));
    }

    #[test]
    fn help_fits_80_columns() {
        let (lines, _) = screen("<F1>", 70, 80);
        for (keys, what) in HELP {
            let text = keys.trim().to_string();
            assert!(text.is_empty() || lines.iter().any(|l| l.contains(&text)), "{text}");
            assert!(what.is_empty() || lines.iter().any(|l| l.contains(what)), "cut: {what}");
        }
    }

    #[test]
    fn preview_shows_the_grouping() {
        let (lines, _) = screen("12+12/3", 12, 80);
        assert!(lines[9].starts_with("    (12 + (12 / 3))") && lines[9].ends_with("= 16 ⧉"), "{}", lines[9]);
        let (lines, _) = screen("100 + 20 + 50 / 10 sqft m2", 12, 80);
        assert!(lines[9].starts_with("    ((100 + 20) + (50 / 10))") && lines[9].ends_with("= 11.6129 m2 ⧉"), "{}", lines[9]);
        // A plain conversion: just the result.
        let (lines, _) = screen("10 km mi", 12, 80);
        assert!(lines[9].ends_with("= 6.2137 mi ⧉") && lines[9].chars().count() <= 80, "{}", lines[9]);
        // Only the source unit so far: what comes next follows the grouping, once the unit is typed
        // (a space after it: `km` may still become `km/h`, and Tab offers those meanwhile).
        let (lines, _) = screen("1+2 km", 12, 100);
        assert!(lines[9].starts_with("    (1 + 2)    Tab: [ km ] [ km2 ] [ km/h ]"), "{}", lines[9]);
        let (lines, _) = screen("1+2 km ", 12, 100);
        assert!(lines[9].starts_with("    (1 + 2) · now the length unit to convert to: in, ft, yd…"), "{}", lines[9]);
        // A real error follows the grouping too, once the word is typed (a space after it).
        let (lines, _) = screen("1+2 km mi m", 12, 100);
        assert!(lines[9].starts_with("    (1 + 2)") && !lines[9].contains("expected"), "{}", lines[9]);
        let (lines, _) = screen("1+2 km mi m ", 12, 100);
        assert!(lines[9].starts_with("    (1 + 2) · expected"), "{}", lines[9]);
    }

    #[test]
    fn keypad_in_the_band() {
        // Not shown until asked for.
        let (lines, _) = screen("", 24, 80);
        assert!(!lines.join("\n").contains("Basic"));
        assert!(lines[19].chars().all(|c| c == '─') && lines[22].chars().all(|c| c == '─'), "the request block between rules");
        // F9 opens the Sci layout when there is room for it and the shortcuts, in the band under the
        // request line: a rule with the tabs, five rows of keys.
        let (lines, _) = screen("12+3<F9>", 24, 80);
        assert!(lines[17].starts_with("─── Basic │ Sci ───") && lines[17].chars().count() == 80, "{}", lines[17]);
        assert!(lines[15].starts_with("  > 12+3") && lines[16].starts_with("    (12 + 3)") && lines[16].ends_with("= 15 ⧉"), "{} / {}", lines[15], lines[16]);
        for (row, keys) in [(18, "sin  cos │"), (19, "tan   √  │"), (20, "ln   log │"), (21, "π    e  │"), (22, "^    °  │")] {
            assert!(lines[row].contains(keys), "{}", lines[row]);
        }
        assert!(lines[18].contains("(    )    %    ÷"), "{}", lines[18]);
        // Short of room (under 59 columns) it opens on Basic: no functions beside the keys.
        let (lines, _) = screen("12+3<F9>", 24, 58);
        for (row, keys) in [(18, "(    )    %    ÷"), (19, "7    8    9    ×"), (20, "4    5    6    −"), (21, "1    2    3    +"), (22, "0    .    ⌫    ⏎")]
        {
            assert!(lines[row].starts_with("   ") && lines[row].contains(keys), "{}", lines[row]);
            assert!(!lines[row].contains("│"), "no Sci keys in Basic: {}", lines[row]);
        }
        // Tab goes round Basic, Sci, Units, Money: Basic is shown as soon as it has the focus.
        let (lines, _) = screen("12+3<F9>\t\t\t", 24, 80);
        assert!(!lines[18].contains("sin") && lines[18].contains("(    )    %    ÷"), "{}", lines[18]);
        // And again F9: hidden.
        let (lines, _) = screen("12+3<F9><F9>", 24, 80);
        assert!(!lines.join("\n").contains("Basic"));
        // The history is not covered: it ends above the request, whatever the keypad.
        let wide: String = (1..=20).map(|i| format!("{}+{i}\n", "1".repeat(70))).collect();
        let (lines, _) = screen(&format!("{wide}<F9>"), 24, 80);
        assert!(lines[13].contains("+20"), "{}", lines[13]);
        assert!(!lines[2..14].iter().any(|l| l.contains("Basic") || l.contains("│")));
        // Too short for it (under 15 rows): not drawn, and not missed.
        let (lines, _) = screen("<F9>", 14, 80);
        assert!(!lines.join("\n").contains("Basic"));
        let (lines, _) = screen("<F9>", 15, 80);
        assert!(lines.join("\n").contains("Basic │ Sci"));
    }

    #[test]
    fn settings_value_list_under_its_row() {
        // Enter on the local currency: its list under the row, `auto` first; typing filters it.
        let (lines, cursor) = screen("<F10><Down>\n", 24, 100);
        let text = lines.join("\n");
        assert!(text.contains("│ auto") && text.contains("│ USD  US dollar") && cursor.is_none(), "{text}");
        let text = screen("<F10><Down>\ndir", 24, 100).0.join("\n");
        assert!(text.contains("│ dir▏") && text.contains("AED  UAE dirham") && !text.contains("USD  US dollar"), "{text}");
        assert!(screen("<F10><Down>\nzzz", 24, 100).0.join("\n").contains("no match"));
        // A choice's list: its values, `auto` as it stands on this device.
        let text = screen("<F10><Down><Down>\n", 24, 100).0.join("\n");
        assert!(text.contains("│ auto (") && text.contains("│ km ") && text.contains("│ mi "), "{text}");
    }

    #[test]
    fn settings_list_the_shortcuts() {
        let (lines, _) = screen("<F10>a", 40, 100);
        let text = lines.join("\n");
        assert!(text.contains("tui.favourite_units") && text.contains("tui.favourite_currencies"), "{text}");
        assert!(text.contains("(none yet: the defaults)"), "{text}");
    }

    /// Types `keys`, then clicks (left button) on the first cell of each text in `texts`, in
    /// order, as drawn at that moment (a click that changes the screen is followed by one on the
    /// new screen). Returns the app.
    fn clicked<'a>(config: &'a Config, opts: &'a Options<'a>, keys: &str, size: (usize, usize), texts: &[&str]) -> App<'a> {
        let mut app = app_after_in(config, opts, keys, size);
        for text in texts {
            let frame = render(&app, size.0, size.1);
            let plain: Vec<String> = frame.lines.iter().map(|l| plain(l)).collect();
            // `last:text` is the last line with it (a panel under the history that shows the same).
            let (last, text) = text.strip_prefix("last:").map_or((false, *text), |t| (true, t));
            let mut found = plain.iter().enumerate().filter(|(_, l)| l.contains(text));
            let (row, line) = if last { found.next_back() } else { found.next() }.unwrap_or_else(|| panic!("no `{text}` on screen:\n{}", plain.join("\n")));
            // Columns, not characters: the toolbar's 👕👟 take two each.
            let col = columns(&line[..line.find(text).unwrap()]);
            app.hits = frame.hits;
            let mouse = |kind| Key::Mouse(super::super::keys::Mouse { kind, row, col });
            app.handle(mouse(super::super::keys::MouseKind::Down(super::super::keys::Button::Left)));
            app.handle(mouse(super::super::keys::MouseKind::Up));
        }
        app
    }

    #[test]
    fn clicking_the_toolbar() {
        let config = Config::default();
        let opts = OPTS;
        let app = clicked(&config, &opts, "", (24, 100), &["F3 Currencies"]);
        assert!(matches!(app.overlay, Some(Overlay::Currencies(_))));
        // The same entry again closes it, as the key does.
        let app = clicked(&config, &opts, "", (24, 100), &["F3 Currencies", "F3 Currencies"]);
        assert!(app.overlay.is_none());
        let app = clicked(&config, &opts, "", (24, 100), &["F2 Measures"]);
        assert!(matches!(app.overlay, Some(Overlay::Measures(_))));
        // The Keys entry shows the keyboard.
        let app = clicked(&config, &opts, "", (24, 100), &["F9 Keys"]);
        assert!(app.keypad != Keypad::Hidden);
        // Short names at 61 columns are still clickable.
        let app = clicked(&config, &opts, "", (24, 61), &["F6 Triv"]);
        assert!(matches!(app.overlay, Some(Overlay::Trivia(_))));
    }

    #[test]
    fn clicking_the_keyboard() {
        let config = Config::default();
        let opts = OPTS;
        // The keys write as typed ones do, and light up.
        let app = clicked(&config, &opts, "<F9>", (24, 80), &["7    8", "×", "9    ×"]);
        assert_eq!((app.line.text().as_str(), app.pressed), ("7*9", Some('9')), "the last key is lit");
        let app = clicked(&config, &opts, "<F9>", (24, 80), &["7    8", "⌫"]);
        assert_eq!(app.line.text(), "");
        let app = clicked(&config, &opts, "<F9>", (24, 80), &["7    8", "⏎"]);
        assert_eq!((app.line.text().as_str(), app.history.entries().len()), ("", 1), "Enter evaluated it");
        // Functions write their bracket.
        let app = clicked(&config, &opts, "<F9>", (24, 80), &["sin", "√", "π"]);
        assert_eq!(app.line.text(), "sin(sqrt(pi");
        // The tabs switch the layout and take the focus; the titles of the columns too.
        let app = clicked(&config, &opts, "<F9>", (24, 80), &["Basic"]);
        assert_eq!((app.keypad, app.focus), (Keypad::Basic, Some(Part::Basic)));
        let app = clicked(&config, &opts, "<F9>", (24, 80), &["Money"]);
        assert_eq!(app.focus, Some(Part::Money));
        // The shortcuts write their entry, as Alt+7 does.
        let app = clicked(&config, &opts, "<F9>", (24, 80), &["7 EUR", "1 km"]);
        assert_eq!((app.line.text().as_str(), app.currencies.hits("EUR")), ("1 EUR km ", Some(1)));
    }

    #[test]
    fn clicking_the_history_the_answer_and_the_line() {
        let config = Config::default();
        let opts = OPTS;
        // A request in the history comes back to the line; its result is copied.
        let app = clicked(&config, &opts, "10 km mi\n", (24, 80), &["10 km mi  ="]);
        assert_eq!(app.line.text(), "10 km mi");
        let app = clicked(&config, &opts, "10 km mi\n", (24, 80), &["6.2137 mi"]);
        assert_eq!(app.copied.as_deref(), Some("6.2137 mi"));
        // The live answer is copied.
        let app = clicked(&config, &opts, "1+2", (24, 80), &["= 3"]);
        assert_eq!(app.copied.as_deref(), Some("3"));
        // A click on the line puts the cursor there.
        let app = clicked(&config, &opts, "100 km mi", (24, 80), &["> 100 km mi"]);
        assert_eq!(app.line.cursor(), 9, "on the prompt: nothing");
        let app = clicked(&config, &opts, "100 km mi", (24, 80), &["00 km mi"]);
        assert_eq!(app.line.cursor(), 1);
        // A suggestion completes the word.
        let app = clicked(&config, &opts, "5 km m", (24, 80), &["[ mi ]"]);
        assert_eq!(app.line.text(), "5 km mi", "{}", app.line.text());
    }

    #[test]
    fn clicking_the_panels() {
        let config = Config::default();
        let opts = OPTS;
        // Measures → a class → a unit (the source), a second click on a unit (the target).
        let app = clicked(&config, &opts, "10 ", (24, 100), &["F2 Measures", "Length", "km", "Imperial", " mi "]);
        assert_eq!(app.line.text(), "10 km mi ");
        // The unit keys' second tab.
        let app = clicked(&config, &opts, "<F2>L", (24, 100), &["Imperial"]);
        assert!(matches!(&app.overlay, Some(Overlay::Picker(p)) if p.tab == System::Imperial));
        // Currencies and the search: a click writes the code, like Enter.
        let app = clicked(&config, &opts, "<F3>", (24, 100), &["JPY"]);
        assert_eq!((app.line.text().as_str(), app.overlay.is_none()), ("1 JPY ", true), "{}", app.line.text());
        let app = clicked(&config, &opts, "<C-k>yen", (24, 100), &["JPY"]);
        assert_eq!(app.line.text(), "1 JPY ");
        let app = clicked(&config, &opts, "<C-k>kilomet", (24, 100), &["km       kilometre"]);
        assert_eq!(app.line.text(), "1 km ");
        // The Trivia: a section's tab, an entry's details.
        let app = clicked(&config, &opts, "<F6>", (24, 100), &["Space"]);
        assert!(matches!(&app.overlay, Some(Overlay::Trivia(v)) if v.section == 1), "second section");
        let app = clicked(&config, &opts, "<F6>", (24, 100), &["Distance between places"]);
        assert!(matches!(&app.overlay, Some(Overlay::Trivia(v)) if v.popup.is_some()), "details");
    }

    #[test]
    fn clicking_the_windows() {
        let config = Config::default();
        let opts = OPTS;
        // The settings: a click on a yes/no changes it (the file in use is not touched: offline
        // and cities are read from a copy in tests only when Enter saves, so look at the view).
        let app = clicked(&config, &opts, "", (30, 100), &["F10 ⚙"]);
        assert!(matches!(app.overlay, Some(Overlay::Settings(_))));
        // Outside a window closes it; inside it, nothing.
        let app = clicked(&config, &opts, "<F1>", (30, 100), &["Help · Esc close"]);
        assert!(matches!(app.overlay, Some(Overlay::Help(_))), "the window's own title: stays");
        let mut app = app_after_in(&config, &opts, "<F1>", (30, 100));
        let frame = render(&app, 30, 100);
        app.hits = frame.hits;
        app.handle(Key::Mouse(super::super::keys::Mouse { kind: super::super::keys::MouseKind::Down(super::super::keys::Button::Left), row: 29, col: 0 }));
        assert!(app.overlay.is_none(), "a click outside the help closes it");
        // The wheel scrolls the help.
        let mut app = app_after_in(&config, &opts, "<F1>", (14, 80));
        app.handle(Key::Mouse(super::super::keys::Mouse { kind: super::super::keys::MouseKind::ScrollDown, row: 5, col: 5 }));
        assert!(matches!(app.overlay, Some(Overlay::Help(n)) if n > 0));
        // An ambiguous date: a click on a reading chooses it, and the request goes on.
        let app = clicked(&config, &opts, "12/04/2026 YYYY/MM/DD\n", (24, 100), &["1  "]);
        assert!(app.overlay.is_none() && app.history.entries().len() == 1, "{:?}", app.history.entries().len());
    }

    #[test]
    fn the_answer_at_the_right_and_the_suggestions_as_buttons() {
        // The answer ends at the right edge, with the sign that a click copies it.
        let (lines, _) = screen("10 km mi", 12, 80);
        assert!(lines[9].ends_with("= 6.2137 mi ⧉"), "{}", lines[9]);
        // The suggestions are buttons; in a narrow window plain words.
        let (lines, _) = screen("5 km m", 12, 80);
        assert!(lines[9].contains("Tab: [ mm ] [ m ] [ mi ]"), "{}", lines[9]);
        let (lines, _) = screen("5 km m", 12, 50);
        assert!(lines[9].contains("Tab: mm m mi") && !lines[9].contains('['), "{}", lines[9]);
        // The answer is kept whole when the left side is long: it is cut instead.
        let (lines, _) = screen("(1+2+3+4+5+6+7+8)/7+(1+2+3+4+5+6)", 12, 60);
        assert!(lines[9].ends_with("⧉") && lines[9].contains("= "), "{}", lines[9]);
    }

    #[test]
    fn the_history_panel() {
        let config = Config::default();
        let opts = OPTS;
        let (lines, _) = screen("1+1\n10 km mi\n2^8\n<F5>", 20, 90);
        let text = lines.join("\n");
        assert!(text.contains("┌─ History"), "{text}");
        // Newest first, with the buttons at the right.
        let rows: Vec<&String> = lines.iter().filter(|l| l.contains(" = ")).collect();
        let panel: Vec<&String> = lines.iter().skip_while(|l| !l.contains("┌─ History")).filter(|l| l.contains("= ")).collect();
        assert!(panel[0].contains("2^8") && panel[1].contains("10 km mi") && panel[2].contains("1+1"), "{rows:?}");
        assert!(panel[0].contains("↩") && panel[0].contains("⧉") && panel[0].contains("✕"), "{}", panel[0]);
        // Typing filters.
        let (lines, _) = screen("1+1\n10 km mi\n2^8\n<F5>km", 20, 90);
        let panel: Vec<&String> = lines.iter().skip_while(|l| !l.contains("┌─ History")).filter(|l| l.contains("= ")).collect();
        assert_eq!(panel.len(), 1, "{panel:?}");
        // Clicks: a row recalls; the result and ⧉ copy; ✕ removes.
        let app = clicked(&config, &opts, "1+1\n10 km mi\n<F5>", (20, 90), &["last: 10 km mi"]);
        assert!(app.overlay.is_none() && app.line.text() == "10 km mi", "{}", app.line.text());
        let app = clicked(&config, &opts, "1+1\n10 km mi\n<F5>", (20, 90), &["last:= 6.2137 mi"]);
        assert_eq!(app.copied.as_deref(), Some("6.2137 mi"));
        let app = clicked(&config, &opts, "1+1\n10 km mi\n<F5>", (20, 90), &["⧉"]);
        assert_eq!(app.copied.as_deref(), Some("6.2137 mi"));
        let app = clicked(&config, &opts, "1+1\n10 km mi\n<F5>", (20, 90), &["✕"]);
        assert_eq!(app.history.entries().len(), 1);
        assert!(app.overlay.is_some(), "the panel stays for the next one");
        // Empty: it says so.
        let (lines, _) = screen("<F5>", 20, 90);
        assert!(lines.join("\n").contains("nothing evaluated yet"));
    }

    #[test]
    fn wide_has_a_pane_at_the_right() {
        // 140 columns: the pane is 56 wide, the separator before it.
        let (lines, _) = screen("10 km mi\n5 kg lb\n<F2>", 30, 140);
        let separator = 140 - 56 - 1;
        for line in &lines[2..28] {
            assert_eq!(line.chars().nth(separator), Some(' '), "a panel has its own border: {line}");
            assert_eq!(line.chars().nth(separator + 1), Some('│'));
        }
        // The history and the request keep the left, whole.
        assert!(lines.iter().any(|l| l.starts_with("    10 km mi  = 6.2137 mi")), "{}", lines.join("\n"));
        assert!(lines.iter().any(|l| l.starts_with("  > ") && !l.contains("Measures")));
        // The Measures are in the pane, from the top to the status line.
        assert!(lines[1].chars().skip(separator + 1).collect::<String>().starts_with("┌─ Measures"), "{}", lines[1]);
        assert!(lines[28].chars().skip(separator + 1).collect::<String>().starts_with("└"), "{}", lines[28]);
        // A click in the pane is the pane's: a class opens its units.
        let config = Config::default();
        let opts = OPTS;
        let app = clicked(&config, &opts, "<F2>", (30, 140), &["Length"]);
        assert!(matches!(app.overlay, Some(Overlay::Picker(_))));
        // Under 120 columns it is the band under the line, as before.
        let (lines, _) = screen("<F2>", 30, 119);
        assert!(lines[2..20].iter().all(|l| l.chars().nth(58) != Some('│')));
        // The keyboard has a divider.
        let (lines, _) = screen("<F9>", 30, 140);
        assert!(lines[2..28].iter().all(|l| l.chars().nth(separator) == Some('│')));
        // The keyboard in the pane: Basic at 140 columns (the pane has no room for Sci and the
        // shortcuts), Sci at 200.
        let (lines, _) = screen("<F9>", 30, 140);
        assert!(lines.iter().any(|l| l.contains("Basic │ Sci")) && !lines.iter().any(|l| l.contains("sin  cos")), "{}", lines.join("\n"));
        let (lines, _) = screen("<F9>", 44, 200);
        assert!(lines.iter().any(|l| l.contains("│ sin │")), "the big keyboard, Sci");
        // Clicks on the keys count the pane's offset.
        let app = clicked(&config, &opts, "<F9>", (30, 140), &["7    8", "×"]);
        assert_eq!(app.line.text(), "7*");
    }

    #[test]
    fn compact_keeps_what_matters() {
        // The smallest terminal: 40 columns, 12 rows. Every line fits; the answer is whole and at
        // the right edge; no keyboard (too short for it).
        let (lines, _) = screen("10 km mi", 12, 40);
        assert!(lines.iter().all(|l| l.chars().count() <= 40), "{}", lines.join("\n"));
        assert!(lines[0].starts_with(" F2  F3  F4  F5  F6  F9  F10"), "keys only: {}", lines[0]);
        assert!(lines.iter().any(|l| l.ends_with("= 6.2137 mi ⧉")), "{}", lines.join("\n"));
        let (lines, _) = screen("<F9>", 12, 40);
        assert!(!lines.join("\n").contains("Basic"));
        // 50 columns, 20 rows: the keyboard in the band, without the shortcuts (no room).
        let (lines, _) = screen("<F9>", 20, 50);
        assert!(lines.iter().any(|l| l.contains("7    8    9")), "{}", lines.join("\n"));
        assert!(lines.iter().all(|l| l.chars().count() <= 50));
        // From 61 columns the toolbar has short names.
        let (lines, _) = screen("", 20, 61);
        assert!(lines[0].starts_with(" F2 Meas  F3 Curr  F4 👕👟  F5 Hist  F6 Triv  F9 Keys  F10 ⚙"), "{}", lines[0]);
    }

    #[test]
    fn the_board_fills_the_wide_pane() {
        let config = Config::default();
        let opts = OPTS;
        let (lines, _) = screen("12 sqft m", 44, 140);
        let text = lines.join("\n");
        assert!(text.contains("Clocks") && text.contains("Measures") && text.contains("Currencies"), "{text}");
        // Every class with its units, in boxes three rows high (the most common first).
        assert!(text.contains("L Length") && text.contains("│ km │ │ m  │ │ cm │ │ mi │ │ ft │"), "{text}");
        assert!(text.find("Currencies").unwrap() < text.find("Length").unwrap(), "the currencies lead");
        assert!(text.contains("┌─USD") && text.contains("┌─EUR"), "{text}");
        // A short screen: one row per class, in brackets, and what does not fit is said.
        let (lines, _) = screen("12 sqft m", 16, 140);
        assert!(lines.iter().any(|l| l.contains("[ km ]") && l.contains("Length")), "{}", lines.join("\n"));
        assert!(lines.iter().any(|l| l.contains("▼") && l.contains("more")), "{}", lines.join("\n"));
        // A click on a unit writes it and counts it; the class name opens its units.
        let app = clicked(&config, &opts, "5 ", (44, 140), &["│ km │"]);
        assert_eq!(app.line.text(), "5 km ");
        assert_eq!(app.usage.hits(aequalis::usage::Kind::Unit, "km"), 1);
        let app = clicked(&config, &opts, "", (44, 140), &["M Mass"]);
        assert!(matches!(&app.overlay, Some(Overlay::Picker(_))));
        // A currency tile writes its code.
        let app = clicked(&config, &opts, "10 ", (44, 140), &["┌─JPY"]);
        assert_eq!(app.line.text(), "10 JPY ");
        // A clock writes its city into the request.
        let app = clicked(&config, &opts, "", (44, 140), &["New York"]);
        assert_eq!(app.line.text(), "now New York ");
        // What is used comes first and is lit: after using the pound twice it is the first tile.
        let mut app = app_after_in(&config, &opts, "", (44, 140));
        app.usage.hit(aequalis::usage::Kind::Currency, "GBP");
        app.usage.hit(aequalis::usage::Kind::Currency, "GBP");
        let frame = render(&app, 44, 140);
        let text = frame.lines.iter().map(|l| plain(l)).collect::<Vec<_>>().join("\n");
        assert!(text.contains("┌─GBP─────┐ ┌─USD"), "{text}");
    }

    /// A mouse event at the first cell of the first line with `text`.
    fn mouse_at(app: &mut App, size: (usize, usize), text: &str, kind: super::super::keys::MouseKind) {
        let frame = render(app, size.0, size.1);
        let plain: Vec<String> = frame.lines.iter().map(|l| plain(l)).collect();
        let (row, line) = plain.iter().enumerate().find(|(_, l)| l.contains(text)).unwrap_or_else(|| panic!("no `{text}`:\n{}", plain.join("\n")));
        let col = line[..line.find(text).unwrap()].chars().count();
        app.hits = frame.hits;
        app.handle(Key::Mouse(super::super::keys::Mouse { kind, row, col }));
    }

    #[test]
    fn the_board_has_buttons_for_measures_and_currencies() {
        use super::super::keys::{Button, MouseKind};
        let config = Config::default();
        let opts = OPTS;
        let size = (30, 140);
        let mut app = app_after_in(&config, &opts, "", size);
        assert_eq!(app.board_scroll, 0);
        mouse_at(&mut app, size, "[ Measures ]", MouseKind::Down(Button::Left));
        mouse_at(&mut app, size, "[ Measures ]", MouseKind::Up);
        assert_eq!(app.board_scroll, 1, "the first class");
        mouse_at(&mut app, size, "[ Currencies ]", MouseKind::Down(Button::Left));
        mouse_at(&mut app, size, "[ Currencies ]", MouseKind::Up);
        assert_eq!(app.board_scroll, 0);
        // The keyboard does the same: Alt+L moves between the two.
        app.handle(Key::Alt('l'));
        assert_eq!(app.board_scroll, 1);
        app.handle(Key::Alt('l'));
        assert_eq!(app.board_scroll, 0);
    }

    #[test]
    fn the_board_scrolls_like_a_phone() {
        use super::super::keys::{Button, MouseKind};
        let config = Config::default();
        let opts = OPTS;
        let size = (30, 140);
        // It starts with the currencies, then the classes; the rest is below, said at the bottom.
        let mut app = app_after_in(&config, &opts, "", size);
        let text = |app: &App| render(app, size.0, size.1).lines.iter().map(|l| plain(l)).collect::<Vec<_>>().join("\n");
        let first = text(&app);
        assert!(first.contains("Currencies") && first.contains("▼") && first.contains("more"), "{first}");
        assert!(!first.contains("W Power"), "{first}");
        // The wheel over the board moves the list one entry; back up it returns.
        app.handle(Key::Mouse(super::super::keys::Mouse { kind: MouseKind::ScrollDown, row: 10, col: 120 }));
        assert_eq!(app.board_scroll, 1);
        let moved = text(&app);
        assert!(!moved.contains("┌─USD") && moved.contains("▲ 1"), "{moved}");
        app.handle(Key::Mouse(super::super::keys::Mouse { kind: MouseKind::ScrollUp, row: 10, col: 120 }));
        assert_eq!(app.board_scroll, 0);
        // The wheel over the history is the history's, not the board's.
        app.handle(Key::Mouse(super::super::keys::Mouse { kind: MouseKind::ScrollDown, row: 10, col: 20 }));
        assert_eq!(app.board_scroll, 0);
        // A finger: press, move up by six rows, release: two entries on, and nothing pressed.
        let mouse = |kind, row| Key::Mouse(super::super::keys::Mouse { kind, row, col: 125 });
        app.handle(mouse(MouseKind::Down(Button::Left), 20));
        app.handle(mouse(MouseKind::Drag(Button::Left), 17));
        app.handle(mouse(MouseKind::Drag(Button::Left), 14));
        app.handle(mouse(MouseKind::Up, 14));
        assert_eq!((app.board_scroll, app.line.text().as_str()), (2, ""), "a drag is not a click");
        // A tap (press and release in place) is a click.
        mouse_at(&mut app, size, "│ km │", MouseKind::Down(Button::Left));
        assert!(app.line.is_empty(), "written when released");
        mouse_at(&mut app, size, "│ km │", MouseKind::Up);
        assert_eq!(app.line.text(), "1 km ");
        // The marker at the bottom moves a page on; the one at the top, back.
        let mut app = app_after_in(&config, &opts, "", size);
        mouse_at(&mut app, size, "▼", MouseKind::Down(Button::Left));
        mouse_at(&mut app, size, "▼", MouseKind::Up);
        assert!(app.board_scroll > 1, "{}", app.board_scroll);
    }

    #[test]
    fn the_big_keyboard_beside_the_board() {
        let config = Config::default();
        let opts = OPTS;
        // 170 columns: the board (52) and the keyboard beside it; Basic keys only.
        let (lines, _) = screen("<F9>", 44, 170);
        let text = lines.join("\n");
        assert!(text.contains("│  7  │") && text.contains("│ to  │") && text.contains("│  C  │") && text.contains("Clocks"), "{text}");
        assert!(!text.contains("│ sin │"), "no room for the Sci keys: {text}");
        // 200 columns: the Sci keys too.
        let (lines, _) = screen("<F9>", 44, 200);
        assert!(lines.join("\n").contains("│ sin │"));
        // Clicks: the keys write, `to` writes a conversion, C clears, the arrows move.
        let app = clicked(&config, &opts, "<F9>", (44, 170), &["│  7  │", "│  8  │", "│ to  │"]);
        assert_eq!(app.line.text(), "78 to ");
        let app = clicked(&config, &opts, "<F9>7", (44, 170), &["│  C  │"]);
        assert!(app.line.is_empty());
        let app = clicked(&config, &opts, "<F9>12", (44, 170), &["│  ←  │", "│  9  │"]);
        assert_eq!(app.line.text(), "192");
        // Short of rows for it (under 17): the compact keyboard.
        let (lines, _) = screen("<F9>", 16, 170);
        assert!(lines.iter().any(|l| l.contains("7    8    9")), "{}", lines.join("\n"));
    }

    #[test]
    fn the_mouse_can_be_given_back_to_the_terminal() {
        let config = Config::default();
        let opts = OPTS;
        let mut app = app_after(&config, &opts, "");
        assert!(app.mouse_on);
        app.handle(Key::Alt('o'));
        assert!(!app.mouse_on && app.status.as_ref().is_some_and(|(_, m)| m.contains("mouse off")));
        app.handle(Key::Alt('o'));
        assert!(app.mouse_on);
    }

    #[test]
    fn the_keyboard_has_no_completion() {
        // With the keyboard shown Tab goes to it: no "Tab: …" suggestions, and the status says so.
        let (lines, _) = screen("5 km m", 24, 80);
        assert!(lines.join("\n").contains("Tab:"), "completion with the keyboard hidden");
        let (lines, _) = screen("5 km m<F9><Esc>", 24, 80);
        assert!(!lines.join("\n").contains("Tab:"), "{}", lines.join("\n"));
        assert!(lines[23].contains("Tab keyboard"), "{}", lines[23]);
        let (lines, _) = screen("5 km m<F9>", 24, 80);
        assert!(lines[23].starts_with(" KEYS ") && lines[23].contains("<Esc> back"), "{}", lines[23]);
        // The focus is drawn: Units reversed once Tab reaches it.
        let config = Config::default();
        let opts = OPTS;
        let app = app_after(&config, &opts, "<F9>\t");
        let frame = render(&app, 24, 80);
        assert!(frame.lines[17].contains("\x1b[7m Units "), "{:?}", frame.lines[17]);
    }

    #[test]
    fn shortcuts_beside_the_keypad() {
        // At the right of the keypad, when the keypad shows: the units (Alt+1…5) and the
        // currencies (Alt+6…0) with their digits, on the rule's row their titles.
        let (lines, _) = screen("<F9>", 24, 80);
        assert!(lines[17].contains("Units") && lines[17].contains("Money"), "{}", lines[17]);
        for (row, units, money) in [(18, "1 km", "6 USD"), (19, "2 kg", "7 EUR"), (20, "3 L", "8 JPY"), (21, "4 °C", "9 CNY"), (22, "5 m", "0 AED")] {
            assert!(lines[row].contains(units) && lines[row].contains(money), "{}", lines[row]);
            assert!(lines[row].trim_end().chars().count() <= 80);
        }
        // Wide, with the board beside it: the big keyboard has no shortcut columns (the board has the
        // units and the currencies).
        let (lines, _) = screen("<F9>", 44, 200);
        assert!(!lines.join("\n").contains(" Units "), "{}", lines.join("\n"));
        // Used ones follow the use: Egyptian pounds (picked in the search) replace the last with no hits.
        let (lines, _) = screen("<C-k>egyptian\n<F9>", 24, 80);
        assert!(lines[22].contains("0 EGP") && !lines.join("\n").contains("AED"), "{}", lines[22]);
        // Narrow: the keypad first, then no shortcuts.
        let (lines, _) = screen("<F9>", 24, 44);
        assert!(!lines.join("\n").contains("Units") && lines[19].contains("7    8    9"), "{}", lines[19]);
        let (lines, _) = screen("<F9>\t", 24, 56);
        assert!(!lines.join("\n").contains("Units") && lines[19].contains("tan"), "Sci needs more room");
        let (lines, _) = screen("<F9>", 24, 80);
        assert!(lines[19].contains("2 kg"), "{}", lines[19]);
        // Not in the band when a window is open, or without the keypad.
        let (lines, _) = screen("", 24, 80);
        assert!(!lines.join("\n").contains("Units"));
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
        app.keypad = Keypad::Basic;
        // The lit key, the lit tab of the keypad (Basic), and the toolbar's Keys.
        let lit = |frame: &Frame| frame.lines.iter().map(|l| l.matches("\x1b[1;30;43m").count()).sum::<usize>();
        for digit in "0123456789".chars() {
            app.handle(Key::Char(digit));
            let frame = render(&app, 24, 80);
            let with_digit: Vec<_> = frame.lines.iter().filter(|l| l.contains(&format!("\x1b[1;30;43m  {digit}  "))).collect();
            assert_eq!(with_digit.len(), 1, "{digit} lit");
            assert_eq!(lit(&frame), 3, "only {digit}, the tab and the toolbar's Keys lit");
        }
        // The operators, as typed, and Backspace and Enter.
        for (typed, key) in [('x', '×'), ('/', '÷'), ('-', '−'), ('+', '+'), ('(', '(')] {
            app.handle(Key::Char(typed));
            let frame = render(&app, 24, 80);
            assert!(frame.lines.iter().any(|l| l.contains(&format!("\x1b[1;30;43m  {key}  "))), "{typed} lights {key}");
        }
        app.handle(Key::Backspace);
        assert!(render(&app, 24, 80).lines.iter().any(|l| l.contains("\x1b[1;30;43m  ⌫  ")));
    }

    #[test]
    fn errors_and_preview() {
        let (lines, _) = screen("10 km kg\n10 km m", 12, 100);
        let text = lines.join("\n");
        assert!(text.contains("10 km kg  ✗ cannot convert `km` to `kg`: different kinds of unit"));
        assert!(lines[9].contains("= 10000 m"), "{}", lines[9]);
        assert!(lines[11].starts_with(" error: "), "{}", lines[11]);
    }

    #[test]
    fn everything_fits() {
        let long = "1+".repeat(60) + "1";
        for (rows, cols) in [(12, 40), (24, 80), (50, 200)] {
            for keys in [long.as_str(), "10 km mi\n<F2>L", "<F2>Lkilometre", "<F1>", "1\n2\n3\n4\n5\n6\n7\n8\n9\n"] {
                let (lines, cursor) = screen(keys, rows, cols);
                assert_eq!(lines.len(), rows);
                assert!(lines.iter().all(|l| l.chars().count() <= cols), "{keys:?} at {rows}x{cols}");
                if let Some((r, c)) = cursor {
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
                    // Columns, not characters: 👕 takes two.
                    columns(&out)
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
