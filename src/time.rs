//! Dates and times: parsing (ISO 8601, Unix timestamps, the locale's format, `now`, `today`),
//! formatting, calendar-aware date math and differences.
//!
//! Instants are exact: `i128` nanoseconds since 1970-01-01T00:00:00Z. Civil dates use the
//! proleptic Gregorian calendar (H. Hinnant's `days_from_civil` / `civil_from_days`), for years
//! -9999 to 9999. The local time zone (with DST) and the locale's date format come from the C
//! library (`sys`).
//!
//! ```text
//! request  = value ( ("+" | "-") (duration | value) )* [target]
//! value    = "now" | "today" | ISO literal | number unix-format | text "local"   (the first value)
//! duration = number [space] time-unit                                            (3d, 1.5 h, 90min)
//! target   = output format (iso, iso-utc, unix-ms, local, …) | time unit (for a difference)
//! ```
//! `ns`…`h` are exact; `d wk mo yr` are calendar steps in the date's zone (same wall time; the day
//! is clamped to the month's length); `wd` steps over working days (Monday to Friday). `date - date`
//! is a duration, or a count of working days.

mod sys;
pub(crate) mod tz;

use crate::config::{Config, DATE_WORDS};
use crate::convert;

const NS: i128 = 1_000_000_000;
const DAY_NS: i128 = 86_400 * NS;
const MIN_YEAR: i64 = -9999;
const MAX_YEAR: i64 = 9999;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Zone {
    /// The device's time zone (DST included).
    Local,
    /// A fixed offset east of UTC, in seconds (`Z` is 0).
    Fixed(i32),
    /// A zone of the system's database (DST included): a city's (`Paris`).
    Named(&'static tz::Tz),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Instant {
    pub ns: i128,
    pub zone: Zone,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Civil {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub nano: u32,
}

#[derive(Debug, PartialEq)]
pub enum TimeError {
    /// A malformed request: exit code 2.
    Usage(String),
    /// A date outside years -9999…9999: exit code 1.
    Range,
    /// A written date that reads as more than one date (`12/04/2026`): the operator chooses.
    Ambiguous(Ambiguity),
}

/// The readings of an ambiguous written date, the locale's first; or the cities a misspelt one
/// may be (`Tokio`: Tokyo).
#[derive(Debug, Clone, PartialEq)]
pub struct Ambiguity {
    /// As written: `12/04/2026`, `Tokio`.
    pub text: String,
    pub choices: Vec<Choice>,
    pub kind: AmbiguityKind,
    /// For misspelt places, what was misspelt: "`Tokio` is not a city", "`jappan` is not a
    /// country", "`jappan`, `romme` are not known places". Empty otherwise.
    pub note: String,
}

/// What is ambiguous.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AmbiguityKind {
    /// A date's day and month order: `Choice::pattern` is a pattern, `date` the date it gives.
    Date,
    /// Misspelt places (cities or countries, one or more): `pattern` is the places that fix the
    /// request (`Japan, Rome`), `date` their zones (`Asia/Tokyo, Europe/Rome`).
    City,
    /// A country with several time zones (`USA`): `pattern` is the zone's most relevant city,
    /// `date` its offset now and its zone (`GMT-4 · America/New_York`).
    Country,
    /// Currencies (`currency.rs`): a name several share (`dollar`) or a misspelt one (`dolar`);
    /// `pattern` is the currencies (`US dollar`), `date` their codes (`USD`).
    Currency,
    /// Cities for a distance (`geo.rs`): a name several share (`Springfield`) or a misspelt one;
    /// `pattern` is the cities (`Springfield, Illinois, United States`), `date` their population
    /// and where they are on a map.
    Place,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    /// The pattern of this reading: `DD/MM/YYYY`.
    pub pattern: String,
    /// The date it gives: `2026-04-12`.
    pub date: String,
    /// The request with that pattern written in, unambiguous: `12/04/2026 DD/MM/YYYY unix`.
    pub request: String,
}

impl Ambiguity {
    /// For scripts, where nobody can choose: `` `12/04/2026` is ambiguous: DD/MM/YYYY (2026-04-12)
    /// or MM/DD/YYYY (2026-12-04); write the pattern: `12/04/2026 DD/MM/YYYY unix` ``.
    pub fn message(&self) -> String {
        let names: Vec<&str> = self.choices.iter().map(|c| c.pattern.as_str()).collect();
        match self.kind {
            AmbiguityKind::City | AmbiguityKind::Currency | AmbiguityKind::Place => {
                return format!("{}: did you mean {}? (`{}`)", self.note, names.join(" or "), self.choices[0].request);
            }
            AmbiguityKind::Country => {
                let zones: Vec<String> = self.choices.iter().map(|c| format!("{} ({})", c.pattern, c.date.split(" · ").next().unwrap_or(""))).collect();
                return format!("`{}` has several time zones: {}; write a city: `{}`", self.text, zones.join(", "), self.choices[0].request);
            }
            AmbiguityKind::Date => {}
        }
        let readings: Vec<String> = self.choices.iter().map(|c| format!("{} ({})", c.pattern, c.date)).collect();
        format!("`{}` is ambiguous: {}; write the pattern: `{}`", self.text, readings.join(" or "), self.choices[0].request)
    }
}

fn usage<T>(msg: impl Into<String>) -> Result<T, TimeError> {
    Err(TimeError::Usage(msg.into()))
}

// ---------------------------------------------------------------- calendar

pub fn is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

pub fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from((m + 9) % 12); // March = 0
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (era * 400 + yoe + i64::from(m <= 2), m, d)
}

/// ISO weekday: Monday = 1 … Sunday = 7.
pub fn weekday(days: i64) -> u32 {
    (days + 3).rem_euclid(7) as u32 + 1
}

/// ISO week date of a day: (week-based year, week, weekday).
pub fn iso_week(days: i64) -> (i64, u32, u32) {
    let wd = weekday(days);
    let thursday = days - i64::from(wd) + 4;
    let (year, _, _) = civil_from_days(thursday);
    let week = ((thursday - days_from_civil(year, 1, 1)) / 7 + 1) as u32;
    (year, week, wd)
}

fn weeks_in_year(year: i64) -> u32 {
    iso_week(days_from_civil(year, 12, 28)).1
}

fn days_from_iso_week(year: i64, week: u32, wd: u32) -> i64 {
    let jan4 = days_from_civil(year, 1, 4);
    jan4 - i64::from(weekday(jan4) - 1) + i64::from(week - 1) * 7 + i64::from(wd - 1)
}

impl Instant {
    /// Offset from UTC in seconds at this instant.
    pub fn offset(&self) -> i32 {
        match self.zone {
            Zone::Fixed(offset) => offset,
            Zone::Local => sys::local_offset(self.ns.div_euclid(NS) as i64).unwrap_or(0),
            Zone::Named(tz) => tz.offset_at(self.ns.div_euclid(NS) as i64),
        }
    }

    /// The wall-clock date and time in the instant's zone.
    pub fn civil(&self) -> Civil {
        let local = self.ns + i128::from(self.offset()) * NS;
        let (days, rem) = (local.div_euclid(DAY_NS), local.rem_euclid(DAY_NS));
        let (year, month, day) = civil_from_days(days as i64);
        let secs = (rem / NS) as u32;
        Civil { year, month, day, hour: secs / 3600, minute: secs / 60 % 60, second: secs % 60, nano: (rem % NS) as u32 }
    }

    fn in_range(self) -> Result<Self, TimeError> {
        let min = i128::from(days_from_civil(MIN_YEAR, 1, 1) - 1) * DAY_NS;
        let max = i128::from(days_from_civil(MAX_YEAR + 1, 1, 1) + 1) * DAY_NS;
        if (min..max).contains(&self.ns) {
            Ok(self)
        } else {
            Err(TimeError::Range)
        }
    }
}

/// The instant of a wall-clock time in `zone`.
fn from_civil(c: &Civil, zone: Zone) -> Result<Instant, TimeError> {
    if !(MIN_YEAR..=MAX_YEAR).contains(&c.year) {
        return Err(TimeError::Range);
    }
    let days = days_from_civil(c.year, c.month, c.day);
    let wall = i128::from(days) * 86_400 + i128::from(c.hour * 3600 + c.minute * 60 + c.second);
    let secs = match zone {
        Zone::Fixed(offset) => wall - i128::from(offset),
        Zone::Local => i128::from(sys::local_to_utc(c).unwrap_or(wall as i64)),
        Zone::Named(tz) => i128::from(tz.to_utc(wall as i64)),
    };
    Instant { ns: secs * NS + i128::from(c.nano), zone }.in_range()
}

pub fn now() -> Instant {
    let ns = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as i128,
        Err(e) => -(e.duration().as_nanos() as i128),
    };
    Instant { ns, zone: Zone::Local }
}

// ---------------------------------------------------------------- ISO 8601 parsing

#[derive(Debug, Clone, Copy, PartialEq)]
enum Form {
    Calendar,
    Ordinal,
    Week,
}

#[derive(Debug)]
struct Iso {
    civil: Civil,
    /// `None`: no zone given, local time.
    offset: Option<i32>,
    form: Form,
    extended: bool,
    has_time: bool,
}

impl Iso {
    /// Whether a word is recognised as a date on its own. Basic forms (`20261010`) and the
    /// extended ordinal (`2026-283`) read as numbers or subtractions, so they need a time part
    /// (`20261010T1500`) or the `iso` input word.
    fn unambiguous(&self) -> bool {
        self.has_time || self.form == Form::Week || (self.extended && self.form == Form::Calendar)
    }
}

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn eat(&mut self, c: u8) -> bool {
        let found = self.peek() == Some(c);
        self.i += usize::from(found);
        found
    }

    fn digits(&mut self, n: usize) -> Option<u32> {
        let s = self.b.get(self.i..self.i + n)?;
        if !s.iter().all(u8::is_ascii_digit) {
            return None;
        }
        self.i += n;
        Some(s.iter().fold(0, |v, d| v * 10 + u32::from(d - b'0')))
    }

    fn count_digits(&self) -> usize {
        self.b[self.i..].iter().take_while(|c| c.is_ascii_digit()).count()
    }
}

fn parse_iso(s: &str) -> Option<Iso> {
    let mut c = Cursor { b: s.as_bytes(), i: 0 };
    let year = i64::from(c.digits(4)?);
    let extended = c.eat(b'-');
    let (form, month, day) = if c.eat(b'W') || c.eat(b'w') {
        let week = c.digits(2)?;
        let wd = if !extended || c.eat(b'-') { c.digits(1).unwrap_or(1) } else { 1 };
        if !(1..=weeks_in_year(year)).contains(&week) || !(1..=7).contains(&wd) {
            return None;
        }
        let (y, m, d) = civil_from_days(days_from_iso_week(year, week, wd));
        if y != year {
            // The week belongs to `year` but the day to the next or previous calendar year.
            return finish(c, Civil { year: y, month: m, day: d, ..Civil::default() }, Form::Week, extended);
        }
        (Form::Week, m, d)
    } else {
        match (extended, c.count_digits()) {
            (true, 3) | (false, 3) => {
                let doy = c.digits(3)?;
                let len = if is_leap(year) { 366 } else { 365 };
                if !(1..=len).contains(&doy) {
                    return None;
                }
                let (_, m, d) = civil_from_days(days_from_civil(year, 1, 1) + i64::from(doy) - 1);
                (Form::Ordinal, m, d)
            }
            (true, 2) => {
                let m = c.digits(2)?;
                if !c.eat(b'-') {
                    return None;
                }
                (Form::Calendar, m, c.digits(2)?)
            }
            (false, n) if n >= 4 => {
                let m = c.digits(2)?;
                (Form::Calendar, m, c.digits(2)?)
            }
            _ => return None,
        }
    };
    if !(1..=12).contains(&month) || !(1..=days_in_month(year, month)).contains(&day) {
        return None;
    }
    finish(c, Civil { year, month, day, ..Civil::default() }, form, extended)
}

/// The optional time and zone after the date.
fn finish(mut c: Cursor, mut civil: Civil, form: Form, extended: bool) -> Option<Iso> {
    let mut offset = None;
    let has_time = c.eat(b'T') || c.eat(b't');
    if has_time {
        civil.hour = c.digits(2)?;
        let colon = c.eat(b':');
        if colon || c.count_digits() >= 2 {
            civil.minute = c.digits(2)?;
            if (colon && c.eat(b':')) || (!colon && c.count_digits() >= 2) {
                civil.second = c.digits(2)?;
                if c.eat(b'.') || c.eat(b',') {
                    let n = c.count_digits();
                    if n == 0 {
                        return None;
                    }
                    let frac = std::str::from_utf8(&c.b[c.i..c.i + n]).ok()?;
                    let padded = format!("{:0<9}", &frac[..n.min(9)]);
                    civil.nano = padded.parse().ok()?;
                    c.i += n;
                }
            }
        }
        // 24:00 is the end of the day.
        let end_of_day = civil.hour == 24 && civil.minute == 0 && civil.second == 0 && civil.nano == 0;
        if (civil.hour > 23 && !end_of_day) || civil.minute > 59 || civil.second > 60 {
            return None;
        }
        offset = parse_zone(&mut c)?;
    }
    (c.i == c.b.len()).then_some(Iso { civil, offset, form, extended, has_time })
}

/// An optional zone at the cursor: `Z`, `±hh`, `±hh:mm`, `±hhmm`. `None`: malformed;
/// `Some(None)`: no zone.
fn parse_zone(c: &mut Cursor) -> Option<Option<i32>> {
    Some(match c.peek() {
        Some(b'Z' | b'z') => {
            c.i += 1;
            Some(0)
        }
        Some(sign @ (b'+' | b'-')) => {
            c.i += 1;
            let h = c.digits(2)?;
            let m = if c.eat(b':') { c.digits(2)? } else { c.digits(2).unwrap_or(0) };
            if h > 23 || m > 59 {
                return None;
            }
            let o = (h * 3600 + m * 60) as i32;
            Some(if sign == b'-' { -o } else { o })
        }
        _ => None,
    })
}

