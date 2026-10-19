//! Written dates: the patterns (`DD/MM/YYYY`…), month names in six languages, dates read with a
//! pattern or guessed, `GMT±N` zone words.

use super::*;

/// The order and year length of a written date: `YYYY/MM/DD`, `DD/MM/YY`…
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fields {
    Ymd4,
    Ymd2,
    Dmy2,
    Dmy4,
    Mdy4,
    Mdy2,
}

/// The date patterns, by name. Each also has the time variants of `VARIANTS`.
pub const PATTERNS: [(&str, Fields); 6] = [
    ("YYYY/MM/DD", Fields::Ymd4),
    ("YY/MM/DD", Fields::Ymd2),
    ("DD/MM/YY", Fields::Dmy2),
    ("DD/MM/YYYY", Fields::Dmy4),
    ("MM/DD/YYYY", Fields::Mdy4),
    ("MM/DD/YY", Fields::Mdy2),
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Clock {
    /// Date only.
    None,
    /// `15:30:00`.
    H24,
    /// `03:30:00 PM`.
    H12,
}

/// Pattern suffixes: the time (24 or 12 hours) and the zone (`-tz`: `+02:00`).
pub const VARIANTS: [(&str, Clock, bool); 5] =
    [("", Clock::None, false), ("-24h", Clock::H24, false), ("-24h-tz", Clock::H24, true), ("-12h", Clock::H12, false), ("-12h-tz", Clock::H12, true)];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pattern {
    pub fields: Fields,
    pub clock: Clock,
    pub tz: bool,
}

impl Pattern {
    /// `DD/MM/YYYY-12h-tz` → the pattern.
    pub fn parse(word: &str) -> Option<Pattern> {
        PATTERNS.iter().find_map(|&(name, fields)| {
            let suffix = word.strip_prefix(name)?;
            VARIANTS.iter().find(|(s, ..)| *s == suffix).map(|&(_, clock, tz)| Pattern { fields, clock, tz })
        })
    }

    pub fn name(&self) -> String {
        let date = PATTERNS.iter().find(|(_, f)| *f == self.fields).unwrap().0;
        let suffix = VARIANTS.iter().find(|(_, c, tz)| *c == self.clock && *tz == self.tz).map_or("", |v| v.0);
        format!("{date}{suffix}")
    }

    pub(super) fn format(&self, instant: &Instant) -> String {
        let c = instant.civil();
        let (y4, y2) = (year(c.year), format!("{:02}", c.year.rem_euclid(100)));
        let (m, d) = (format!("{:02}", c.month), format!("{:02}", c.day));
        let mut out = match self.fields {
            Fields::Ymd4 => format!("{y4}/{m}/{d}"),
            Fields::Ymd2 => format!("{y2}/{m}/{d}"),
            Fields::Dmy2 => format!("{d}/{m}/{y2}"),
            Fields::Dmy4 => format!("{d}/{m}/{y4}"),
            Fields::Mdy4 => format!("{m}/{d}/{y4}"),
            Fields::Mdy2 => format!("{m}/{d}/{y2}"),
        };
        let secs = format!("{:02}:{:02}{}", c.minute, c.second, fraction(c.nano));
        match self.clock {
            Clock::None => {}
            Clock::H24 => out.push_str(&format!(" {:02}:{secs}", c.hour)),
            Clock::H12 => {
                let h = if c.hour.is_multiple_of(12) { 12 } else { c.hour % 12 };
                out.push_str(&format!(" {h:02}:{secs} {}", if c.hour < 12 { "AM" } else { "PM" }));
            }
        }
        if self.tz {
            let o = instant.offset();
            out.push_str(&format!(" {}{:02}:{:02}", if o < 0 { '-' } else { '+' }, o.abs() / 3600, o.abs() / 60 % 60));
        }
        out
    }
}

/// What a date format is, for the TUI's card: `DD/MM/YY-12h-tz` → "day/month/2-digit year, 12-hour
/// clock, with the zone".
pub fn describe_format(word: &str) -> String {
    let fixed = match word {
        "iso" => "ISO 8601, in the local zone with its offset",
        "iso-utc" => "ISO 8601 in UTC (Z)",
        "iso-basic" => "ISO 8601 basic form (no separators)",
        "iso-date" => "ISO 8601 calendar date",
        "iso-time" => "ISO 8601 time with the offset",
        "iso-ordinal" => "ISO 8601 ordinal date: year and day of the year",
        "iso-week" => "ISO 8601 week date: week-year, week, weekday",
        "unix" => "seconds since 1970-01-01T00:00Z",
        "unix-ms" => "milliseconds since 1970-01-01T00:00Z",
        "unix-us" => "microseconds since 1970-01-01T00:00Z",
        "unix-ns" => "nanoseconds since 1970-01-01T00:00Z",
        "local" => "the locale's date and time format",
        _ => "",
    };
    if !fixed.is_empty() {
        return fixed.to_string();
    }
    let Some(p) = Pattern::parse(word) else { return String::new() };
    let date = match p.fields {
        Fields::Ymd4 => "year/month/day",
        Fields::Ymd2 => "2-digit year/month/day",
        Fields::Dmy2 => "day/month/2-digit year",
        Fields::Dmy4 => "day/month/year",
        Fields::Mdy4 => "month/day/year",
        Fields::Mdy2 => "month/day/2-digit year",
    };
    let clock = match p.clock {
        Clock::None => "",
        Clock::H24 => ", 24-hour clock",
        Clock::H12 => ", 12-hour clock (AM / PM)",
    };
    format!("{date}{clock}{}", if p.tz { ", with the zone" } else { "" })
}

/// Every pattern format word (`YYYY/MM/DD`, `YYYY/MM/DD-24h`, … `MM/DD/YY-12h-tz`).
pub fn pattern_words() -> Vec<String> {
    PATTERNS.iter().flat_map(|(name, _)| VARIANTS.iter().map(move |(s, ..)| format!("{name}{s}"))).collect()
}

/// A written date as it was read: the date and time, the zone (`None`: local) and the pattern
/// that describes it.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub civil: Civil,
    pub offset: Option<i32>,
    pub pattern: Pattern,
    /// Read in the locale's order (`false`: the only valid order, or year first).
    pub by_locale: bool,
    /// When guessing: every valid reading giving a different date (pattern, year, month, day),
    /// this one first. More than one: the date is ambiguous.
    pub readings: Vec<(Pattern, i64, u32, u32)>,
    /// The month written by name (`12 Sept 2026`), if it was.
    pub month_name: Option<MonthName>,
}

