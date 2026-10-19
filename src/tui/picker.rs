//! The unit picker: the units of one category (opened from the Measures list), split into two
//! tabs (metric / imperial, decimal / binary for bits and bytes, durations / formats for dates);
//! and placing the chosen unit into the expression as its source or target unit.

use aequalis::calc;
use aequalis::config::{Category, Config, System, Unit};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

pub struct Picker {
    /// Index into `Config::categories`: the category shown.
    pub category: usize,
    /// The tab shown: `Metric` (the first) or `Imperial` (the second).
    pub tab: System,
    /// Index into `items()`.
    pub selected: usize,
    /// On the Formats tab: (row, column) in `format_rows()`.
    pub cell: (usize, usize),
}

/// A row of the Time category's Formats tab: a label and its formats (short label, format word).
pub struct FormatRow {
    pub label: &'static str,
    pub cells: Vec<(&'static str, String)>,
}

/// The Formats tab: every date format, by family; the patterns with their time variants.
pub fn format_rows() -> Vec<FormatRow> {
    let row = |label, cells: &[(&'static str, &str)]| FormatRow { label, cells: cells.iter().map(|&(l, w)| (l, w.to_string())).collect() };
    let mut rows = vec![
        row("ISO", &[("iso", "iso"), ("utc", "iso-utc"), ("basic", "iso-basic"), ("date", "iso-date"), ("time", "iso-time")]),
        row("", &[("ordinal", "iso-ordinal"), ("week", "iso-week")]),
        row("Unix/local", &[("s", "unix"), ("ms", "unix-ms"), ("µs", "unix-us"), ("ns", "unix-ns"), ("local", "local")]),
    ];
    let labels = ["date", "24h", "24h tz", "12h", "12h tz"];
    for (name, _) in aequalis::time::PATTERNS {
        let cells = aequalis::time::VARIANTS.iter().zip(labels).map(|((suffix, ..), label)| (label, format!("{name}{suffix}"))).collect();
        rows.push(FormatRow { label: name, cells });
    }
    rows
}

impl Picker {
    /// Metric / imperial categories open on `device`'s system; the others (Data: decimal /
    /// binary, Time: short / long) on their first tab.
    pub fn new(config: &Config, category: usize, device: System) -> Self {
        let metric_imperial = config.categories[category].systems == ["Metric", "Imperial"];
        let tab = if metric_imperial { device } else { System::Metric };
        Picker { category, tab, selected: 0, cell: (0, 0) }
    }

    /// Tab / Shift-Tab: the other tab (a category has two: next and previous are the same).
    pub fn switch_tab(&mut self) {
        self.selected = 0;
        self.cell = (0, 0);
        self.tab = if self.tab == System::Imperial { System::Metric } else { System::Imperial };
    }

    /// The window's title: the category's name.
    pub fn title<'a>(&self, config: &'a Config) -> &'a str {
        self.category(config).name
    }

    /// The tabs' names, and whether each is the one shown.
    pub fn tabs<'a>(&self, config: &'a Config) -> Vec<(&'a str, bool)> {
        let systems = self.category(config).systems;
        vec![(systems[0], self.tab != System::Imperial), (systems[1], self.tab == System::Imperial)]
    }

    /// All the units of `tab` (before searching).
    pub fn units_of<'a>(&self, config: &'a Config, tab: System) -> impl Iterator<Item = &'a Unit> {
        self.category(config).units.iter().filter(move |u| u.system == tab || u.system == System::Both)
    }

    pub fn category<'a>(&self, config: &'a Config) -> &'a Category {
        &config.categories[self.category]
    }

    /// Unit buttons per row: the category's `columns`.
    pub fn columns(&self, config: &Config) -> usize {
        self.category(config).columns
    }

    /// The units of the tab shown, in table order.
    pub fn items<'a>(&self, config: &'a Config) -> Vec<&'a Unit> {
        self.units_of(config, self.tab).collect()
    }

    /// How many of `items()` each row of the table holds: rows of the category's `columns`.
    pub fn rows(&self, config: &Config) -> Vec<usize> {
        let (len, cols) = (self.items(config).len(), self.columns(config));
        (0..len.div_ceil(cols)).map(|r| cols.min(len - r * cols)).filter(|&n| n > 0).collect()
    }

    /// The (row, column) of item `i` in `rows`.
    pub fn position(rows: &[usize], i: usize) -> (usize, usize) {
        let mut start = 0;
        for (r, &n) in rows.iter().enumerate() {
            if i < start + n {
                return (r, i - start);
            }
            start += n;
        }
        (0, 0)
    }

    /// Whether the tab shown is Time's Formats tab (formats instead of units).
    pub fn shows_formats(&self, config: &Config) -> bool {
        self.category(config).name == aequalis::config::TIME && self.tab == System::Imperial
    }

    /// The format word selected on the Formats tab.
    pub fn selected_format(&self) -> Option<String> {
        format_rows().get(self.cell.0)?.cells.get(self.cell.1).map(|(_, word)| word.clone())
    }

    /// One step in a direction, circular: past the last unit of a row (or the list) back to the
    /// first, past the last row back to the first row (same column), and the other way round.
    pub fn step(&mut self, config: &Config, direction: Direction) {
        if self.shows_formats(config) {
            return self.step_cell(direction);
        }
        let rows = self.rows(config);
        let len: usize = rows.iter().sum();
        if len == 0 {
            return;
        }
        let s = self.selected;
        let (r, c) = Self::position(&rows, s);
        // Up and down keep the column, or go to the last button of a shorter row.
        let to_row = |r: usize| rows[..r].iter().sum::<usize>() + c.min(rows[r] - 1);
        self.selected = match direction {
            Direction::Left => (s + len - 1) % len,
            Direction::Right => (s + 1) % len,
            Direction::Down => to_row((r + 1) % rows.len()),
            Direction::Up => to_row((r + rows.len() - 1) % rows.len()),
        };
    }
}