/// The order of day and month in dates that do not start with the year.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Order {
    DayFirst,
    MonthFirst,
    YearFirst,
}

impl Order {
    /// From a `strftime` date format such as the locale's `D_FMT`: `%d/%m/%Y` → day first.
    pub fn from_format(format: &str) -> Order {
        let at = |specs: &[&str]| specs.iter().filter_map(|s| format.find(s)).min();
        match (at(&["%Y", "%y", "%C"]), at(&["%d", "%e"]), at(&["%m", "%b", "%B"])) {
            (Some(y), Some(d), Some(m)) if y < d && y < m => Order::YearFirst,
            (_, Some(d), Some(m)) if d < m => Order::DayFirst,
            _ => Order::MonthFirst,
        }
    }

    /// The device locale's order (the C locale's `%m/%d/%y` when unknown).
    pub fn of_locale() -> Order {
        sys::date_format().map_or(Order::MonthFirst, |f| Order::from_format(&f))
    }

    fn describe(self) -> &'static str {
        match self {
            Order::MonthFirst => "year/month/day, or month/day/year here",
            Order::DayFirst | Order::YearFirst => "year/month/day, or day/month/year here",
        }
    }
}

// ---------------------------------------------------------------- written dates and patterns

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
pub const VARIANTS: [(&str, Clock, bool); 5] = [
    ("", Clock::None, false),
    ("-24h", Clock::H24, false),
    ("-24h-tz", Clock::H24, true),
    ("-12h", Clock::H12, false),
    ("-12h-tz", Clock::H12, true),
];

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

    fn format(&self, instant: &Instant) -> String {
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
                let h = if c.hour % 12 == 0 { 12 } else { c.hour % 12 };
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
#[cfg_attr(not(feature = "tui"), allow(dead_code))]
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
const MONTH_NAMES: [&[&str]; 12] = [
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
const MONTH_ABBREVIATIONS: [(&str, u32); 1] = [("mrz", 3)];

const ENGLISH_MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// Lowercase, without accents: `Févr` → `fevr`, `März` → `marz`, `Março` → `marco`.
fn fold(word: &str) -> String {
    word.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            c => c,
        })
        .collect()
}

/// Whether `a` and `b` are at most one edit apart: a letter added, removed, changed, or two
/// neighbours swapped.
fn one_edit_apart(a: &[u8], b: &[u8]) -> bool {
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
fn numeric_from_named(text: &str) -> Option<(String, Fields, MonthName)> {
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

fn read_numeric_date(text: &str, fields: Option<Fields>, order: Order) -> Option<Reading> {
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
        if civil.hour > 23 || civil.minute > 59 || civil.second > 59 {
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
fn read_with_pattern(text: &str, pattern: Pattern) -> Option<Reading> {
    read_date(text, Some(pattern.fields), Order::MonthFirst)
        .filter(|r| r.pattern.clock == pattern.clock && r.pattern.tz == pattern.tz)
}

/// The output format that writes a date like this ISO literal: `2026-12-04` → `iso-date`,
/// `2026-W49-5` → `iso-week`, `…T15:00Z` → `iso-utc`, basic forms → `iso-basic`.
fn iso_format(iso: &Iso) -> &'static str {
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
        let attached = (1..word.len()).filter(|&i| word.is_char_boundary(i)).find(|&i| {
            word[..i].ends_with(|c: char| c.is_ascii_digit() || c == 'M' || c == 'm') && gmt_zone(&word[i..]).is_some()
        });
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

fn iso_instant_in(iso: &Iso, default: Zone) -> Result<Instant, TimeError> {
    let zone = iso.offset.map_or(default, Zone::Fixed);
    let mut c = iso.civil;
    let extra = i128::from(c.hour == 24) * DAY_NS + i128::from(c.second == 60) * NS;
    c.hour %= 24;
    c.second = c.second.min(59);
    let instant = from_civil(&c, zone)?;
    Instant { ns: instant.ns + extra, zone }.in_range()
}

// ---------------------------------------------------------------- numbers

/// `text` (digits, optional `.` fraction, optional leading `-`) times `scale`, rounded to an integer.
fn scaled(text: &str, scale: i128) -> Option<i128> {
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, text),
    };
    let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
    if int.is_empty() && frac.is_empty() || !int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let int: i128 = if int.is_empty() { 0 } else { int.parse().ok()? };
    let mut value = int.checked_mul(scale)?;
    if !frac.is_empty() {
        let frac = &frac[..frac.len().min(30)];
        let den = 10i128.checked_pow(frac.len() as u32)?;
        let num: i128 = frac.parse().ok()?;
        value = value.checked_add((num.checked_mul(scale)? * 2 + den) / (2 * den))?;
    }
    Some(sign * value)
}

/// `value / 10^digits` as a decimal, trailing zeros trimmed.
fn decimal(value: i128, digits: u32) -> String {
    let den = 10i128.pow(digits);
    let sign = if value < 0 { "-" } else { "" };
    let (int, frac) = (value.abs() / den, value.abs() % den);
    if frac == 0 {
        return format!("{sign}{int}");
    }
    let frac = format!("{frac:0width$}", width = digits as usize);
    format!("{sign}{int}.{}", frac.trim_end_matches('0'))
}

// ---------------------------------------------------------------- formatting

/// The output formats (the words in `config::DATE_WORDS` after `now` and `today`).
pub fn is_format(word: &str) -> bool {
    DATE_WORDS[3..].contains(&word) || Pattern::parse(word).is_some()
}

/// Nanoseconds per unit of each Unix format.
fn unix_scale(word: &str) -> Option<i128> {
    match word {
        "unix" => Some(NS),
        "unix-ms" => Some(1_000_000),
        "unix-us" => Some(1_000),
        "unix-ns" => Some(1),
        _ => None,
    }
}

fn year(y: i64) -> String {
    if y < 0 {
        format!("-{:04}", -y)
    } else {
        format!("{y:04}")
    }
}

fn fraction(nano: u32) -> String {
    match nano {
        0 => String::new(),
        n => format!(".{}", format!("{n:09}").trim_end_matches('0')),
    }
}

fn zone_suffix(instant: &Instant, extended: bool) -> String {
    let offset = instant.offset();
    if instant.zone == Zone::Fixed(0) {
        return "Z".into();
    }
    let sign = if offset < 0 { '-' } else { '+' };
    let (h, m) = (offset.abs() / 3600, offset.abs() / 60 % 60);
    if extended {
        format!("{sign}{h:02}:{m:02}")
    } else {
        format!("{sign}{h:02}{m:02}")
    }
}

pub fn format(instant: &Instant, word: &str) -> Result<String, TimeError> {
    format_as(instant, word, true)
}

/// `format`, with the ISO forms' seconds left out when `seconds` is false and they are zero:
/// `2026-12-04T22:00+01:00` for an input written `2026/12/04 10:00 PM`.
fn format_as(instant: &Instant, word: &str, seconds: bool) -> Result<String, TimeError> {
    // Seconds, ms, µs or ns since the epoch: the nanoseconds with the decimal point moved.
    if let Some(scale) = unix_scale(word) {
        return Ok(decimal(instant.ns, scale.ilog10()));
    }
    if word == "iso-utc" {
        return format_as(&Instant { zone: Zone::Fixed(0), ..*instant }, "iso", seconds);
    }
    if let Some(pattern) = Pattern::parse(word) {
        return Ok(pattern.format(instant));
    }
    let c = instant.civil();
    let date = format!("{}-{:02}-{:02}", year(c.year), c.month, c.day);
    let short = !seconds && c.second == 0 && c.nano == 0;
    let time = if short {
        format!("{:02}:{:02}", c.hour, c.minute)
    } else {
        format!("{:02}:{:02}:{:02}{}", c.hour, c.minute, c.second, fraction(c.nano))
    };
    Ok(match word {
        "iso" => format!("{date}T{time}{}", zone_suffix(instant, true)),
        "iso-basic" => format!(
            "{}{:02}{:02}T{}{}",
            year(c.year),
            c.month,
            c.day,
            time.replace(':', ""),
            zone_suffix(instant, false)
        ),
        "iso-date" => date,
        "iso-time" => format!("{time}{}", zone_suffix(instant, true)),
        "iso-ordinal" => {
            let doy = days_from_civil(c.year, c.month, c.day) - days_from_civil(c.year, 1, 1) + 1;
            format!("{}-{doy:03}", year(c.year))
        }
        "iso-week" => {
            let (wy, week, wd) = iso_week(days_from_civil(c.year, c.month, c.day));
            format!("{}-W{week:02}-{wd}", year(wy))
        }
        "local" => sys::format_local(instant.ns.div_euclid(NS) as i64)
            .ok_or_else(|| TimeError::Usage("the local date format is not available on this platform".into()))?,
        other => return usage(format!("unknown date format `{other}`")),
    })
}

// ---------------------------------------------------------------- human format

/// The default human formats (the `format.date` and `format.datetime` settings).
pub const HUMAN_DATE: &str = "DD MMM YY";
pub const HUMAN_DATETIME: &str = "DD MMM YY h:mm AM tz";

/// Month abbreviations for `MMM`: `Sept` (the British style), the others in three letters.
const MONTH_ABBREVIATIONS_EN: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sept", "Oct", "Nov", "Dec"];
const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// The tokens of a human format, longest first (so `MMMM` is not read as `MM` twice).
const HUMAN_TOKENS: [&str; 22] = [
    "YYYY", "MMMM", "dddd", "H24", "H12", "MMM", "ddd", "YY", "MM", "DD", "HH", "hh", "mm", "ss", "tz", "ZZ", "AM", "D", "M", "H",
    "h", "A",
];

/// A date for people: `format` with its tokens replaced, the rest kept (`[text]`: literal).
///
/// `YYYY` 2026 · `YY` 26 · `MMMM` September · `MMM` Sept · `MM` 09 · `M` 9 · `DD` 05 · `D` 5 ·
/// `dddd` Saturday · `ddd` Sat · `H24` / `HH` 20 (24-hour) · `H` 20 · `H12` / `hh` 08 · `h` 8 ·
/// `mm` 05 · `ss` 07 · `AM` / `A` PM · `tz` GMT+2 (GMT+5:30, GMT) · `ZZ` +02:00.
/// A day for people, in the local zone: `3 Jan 2027`.
pub fn day_for_people(unix_seconds: f64) -> String {
    format_human(&Instant { ns: unix_seconds as i128 * NS, zone: Zone::Local }, "D MMM YYYY")
}

pub fn format_human(instant: &Instant, format: &str) -> String {
    let c = instant.civil();
    let offset = instant.offset();
    let h12 = if c.hour % 12 == 0 { 12 } else { c.hour % 12 };
    let wd = weekday(days_from_civil(c.year, c.month, c.day)) as usize - 1;
    let mut out = String::new();
    let mut rest = format;
    while let Some(ch) = rest.chars().next() {
        if ch == '[' {
            if let Some(end) = rest.find(']') {
                out.push_str(&rest[1..end]);
                rest = &rest[end + 1..];
                continue;
            }
        }
        let Some(token) = HUMAN_TOKENS.iter().find(|t| rest.starts_with(**t)) else {
            out.push(ch);
            rest = &rest[ch.len_utf8()..];
            continue;
        };
        let text = match *token {
            "YYYY" => year(c.year),
            "YY" => format!("{:02}", c.year.rem_euclid(100)),
            "MMMM" => ENGLISH_MONTHS[c.month as usize - 1].to_string(),
            "MMM" => MONTH_ABBREVIATIONS_EN[c.month as usize - 1].to_string(),
            "MM" => format!("{:02}", c.month),
            "M" => c.month.to_string(),
            "DD" => format!("{:02}", c.day),
            "D" => c.day.to_string(),
            "dddd" => WEEKDAYS[wd].to_string(),
            "ddd" => WEEKDAYS[wd][..3].to_string(),
            "H24" | "HH" => format!("{:02}", c.hour),
            "H" => c.hour.to_string(),
            "H12" | "hh" => format!("{h12:02}"),
            "h" => h12.to_string(),
            "mm" => format!("{:02}", c.minute),
            "ss" => format!("{:02}", c.second),
            "AM" | "A" => (if c.hour < 12 { "AM" } else { "PM" }).to_string(),
            "tz" => match (offset.abs() / 3600, offset.abs() / 60 % 60) {
                (0, 0) => "GMT".to_string(),
                (h, 0) => format!("GMT{}{h}", if offset < 0 { '-' } else { '+' }),
                (h, m) => format!("GMT{}{h}:{m:02}", if offset < 0 { '-' } else { '+' }),
            },
            _ => format!("{}{:02}:{:02}", if offset < 0 { '-' } else { '+' }, offset.abs() / 3600, offset.abs() / 60 % 60),
        };
        out.push_str(&text);
        rest = &rest[token.len()..];
    }
    out
}

/// A date for people, in the `format.datetime` setting (default `DD MMM YY h:mm AM tz`: `12 Sept 26
/// 8:00 PM GMT+2`), or `format.date` (`DD MMM YY`: `12 Sept 26`) when `date_only`.
pub fn human(instant: &Instant, date_only: bool) -> String {
    let settings = crate::settings::current();
    format_human(instant, if date_only { settings.format_date() } else { settings.format_datetime() })
}

/// The date in ISO and in `target`, each named: `2026-04-12T00:00:00+02:00 iso = 1775944800 unix`.
/// With `iso-utc` the ISO side is in UTC too.
pub fn both_sides(instant: &Instant, target: &str, seconds: bool) -> Result<String, TimeError> {
    let iso = format_as(instant, if target == "iso-utc" { "iso-utc" } else { "iso" }, seconds)?;
    let converted = format_as(instant, target, seconds)?;
    // The same text on both sides (`…Z` in iso-utc): once, named by the target.
    if converted == iso {
        return Ok(format!("{converted} {target}"));
    }
    Ok(format!("{iso} iso = {converted} {target}"))
}

/// A duration as `75d 8h 6min 51.5s`: the non-zero parts, exact.
pub fn breakdown(ns: i128) -> String {
    let sign = if ns < 0 { "-" } else { "" };
    let mut rest = ns.abs();
    let mut parts = Vec::new();
    for (unit, size) in [("d", DAY_NS), ("h", 3600 * NS), ("min", 60 * NS)] {
        if rest >= size {
            parts.push(format!("{}{unit}", rest / size));
            rest %= size;
        }
    }
    if rest > 0 || parts.is_empty() {
        parts.push(format!("{}s", decimal(rest, 9)));
    }
    format!("{sign}{}", parts.join(" "))
}

// ---------------------------------------------------------------- date math

#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    /// Nanoseconds per unit.
    Exact(i128),
    Days(i64),
    /// Working days: Monday to Friday.
    WorkDays,
    Months(i64),
}

fn step(symbol: &str) -> Option<Step> {
    Some(match symbol {
        "ns" => Step::Exact(1),
        "us" => Step::Exact(1_000),
        "ms" => Step::Exact(1_000_000),
        "s" => Step::Exact(NS),
        "min" => Step::Exact(60 * NS),
        "h" => Step::Exact(3600 * NS),
        "d" => Step::Days(1),
        "wk" => Step::Days(7),
        WORKDAY => Step::WorkDays,
        "mo" => Step::Months(1),
        "yr" => Step::Months(12),
        _ => return None,
    })
}

/// `instant` plus `amount` (a decimal; whole for calendar units) of `symbol`, `sign` = ±1.
fn add(instant: Instant, amount: &str, symbol: &str, sign: i128) -> Result<Instant, TimeError> {
    let step = step(symbol).ok_or_else(|| TimeError::Usage(format!("`{symbol}` is not a time unit")))?;
    let whole = || -> Result<i64, TimeError> {
        amount.parse::<i64>().map_err(|_| TimeError::Usage(format!("`{amount}{symbol}`: calendar steps need a whole number")))
    };
    match step {
        Step::Exact(size) => {
            let delta = scaled(amount, size).ok_or(TimeError::Range)?;
            Instant { ns: instant.ns.checked_add(sign * delta).ok_or(TimeError::Range)?, ..instant }.in_range()
        }
        Step::Days(days) => {
            let c = instant.civil();
            let n = whole()?.checked_mul(days).ok_or(TimeError::Range)? * sign as i64;
            let (year, month, day) = civil_from_days(days_from_civil(c.year, c.month, c.day).checked_add(n).ok_or(TimeError::Range)?);
            from_civil(&Civil { year, month, day, ..c }, instant.zone)
        }
        Step::WorkDays => {
            let c = instant.civil();
            let n = whole()?.checked_mul(sign as i64).ok_or(TimeError::Range)?;
            let days = days_from_civil(c.year, c.month, c.day);
            let days = add_workdays(days, n).ok_or(TimeError::Range)?;
            let (year, month, day) = civil_from_days(days);
            from_civil(&Civil { year, month, day, ..c }, instant.zone)
        }
        Step::Months(months) => {
            let c = instant.civil();
            let n = whole()?.checked_mul(months).ok_or(TimeError::Range)? * sign as i64;
            let total = (c.year * 12 + i64::from(c.month) - 1).checked_add(n).ok_or(TimeError::Range)?;
            let (year, month) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
            if !(MIN_YEAR..=MAX_YEAR).contains(&year) {
                return Err(TimeError::Range);
            }
            let day = c.day.min(days_in_month(year, month));
            from_civil(&Civil { year, month, day, ..c }, instant.zone)
        }
    }
}

/// The working-day step (`5wd`, Monday to Friday); not a duration, so not in the unit table.
const WORKDAY: &str = "wd";
const WORKDAY_NAMES: [&str; 5] = [WORKDAY, "workday", "workdays", "weekday", "weekdays"];

fn is_weekend(days: i64) -> bool {
    weekday(days) >= 6
}

/// `days` plus `n` working days (`WORKDAY` in spreadsheets): each step lands on a Monday–Friday,
/// weekends are skipped. From a weekend day, `+1wd` is the next Monday and `-1wd` the previous
/// Friday; `0wd` is the day itself.
fn add_workdays(days: i64, n: i64) -> Option<i64> {
    if n == 0 {
        return Some(days);
    }
    let sign = n.signum();
    // From a weekend, start from the working day before it, in the direction of travel.
    let mut d = days;
    while is_weekend(d) {
        d -= sign;
    }
    // Whole weeks of 5 working days, then the rest one working day at a time.
    d = d.checked_add((n / 5).checked_mul(7)?)?;
    for _ in 0..(n % 5).abs() {
        d += sign;
        while is_weekend(d) {
            d += sign;
        }
    }
    Some(d)
}

/// Working days from day `from` to day `to`: those in `(from, to]` (negative when `to` is earlier),
/// so that `from + n wd = to` when `to` is a working day.
fn workdays_between(from: i64, to: i64) -> i64 {
    // Working days before day `d`, counted from Monday 1969-12-29 (day -3).
    let before = |d: i64| {
        let wd = i64::from(weekday(d));
        (d - wd + 1 + 3).div_euclid(7) * 5 + (wd - 1).min(5)
    };
    before(to + 1) - before(from + 1)
}

// ---------------------------------------------------------------- requests

/// Whether a request is a date request (see `request`): it starts with `now` / `today` or an ISO
/// date, or its second word is an input format (`1791644400 unix`, `20261010 iso`), or it is
/// text in the locale's format followed by `local`.
pub fn is_date_request(input: &str) -> bool {
    // A city (`Paris`), or a time of day first (`10:00AM Par…`, being typed).
    let words: Vec<&str> = input.split_whitespace().collect();
    if names_city(input) || leading_time(&words).is_some() {
        return true;
    }
    match take_zone(input) {
        // A zone word (`2026/12/04 GMT+2`) makes it a date; a wrong one is reported as a date error.
        Ok((rest, Some(_))) => rest.is_empty() || looks_like_date(&rest) || read_date(&rest, None, Order::of_locale()).is_some(),
        Ok((rest, None)) => looks_like_date(&rest),
        Err(_) => true,
    }
}

fn looks_like_date(input: &str) -> bool {
    let words: Vec<&str> = input.split_whitespace().collect();
    let Some(first) = words.first() else { return false };
    // `… num` asks for a number; `2026-12-02/12` is arithmetic (bug 005).
    if words.last().is_some_and(|w| crate::config::is_number_word(w)) || arithmetic_after_iso(input) {
        return false;
    }
    starts_with_keyword(first)
        || leading_iso(first).is_some()
        || words.get(1).is_some_and(|w| unix_scale(w).is_some() || *w == "iso")
        || (words[1..].contains(&"local") || words[1..].contains(&"date")) && !first.starts_with('-')
        || words[1..].iter().any(|w| Pattern::parse(w).is_some())
        || guessed(&words).is_some()
        || written_date_then_math(input).is_some()
        // A date with a month name, alone or before a format (also an impossible one: `31 Feb 2026`
        // is a wrong date, not a unit conversion).
        || numeric_from_named(input.trim()).is_some()
        || !first.starts_with('-') && timed_date(&words).is_some()
        || words.len() > 1 && numeric_from_named(&words[..words.len() - 1].join(" ")).is_some()
}

/// A written date followed by date math (`2026/12/04 - 12d`): subtracting or adding a duration
/// makes the value before it a date (bug 006). The byte position of that `+` / `-` in `text`
/// (trimmed at the start), at the first one after which a duration follows and before which a
/// written date stands.
fn written_date_then_math(text: &str) -> Option<usize> {
    let text = text.trim_start();
    text.char_indices().filter(|&(_, c)| c == '+' || c == '-').map(|(i, _)| i).find(|&i| {
        let date = text[..i].trim();
        !date.is_empty() && starts_with_duration(&text[i + 1..]) && read_date(date, None, Order::of_locale()).is_some()
    })
}

/// Whether `text` starts with a duration: a number, optional spaces, a time unit (`12d`, `3 hours`).
fn starts_with_duration(text: &str) -> bool {
    static CONFIG: std::sync::OnceLock<Config> = std::sync::OnceLock::new();
    let config = CONFIG.get_or_init(Config::default);
    let text = text.trim_start();
    let n = text.bytes().take_while(|b| b.is_ascii_digit() || *b == b'.').count();
    let unit = text[n..].trim_start();
    let unit = &unit[..unit.find(|c: char| c.is_whitespace() || c == '+' || c == '-').unwrap_or(unit.len())];
    n > 0 && time_unit(config, unit).is_some()
}

/// Whether `text` is a date value on its own: `now`, an ISO date, a date request, or a written
/// date (`12/04/26 3:20PM`). Used to tell an input format from an output one.
#[cfg_attr(not(feature = "tui"), allow(dead_code))]
pub fn is_date_value(text: &str) -> bool {
    is_date_request(text) || read_date(text, None, Order::of_locale()).is_some()
}

/// A written date followed by a single output format (`12/04/26 3:20PM unix`): the date is read
/// with a guessed order.
fn guessed(words: &[&str]) -> Option<Reading> {
    let (last, before) = words.split_last()?;
    if before.is_empty() || !is_format(last) {
        return None;
    }
    read_date(&before.join(" "), None, Order::of_locale())
}

/// An ISO date shape followed by `/`, `*`, `^`, `x` or `(`: arithmetic, not a date
/// (`2026-12-02/12` = 2026 − 12 − 2/12). Date math only uses `+` and `-`.
fn arithmetic_after_iso(input: &str) -> bool {
    let t = input.trim_start();
    let Some(first) = t.split_whitespace().next() else { return false };
    let Some((_, len)) = leading_iso(first) else { return false };
    t[len..].trim_start().starts_with(['/', '*', '^', 'x', 'X', '('])
}

/// Whether a request writes seconds: `10:00:30`, `T150405` (ISO basic), a Unix timestamp with a
/// fraction aside.
fn writes_seconds(text: &str) -> bool {
    let b = text.as_bytes();
    let digit = |i: usize| b.get(i).is_some_and(u8::is_ascii_digit);
    (0..b.len()).any(|i| {
        (b[i] == b':' && digit(i + 1) && digit(i + 2) && b.get(i + 3) == Some(&b':'))
            || ((b[i] == b'T' || b[i] == b't') && (1..=6).all(|k| digit(i + k)))
    })
}

/// A written date with a time at the start of `words` (`2026/12/04 10:00 PM`, `2026-12-04 22:00`,
/// `2026-12-04T10:00PM`): how many words, and the reading. Not an ISO literal on its own
/// (`2026-12-04T22:00Z`), which keeps its ISO format.
fn timed_date(words: &[&str]) -> Option<(usize, Reading)> {
    (1..=words.len().min(8)).rev().find_map(|n| {
        if n == 1 && parse_iso(words[0]).is_some() {
            return None;
        }
        let reading = read_date(&words[..n].join(" "), None, Order::of_locale())?;
        (reading.pattern.clock != Clock::None).then_some((n, reading))
    })
}

/// For the stdin rule: a first argument that is a date value.
pub fn starts_date(token: &str) -> bool {
    starts_with_keyword(token)
        || leading_iso(token).is_some()
        || gmt_zone(token).is_some()
        // A month name first (`September 12, 2026`, also quoted as one argument).
        || token.split(|c: char| !c.is_alphabetic()).next().is_some_and(|w| month_named(w).is_some())
}

fn starts_with_keyword(word: &str) -> bool {
    ["now", "today"].iter().any(|k| {
        word.strip_prefix(k).is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '+', '-']))
    })
}