/// A month written by name, as read.
#[derive(Debug, Clone, PartialEq)]
pub struct MonthName {
    /// As written: `Sept`, `septiembre`, `Sptember`.
    pub written: String,
    pub month: u32,
    /// Not a name or a prefix of one, but one letter off a single month's name (`Sptember`).
    pub typo: bool,
}

// ---------------------------------------------------------------- month names

/// Month names in English, Spanish, French, Italian, German and Portuguese, lowercase without
/// accents (`fevrier`, `marz`, `marco`), with common variants (`setiembre`, `janner`, `feber`). An
/// abbreviation is any prefix of 3 letters or more that only one month's names start with (`Sep`,
/// `Sept`, `janv`, `févr`, `Okt`, `giu`); `MONTH_ABBREVIATIONS` adds the few that are not prefixes.
pub(super) const MONTH_NAMES: [&[&str]; 12] = [
    &["january", "enero", "janvier", "gennaio", "januar", "janner", "janeiro"],
    &["february", "febrero", "fevrier", "febbraio", "februar", "feber", "fevereiro"],
    &["march", "marzo", "mars", "marz", "marco"],
    &["april", "abril", "avril", "aprile"],
    &["may", "mayo", "mai", "maggio", "maio"],
    &["june", "junio", "juin", "giugno", "juni", "junho"],
    &["july", "julio", "juillet", "luglio", "juli", "julho"],
    &["august", "agosto", "aout"],
    &["september", "septiembre", "setiembre", "septembre", "settembre", "setembro"],
    &["october", "octubre", "octobre", "ottobre", "oktober", "outubro"],
    &["november", "noviembre", "novembre", "novembro"],
    &["december", "diciembre", "decembre", "dicembre", "dezember", "dezembro"],
];

/// Abbreviations that are not a prefix of a name: German `Mrz` (März).
pub(super) const MONTH_ABBREVIATIONS: [(&str, u32); 1] = [("mrz", 3)];

pub(super) const ENGLISH_MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// Lowercase, without accents: `Févr` → `fevr`, `März` → `marz`, `Março` → `marco`.
pub(super) fn fold(word: &str) -> String {
    word.chars().flat_map(char::to_lowercase).map(unaccented).collect()
}

/// A lower-case letter without its accent: `é` → `e`.
pub(crate) fn unaccented(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        c => c,
    }
}