impl Picker {
    /// On the Formats tab: left / right through every format in reading order, up / down a row
    /// (the column kept where the row has it); all circular.
    fn step_cell(&mut self, direction: Direction) {
        let rows = format_rows();
        let (r, c) = self.cell;
        let flat: Vec<(usize, usize)> = rows.iter().enumerate().flat_map(|(r, row)| (0..row.cells.len()).map(move |c| (r, c))).collect();
        let i = flat.iter().position(|&x| x == (r, c)).unwrap_or(0);
        self.cell = match direction {
            Direction::Left => flat[(i + flat.len() - 1) % flat.len()],
            Direction::Right => flat[(i + 1) % flat.len()],
            Direction::Down | Direction::Up => {
                let next = if direction == Direction::Down { (r + 1) % rows.len() } else { (r + rows.len() - 1) % rows.len() };
                (next, c.min(rows[next].cells.len() - 1))
            }
        };
    }
}

/// Puts the output format `word` at the end of the expression: it replaces a format already there
/// (`now unix` → `now iso-week`), else it is added (`1775944800 unix` → `1775944800 unix iso-week`,
/// the first one being the input format). An empty line becomes `now <word>`.
pub fn place_format(line: &str, word: &str) -> String {
    let line = line.trim_end();
    if line.trim().is_empty() {
        return format!("now {word}");
    }
    match line.rsplit_once(' ') {
        Some((head, last)) if aequalis::time::is_format(last) && aequalis::time::is_date_value(head) => format!("{head} {word}"),
        _ => format!("{line} {word}"),
    }
}

/// Puts `unit` into the expression `line`. If the cursor is on the source or target unit, that
/// unit is replaced. Otherwise the first empty slot is filled (source, then target), and with
/// both set the target is replaced. An empty line becomes `1 <unit>`. When the unit ends the
/// expression, a space follows it, ready for the next unit.
/// Returns the new line; the cursor goes to its end, so the next pick fills the next slot.
pub fn place_unit(line: &str, cursor: usize, unit: &str) -> String {
    let placed = place(line, cursor, unit);
    match placed.split_whitespace().last() == Some(unit) {
        true => format!("{} ", placed.trim_end()),
        false => placed,
    }
}

/// Whether the expression has its source and target units: it is complete.
pub fn has_target(line: &str) -> bool {
    // A date request is complete too: `2026/12/24 - 12 d` (the unit was the duration's; bug 007).
    calc::parse_prefix(line).is_ok_and(|(_, rest)| rest.split_whitespace().count() >= 2) || aequalis::time::is_date_request(line)
}