/// The longest prefix of `word` that is an unambiguous ISO date, and its length.
fn leading_iso(word: &str) -> Option<(Iso, usize)> {
    (8..=word.len().min(40)).rev().filter(|&n| word.is_char_boundary(n)).find_map(|n| {
        parse_iso(&word[..n]).filter(Iso::unambiguous).map(|iso| (iso, n))
    })
}

struct Parser<'a> {
    s: &'a str,
    pos: usize,
    /// How guessed dates were read, for the user to check.
    notes: Vec<String>,
    /// The format the first date was written in: the default target.
    input_format: Option<String>,
    /// The zone of dates written without one: the `GMT±N` word, else the device's.
    zone: Zone,
    /// The `GMT±N` word, when given (a date with its own offset too is an error).
    zone_word: Option<String>,
    /// The cities of the request (`10:00 New York to Paris`), taken out of it.
    cities: Option<Cities>,
    /// `--verbose`: the date in ISO too (`<iso> iso = <value> <format>`), not only the result.
    verbose: bool,
}

/// The cities of a request: the target (shown in its zone) and, before `to`, the source (the zone
/// the request is written in), with the words they were written as.
#[derive(Debug, Clone)]
struct Cities {
    source: Option<tz::City>,
    target: tz::City,
    text: String,
}

/// Takes the cities out of a request: a city at the end is the target (`10:00AM Paris`), and one
/// before it the source, with or without `to` (`10:00AM New York to Paris`, `10:00AM New York
/// Paris`). `Ok(None)`: no city. A city elsewhere, or
/// a zone that is not installed, is an error.
fn take_cities(input: &str) -> Result<(String, Option<Cities>), String> {
    let words: Vec<&str> = input.split_whitespace().collect();
    // The cities found: (first word, word count, city).
    let mut found = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let longest = tz::longest_name().min(words.len() - i);
        let hit = (1..=longest).rev().find_map(|n| tz::city(&words[i..i + n].join(" ")).map(|c| (n, c)));
        match hit {
            Some((n, city)) => {
                found.push((i, n, city?));
                i += n;
            }
            None => i += 1,
        }
    }
    let Some(&(t_at, t_len, target)) = found.last() else { return Ok((input.to_string(), None)) };
    // Being typed: `10:00AM New York to`.
    if t_at + t_len + 1 == words.len() && ["to", "->"].contains(&words[t_at + t_len]) {
        return Err(format!("now the city to convert to: `{} to Paris`", target.name));
    }
    let misplaced = || format!("put the city at the end (`10:00 Paris`), or `<city> [to] <city>`: `{}`", words[found[0].0]);
    if t_at + t_len != words.len() {
        return Err(misplaced());
    }
    let (source, start) = match found[..found.len() - 1] {
        [] => (None, t_at),
        [(s_at, s_len, source)] if s_at + s_len + 1 == t_at && ["to", "->"].contains(&words[t_at - 1]) => (Some(source), s_at),
        // `to` is optional: the source right before the target.
        [(s_at, s_len, source)] if s_at + s_len == t_at => (Some(source), s_at),
        _ => return Err(misplaced()),
    };
    // `to` with no city before it: `10:00 to Paris` is `10:00 Paris`.
    let start = if source.is_none() && start > 0 && ["to", "->"].contains(&words[start - 1]) { start - 1 } else { start };
    let text = words[start..].join(" ");
    Ok((words[..start].join(" "), Some(Cities { source, target, text })))
}