/// Whether `a` and `b` are at most one edit apart: a letter added, removed, changed, or two
/// neighbours swapped.
pub(super) fn one_edit_apart(a: &[u8], b: &[u8]) -> bool {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let prefix = short.iter().zip(long).take_while(|(x, y)| x == y).count();
    match long.len() - short.len() {
        0 if prefix == short.len() => true,
        0 => {
            let rest = prefix + 1;
            short[rest..] == long[rest..]
                || (rest < short.len() && short[prefix] == long[rest] && short[rest] == long[prefix] && short[rest + 1..] == long[rest + 1..])
        }
        1 => short[prefix..] == long[prefix + 1..],
        _ => false,
    }
}

/// The month a word names (any case, accents optional, a trailing `.` allowed): a full name, an
/// abbreviation (see `MONTH_NAMES`) or, for words of 5 letters or more, one letter off a single
/// month's name.
pub fn month_named(word: &str) -> Option<MonthName> {
    let folded = fold(word.strip_suffix('.').unwrap_or(word));
    if folded.len() < 3 || !folded.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    let found = |month: u32, typo: bool| Some(MonthName { written: word.to_string(), month, typo });
    if let Some(&(_, m)) = MONTH_ABBREVIATIONS.iter().find(|(a, _)| *a == folded) {
        return found(m, false);
    }
    // The single month whose names pass the test.
    let only = |test: &dyn Fn(&str) -> bool| -> Option<u32> {
        let mut months = (1..=12u32).filter(|&m| MONTH_NAMES[m as usize - 1].iter().any(|n| test(n)));
        let m = months.next()?;
        months.next().is_none().then_some(m)
    };
    if let Some(m) = only(&|name| name.starts_with(folded.as_str())) {
        return found(m, false);
    }
    if folded.len() >= 5 {
        if let Some(m) = only(&|name| one_edit_apart(name.as_bytes(), folded.as_bytes())) {
            return found(m, true);
        }
    }
    None
}

/// A date written with the month's name, day first (`12 Sept 2026`, `12-Sep-2026`,
/// `12. September 2026`, `12 de septiembre de 2026`, `1er janvier 2026`, `1st of May 2026`), month
/// first (`September 12, 2026`, `Sep 12th 2026`) or year first (`2026 Sep 12`), a time and zone
/// optionally following. The same date written with numbers (`12/09/2026 15:30`), its fields, and
/// the month as read.
pub(super) fn numeric_from_named(text: &str) -> Option<(String, Fields, MonthName)> {
    // Words: letters and digits (and `º ª °` after a day); `/ . , -` and spaces separate them.
    let mut words: Vec<(&str, usize)> = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices().chain([(text.len(), ' ')]) {
        let part = c.is_alphanumeric() || "ºª°".contains(c);
        match (start, part) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                words.push((&text[s..i], i));
                start = None;
                // The date is in the first few words; stop at the time (`15:30`).
                if c == ':' || words.len() == 7 {
                    break;
                }
            }
            _ => {}
        }
        if start.is_none() && !(c.is_whitespace() || "/.,-".contains(c)) && i < text.len() {
            break;
        }
    }
    // Words between the parts: `12 de septiembre de 2026`, `1st of May`.
    let filler = |w: &str| ["de", "del", "of", "the"].contains(&fold(w).as_str());
    let words: Vec<(&str, usize)> = words.into_iter().filter(|(w, _)| !filler(w)).collect();
    let day = |w: &str| -> Option<u32> {
        let n = w.bytes().take_while(u8::is_ascii_digit).count();
        let suffix = fold(&w[n..]);
        let ordinal = ["", "st", "nd", "rd", "th", "er", "re", "º", "ª", "°"].contains(&suffix.as_str());
        ((1..=2).contains(&n) && ordinal).then(|| w[..n].parse().ok()).flatten()
    };
    let year = |w: &str| -> Option<(u32, bool)> {
        (w.bytes().all(|b| b.is_ascii_digit()) && (w.len() == 4 || w.len() == 2)).then(|| (w.parse().ok().unwrap(), w.len() == 4))
    };
    let [(a, _), (b, _), (z, end), ..] = words[..] else { return None };
    let (fields, d, month, (y, long)) = if let (Some(d), Some(m), Some(y)) = (day(a), month_named(b), year(z)) {
        (if y.1 { Fields::Dmy4 } else { Fields::Dmy2 }, d, m, y)
    } else if let (Some(m), Some(d), Some(y)) = (month_named(a), day(b), year(z)) {
        (if y.1 { Fields::Mdy4 } else { Fields::Mdy2 }, d, m, y)
    } else if let (Some(y), Some(m), Some(d)) = (year(a).filter(|y| y.1), month_named(b), day(z)) {
        (Fields::Ymd4, d, m, y)
    } else {
        return None;
    };
    let y = if long { format!("{y:04}") } else { format!("{y:02}") };
    let m = month.month;
    let date = match fields {
        Fields::Dmy4 | Fields::Dmy2 => format!("{d}/{m}/{y}"),
        Fields::Mdy4 | Fields::Mdy2 => format!("{m}/{d}/{y}"),
        _ => format!("{y}/{m}/{d}"),
    };
    // The rest (a time, a zone) after `, ` or a space.
    let rest = text[end..].trim_start_matches([' ', ',']);
    let numeric = if rest.is_empty() { date } else { format!("{date} {rest}") };
    Some((numeric, fields, month))
}

