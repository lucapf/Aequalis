//! ISO 8601 parsing, and the order of day and month in the locale's dates.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Form {
    Calendar,
    Ordinal,
    Week,
}

#[derive(Debug)]
pub(super) struct Iso {
    pub(super) civil: Civil,
    /// `None`: no zone given, local time.
    pub(super) offset: Option<i32>,
    pub(super) form: Form,
    pub(super) extended: bool,
    pub(super) has_time: bool,
}

impl Iso {
    /// Whether a word is recognised as a date on its own. Basic forms (`20261010`) and the
    /// extended ordinal (`2026-283`) read as numbers or subtractions, so they need a time part
    /// (`20261010T1500`) or the `iso` input word.
    pub(super) fn unambiguous(&self) -> bool {
        self.has_time || self.form == Form::Week || (self.extended && self.form == Form::Calendar)
    }
}

pub(super) struct Cursor<'a> {
    pub(super) b: &'a [u8],
    pub(super) i: usize,
}

impl Cursor<'_> {
    pub(super) fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    pub(super) fn eat(&mut self, c: u8) -> bool {
        let found = self.peek() == Some(c);
        self.i += usize::from(found);
        found
    }

    pub(super) fn digits(&mut self, n: usize) -> Option<u32> {
        let s = self.b.get(self.i..self.i + n)?;
        if !s.iter().all(u8::is_ascii_digit) {
            return None;
        }
        self.i += n;
        Some(s.iter().fold(0, |v, d| v * 10 + u32::from(d - b'0')))
    }

    pub(super) fn count_digits(&self) -> usize {
        self.b[self.i..].iter().take_while(|c| c.is_ascii_digit()).count()
    }
}

pub(super) fn parse_iso(s: &str) -> Option<Iso> {
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
pub(super) fn finish(mut c: Cursor, mut civil: Civil, form: Form, extended: bool) -> Option<Iso> {
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
pub(super) fn parse_zone(c: &mut Cursor) -> Option<Option<i32>> {
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
// The names people use: day first, month first, year first.
#[allow(clippy::enum_variant_names)]
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

    /// The device locale's order (the C locale's `%m/%d/%y` when unknown), read once: a date
    /// request asks several times, and on Windows each reading is a system query.
    pub fn of_locale() -> Order {
        pub(super) static ORDER: std::sync::OnceLock<Order> = std::sync::OnceLock::new();
        *ORDER.get_or_init(|| sys::date_format().map_or(Order::MonthFirst, |f| Order::from_format(&f)))
    }

    pub(super) fn describe(self) -> &'static str {
        match self {
            Order::MonthFirst => "year/month/day, or month/day/year here",
            Order::DayFirst | Order::YearFirst => "year/month/day, or day/month/year here",
        }
    }
}