/// A time of day alone (`10:00AM`, `10 pm`, `22:00`, `9:30:15`), on `day` (year, month, day).
fn read_time_only(text: &str, (y, m, d): (i64, u32, u32)) -> Option<Reading> {
    // `10AM`, `10 pm`: on the hour.
    let t = text.trim();
    let n = t.bytes().take_while(u8::is_ascii_digit).count();
    let after = t[n..].trim_start();
    let meridiem = ["am", "pm", "a.m.", "p.m."].contains(&after.to_ascii_lowercase().as_str());
    let t = if (1..=2).contains(&n) && meridiem { format!("{}:00 {after}", &t[..n]) } else { t.to_string() };
    if !t.contains(':') {
        return None;
    }
    let reading = read_date(&format!("{}/{m:02}/{d:02} {t}", year(y)), Some(Fields::Ymd4), Order::YearFirst)?;
    (reading.pattern.clock != Clock::None).then_some(reading)
}

impl<'a> Parser<'a> {
    /// A zone is given for the whole request: a `GMT±N` word or a source city (`New York to …`).
    fn zone_given(&self) -> bool {
        self.zone_word.is_some() || self.cities.as_ref().is_some_and(|c| c.source.is_some())
    }

    /// The zone of a date read with `offset` (its own, if any): its own, else the request's.
    fn zone_for(&self, offset: Option<i32>, text: &str) -> Result<Zone, TimeError> {
        match (offset, self.zone_given()) {
            (Some(_), true) => usage(format!("`{text}` has its own time zone, and a GMT zone or a city is given too")),
            (Some(o), false) => Ok(Zone::Fixed(o)),
            (None, _) => Ok(self.zone),
        }
    }

    fn rest(&self) -> &'a str {
        &self.s[self.pos..]
    }

    fn skip_ws(&mut self) {
        self.pos = self.s.len() - self.rest().trim_start().len();
    }

    /// The next whitespace-separated word, without consuming it.
    fn word(&self) -> &'a str {
        self.rest().trim_start().split_whitespace().next().unwrap_or("")
    }

    fn take_word(&mut self) -> &'a str {
        self.skip_ws();
        let w = self.word();
        self.pos += w.len();
        w
    }

    /// A date value; `first` allows the forms that need an input-format word.
    fn value(&mut self, first: bool) -> Result<Instant, TimeError> {
        self.skip_ws();
        let rest = self.rest();
        // `<text> date`: a loosely written date (checked first: `2026-04-12 date` is one too).
        if first {
            let words: Vec<&str> = rest.split_whitespace().collect();
            if let Some(i) = words.iter().position(|w| *w == "date").filter(|&i| i > 0) {
                let text = words[..i].join(" ");
                for _ in 0..=i {
                    self.take_word();
                }
                let order = Order::of_locale();
                let reading = read_date(&text, None, order)
                    .ok_or_else(|| TimeError::Usage(format!("`{text}` is not a date ({})", order.describe())))?;
                self.unambiguous(&text, &reading)?;
                self.note(&text, &reading);
                self.input_format = Some(reading.pattern.name());
                return from_civil(&reading.civil, self.zone_for(reading.offset, &text)?);
            }
            // `<text> <pattern> <format>`: read with that pattern exactly.
            if let Some(i) = (1..words.len().saturating_sub(1)).find(|&i| Pattern::parse(words[i]).is_some()) {
                let pattern = Pattern::parse(words[i]).unwrap();
                let text = words[..i].join(" ");
                for _ in 0..=i {
                    self.take_word();
                }
                let reading = read_with_pattern(&text, pattern)
                    .ok_or_else(|| TimeError::Usage(format!("`{text}` is not a date in the {} format", pattern.name())))?;
                self.input_format = Some(pattern.name());
                return from_civil(&reading.civil, self.zone_for(reading.offset, &text)?);
            }
        }
        for keyword in ["now", "today"] {
            if let Some(after) = rest.strip_prefix(keyword) {
                if after.is_empty() || after.starts_with([' ', '+', '-']) {
                    self.pos += keyword.len();
                    // In the request's zone: `now GMT+2` shows +02:00; `today` starts there.
                    let now = Instant { zone: self.zone, ..now() };
                    if keyword == "now" {
                        return Ok(now);
                    }
                    let c = now.civil();
                    return from_civil(&Civil { year: c.year, month: c.month, day: c.day, ..Civil::default() }, self.zone);
                }
            }
        }
        // A written date with a time (`2026/12/04 10:00 PM`, `2026-12-04 22:00`): read whole, before
        // its date part is taken for an ISO date. Year first with `-` is ISO-like: shown in ISO.
        // A time of day alone needs a city (`10:00AM Paris`).
        if first && self.cities.is_none() {
            let words: Vec<&str> = rest.split_whitespace().collect();
            if let Some(n) = leading_time(&words) {
                return match words[n..].join(" ") {
                    after if after.is_empty() => usage("a time alone takes a city: `10:00AM Paris`"),
                    after => usage(format!("`{after}` is not a city (`10:00AM Paris`, `10:00AM New York to Paris`)")),
                };
            }
        }
        // With a city, a time of day alone (`10:00AM Paris`): today, in the request's zone.
        if first && self.cities.is_some() {
            let words: Vec<&str> = rest.split_whitespace().collect();
            let c = Instant { zone: self.zone, ..now() }.civil();
            for n in (1..=words.len().min(3)).rev() {
                if let Some(reading) = read_time_only(&words[..n].join(" "), (c.year, c.month, c.day)) {
                    for _ in 0..n {
                        self.take_word();
                    }
                    return from_civil(&reading.civil, self.zone);
                }
            }
        }
        if first {
            let words: Vec<&str> = rest.split_whitespace().collect();
            if let Some((n, reading)) = timed_date(&words) {
                let text = words[..n].join(" ");
                for _ in 0..n {
                    self.take_word();
                }
                self.unambiguous(&text, &reading)?;
                self.note(&text, &reading);
                let iso_like = reading.pattern.fields == Fields::Ymd4 && text.as_bytes().get(4) == Some(&b'-');
                self.input_format = Some(if iso_like { "iso".to_string() } else { reading.pattern.name() });
                return from_civil(&reading.civil, self.zone_for(reading.offset, &text)?);
            }
        }
        let word = self.word();
        if let Some((iso, len)) = leading_iso(word) {
            self.pos += len;
            if first {
                self.input_format = Some(iso_format(&iso).to_string());
            }
            let zone = self.zone_for(iso.offset, &word[..len])?;
            return iso_instant_in(&iso, zone);
        }
        if first {
            let words: Vec<&str> = rest.split_whitespace().collect();
            // `<number> unix…` is a timestamp and `<literal> iso` an ISO date, the word being the input
            // format. Anything else before it (`2026/12/04 unix`) is a written date with its target:
            // guessed below (bug 004).
            if let [value, format, ..] = words[..] {
                if let Some(ns) = unix_scale(format).and_then(|scale| scaled(value, scale)) {
                    self.take_word();
                    self.take_word();
                    self.input_format = Some(format.to_string());
                    return Instant { ns, zone: self.zone }.in_range();
                }
                if let Some(iso) = (format == "iso").then(|| parse_iso(value)).flatten() {
                    self.take_word();
                    self.take_word();
                    self.input_format = Some(iso_format(&iso).to_string());
                    let zone = self.zone_for(iso.offset, value)?;
                    return iso_instant_in(&iso, zone);
                }
            }
            if let Some(i) = words.iter().position(|w| *w == "local") {
                let text = words[..i].join(" ");
                for _ in 0..=i {
                    self.take_word();
                }
                if self.zone_given() {
                    return usage("the local format is read in the device's time zone: remove the GMT zone");
                }
                let secs = sys::parse_local(&text)
                    .ok_or_else(|| TimeError::Usage(format!("`{text}` is not a date in the local format")))?;
                self.input_format = Some("local".into());
                return Instant { ns: i128::from(secs) * NS, zone: Zone::Local }.in_range();
            }
            // A written date followed by date math (`2026/12/04 - 12d`): guess how to read the date;
            // the date math (and a target, if any) follows.
            if let Some(i) = written_date_then_math(rest) {
                let text = rest[..i].trim().to_string();
                let reading = read_date(&text, None, Order::of_locale()).expect("checked by written_date_then_math");
                self.pos += i;
                self.unambiguous(&text, &reading)?;
                self.note(&text, &reading);
                self.input_format = Some(reading.pattern.name());
                return from_civil(&reading.civil, self.zone_for(reading.offset, &text)?);
            }
            // A written date and one format, the target: guess how to read the date.
            if let Some(reading) = guessed(&words) {
                let text = words[..words.len() - 1].join(" ");
                for _ in 0..words.len() - 1 {
                    self.take_word();
                }
                self.unambiguous(&text, &reading)?;
                self.note(&text, &reading);
                return from_civil(&reading.civil, self.zone_for(reading.offset, &text)?);
            }
        }
        // A written date on its own, with a GMT zone (`2026/12/04 GMT+2`) or a month name
        // (`12 Sept 2026`).
        if first {
            let text = rest.trim().to_string();
            let reading = read_date(&text, None, Order::of_locale())
                .filter(|r| self.zone_given() || r.month_name.is_some());
            if let Some(reading) = reading {
                self.pos = self.s.len();
                self.unambiguous(&text, &reading)?;
                self.note(&text, &reading);
                self.input_format = Some(reading.pattern.name());
                return from_civil(&reading.civil, self.zone_for(reading.offset, &text)?);
            }
        }
        // A later date of a difference, written (`2026-12-25 - 12 Sept 2026 d`), or a first one with
        // a month name (`12 Dec 2026 - 12 Sept 2026 wd`): the longest run of words that reads as a
        // date.
        {
            let words: Vec<&str> = rest.split_whitespace().take(8).collect();
            for n in (1..=words.len()).rev() {
                let text = words[..n].join(" ");
                let reading = read_date(&text, None, Order::of_locale()).filter(|r| !first || r.month_name.is_some());
                if let Some(reading) = reading {
                    if first {
                        self.input_format = Some(reading.pattern.name());
                    }
                    for _ in 0..n {
                        self.take_word();
                    }
                    self.unambiguous(&text, &reading)?;
                    self.note(&text, &reading);
                    return from_civil(&reading.civil, self.zone_for(reading.offset, &text)?);
                }
            }
        }
        // A date with a month name that does not exist (`31 Feb 2026`).
        let words: Vec<&str> = rest.split_whitespace().collect();
        for n in 1..=words.len() {
            let text = words[..n].join(" ");
            if numeric_from_named(&text).is_some() {
                return usage(format!("`{text}` is not a date (no such day in that month)"));
            }
        }
        usage(format!("`{word}` is not a date"))
    }

    /// A guessed date with more than one valid reading is not guessed: the operator chooses. Each
    /// choice is the request with the pattern written in after the date (it replaces `date`).
    fn unambiguous(&self, text: &str, reading: &Reading) -> Result<(), TimeError> {
        if reading.readings.len() < 2 {
            return Ok(());
        }
        let rest = self.rest().trim();
        let rest = rest.strip_prefix("date").map_or(rest, str::trim_start);
        // With a city, the city is the target: no `iso`.
        let rest = if rest.is_empty() && self.cities.is_none() { "iso" } else { rest };
        let after = [self.zone_word.as_deref(), self.cities.as_ref().map(|c| c.text.as_str())];
        let choices = reading
            .readings
            .iter()
            .map(|(pattern, y, m, d)| {
                let name = pattern.name();
                let parts = [Some(text), Some(name.as_str()), Some(rest), after[0], after[1]];
                Choice {
                    pattern: pattern.name(),
                    date: format!("{}-{m:02}-{d:02}", year(*y)),
                    request: parts.iter().flatten().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join(" "),
                }
            })
            .collect();
        Err(TimeError::Ambiguous(Ambiguity { text: text.to_string(), choices, kind: AmbiguityKind::Date, note: String::new() }))
    }

    /// How a guessed date was read: `read 12/04/26 3:20PM as DD/MM/YY-12h (the locale's order)`.
    fn note(&mut self, text: &str, reading: &Reading) {
        if let Some(m) = &reading.month_name {
            let c = &reading.civil;
            let name = ENGLISH_MONTHS[m.month as usize - 1];
            let typo = if m.typo { ", read as a misspelling" } else { "" };
            self.notes.push(format!("read `{text}` as {}-{:02}-{:02} (`{}`: {name}{typo})", year(c.year), c.month, c.day, m.written));
            return;
        }
        let why = match (reading.by_locale, reading.pattern.fields) {
            (_, Fields::Ymd4) => "year first",
            (true, _) => "the locale's order",
            (false, _) => "the only valid order",
        };
        self.notes.push(format!("read `{text}` as {} ({why})", reading.pattern.name()));
    }

    /// `<number>[ ]<time unit>` at the cursor: (amount, unit symbol), consumed; `None` if it isn't one.
    fn duration(&mut self, config: &Config) -> Option<(&'a str, &'static str)> {
        self.skip_ws();
        let rest = self.rest();
        let n = rest.bytes().take_while(|b| b.is_ascii_digit() || *b == b'.').count();
        if n == 0 {
            return None;
        }
        let after = &rest[n..];
        let unit_text = after.trim_start();
        let len = unit_text.find(|c: char| c.is_whitespace() || c == '+' || c == '-').unwrap_or(unit_text.len());
        let symbol = time_unit(config, &unit_text[..len])?;
        self.pos += n + (after.len() - unit_text.len()) + len;
        Some((&rest[..n], symbol))
    }
}