/// Reads a written date: three numbers separated by `/ . -` or a space, then optionally a time
/// after a space or `T` (`15:30`, `3:30:45.5 PM`, `3:20PM`) and a zone (`Z`, `+02:00`).
/// With `fields` the date must be in that order (and that year length); without it the order is
/// guessed: year first when the first number has 4 digits, otherwise the locale's `order`, or
/// another order when the locale's gives an invalid date. 2-digit years follow POSIX `%y`
/// (69–99 → 19xx, 00–68 → 20xx).
///
/// The month may also be written by name in English, Spanish, French, Italian, German or Portuguese
/// (`12 Sept 2026`, `September 12, 2026 3:20 PM`, `12 de septiembre de 2026`): then the order is
/// the one written, never ambiguous.
pub fn read_date(text: &str, fields: Option<Fields>, order: Order) -> Option<Reading> {
    read_numeric_date(text, fields, order).or_else(|| {
        let (numeric, named_fields, month) = numeric_from_named(text.trim())?;
        if fields.is_some_and(|f| f != named_fields) {
            return None;
        }
        let reading = read_numeric_date(&numeric, Some(named_fields), order)?;
        Some(Reading { month_name: Some(month), ..reading })
    })
}

pub(super) fn read_numeric_date(text: &str, fields: Option<Fields>, order: Order) -> Option<Reading> {
    let mut c = Cursor { b: text.trim().as_bytes(), i: 0 };
    let field = |c: &mut Cursor| -> Option<(u32, usize)> {
        let n = c.count_digits();
        (1..=4).contains(&n).then(|| (c.digits(n).unwrap(), n))
    };
    let (a, a_len) = field(&mut c)?;
    let sep = c.peek().filter(|b| b"/.- ".contains(b))?;
    c.i += 1;
    let (b, b_len) = field(&mut c)?;
    if !c.eat(sep) {
        return None;
    }
    let (z, z_len) = field(&mut c)?;
    let pivot = |y: u32| if y >= 69 { 1900 + i64::from(y) } else { 2000 + i64::from(y) };
    // (year, month, day) in each order, if the lengths fit it.
    let assign = |f: Fields| -> Option<(i64, u32, u32)> {
        let small = |n: usize| (1..=2).contains(&n);
        match f {
            Fields::Ymd4 if a_len == 4 && small(b_len) && small(z_len) => Some((i64::from(a), b, z)),
            Fields::Ymd2 if a_len == 2 && small(b_len) && small(z_len) => Some((pivot(a), b, z)),
            Fields::Dmy4 if small(a_len) && small(b_len) && z_len == 4 => Some((i64::from(z), b, a)),
            Fields::Dmy2 if small(a_len) && small(b_len) && z_len == 2 => Some((pivot(z), b, a)),
            Fields::Mdy4 if small(a_len) && small(b_len) && z_len == 4 => Some((i64::from(z), a, b)),
            Fields::Mdy2 if small(a_len) && small(b_len) && z_len == 2 => Some((pivot(z), a, b)),
            _ => None,
        }
    };
    let valid = |(y, m, d): (i64, u32, u32)| (1..=12).contains(&m) && (1..=days_in_month(y, m)).contains(&d);
    let candidates: Vec<Fields> = match (fields, order) {
        (Some(f), _) => vec![f],
        (None, Order::DayFirst) => vec![Fields::Ymd4, Fields::Dmy4, Fields::Dmy2, Fields::Mdy4, Fields::Mdy2, Fields::Ymd2],
        (None, Order::MonthFirst) => vec![Fields::Ymd4, Fields::Mdy4, Fields::Mdy2, Fields::Dmy4, Fields::Dmy2, Fields::Ymd2],
        (None, Order::YearFirst) => vec![Fields::Ymd4, Fields::Ymd2, Fields::Dmy4, Fields::Dmy2, Fields::Mdy4, Fields::Mdy2],
    };
    let fitting: Vec<Fields> = candidates.iter().copied().filter(|&f| assign(f).is_some()).collect();
    let chosen = *fitting.iter().find(|&&f| assign(f).is_some_and(valid))?;
    let by_locale = fields.is_none() && chosen == fitting[0] && chosen != Fields::Ymd4;
    let (year, month, day) = assign(chosen)?;
    // Every other order that gives a valid, different date.
    let mut dates = vec![(chosen, (year, month, day))];
    if fields.is_none() {
        for &f in &fitting {
            if let Some(date) = assign(f).filter(|&d| valid(d)) {
                if !dates.iter().any(|(_, d)| *d == date) {
                    dates.push((f, date));
                }
            }
        }
    }
    let mut civil = Civil { year, month, day, ..Civil::default() };
    let mut clock = Clock::None;
    let mut offset = None;
    if c.eat(b' ') || c.eat(b'T') || c.eat(b't') {
        while c.eat(b' ') {}
        let h = c.count_digits();
        if !(1..=2).contains(&h) {
            return None;
        }
        civil.hour = c.digits(h)?;
        if !c.eat(b':') {
            return None;
        }
        civil.minute = c.digits(2)?;
        if c.eat(b':') {
            civil.second = c.digits(2)?;
            if c.eat(b'.') || c.eat(b',') {
                let n = c.count_digits();
                if n == 0 {
                    return None;
                }
                let frac = std::str::from_utf8(&c.b[c.i..c.i + n]).ok()?;
                civil.nano = format!("{:0<9}", &frac[..n.min(9)]).parse().ok()?;
                c.i += n;
            }
        }
        clock = Clock::H24;
        // AM / PM (also am, a.m., A, P), with or without a space.
        let save = c.i;
        while c.eat(b' ') {}
        let meridiem = match c.peek().map(|b| b.to_ascii_lowercase()) {
            Some(b'a') => Some(false),
            Some(b'p') => Some(true),
            _ => None,
        };
        if let Some(pm) = meridiem {
            c.i += 1;
            c.eat(b'.');
            if c.peek().is_some_and(|b| b.eq_ignore_ascii_case(&b'm')) {
                c.i += 1;
                c.eat(b'.');
            }
            if !(1..=12).contains(&civil.hour) {
                return None;
            }
            civil.hour = civil.hour % 12 + if pm { 12 } else { 0 };
            clock = Clock::H12;
        } else {
            c.i = save;
        }
        // 24:00 is the end of the day, as in ISO (not 24:00 PM).
        let end_of_day = clock == Clock::H24 && civil.hour == 24 && civil.minute == 0 && civil.second == 0 && civil.nano == 0;
        if (civil.hour > 23 && !end_of_day) || civil.minute > 59 || civil.second > 59 {
            return None;
        }
        while c.eat(b' ') {}
        offset = parse_zone(&mut c)?;
    }
    if c.i != c.b.len() {
        return None;
    }
    let pattern = Pattern { fields: chosen, clock, tz: offset.is_some() };
    let readings = dates.into_iter().map(|(fields, (y, m, d))| (Pattern { fields, ..pattern }, y, m, d)).collect();
    Some(Reading { civil, offset, pattern, by_locale, readings, month_name: None })
}