fn place(line: &str, cursor: usize, unit: &str) -> String {
    if line.trim().is_empty() {
        return format!("1 {unit}");
    }
    let Ok((_, rest)) = calc::parse_prefix(line) else {
        return format!("{} {unit}", line.trim_end());
    };
    let rest_start = line.len() - rest.len();
    // Byte ranges of the unit words in `line`.
    let words: Vec<(usize, usize)> = rest
        .split_whitespace()
        .map(|w| {
            let start = rest_start + (w.as_ptr() as usize - rest.as_ptr() as usize);
            (start, start + w.len())
        })
        .collect();
    let byte_cursor = line.char_indices().nth(cursor).map_or(line.len(), |(i, _)| i);
    let on = |&(start, end): &(usize, usize)| start <= byte_cursor && byte_cursor < end;
    let replace = |(start, end): (usize, usize)| format!("{}{unit}{}", &line[..start], &line[end..]);
    match words[..] {
        [from, ..] if on(&from) => replace(from),
        [_, to, ..] if on(&to) => replace(to),
        [] | [_] => format!("{} {unit}", line.trim_end()),
        [_, to, ..] => replace(to),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picker(category: &str, tab: System) -> (Config, Picker) {
        let config = Config::default();
        let index = config.categories.iter().position(|c| c.name == category).unwrap();
        let mut p = Picker::new(&config, index, System::Metric);
        if p.tab != tab {
            p.switch_tab();
        }
        (config, p)
    }

    fn symbols_in(category: &str, tab: System) -> Vec<&'static str> {
        let (config, p) = picker(category, tab);
        p.items(&config).iter().map(|u| u.symbol).collect()
    }

    #[test]
    fn tabs() {
        use System::*;
        assert_eq!(symbols_in("Length", Metric), ["mm", "cm", "m", "km", "au", "ly", "pc"]);
        assert_eq!(symbols_in("Length", Imperial), ["in", "ft", "yd", "mi"]);
        assert_eq!(symbols_in("Temperature", Imperial), ["F"]);
        assert_eq!(symbols_in("Volume", Imperial)[..3], ["floz-us", "pt-us", "qt-us"]);
        assert_eq!(symbols_in("Speed", Imperial), ["mph", "ft/s", "kn"]);
        // Data: decimal and binary; b and B in both.
        // Bytes first, then bits.
        assert_eq!(symbols_in(aequalis::config::BITS, Metric), ["B", "kB", "MB", "GB", "TB", "b", "kb", "Mb", "Gb", "Tb"]);
        assert_eq!(symbols_in(aequalis::config::BITS, Imperial), ["B", "KiB", "MiB", "GiB", "TiB", "b", "Kib", "Mib", "Gib", "Tib"]);
        let (config, p) = picker(aequalis::config::BITS, Imperial);
        assert_eq!((p.tab, p.category(&config).systems), (Imperial, ["Decimal", "Binary"]));
    }

    #[test]
    fn opens_on_the_device_system_except_data() {
        let config = Config::default();
        let index = |name: &str| config.categories.iter().position(|c| c.name == name).unwrap();
        assert_eq!(Picker::new(&config, index("Length"), System::Imperial).tab, System::Imperial);
        assert_eq!(Picker::new(&config, index("Length"), System::Metric).tab, System::Metric);
        assert_eq!(Picker::new(&config, index(aequalis::config::BITS), System::Imperial).tab, System::Metric, "decimal");
    }

    #[test]
    fn tabs_are_circular() {
        let (_, mut p) = picker(aequalis::config::BITS, System::Metric);
        p.switch_tab();
        assert_eq!(p.tab, System::Imperial);
        p.switch_tab();
        assert_eq!(p.tab, System::Metric);
        p.switch_tab();
        assert_eq!(p.tab, System::Imperial);
    }

    #[test]
    fn selection_moves_within_the_list() {
        let config = Config::default();
        let mut p = Picker::new(&config, 0, System::Metric);
        p.selected = 2;
        p.switch_tab();
        assert_eq!(p.selected, 0, "a new tab starts from its first unit");
    }

    #[test]
    fn steps_are_circular() {
        use Direction::*;
        // Data, Decimal: B kB MB GB TB / b kb Mb Gb Tb (5 per row).
        let (config, mut p) = picker(aequalis::config::BITS, System::Metric);
        let at = |p: &mut Picker, d| {
            p.step(&config, d);
            p.items(&config)[p.selected].symbol
        };
        assert_eq!(at(&mut p, Left), "Tb", "left of the first: the last");
        assert_eq!(at(&mut p, Right), "B", "right of the last: the first");
        assert_eq!(at(&mut p, Down), "b");
        assert_eq!(at(&mut p, Down), "B", "below the last row: the first row");
        assert_eq!(at(&mut p, Up), "b", "above the first row: the last row");
        // Pressure, Metric: Pa hPa kPa MPa / mbar bar atm mmHg / Torr: the last row is shorter.
        let (config, mut p) = picker("Pressure", System::Metric);
        p.selected = 7;
        p.step(&config, Down);
        assert_eq!(p.items(&config)[p.selected].symbol, "Torr", "the last button of the shorter row");
        p.step(&config, Down);
        assert_eq!(p.items(&config)[p.selected].symbol, "Pa", "column 0 kept from there");
    }

    #[test]
    fn fills_the_next_empty_slot() {
        // A space follows the unit, ready for the next one.
        assert_eq!(place_unit("", 0, "km"), "1 km ");
        assert_eq!(place_unit("10", 2, "km"), "10 km ");
        assert_eq!(place_unit("10 km ", 6, "mi"), "10 km mi ");
        assert_eq!(place_unit("10 km", 5, "mi"), "10 km mi ");
        assert_eq!(place_unit("100 + 20 ", 9, "sqft"), "100 + 20 sqft ");
        assert_eq!(place_unit("(1+2)/3 km", 10, "mi"), "(1+2)/3 km mi ");
        // Both set: the target is replaced.
        assert_eq!(place_unit("10 km mi ", 9, "ft"), "10 km ft ");
    }

    #[test]
    fn replaces_the_unit_under_the_cursor() {
        // Inside the expression: no space added.
        assert_eq!(place_unit("10 km mi", 3, "m"), "10 m mi");
        assert_eq!(place_unit("10 km mi", 4, "m"), "10 m mi");
        assert_eq!(place_unit("10km mi", 2, "m"), "10m mi");
        // At the end: the space follows.
        assert_eq!(place_unit("10 km mi", 6, "ft"), "10 km ft ");
        // On the value: the next empty slot.
        assert_eq!(place_unit("10 km", 0, "mi"), "10 km mi ");
    }

    #[test]
    fn date_formats_tab() {
        let rows = format_rows();
        assert_eq!(rows.len(), 9);
        assert_eq!(rows[0].cells[1].1, "iso-utc");
        assert_eq!(rows[2].cells[4].1, "local");
        assert_eq!(rows[3].label, "YYYY/MM/DD");
        assert_eq!(
            rows[5].cells.iter().map(|c| c.1.as_str()).collect::<Vec<_>>(),
            ["DD/MM/YY", "DD/MM/YY-24h", "DD/MM/YY-24h-tz", "DD/MM/YY-12h", "DD/MM/YY-12h-tz"]
        );
        // Every cell is a format the CLI knows.
        assert!(rows.iter().flat_map(|r| &r.cells).all(|(_, w)| aequalis::time::is_format(w)));
        assert_eq!(rows.iter().map(|r| r.cells.len()).sum::<usize>(), 7 + 5 + 30);
        let (config, mut p) = picker(aequalis::config::TIME, System::Metric);
        assert!(!p.shows_formats(&config));
        p.switch_tab();
        assert!(p.shows_formats(&config));
        assert_eq!(p.selected_format().as_deref(), Some("iso"));
        use Direction::*;
        p.step(&config, Left);
        assert_eq!(p.selected_format().as_deref(), Some("MM/DD/YY-12h-tz"), "circular: before the first, the last");
        p.step(&config, Right);
        p.step(&config, Up);
        assert_eq!(p.selected_format().as_deref(), Some("MM/DD/YY"), "above the first row: the last");
        for _ in 0..6 {
            p.step(&config, Up);
        }
        assert_eq!(p.selected_format().as_deref(), Some("unix"), "column kept: the Unix row's first");
        p.cell = (0, 4);
        p.step(&config, Down);
        assert_eq!(p.selected_format().as_deref(), Some("iso-week"), "the row is shorter: its last cell");
    }

    #[test]
    fn placing_a_format() {
        assert_eq!(place_format("", "unix"), "now unix");
        assert_eq!(place_format("now", "unix"), "now unix");
        assert_eq!(place_format("now unix", "iso-week"), "now iso-week");
        assert_eq!(place_format("now + 3d ", "unix-ms"), "now + 3d unix-ms");
        assert_eq!(place_format("1775944800 unix", "iso-week"), "1775944800 unix iso-week");
        assert_eq!(place_format("12/04/26 3:20PM", "unix"), "12/04/26 3:20PM unix");
        assert_eq!(place_format("12/04/26 3:20PM unix", "iso"), "12/04/26 3:20PM iso");
        assert_eq!(place_format("2026/04/12 date", "unix"), "2026/04/12 date unix");
    }

    #[test]
    fn complete_when_the_target_is_set() {
        assert!(has_target("10 km mi "));
        assert!(has_target("(1+2)km mi"));
        assert!(!has_target("10 km "));
        assert!(!has_target("1 floz-us "));
        assert!(!has_target("10"));
        assert!(!has_target("1 +"));
        assert!(!has_target(""));
    }

    #[test]
    fn incomplete_expression() {
        assert_eq!(place_unit("1 +", 3, "km"), "1 + km ");
    }
}