/// The symbol of a time unit name (symbol or alias, case-insensitive), if it is one.
fn time_unit(config: &Config, name: &str) -> Option<&'static str> {
    if WORKDAY_NAMES.iter().any(|w| w.eq_ignore_ascii_case(name)) {
        return Some(WORKDAY);
    }
    let (category, unit) = convert::find_unit(config, name)?;
    (category.name == "Time").then_some(unit.symbol)
}

/// Evaluates a date request: the result and notes on how guessed dates were read. `None` if
/// `input` is not one (then it is a numeric request). `precision` formats a difference given in a
/// time unit.
/// `verbose`: the ISO side too (`<iso> iso = <value> <format>`), not only the result. A request
/// that fails with a misspelt city (`10:00 Tokio`, within 2 letters) is an `Ambiguous` error with
/// the cities it may be, each with the corrected request.
pub fn request(config: &Config, input: &str, precision: usize, verbose: bool) -> Option<Result<(String, Vec<String>), TimeError>> {
    request_with(config, input, precision, verbose, true)
}

/// `request`; with `typos` false, a request that fails is not checked for misspelt places (the
/// check itself tries its corrections this way).
fn request_with(config: &Config, input: &str, precision: usize, verbose: bool, typos: bool) -> Option<Result<(String, Vec<String>), TimeError>> {
    // A country: its city (`Italy` → Rome), or which one when it has several zones (`USA`).
    match countries_in(config, input) {
        Some(Err(a)) => return Some(Err(TimeError::Ambiguous(a))),
        Some(Ok((rewritten, note))) => {
            return request_with(config, &rewritten, precision, verbose, typos).map(|r| {
                r.map(|(out, mut notes)| {
                    notes.insert(0, note);
                    (out, notes)
                })
            })
        }
        None => {}
    }
    match exact_request(config, input, precision, verbose) {
        result @ (Some(Ok(_)) | Some(Err(TimeError::Ambiguous(_)))) => result,
        other if typos => place_typos(config, input, precision, verbose).map(Err).or(other),
        other => other,
    }
}

fn exact_request(config: &Config, input: &str, precision: usize, verbose: bool) -> Option<Result<(String, Vec<String>), TimeError>> {
    if !is_date_request(input) {
        return None;
    }
    // A `GMT±N` word is the zone of the dates written without one (`GMT+2` alone: now there).
    let (rest, zone_word) = match take_zone(input) {
        Ok(found) => found,
        Err(msg) => return Some(Err(TimeError::Usage(msg))),
    };
    // Cities: `Paris` (now there), `10:00AM Paris`, `10:00AM New York to Paris`.
    let (rest, cities) = match take_cities(&rest) {
        Ok(found) => found,
        Err(msg) => return Some(Err(TimeError::Usage(msg))),
    };
    let source = cities.as_ref().and_then(|c| c.source);
    if let (Some(source), Some((word, _))) = (source, &zone_word) {
        return Some(Err(TimeError::Usage(format!("two time zones: `{word}` and {}", source.name))));
    }
    let rest = if rest.trim().is_empty() { "now".to_string() } else { rest };
    let zone = match (source, &zone_word) {
        (Some(city), _) => Zone::Named(city.tz),
        (None, Some((_, offset))) => Zone::Fixed(*offset),
        (None, None) => Zone::Local,
    };
    let zone_word = zone_word.map(|(word, _)| word);
    let mut p = Parser { s: rest.trim(), pos: 0, notes: Vec::new(), input_format: None, zone, zone_word, cities, verbose };
    Some(evaluate(config, &mut p, precision).map(|out| (out, p.notes)))
}

fn evaluate(config: &Config, p: &mut Parser, precision: usize) -> Result<String, TimeError> {
    // ISO results show seconds when the request wrote some (`10:00:30`), or they are not zero.
    let seconds = writes_seconds(p.s);
    let mut date = p.value(true)?;
    // The difference of two dates: (the first, the second).
    let mut difference: Option<(Instant, Instant)> = None;
    // Date math was done (`+ 5wd`): without a target, only the result is shown.
    let mut stepped = false;
    loop {
        p.skip_ws();
        let sign = match p.rest().chars().next() {
            Some('+') => 1,
            Some('-') => -1,
            _ => break,
        };
        p.pos += 1;
        if let Some((amount, symbol)) = p.duration(config) {
            if difference.is_some() {
                return usage("a difference is a duration: nothing can be added to it");
            }
            date = add(date, amount, symbol, sign)?;
            stepped = true;
        } else if sign == -1 && difference.is_none() {
            let other = p.value(false)?;
            difference = Some((date, other));
        } else {
            return usage(format!("expected a duration (like 3d or 90min) after `{}`", if sign > 0 { '+' } else { '-' }));
        }
    }
    let target = p.take_word();
    if !p.rest().trim().is_empty() {
        return usage(format!("unexpected `{}`", p.rest().trim()));
    }
    if let Some(cities) = p.cities.clone() {
        return city_result(&cities, date, difference.is_some(), target, p.s.trim() == "now", p.verbose, seconds);
    }
    match (difference, target) {
        (Some((a, b)), "") => Ok(breakdown(a.ns - b.ns)),
        (Some((a, b)), unit) => {
            let symbol = time_unit(config, unit).ok_or_else(|| {
                TimeError::Usage(format!("`{unit}`: a difference of dates is a duration; give a time unit (d, h, s…)"))
            })?;
            if symbol == WORKDAY {
                // Working days between the two days (each in its own zone), times of day aside.
                let day = |i: &Instant| {
                    let c = i.civil();
                    days_from_civil(c.year, c.month, c.day)
                };
                return Ok(format!("{} {WORKDAY}", workdays_between(day(&b), day(&a))));
            }
            let seconds = (a.ns - b.ns) as f64 / NS as f64;
            let (value, unit) = convert::convert(config, seconds, "s", symbol).map_err(|_| TimeError::Range)?;
            Ok(format!("{} {}", convert::format_value(value, precision), unit.symbol))
        }
        // No target: the format the date was written in. After date math only the result
        // (`2026/12/04 + 5wd` → `2026/12/11 YYYY/MM/DD`); a date alone is read into ISO too, as
        // its own format would only repeat it (`1791644400 unix` → `… iso = 1791644400 unix`).
        // `now` and plain `iso`: ISO.
        // A date alone (no math): its ISO reading (with --verbose, both sides).
        // Not verbose, the date for people (`12 Sept 26`, `12 Sept 26 20:00 GMT+2`) wherever it would
        // be ISO: a date alone, `now`, an ISO date after date math.
        (None, "") => match p.input_format.take() {
            Some(word) if word != "iso" && stepped && (p.verbose || !word.starts_with("iso")) => {
                Ok(format!("{} {word}", format_as(&date, &word, seconds)?))
            }
            Some(word) if word != "iso" && p.verbose => both_sides(&date, &word, seconds),
            _ if p.verbose => Ok(format!("{} iso", format_as(&date, "iso", seconds)?)),
            _ => Ok(human(&date, date_only(&date, p.s))),
        },
        (None, "iso") => Ok(format!("{} iso", format_as(&date, "iso", seconds)?)),
        // A target: only the converted value (with --verbose, the ISO side too).
        (None, word) if is_format(word) && p.verbose => both_sides(&date, word, seconds),
        (None, word) if is_format(word) => Ok(format!("{} {word}", format_as(&date, word, seconds)?)),
        // A time being typed after the date (`2026-12-04 10`, `10:`): not a format.
        (None, word) if word.starts_with(|c: char| c.is_ascii_digit()) && word.bytes().all(|b| b.is_ascii_digit() || b == b':') => {
            usage(format!("`{word}`: write the time as H:MM, like 10:00 (then AM / PM if 12-hour)"))
        }
        (None, word) => usage(format!("unknown date format `{word}` (formats: {})", DATE_WORDS[3..].join(", "))),
    }
}

/// A date shown in a city's zone: `<date> iso = <date there> Paris`, or with a source city
/// `<date> New York = <date there> Paris`, or for the city alone (now) only the time there. To the
/// minute, unless the request wrote seconds.
/// `alone`: the city alone (now there). Not `verbose`: the time there for people (`11 Oct 26 16:00
/// GMT+2 Paris`).
fn city_result(cities: &Cities, date: Instant, difference: bool, target: &str, alone: bool, verbose: bool, seconds: bool) -> Result<String, TimeError> {
    let to = cities.target;
    if difference {
        return usage(format!("a difference of dates is a duration: it has no time in {}", to.name));
    }
    if !target.is_empty() {
        return usage(format!("`{target}`: the city is the target (`10:00 {}`)", to.name));
    }
    let date = if seconds { date } else { Instant { ns: date.ns - date.ns.rem_euclid(60 * NS), ..date } };
    let iso = |zone: Zone| format_as(&Instant { zone, ..date }, "iso", seconds);
    if !verbose {
        return Ok(format!("{} {}", human(&Instant { zone: Zone::Named(to.tz), ..date }, false), to.name));
    }
    let there = format!("{} {}", iso(Zone::Named(to.tz))?, to.name);
    Ok(match cities.source {
        _ if alone => there,
        Some(from) => format!("{} {} = {there}", iso(Zone::Named(from.tz))?, from.name),
        None => format!("{} iso = {there}", iso(date.zone)?),
    })
}

/// Whether a date is shown without its time: midnight, and the request wrote no time (`12 Sept 2026`,
/// `2026-12-04`, a Unix timestamp at midnight), and not `now`.
fn date_only(date: &Instant, request: &str) -> bool {
    let c = date.civil();
    let midnight = c.hour == 0 && c.minute == 0 && c.second == 0 && c.nano == 0;
    let b = request.as_bytes();
    let writes_time = (1..b.len()).any(|i| b[i] == b':' && b[i - 1].is_ascii_digit());
    midnight && !writes_time && !request.contains("now")
}

/// The words at the start of `words` that are a time of day (`10:00AM`, `10 pm`), if any.
fn leading_time(words: &[&str]) -> Option<usize> {
    (1..=words.len().min(3)).rev().find(|&n| read_time_only(&words[..n].join(" "), (2000, 1, 1)).is_some())
}

/// The countries of a request, each written as its city: `Ok` with the request rewritten
/// (`10:00 Italy to Japan` → `10:00 Rome to Tokyo`) and a note, or the choice of a country with
/// several time zones (the first such one), each with the request using that zone's city.
/// `None`: no country. A city's name (`Mexico City`, `Singapore`) or a unit (`us`) is not one.
fn countries_in(config: &Config, input: &str) -> Option<Result<(String, String), Ambiguity>> {
    let words: Vec<&str> = input.split_whitespace().collect();
    let mut out: Vec<String> = Vec::new();
    let mut named: Vec<String> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let longest = tz::longest_country().max(tz::longest_name()).min(words.len() - i);
        let mut hit = None;
        for n in (1..=longest).rev() {
            let span = words[i..i + n].join(" ");
            if tz::city(&span).is_some() {
                break;
            }
            if let Some(c) = tz::country(&span).filter(|_| !reserved_word(config, &span)) {
                hit = Some((n, span, c));
                break;
            }
        }
        let Some((n, span, country)) = hit else {
            out.push(words[i].to_string());
            i += 1;
            continue;
        };
        if let [one] = &country.cities[..] {
            named.push(format!("{} is {one}", country.name));
            out.push(one.clone());
            i += n;
            continue;
        }
        // Several zones: one choice per zone, with its most relevant city and its offset now.
        let now = now().ns.div_euclid(NS) as i64;
        let choices = country
            .cities
            .iter()
            .filter_map(|city| {
                let tz = tz::city(city)?.ok()?.tz;
                let offset = tz.offset_at(now);
                let gmt = match (offset.abs() / 3600, offset.abs() / 60 % 60) {
                    (0, 0) => "GMT".to_string(),
                    (h, 0) => format!("GMT{}{h}", if offset < 0 { '-' } else { '+' }),
                    (h, m) => format!("GMT{}{h}:{m:02}", if offset < 0 { '-' } else { '+' }),
                };
                let request = [&out.iter().map(String::as_str).collect::<Vec<_>>()[..], &[city.as_str()][..], &words[i + n..]].concat().join(" ");
                Some(Choice { pattern: city.clone(), date: format!("{gmt} · {}", tz.name), request })
            })
            .collect();
        return Some(Err(Ambiguity { text: span, choices, kind: AmbiguityKind::Country, note: String::new() }));
    }
    (!named.is_empty()).then(|| Ok((out.join(" "), named.join(", "))))
}

/// Words that are not misspelt cities: units, formats, months, keywords.
fn known_word(config: &Config, text: &str) -> bool {
    tz::city(text).is_some() || tz::country(text).is_some() || reserved_word(config, text)
}

/// Words that are neither places nor misspelt ones: units, formats, months, keywords, currencies.
fn reserved_word(config: &Config, text: &str) -> bool {
    convert::find_unit(config, text).is_some() || date_word(text) || crate::currency::exact(text).is_some_and(|_| tz::country(text).is_none())
}

/// Whether a word is something else than a place to tally: a unit, a currency (not a country), a
/// format, a month, a keyword (`geo.rs`: such a word is not a city unless qualified).
pub fn reserved(config: &Config, text: &str) -> bool {
    convert::find_unit(config, text).is_some() || date_word(text) || crate::currency::exact(text).is_some_and(|_| tz::country(text).is_none())
}

/// Whether a word means a date or a place to tally: a format, a month, a keyword, a number word, a
/// time zone, a city or a country (`currency.rs` checks its names against it).
#[cfg(test)]
pub fn date_or_place_word(config: &Config, text: &str) -> bool {
    let _ = config;
    date_word(text) || tz::city(text).is_some() || tz::country(text).is_some()
}

/// Formats, months, keywords, number words, time zones.
fn date_word(text: &str) -> bool {
    is_format(text)
        || DATE_WORDS.contains(&text)
        || month_named(text).is_some()
        || gmt_zone(text).is_some()
        || crate::config::is_number_word(text)
        || ["to", "of", "de", "del", "the", "am", "pm", "a.m.", "p.m."].contains(&fold(text).as_str())
}