/// A written date read with an explicit pattern: its order, and its time and zone parts exactly.
pub(super) fn read_with_pattern(text: &str, pattern: Pattern) -> Option<Reading> {
    read_date(text, Some(pattern.fields), Order::MonthFirst).filter(|r| r.pattern.clock == pattern.clock && r.pattern.tz == pattern.tz)
}

/// The output format that writes a date like this ISO literal: `2026-12-04` → `iso-date`,
/// `2026-W49-5` → `iso-week`, `…T15:00Z` → `iso-utc`, basic forms → `iso-basic`.
pub(super) fn iso_format(iso: &Iso) -> &'static str {
    match (iso.form, iso.has_time, iso.extended, iso.offset) {
        (Form::Week, false, ..) => "iso-week",
        (Form::Ordinal, false, ..) => "iso-ordinal",
        (Form::Calendar, false, true, _) => "iso-date",
        (_, _, false, _) => "iso-basic",
        (_, true, true, Some(0)) => "iso-utc",
        _ => "iso",
    }
}

/// A `GMT` / `UTC` time zone word (any case): `GMT` (+00:00), `GMT+2`, `UTC-5`, `GMT+05:30`,
/// `GMT+0530`; the offset east of UTC in seconds. `None`: not a zone word; `Some(Err)`: a zone
/// word with an impossible offset (`GMT+25`).
pub fn gmt_zone(word: &str) -> Option<Result<i32, String>> {
    let prefix = word.get(..3)?;
    if !prefix.eq_ignore_ascii_case("GMT") && !prefix.eq_ignore_ascii_case("UTC") {
        return None;
    }
    let rest = &word[3..];
    if rest.is_empty() {
        return Some(Ok(0));
    }
    let sign = match rest.as_bytes()[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let digits = &rest[1..];
    let (h, m) = match digits.split_once(':') {
        Some((h, m)) => (h, m),
        // `hhmm`: the last two digits are the minutes. Only digits are cut by bytes: text after
        // the sign may be anything (`GMT+2 €` is a request's words, not a zone), never a zone.
        None if !digits.is_ascii() => (digits, "-"),
        None if digits.len() > 2 => digits.split_at(digits.len() - 2),
        None => (digits, "0"),
    };
    let valid = |s: &str, max: usize| !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_digit());
    if !valid(h, 2) || !valid(m, 2) {
        return Some(Err(format!("`{word}` is not a time zone (write GMT+2, GMT-05:30, UTC+0100)")));
    }
    let (h, m): (i32, i32) = (h.parse().unwrap(), m.parse().unwrap());
    if h > 14 || m > 59 || (h == 14 && m > 0) {
        return Some(Err(format!("`{word}`: time zones go from GMT-14 to GMT+14")));
    }
    Some(Ok(sign * (h * 3600 + m * 60)))
}