/// A place a misspelt word may be: its name, its zone (or "several time zones"), and whether it is
/// a country.
struct NearPlace {
    name: String,
    zone: String,
    country: bool,
}

/// The cities and countries within 2 letters of `text` (`Tokio`: Tokyo; `jappan`: Japan).
fn near_places(text: &str) -> Vec<NearPlace> {
    let cities = tz::near(text).into_iter().map(|c| NearPlace { name: c.name.to_string(), zone: c.tz.name.to_string(), country: false });
    let countries = tz::near_countries(text).into_iter().map(|c| {
        let zone = match &c.cities[..] {
            [one] => tz::city(one).and_then(Result::ok).map_or_else(|| one.clone(), |city| city.tz.name.to_string()),
            _ => "several time zones".to_string(),
        };
        NearPlace { name: c.name.clone(), zone, country: true }
    });
    // Only the nearest, cities and countries together: `Tokio` is Tokyo (1 letter), not Togo (2).
    let all: Vec<(usize, NearPlace)> = cities
        .chain(countries)
        .map(|p| {
            let names: Vec<&str> = match tz::country(&p.name) {
                Some(c) if p.country => std::iter::once(c.name.as_str()).chain(c.aliases.iter().map(String::as_str)).collect(),
                _ => vec![p.name.as_str()],
            };
            (tz::distance(text, &names), p)
        })
        .collect();
    let best = all.iter().map(|(d, _)| *d).min().unwrap_or(0);
    all.into_iter().filter(|(d, _)| *d == best).map(|(_, p)| p).collect()
}

/// A request that failed because of misspelt places (`10:00 Tokio`, `New Yrok to Paris`,
/// `10:00am jappan romme`): every run of up to 3 words, left to right, that is not a unit, format,
/// month or keyword and is within **2 letters** of a city or a country (case and spaces aside) is
/// corrected; the corrections, together, whose request works are an `Ambiguous` error to choose
/// from (at most 8). `None`: no such places.
fn place_typos(config: &Config, input: &str, precision: usize, verbose: bool) -> Option<TimeError> {
    let words: Vec<&str> = input.split_whitespace().collect();
    let wordlike = |w: &&str| w.chars().all(|c| c.is_alphabetic() || "'’-.".contains(c)) && !["to", "->"].contains(w);
    // The misspelt runs: (first word, word count, what they may be), longest first.
    let mut spans: Vec<(usize, usize, Vec<NearPlace>)> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let found = (1..=(words.len() - i).min(3)).rev().find_map(|n| {
            let span = &words[i..i + n];
            let text = span.join(" ");
            if !span.iter().all(wordlike) || known_word(config, &text) {
                return None;
            }
            let near = near_places(&text);
            (!near.is_empty()).then_some((n, near))
        });
        match found {
            Some((n, near)) => {
                spans.push((i, n, near));
                i += n;
            }
            None => i += 1,
        }
    }
    if spans.is_empty() {
        return None;
    }
    // Every combination of corrections (at most 8), each tried as a request.
    let mut combos: Vec<Vec<&NearPlace>> = vec![Vec::new()];
    for (_, _, near) in &spans {
        combos = combos.iter().flat_map(|c| near.iter().map(move |p| [c.as_slice(), &[p]].concat())).take(8).collect();
    }
    let choices: Vec<Choice> = combos
        .into_iter()
        .filter_map(|combo| {
            let mut out: Vec<&str> = Vec::new();
            let mut at = 0;
            for ((start, n, _), place) in spans.iter().zip(&combo) {
                out.extend(&words[at..*start]);
                out.push(&place.name);
                at = start + n;
            }
            out.extend(&words[at..]);
            let request = out.join(" ");
            let works = matches!(request_with(config, &request, precision, verbose, false), Some(Ok(_)) | Some(Err(TimeError::Ambiguous(_))));
            // Several corrections: each named with what it is (`Japan (country), Rome (city)`).
            let named = |p: &&NearPlace| match (spans.len(), p.country) {
                (1, _) => p.name.clone(),
                (_, true) => format!("{} (country)", p.name),
                (_, false) => format!("{} (city)", p.name),
            };
            works.then(|| Choice {
                pattern: combo.iter().map(named).collect::<Vec<_>>().join(", "),
                date: combo.iter().map(|p| p.zone.as_str()).collect::<Vec<_>>().join(", "),
                request,
            })
        })
        .collect();
    if choices.is_empty() {
        return None;
    }
    let misspelt: Vec<String> = spans.iter().map(|(i, n, _)| words[*i..i + n].join(" ")).collect();
    // What a single misspelt word is, from the places offered (`Tokio`: a city, not Togo).
    let offered = |p: &&NearPlace| choices.iter().any(|c| c.pattern.split(", ").any(|name| name == p.name || name.starts_with(&format!("{} (", p.name))));
    let note = match &spans[..] {
        [(_, _, near)] => {
            let near: Vec<&NearPlace> = near.iter().filter(offered).collect();
            let what = match (near.iter().all(|p| p.country), near.iter().any(|p| p.country)) {
                (true, _) => "a country",
                (false, false) => "a city",
                _ => "a known place",
            };
            format!("`{}` is not {what}", misspelt[0])
        }
        _ => format!("{} are not known places", misspelt.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(", ")),
    };
    Some(TimeError::Ambiguous(Ambiguity { text: misspelt.join(", "), choices, kind: AmbiguityKind::City, note }))
}

/// Whether a request is a misspelt city request (`Tokio`), for the stdin rule.
pub fn suggests_city(config: &Config, input: &str) -> bool {
    place_typos(config, input, 0, false).is_some()
}

/// Whether a request names a city (`Paris`, `10:00AM New York to Paris`), for the stdin rule.
pub fn names_city(input: &str) -> bool {
    take_zone(input).is_ok_and(|(rest, _)| !matches!(take_cities(&rest), Ok((_, None))))
        || countries_in(&Config::default(), input).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> Instant {
        let iso = parse_iso(s).unwrap_or_else(|| panic!("{s}"));
        iso_instant_in(&iso, Zone::Local).unwrap()
    }

    #[test]
    fn civil_round_trips() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        for days in (-4_000_000..4_000_000).step_by(997) {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        assert_eq!(civil_from_days(days_from_civil(-9999, 1, 1)), (-9999, 1, 1));
    }

    #[test]
    fn weeks() {
        let w = |y, m, d| iso_week(days_from_civil(y, m, d));
        assert_eq!(w(2020, 12, 31), (2020, 53, 4));
        assert_eq!(w(2021, 1, 3), (2020, 53, 7));
        assert_eq!(w(2021, 1, 4), (2021, 1, 1));
        assert_eq!(w(2026, 10, 10), (2026, 41, 6));
        assert_eq!(w(2008, 12, 29), (2009, 1, 1));
        assert_eq!(weeks_in_year(2020), 53);
        assert_eq!(weeks_in_year(2021), 52);
    }

    #[test]
    fn iso_forms_parse_to_the_same_instant() {
        let want = utc("2026-10-10T15:04:05Z");
        for s in [
            "2026-10-10T15:04:05Z",
            "20261010T150405Z",
            "2026-283T15:04:05Z",
            "2026283T150405Z",
            "2026-W41-6T15:04:05Z",
            "2026W416T150405Z",
            "2026-10-10T17:04:05+02:00",
            "2026-10-10T17:04:05+0200",
            "2026-10-10T10:04:05-05",
            "2026-10-10t15:04:05.000z",
        ] {
            assert_eq!(utc(s).ns, want.ns, "{s}");
        }
        assert_eq!(utc("2026-10-10T15:04:05,5Z").ns - want.ns, NS / 2);
        assert_eq!(utc("2026-10-10T15:04:05.123456789Z").ns - want.ns, 123_456_789);
        assert_eq!(utc("2026-10-10T24:00Z").ns, utc("2026-10-11T00:00Z").ns);
        assert_eq!(utc("2026-10-10T15:04Z").ns, want.ns - 5 * NS);
        assert_eq!(utc("2026-10-10T15Z").ns, want.ns - 245 * NS);
    }

    #[test]
    fn invalid_iso() {
        for s in ["2026-13-01", "2026-02-29", "2026-W54-1", "2026-W41-8", "2026-367", "2026-10-10T25:00", "2026-10-10T12:60", "2026-10-10x", "2026-10", "202610"] {
            assert!(parse_iso(s).is_none(), "{s}");
        }
        assert!(parse_iso("2024-02-29").is_some());
        assert!(parse_iso("2024-366").is_some());
    }

    #[test]
    fn ambiguous_forms_need_a_time() {
        let auto = |s: &str| parse_iso(s).is_some_and(|i| i.unambiguous());
        assert!(auto("2026-10-10") && auto("2026-W41-6") && auto("2026W416") && auto("20261010T1500"));
        assert!(!auto("20261010") && !auto("2026-283") && !auto("2026283"));
    }

    #[test]
    fn formats() {
        let t = utc("2026-10-10T15:04:05.25Z");
        let f = |w| format(&t, w).unwrap();
        assert_eq!(f("iso"), "2026-10-10T15:04:05.25Z");
        assert_eq!(f("iso-utc"), "2026-10-10T15:04:05.25Z");
        assert_eq!(f("iso-basic"), "20261010T150405.25Z");
        assert_eq!(f("iso-date"), "2026-10-10");
        assert_eq!(f("iso-time"), "15:04:05.25Z");
        assert_eq!(f("iso-ordinal"), "2026-283");
        assert_eq!(f("iso-week"), "2026-W41-6");
        assert_eq!(f("unix"), "1791644645.25");
        assert_eq!(f("unix-ms"), "1791644645250");
        assert_eq!(f("unix-us"), "1791644645250000");
        assert_eq!(f("unix-ns"), "1791644645250000000");
        let rome = Instant { zone: Zone::Fixed(7200), ..t };
        assert_eq!(format(&rome, "iso").unwrap(), "2026-10-10T17:04:05.25+02:00");
        assert_eq!(format(&rome, "iso-basic").unwrap(), "20261010T170405.25+0200");
        let before_1970 = utc("1969-12-31T23:59:59.5Z");
        assert_eq!(format(&before_1970, "unix").unwrap(), "-0.5");
        assert_eq!(format(&before_1970, "unix-ms").unwrap(), "-500");
    }

    #[test]
    fn calendar_math() {
        let z = |s: &str| utc(s);
        let plus = |t: Instant, a: &str, u: &str| format_as(&add(t, a, u, 1).unwrap(), "iso", false).unwrap();
        assert_eq!(plus(z("2026-01-31T10:00Z"), "1", "mo"), "2026-02-28T10:00Z");
        assert_eq!(plus(z("2024-02-29T00:00Z"), "1", "yr"), "2025-02-28T00:00Z");
        assert_eq!(plus(z("2024-02-29T00:00Z"), "4", "yr"), "2028-02-29T00:00Z");
        assert_eq!(plus(z("2026-12-31T23:00Z"), "2", "h"), "2027-01-01T01:00Z");
        assert_eq!(plus(z("2026-10-10T00:00Z"), "1.5", "h"), "2026-10-10T01:30Z");
        assert_eq!(plus(z("2026-10-10T00:00Z"), "3", "wk"), "2026-10-31T00:00Z");
        assert_eq!(plus(z("2026-10-10T00:00Z"), "1", "ns"), "2026-10-10T00:00:00.000000001Z");
        assert_eq!(format(&add(z("2026-03-31T00:00Z"), "1", "mo", -1).unwrap(), "iso-date").unwrap(), "2026-02-28");
        assert!(matches!(add(z("2026-10-10T00:00Z"), "1.5", "d", 1), Err(TimeError::Usage(_))));
        assert_eq!(add(z("9999-12-31T00:00Z"), "1", "yr", 1), Err(TimeError::Range));
    }

    #[test]
    fn differences() {
        assert_eq!(breakdown(75 * DAY_NS + (8 * 3600 + 6 * 60 + 51) * NS), "75d 8h 6min 51s");
        assert_eq!(breakdown(-(DAY_NS + 5 * NS)), "-1d 5s");
        assert_eq!(breakdown(1_500_000_001), "1.500000001s");
        assert_eq!(breakdown(0), "0s");
    }

    #[test]
    fn scaled_numbers() {
        assert_eq!(scaled("1.5", 3600), Some(5400));
        assert_eq!(scaled("1791644400", NS), Some(1_791_644_400 * NS));
        assert_eq!(scaled("-0.5", NS), Some(-NS / 2));
        assert_eq!(scaled("0.0000000005", NS), Some(1), "rounded to the nearest ns");
        assert_eq!(scaled("abc", NS), None);
        assert_eq!(scaled(".", NS), None);
        assert_eq!(decimal(1_250_000_000, 9), "1.25");
        assert_eq!(decimal(-1, 9), "-0.000000001");
    }

    fn req(input: &str) -> Result<String, TimeError> {
        request(&Config::default(), input, 4, true).expect("a date request").map(|(out, _)| out)
    }

    fn notes(input: &str) -> Vec<String> {
        request(&Config::default(), input, 4, true).expect("a date request").unwrap().1
    }

    #[test]
    fn requests_in_utc() {
        // Only zone-explicit inputs here; local time is tested end to end with TZ pinned.
        // Both sides, named; the iso target alone; the same text on both sides only once.
        assert_eq!(req("2026-10-10T15:00Z unix").unwrap(), "2026-10-10T15:00Z iso = 1791644400 unix");
        assert_eq!(req("2026-10-10T15:00Z iso").unwrap(), "2026-10-10T15:00Z iso");
        assert_eq!(req("2026-10-10T15:00Z+3d-90min iso-utc").unwrap(), "2026-10-13T13:30Z iso-utc");
        // No target: the input's format (a Z time: iso-utc).
        assert_eq!(req("2026-10-10T15:00Z + 3d").unwrap(), "2026-10-13T15:00Z iso-utc");
        assert_eq!(req("2026-01-31T00:00Z + 1 mo + 1 day").unwrap(), "2026-03-01T00:00Z iso-utc");
        assert_eq!(req("2026-10-10T15:00+02:00 + 1h").unwrap(), "2026-10-10T16:00+02:00 iso");
        // Only the result, not the ISO side too.
        assert_eq!(req("1791644400 unix + 1h").unwrap(), "1791648000 unix");
        assert_eq!(req("2026/04/12 00:00Z - 1d").unwrap(), "2026/04/11 00:00:00 +00:00 YYYY/MM/DD-24h-tz");
        assert_eq!(req("2026-10-10T15:00Z iso-week").unwrap(), "2026-10-10T15:00Z iso = 2026-W41-6 iso-week");
        assert_eq!(req("2026-12-25T00:00Z - 2026-10-10T15:00Z").unwrap(), "75d 9h");
        assert_eq!(req("2026-12-25T00:00Z - 2026-10-10T15:00Z d").unwrap(), "75.375 d");
        assert_eq!(req("2026-12-25T00:00Z - 2026-10-10T15:00Z hours").unwrap(), "1809 h");
        assert_eq!(req("20261010T150000Z iso-date").unwrap(), "2026-10-10T15:00:00Z iso = 2026-10-10 iso-date", "seconds written");
        // The date word with a zone (so independent of the machine's zone).
        assert_eq!(req("2026/04/12 00:00Z date unix").unwrap(), "2026-04-12T00:00Z iso = 1775952000 unix");
        assert_eq!(req("20261010T15Z iso iso-ordinal").is_err(), true, "iso input word takes a single literal");
    }

    #[test]
    fn loose_dates() {
        let d = |s: &str, o| read_date(s, None, o).map(|r| (r.civil.year, r.civil.month, r.civil.day, r.civil.hour, r.civil.minute, r.civil.second, r.civil.nano, r.offset));
        use Order::*;
        for s in ["2026/04/12", "2026.04.12", "2026-4-12", "2026 4 12", "2026/4/12"] {
            assert_eq!(d(s, MonthFirst), Some((2026, 4, 12, 0, 0, 0, 0, None)), "{s}");
            assert_eq!(d(s, DayFirst), Some((2026, 4, 12, 0, 0, 0, 0, None)), "year first in any order: {s}");
        }
        assert_eq!(d("12/04/2026", DayFirst).map(|x| (x.1, x.2)), Some((4, 12)));
        assert_eq!(d("12/04/2026", MonthFirst).map(|x| (x.1, x.2)), Some((12, 4)));
        assert_eq!(d("12/04/2026", YearFirst).map(|x| (x.1, x.2)), Some((4, 12)), "a year-first locale: day first");
        assert_eq!(d("4.12.26", MonthFirst).map(|x| x.0), Some(2026));
        assert_eq!(d("4.12.99", MonthFirst).map(|x| x.0), Some(1999), "POSIX %y pivot");
        assert_eq!(d("26/04/12", DayFirst).map(|x| (x.0, x.1, x.2)), Some((2012, 4, 26)), "2-digit year, day first");
        assert_eq!(d("2026/04/12 15:30", MonthFirst), Some((2026, 4, 12, 15, 30, 0, 0, None)));
        assert_eq!(d("2026/04/12T9:05:07.5", MonthFirst), Some((2026, 4, 12, 9, 5, 7, 500_000_000, None)));
        assert_eq!(d("2026/04/12 15:30 +02:00", MonthFirst).map(|x| x.7), Some(Some(7200)));
        assert_eq!(d("2026/04/12 15:30Z", MonthFirst).map(|x| x.7), Some(Some(0)));
        for bad in ["2026/02/30", "2026/13/01", "2026/04-12", "12/04/26/1", "2026/04/12 25:00", "2026/04/12 15", "abc", ""] {
            assert_eq!(d(bad, DayFirst), None, "{bad}");
        }
    }

    #[test]
    fn patterns() {
        let t = utc("2026-04-12T15:20:05Z");
        let f = |w| format(&t, w).unwrap();
        assert_eq!(f("YYYY/MM/DD"), "2026/04/12");
        assert_eq!(f("YY/MM/DD"), "26/04/12");
        assert_eq!(f("DD/MM/YY"), "12/04/26");
        assert_eq!(f("DD/MM/YYYY-24h"), "12/04/2026 15:20:05");
        assert_eq!(f("MM/DD/YYYY-12h"), "04/12/2026 03:20:05 PM");
        assert_eq!(f("MM/DD/YY-12h-tz"), "04/12/26 03:20:05 PM +00:00");
        assert_eq!(f("DD/MM/YYYY-24h-tz"), "12/04/2026 15:20:05 +00:00");
        let midnight = utc("2026-04-12T00:30Z");
        assert_eq!(format(&midnight, "DD/MM/YYYY-12h").unwrap(), "12/04/2026 12:30:00 AM");
        assert_eq!(pattern_words().len(), 30);
        assert!(pattern_words().iter().all(|w| Pattern::parse(w).is_some_and(|p| p.name() == *w)));
        assert_eq!(Pattern::parse("DD/MM/YYYY-12h-tz"), Some(Pattern { fields: Fields::Dmy4, clock: Clock::H12, tz: true }));
        assert_eq!(Pattern::parse("DD/MM/YYYY-13h"), None);
        assert!(describe_format("DD/MM/YY-12h-tz").contains("12-hour clock"));
    }

    #[test]
    fn reading_with_and_without_a_pattern() {
        use Order::*;
        let r = |s, f, o| read_date(s, f, o).map(|r| (r.civil.year, r.civil.month, r.civil.day, r.civil.hour, r.civil.minute, r.pattern.name(), r.by_locale));
        // 12-hour times, with or without a space, a.m. / pm.
        assert_eq!(r("12/04/26 3:20PM", None, DayFirst), Some((2026, 4, 12, 15, 20, "DD/MM/YY-12h".into(), true)));
        assert_eq!(r("12/04/26 3:20 pm", None, MonthFirst), Some((2026, 12, 4, 15, 20, "MM/DD/YY-12h".into(), true)));
        assert_eq!(r("12/04/2026 12:05 a.m.", None, DayFirst).map(|x| x.3), Some(0));
        assert_eq!(r("12/04/2026 12:05 PM", None, DayFirst).map(|x| x.3), Some(12));
        assert!(read_date("12/04/2026 13:05 PM", None, DayFirst).is_none(), "13 PM");
        // Not valid in the locale's order: the other one, said so.
        assert_eq!(r("25/12/2026 18:00", None, MonthFirst), Some((2026, 12, 25, 18, 0, "DD/MM/YYYY-24h".into(), false)));
        // Year first, and 2-digit years with a year-first locale.
        assert_eq!(r("2026/04/12 15:30 +02:00", None, DayFirst).map(|x| x.5), Some("YYYY/MM/DD-24h-tz".into()));
        assert_eq!(r("26/04/12", None, YearFirst).map(|x| (x.0, x.1, x.2)), Some((2026, 4, 12)));
        // With a pattern: that order only.
        assert_eq!(r("12/04/26", Some(Fields::Mdy2), DayFirst).map(|x| (x.1, x.2)), Some((12, 4)));
        assert!(read_date("12/04/2026", Some(Fields::Mdy2), DayFirst).is_none(), "the year length must fit");
        let p = Pattern::parse("DD/MM/YY-24h").unwrap();
        assert!(read_with_pattern("12/04/26 15:20", p).is_some());
        assert!(read_with_pattern("12/04/26 3:20PM", p).is_none(), "12-hour time for a 24h pattern");
        assert!(read_with_pattern("12/04/26", p).is_none(), "the pattern has a time");
    }

    #[test]
    fn guessed_dates_are_noted() {
        assert_eq!(notes("2026/04/12 15:20Z unix"), ["read `2026/04/12 15:20Z` as YYYY/MM/DD-24h-tz (year first)"]);
        assert!(notes("2026-04-12T15:20Z unix").is_empty(), "ISO: nothing to guess");
        assert!(notes("12/04/26 3:20PM +00:00 DD/MM/YY-12h-tz unix").is_empty(), "an explicit pattern");
        assert_eq!(req("12/04/26 3:20PM +00:00 DD/MM/YY-12h-tz unix").unwrap(), "2026-04-12T15:20Z iso = 1776007200 unix");
    }

    #[test]
    fn locale_orders() {
        assert_eq!(Order::from_format("%d/%m/%Y"), Order::DayFirst);
        assert_eq!(Order::from_format("%m/%d/%y"), Order::MonthFirst);
        assert_eq!(Order::from_format("%Y-%m-%d"), Order::YearFirst);
        assert_eq!(Order::from_format("%d.%m.%Y"), Order::DayFirst);
        assert_eq!(Order::from_format("%e %B %Y"), Order::DayFirst);
        assert_eq!(Order::from_format(""), Order::MonthFirst);
    }

    #[test]
    fn request_errors() {
        assert!(matches!(req("2026-10-10T15:00Z + 3"), Err(TimeError::Usage(_))));
        assert!(matches!(req("2026-10-10T15:00Z + 3 km"), Err(TimeError::Usage(_))));
        assert!(matches!(req("2026-10-10T15:00Z km"), Err(TimeError::Usage(_))));
        assert!(matches!(req("2026-10-10T15:00Z - 2026-01-01T00:00Z iso"), Err(TimeError::Usage(_))));
        assert!(matches!(req("2026-10-10T15:00Z - 2026-01-01T00:00Z + 1d"), Err(TimeError::Usage(_))));
        assert!(matches!(req("abc unix"), Err(TimeError::Usage(_))));
        assert_eq!(req("9999-12-31T00:00Z + 1yr"), Err(TimeError::Range));
    }

    #[test]
    fn human_formats() {
        let t = utc("2026-09-05T20:07:09Z");
        assert_eq!(format_human(&t, HUMAN_DATE), "05 Sept 26");
        assert_eq!(format_human(&t, HUMAN_DATETIME), "05 Sept 26 8:07 PM GMT");
        assert_eq!(format_human(&t, "D M YYYY h:mm:ss A"), "5 9 2026 8:07:09 PM");
        assert_eq!(format_human(&t, "dddd, MMMM D"), "Saturday, September 5");
        assert_eq!(format_human(&Instant { zone: Zone::Fixed(19_800), ..t }, "HH:mm tz ZZ"), "01:37 GMT+5:30 +05:30");
        assert_eq!(format_human(&Instant { zone: Zone::Fixed(-4 * 3600), ..t }, "H24 tz"), "16 GMT-4");
        assert_eq!(format_human(&t, "[Day] D [of] MMMM"), "Day 5 of September");
        // Words need brackets: their letters can be tokens (`h` is the hour).
        assert_eq!(format_human(&t, "[no tokens here?]"), "no tokens here?");
        assert_eq!(format_human(&t, "here"), "8ere");
        // Every month's abbreviation: three letters, `Sept`.
        assert_eq!(MONTH_ABBREVIATIONS_EN.iter().filter(|m| m.len() != 3).collect::<Vec<_>>(), [&"Sept"]);
    }

    #[test]
    fn countries() {
        let config = Config::default();
        // Every country: a name, and cities the time zones know.
        assert!(tz::countries().len() >= 190);
        for c in tz::countries() {
            assert!(!c.cities.is_empty(), "{}", c.name);
            for city in &c.cities {
                assert!(tz::place_name(city).is_some(), "{}: {city}", c.name);
                assert!(crate::geo::known(city).is_some(), "{}: {city} has no coordinates", c.name);
            }
        }
        // One zone: its city.
        let rewritten = |s: &str| countries_in(&config, s).unwrap().unwrap().0;
        assert_eq!(rewritten("Italy"), "Rome");
        assert_eq!(rewritten("10:00 italia to Japan"), "10:00 Rome to Tokyo");
        assert_eq!(rewritten("U.K."), "London");
        assert_eq!(rewritten("Turkiye"), "Ankara", "accents aside");
        // Several: which zone, each with its city, offset and zone, and the request for it.
        let Some(Err(a)) = countries_in(&config, "10:00AM USA to Paris") else { panic!("ambiguous") };
        assert_eq!((a.text.as_str(), a.kind), ("USA", AmbiguityKind::Country));
        assert_eq!(a.choices.iter().map(|c| c.pattern.as_str()).collect::<Vec<_>>(), ["New York", "Chicago", "Denver", "Phoenix", "Los Angeles", "Anchorage", "Honolulu"]);
        assert!(a.choices[0].date.ends_with(" · America/New_York") && a.choices[0].date.starts_with("GMT-"), "{}", a.choices[0].date);
        assert_eq!(a.choices[1].request, "10:00AM Chicago to Paris");
        assert!(a.message().starts_with("`USA` has several time zones: New York (GMT-"), "{}", a.message());
        let Some(Err(a)) = countries_in(&config, "Russia") else { panic!("ambiguous") };
        assert_eq!(a.choices.len(), 12);
        // Not countries: cities with a country's name, units, nothing.
        assert!(countries_in(&config, "Mexico City").is_none());
        assert!(countries_in(&config, "Singapore").is_none());
        assert!(countries_in(&config, "10 us ms").is_none());
        assert!(countries_in(&config, "10 km mi").is_none());
        // Through a request.
        assert!(matches!(request(&config, "Canada", 4, false), Some(Err(TimeError::Ambiguous(_)))));
        let (out, notes) = request(&config, "Japan", 4, true).unwrap().unwrap();
        assert!(out.ends_with(" Tokyo") && notes[0] == "Japan is Tokyo", "{out} {notes:?}");
    }

    #[test]
    fn to_is_optional_between_cities() {
        let config = Config::default();
        let out = |s: &str| request(&config, s, 4, false).unwrap().unwrap().0;
        assert_eq!(out("10:00 Rome Doha"), out("10:00 Rome to Doha"));
        assert!(out("10:00 Rome Doha").ends_with(" 11:00 AM GMT+3 Doha"));
        assert_eq!(out("now New York Tokyo"), out("now New York to Tokyo"));
        assert_eq!(out("now Italy Japan"), out("now Italy to Japan"));
        // A country with several zones: which one, with or without `to`; each choice works.
        for input in ["now usa japan", "now usa to japan"] {
            let Some(Err(TimeError::Ambiguous(a))) = request(&config, input, 4, false) else { panic!("{input}: ambiguous") };
            assert_eq!(a.kind, AmbiguityKind::Country);
            assert!(a.choices[0].request.starts_with("now New York "), "{}", a.choices[0].request);
            assert!(request(&config, &a.choices[0].request, 4, false).unwrap().unwrap().0.ends_with(" Tokyo"));
        }
        // Three cities, or a city in the middle, are still errors.
        assert!(request(&config, "now Rome Paris Tokyo", 4, false).unwrap().is_err());
    }

    #[test]
    fn misspelt_places() {
        let config = Config::default();
        let ask = |s: &str| match request(&config, s, 4, false) {
            Some(Err(TimeError::Ambiguous(a))) => a,
            other => panic!("{s}: {other:?}"),
        };
        // A misspelt country and a misspelt city, together: one question, each named.
        let a = ask("10:00am jappan romme");
        assert_eq!(a.kind, AmbiguityKind::City);
        assert_eq!(a.note, "`jappan`, `romme` are not known places");
        assert_eq!(a.choices[0].pattern, "Japan (country), Rome (city)");
        assert_eq!(a.choices[0].date, "Asia/Tokyo, Europe/Rome");
        assert_eq!(a.choices[0].request, "10:00am Japan Rome");
        assert!(a.message().contains("did you mean Japan (country), Rome (city)? (`10:00am Japan Rome`)"), "{}", a.message());
        // The corrected request converts: 10:00 in Tokyo, in Rome.
        let out = request(&config, &a.choices[0].request, 4, false).unwrap().unwrap().0;
        assert!(out.ends_with(" GMT+2 Rome") || out.ends_with(" GMT+1 Rome"), "{out}");
        // One misspelt country, alone or with a correct city; one misspelt city as before.
        assert_eq!(ask("jappan").note, "`jappan` is not a country");
        assert_eq!(ask("jappan").choices[0].request, "Japan");
        assert_eq!(ask("10:00am jappan rome").choices[0].request, "10:00am Japan rome");
        let tokio = ask("10:00 Tokio");
        assert_eq!((tokio.note.as_str(), tokio.choices[0].pattern.as_str()), ("`Tokio` is not a city", "Tokyo"));
        // Nothing near: no question.
        assert!(!matches!(request(&config, "10:00 Gotham", 4, false), Some(Err(TimeError::Ambiguous(_)))));
    }

    #[test]
    fn no_unit_is_a_country() {
        for category in &Config::default().categories {
            for unit in &category.units {
                for name in std::iter::once(&unit.symbol).chain(unit.aliases.iter()) {
                    let country = tz::country(name).map(|c| c.name.as_str());
                    // `us` (microsecond) is also the United States: the unit wins (`countries_in`).
                    assert!(country.is_none() || *name == "us", "unit `{name}` reads as {country:?}");
                }
            }
        }
    }

    #[test]
    fn no_unit_is_a_city() {
        // A city in a request makes it a date request: no unit may read as one.
        for category in &Config::default().categories {
            for unit in &category.units {
                for name in std::iter::once(&unit.symbol).chain(unit.aliases.iter()) {
                    assert!(tz::city(name).is_none(), "unit `{name}` reads as a city");
                }
            }
        }
    }

    #[test]
    fn month_names() {
        let m = |w: &str| month_named(w).map(|n| (n.month, n.typo));
        // Full names and abbreviations in the six languages, any case, accents optional.
        for (month, words) in [
            (1, &["January", "Jan", "enero", "ene", "janvier", "janv.", "gennaio", "gen", "Januar", "Jänner", "janeiro"][..]),
            (2, &["February", "Feb", "febrero", "février", "fevrier", "févr", "febbraio", "Februar", "fevereiro", "fev"]),
            (3, &["March", "Mar", "marzo", "mars", "März", "Marz", "Mrz", "MÄR", "março", "marco"]),
            (4, &["April", "Apr", "abril", "abr", "avril", "avr", "aprile"]),
            (5, &["May", "mayo", "mai", "maggio", "mag", "maio"]),
            (6, &["June", "Jun", "junio", "juin", "giugno", "giu", "Juni", "junho"]),
            (7, &["July", "Jul", "julio", "juillet", "juil.", "luglio", "lug", "Juli", "julho"]),
            (8, &["August", "Aug", "agosto", "ago", "août", "aout", "aoû"]),
            (9, &["September", "Sep", "Sept", "sept.", "septiembre", "setiembre", "set", "septembre", "settembre", "setembro"]),
            (10, &["October", "Oct", "octubre", "octobre", "ottobre", "ott", "Oktober", "Okt", "outubro", "out"]),
            (11, &["November", "Nov", "noviembre", "novembre", "novembro"]),
            (12, &["December", "Dec", "diciembre", "dic", "décembre", "déc", "dicembre", "Dezember", "Dez", "dezembro"]),
        ] {
            for w in words {
                assert_eq!(m(w), Some((month, false)), "{w}");
            }
        }
        // One letter off a single month's name (5 letters or more): a typo, noted as such.
        assert_eq!(m("Sptember"), Some((9, true)));
        assert_eq!(m("Setpember"), Some((9, true)));
        assert_eq!(m("Novmber"), Some((11, true)));
        assert_eq!(m("Novembr"), Some((11, false)), "a prefix: an abbreviation");
        assert_eq!(m("Febuary"), Some((2, true)));
        // Too short, shared by two months, or not a month.
        for not in ["ma", "ju", "jui", "a", "", "12", "km", "Sep2", "Septemberx2", "hello", "maybe"] {
            assert_eq!(m(not), None, "{not}");
        }
        // Every name is its own month, and no prefix of one is read as another month (a prefix two
        // months share, like `jui`, is none).
        for (i, names) in MONTH_NAMES.iter().enumerate() {
            for name in *names {
                assert_eq!(m(name), Some((i as u32 + 1, false)), "{name}");
                for n in 3..name.len() {
                    let month = m(&name[..n]).map(|(m, _)| m);
                    assert!(month.is_none() || month == Some(i as u32 + 1), "{}: {month:?}", &name[..n]);
                }
            }
        }
        // No unit is read as a month (a month word starts a date request).
        let config = Config::default();
        for category in &config.categories {
            for unit in &category.units {
                for name in std::iter::once(&unit.symbol).chain(unit.aliases.iter()) {
                    assert_eq!(m(name), None, "unit `{name}` reads as a month");
                }
            }
        }
    }

    #[test]
    fn dates_with_month_names() {
        let r = |s: &str| read_date(s, None, Order::MonthFirst).map(|r| (r.civil.year, r.civil.month, r.civil.day, r.civil.hour, r.civil.minute, r.pattern.name()));
        assert_eq!(r("12 Sept 2026"), Some((2026, 9, 12, 0, 0, "DD/MM/YYYY".into())));
        assert_eq!(r("12-Sep-2026"), Some((2026, 9, 12, 0, 0, "DD/MM/YYYY".into())));
        assert_eq!(r("12. Oktober 2026"), Some((2026, 10, 12, 0, 0, "DD/MM/YYYY".into())));
        assert_eq!(r("12 de septiembre de 2026"), Some((2026, 9, 12, 0, 0, "DD/MM/YYYY".into())));
        assert_eq!(r("1er janvier 2026"), Some((2026, 1, 1, 0, 0, "DD/MM/YYYY".into())));
        assert_eq!(r("1º de maio de 2026"), Some((2026, 5, 1, 0, 0, "DD/MM/YYYY".into())));
        assert_eq!(r("1st of May 2026"), Some((2026, 5, 1, 0, 0, "DD/MM/YYYY".into())));
        assert_eq!(r("12 Sep 26"), Some((2026, 9, 12, 0, 0, "DD/MM/YY".into())));
        assert_eq!(r("September 12, 2026"), Some((2026, 9, 12, 0, 0, "MM/DD/YYYY".into())));
        assert_eq!(r("Sep 12th 2026 3:20 PM"), Some((2026, 9, 12, 15, 20, "MM/DD/YYYY-12h".into())));
        assert_eq!(r("2026 Sep 12 15:30"), Some((2026, 9, 12, 15, 30, "YYYY/MM/DD-24h".into())));
        assert_eq!(r("12 Sept 2026, 15:30+02:00").map(|x| x.5), Some("DD/MM/YYYY-24h-tz".into()));
        // Never ambiguous; an impossible day or a month shared by two names is no date.
        assert_eq!(read_date("4 Dec 2026", None, Order::MonthFirst).unwrap().readings.len(), 1);
        for no in ["31 Feb 2026", "12 jui 2026", "12 Sept", "Sept 2026", "12 Sept 2026 xyz", "12 km 2026", "123 Sep 2026"] {
            assert_eq!(r(no), None, "{no}");
        }
        // An explicit pattern must match the order written.
        assert!(read_date("12 Sept 2026", Some(Fields::Dmy4), Order::MonthFirst).is_some());
        assert!(read_date("12 Sept 2026", Some(Fields::Mdy4), Order::MonthFirst).is_none());
    }

    #[test]
    fn workdays() {
        let day = |s: &str| {
            let c = utc(&format!("{s}T00:00Z")).civil();
            days_from_civil(c.year, c.month, c.day)
        };
        let iso = |d: i64| {
            let (y, m, dd) = civil_from_days(d);
            format!("{y}-{m:02}-{dd:02}")
        };
        // Monday 2026-10-05.
        for (from, n, to) in [
            ("2026-10-05", 5, "2026-10-12"),
            ("2026-10-05", 1, "2026-10-06"),
            ("2026-10-05", 4, "2026-10-09"),
            ("2026-10-09", 1, "2026-10-12"), // Friday → Monday
            ("2026-10-10", 1, "2026-10-12"), // Saturday → Monday
            ("2026-10-11", 5, "2026-10-16"), // Sunday → Friday
            ("2026-10-10", -1, "2026-10-09"), // Saturday → Friday
            ("2026-10-12", -1, "2026-10-09"), // Monday → Friday
            ("2026-10-10", 0, "2026-10-10"),
            ("2026-10-05", -5, "2026-09-28"),
            ("2026-10-05", 23, "2026-11-05"),
            ("2024-02-28", 2, "2024-03-01"), // over a leap day
        ] {
            assert_eq!(iso(add_workdays(day(from), n).unwrap()), to, "{from} {n:+}wd");
        }
        // The count of working days is the inverse of adding them, landing on any working day.
        let start = day("2026-09-01");
        for from in start..start + 21 {
            for n in -30..=30 {
                let to = add_workdays(from, n).unwrap();
                assert!(!is_weekend(to) || n == 0);
                if !is_weekend(from) || n == 0 {
                    assert_eq!(workdays_between(from, to), n, "{} {n:+}wd", iso(from));
                }
            }
        }
        assert_eq!(workdays_between(day("2026-10-10"), day("2026-12-25")), 55);
        assert_eq!(workdays_between(day("2026-10-10"), day("2026-10-11")), 0, "a weekend");
        assert_eq!(add_workdays(i64::MAX - 10, 100), None);
    }

    #[test]
    fn zone_words() {
        assert_eq!(gmt_zone("GMT"), Some(Ok(0)));
        assert_eq!(gmt_zone("utc"), Some(Ok(0)));
        assert_eq!(gmt_zone("GMT+2"), Some(Ok(7200)));
        assert_eq!(gmt_zone("UTC-5"), Some(Ok(-18000)));
        assert_eq!(gmt_zone("GMT+05:30"), Some(Ok(19800)));
        assert_eq!(gmt_zone("gmt+0530"), Some(Ok(19800)));
        assert_eq!(gmt_zone("GMT-14"), Some(Ok(-50400)));
        for bad in ["GMT+15", "GMT+14:30", "GMT+5:60", "GMT+", "GMT+abc", "GMT+123456"] {
            assert!(matches!(gmt_zone(bad), Some(Err(_))), "{bad}");
        }
        for not in ["iso-utc", "GMTX", "UT", "gm", "", "utc2"] {
            assert_eq!(gmt_zone(not), None, "{not}");
        }
        assert_eq!(take_zone("2026/12/04 15:30 GMT+2 unix"), Ok(("2026/12/04 15:30 unix".into(), Some(("GMT+2".into(), 7200)))));
        assert_eq!(take_zone("2026/12/04 15:30GMT+2"), Ok(("2026/12/04 15:30".into(), Some(("GMT+2".into(), 7200)))));
        assert_eq!(take_zone("2026/12/04 3:20PMUTC-5"), Ok(("2026/12/04 3:20PM".into(), Some(("UTC-5".into(), -18000)))));
        assert_eq!(take_zone("now iso-utc"), Ok(("now iso-utc".into(), None)));
        assert!(take_zone("2026/12/04 GMT+1 UTC+2").is_err());
        assert!(take_zone("2026/12/04 GMT+25").is_err());
    }

    #[test]
    fn detection() {
        for yes in ["now", "now+3d", "today iso", "2026-10-10", "2026-W41-6 unix", "1791644400 unix", "20261010 iso", "10/10/2026 local", "2026/04/12 date unix", "12/04/2026 15:00 date", "12/04/26 3:20PM unix", "12/04/26 DD/MM/YY iso", "2026/04/12 iso-week", "2026/12/04 GMT+2", "GMT+2", "2026/12/04 15:30UTC-5 unix", "2026/12/04 GMT+99"] {
            assert!(is_date_request(yes), "{yes}");
        }
        // Numbers, subtractions and conversions stay numeric.
        for no in ["10 km mi", "20261010", "2026-283", "2026-10", "100 + 20", "nowhere", "-5 local", "", "12/04/26", "2026-12-02/12", "2026-12-02 / 12", "2026-12-02*2", "2026-12-02 num", "now N", "2026/12/04 NUMBER"] {
            assert!(!is_date_request(no), "{no}");
        }
    }

    #[test]
    fn now_is_now() {
        let a = now().ns;
        let b = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as i128;
        assert!((b - a).abs() < NS);
    }
}