/// Takes the `GMT±N` zone word out of a request: a word of its own (`2026/12/04 15:30 GMT+2`) or
/// at the end of the time (`15:30GMT+2`, `3:20PMUTC-5`). Returns the request without it and the
/// zone word with its offset. At most one zone; a malformed one is an error.
pub fn take_zone(input: &str) -> Result<(String, Option<(String, i32)>), String> {
    let mut zone: Option<(String, i32)> = None;
    let mut words = Vec::new();
    for word in input.split_whitespace() {
        // A word ending with a zone after a digit or AM/PM: `15:30GMT+2`.
        let attached = (1..word.len())
            .filter(|&i| word.is_char_boundary(i))
            .find(|&i| word[..i].ends_with(|c: char| c.is_ascii_digit() || c == 'M' || c == 'm') && gmt_zone(&word[i..]).is_some());
        let (keep, found) = match (gmt_zone(word), attached) {
            (Some(z), _) => ("", Some((word, z))),
            (None, Some(i)) => (&word[..i], Some((&word[i..], gmt_zone(&word[i..]).unwrap()))),
            (None, None) => (word, None),
        };
        if let Some((text, offset)) = found {
            let offset = offset?;
            if let Some((first, _)) = &zone {
                return Err(format!("two time zones: `{first}` and `{text}`"));
            }
            zone = Some((text.to_string(), offset));
        }
        if !keep.is_empty() {
            words.push(keep);
        }
    }
    Ok((words.join(" "), zone))
}

/// The instant of an ISO literal (24:00 and leap seconds roll over), in `default` when it has no
/// zone of its own.
/// An ISO 8601 date-time (`2026-10-11T00:00:00Z`; no zone = UTC) as Unix seconds.
pub fn parse_utc(s: &str) -> Option<i64> {
    let instant = iso_instant_in(&parse_iso(s)?, Zone::Fixed(0)).ok()?;
    Some(instant.ns.div_euclid(NS) as i64)
}

pub(super) fn iso_instant_in(iso: &Iso, default: Zone) -> Result<Instant, TimeError> {
    let zone = iso.offset.map_or(default, Zone::Fixed);
    let mut c = iso.civil;
    let extra = i128::from(c.hour == 24) * DAY_NS + i128::from(c.second == 60) * NS;
    c.hour %= 24;
    c.second = c.second.min(59);
    let instant = from_civil(&c, zone)?;
    Instant { ns: instant.ns + extra, zone }.in_range()
}
