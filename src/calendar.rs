//! The calendar: `cal` (this month), `cal march`, `cal march 2027`, `cal 2027-03`, `cal next month`,
//! `cal 2027` (the year), with the public holidays of up to `MAX_COUNTRIES` countries, a colour each
//! (`cal march UAE,USA,Italy`, `cal 2027 de fr`; none named: the `holidays.countries` setting, the
//! time zone's country by default). The weeks start as the device's region does (`calendar.week_start`),
//! with their ISO numbers; today is marked. The holidays come from the archive (`holidays.rs`); not
//! downloaded, there are none.
//!
//! A `View` for the apps and the interactive mode, its text for the command line (`text`, coloured
//! at a terminal) and its JSON (`json`).

use crate::holidays::{Archive, Holiday};
use crate::time::{self, tz::Country};

/// Countries at most in one calendar: a colour each.
pub const MAX_COUNTRIES: usize = 5;

/// The first day of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeekStart {
    Monday,
    Sunday,
    Saturday,
}

impl WeekStart {
    pub fn name(self) -> &'static str {
        match self {
            WeekStart::Monday => "monday",
            WeekStart::Sunday => "sunday",
            WeekStart::Saturday => "saturday",
        }
    }

    /// Its ISO weekday (Monday = 1 … Sunday = 7).
    fn iso(self) -> u32 {
        match self {
            WeekStart::Monday => 1,
            WeekStart::Sunday => 7,
            WeekStart::Saturday => 6,
        }
    }

    fn from_name(name: &str) -> Option<WeekStart> {
        [WeekStart::Monday, WeekStart::Sunday, WeekStart::Saturday].into_iter().find(|w| w.name() == name)
    }
}

/// Where the week starts on Sunday, or Saturday (CLDR's week data); elsewhere on Monday.
const SUNDAY_FIRST: &[&str] = &[
    "AG", "AS", "BD", "BR", "BS", "BT", "BW", "BZ", "CA", "CN", "CO", "DM", "DO", "ET", "GT", "GU", "HK", "HN", "ID", "IL", "IN", "JM", "JP", "KE", "KH", "KR", "LA", "MH", "MM", "MO",
    "MT", "MX", "MZ", "NI", "NP", "PA", "PE", "PH", "PK", "PR", "PT", "PY", "SA", "SG", "SV", "TH", "TT", "TW", "UM", "US", "VE", "VI", "WS", "YE", "ZA", "ZW",
];
const SATURDAY_FIRST: &[&str] = &["AE", "AF", "BH", "DJ", "DZ", "EG", "IQ", "IR", "JO", "KW", "LY", "OM", "QA", "SD", "SY"];

/// The first day of the week in a country (`IT`).
pub fn week_start_of(country: &str) -> WeekStart {
    if SUNDAY_FIRST.contains(&country) {
        WeekStart::Sunday
    } else if SATURDAY_FIRST.contains(&country) {
        WeekStart::Saturday
    } else {
        WeekStart::Monday
    }
}

/// The weekend's ISO weekdays in a country: Friday and Saturday in most of the Arab world, Friday
/// in Iran, Thursday and Friday in Afghanistan, Sunday alone in India and Uganda; else Saturday and
/// Sunday.
fn weekend_of(country: Option<&str>) -> &'static [u32] {
    match country.unwrap_or("") {
        "BH" | "DZ" | "EG" | "IL" | "IQ" | "JO" | "KW" | "LY" | "OM" | "QA" | "SA" | "SD" | "SY" | "YE" => &[5, 6],
        "IR" => &[5],
        "AF" => &[4, 5],
        "IN" | "UG" => &[7],
        _ => &[6, 7],
    }
}

static DEVICE_WEEK_START: std::sync::OnceLock<WeekStart> = std::sync::OnceLock::new();

/// The device's first day of the week as the app embedding aequalis knows it (`monday`, `sunday`,
/// `saturday`; ISO numbers too: `1`, `7`, `6`); set once, before use.
pub fn set_device_week_start(name: &str) {
    let week = match name.trim().to_ascii_lowercase().as_str() {
        "1" | "monday" => WeekStart::Monday,
        "7" | "sunday" => WeekStart::Sunday,
        "6" | "saturday" => WeekStart::Saturday,
        _ => return,
    };
    let _ = DEVICE_WEEK_START.set(week);
}

/// The device's first day of the week: the app's, else the locale's region, else the time zone's
/// country, else Monday.
pub fn device_week_start() -> WeekStart {
    if let Some(&week) = DEVICE_WEEK_START.get() {
        return week;
    }
    crate::locale::territory().or_else(device_country).map_or(WeekStart::Monday, |c| week_start_of(&c))
}

/// The time zone's country (`IT`).
fn device_country() -> Option<String> {
    time::device_zone().and_then(time::tz::zone_country).map(|c| c.code.to_string())
}

/// The week start in effect: the `calendar.week_start` setting, `auto` the device's.
fn week_start(settings: &crate::settings::Settings) -> WeekStart {
    WeekStart::from_name(settings.get("calendar.week_start")).unwrap_or_else(device_week_start)
}

// ---------------------------------------------------------------- the request

/// What a calendar request asks for.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub span: Span,
    /// The codes of the countries named (`IT`), in their order (none: the setting's).
    pub countries: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    Month(i64, u32),
    Year(i64),
}

/// Whether the request is a calendar's: it starts with `cal` or `calendar`.
pub fn is_request(input: &str) -> bool {
    input.split_whitespace().next().is_some_and(|w| w.eq_ignore_ascii_case("cal") || w.eq_ignore_ascii_case("calendar"))
}

/// The calendar `input` asks for, `today` being (year, month); `None` when it is no calendar
/// request, the reason when its words are not a month, a year or countries. Without `cal`, a request
/// that starts with a month's name and is nothing but a calendar's words is one too (`oct egypt`,
/// `march 2027`); with any other word it is left to the others (`jan 30 2025` is a date).
pub fn request(input: &str, today: (i64, u32)) -> Option<Result<Request, String>> {
    if is_request(input) {
        let rest = input.split_once(char::is_whitespace).map_or("", |(_, r)| r);
        return Some(read(rest, today));
    }
    let first = input.split([' ', ',']).next()?;
    month_and_year(first)?;
    read(input, today).ok().map(Ok)
}

fn read(rest: &str, (this_year, this_month): (i64, u32)) -> Result<Request, String> {
    let (mut month, mut year, mut whole_year) = (None::<u32>, None::<i64>, false);
    // A year written twice (`jan 30 2025`, a date) is no calendar's; two digits are a year after a
    // month (`oct 26`: October 2026).
    let (mut years, mut short) = (0, None::<(i64, String)>);
    let mut countries: Vec<&'static str> = Vec::new();
    // Commas part countries (`UAE,USA,Italy`); within a part, a country can be several words.
    for part in rest.split(',') {
        let words: Vec<&str> = part.split_whitespace().collect();
        let mut i = 0;
        while i < words.len() {
            let word = words[i];
            let lower = word.to_lowercase();
            let next = words.get(i + 1).map(|w| w.to_lowercase());
            // `next month`, `last year`, `this month`.
            if let (Some(step @ ("next" | "last" | "this" | "previous")), Some(unit @ ("month" | "year"))) = (Some(lower.as_str()), next.as_deref()) {
                let by = match step {
                    "next" => 1,
                    "last" | "previous" => -1,
                    _ => 0,
                };
                if unit == "month" {
                    let total = this_year * 12 + i64::from(this_month) - 1 + by;
                    (year, month) = (Some(total.div_euclid(12)), Some((total.rem_euclid(12) + 1) as u32));
                } else {
                    (year, whole_year) = (Some(this_year + by), month.is_none());
                }
                i += 2;
                continue;
            }
            if ["in", "of", "for", "on"].contains(&lower.as_str()) {
                i += 1;
                continue;
            }
            if let Some((y, m)) = year_month(word) {
                (year, month) = (Some(y), Some(m));
                i += 1;
                continue;
            }
            if let Some(y) = year_of(word) {
                (year, years) = (Some(y), years + 1);
                i += 1;
                continue;
            }
            if word.len() == 2 && word.bytes().all(|b| b.is_ascii_digit()) {
                short = Some((2000 + word.parse::<i64>().unwrap_or(0), word.to_string()));
                years += 1;
                i += 1;
                continue;
            }
            // `oct`, `OCT26`, `oct2026`, `0ct 26` (a zero for the O).
            if let Some((m, y)) = month_and_year(word) {
                month = Some(m);
                if let Some(y) = y {
                    (year, years) = (Some(y), years + 1);
                }
                i += 1;
                continue;
            }
            // The longest run of words that names a country (`United Arab Emirates`).
            if let Some((country, taken)) = (1..=(words.len() - i).min(4)).rev().find_map(|n| country(&words[i..i + n].join(" ")).map(|c| (c, n))) {
                if !countries.contains(&country.code) {
                    countries.push(country.code);
                }
                i += taken;
                continue;
            }
            if let Some(m) = time::month_named(word) {
                month = Some(m.month);
                i += 1;
                continue;
            }
            return Err(format!("`{word}` is not a month, a year or a country"));
        }
    }
    if countries.len() > MAX_COUNTRIES {
        return Err(format!("at most {MAX_COUNTRIES} countries at a time"));
    }
    if years > 1 {
        return Err("a month and one year: `oct 2026`".to_string());
    }
    if let Some((y, written)) = short {
        if month.is_none() {
            return Err(format!("`{written}` is not a month, a year or a country"));
        }
        year = Some(y);
    }
    let span = match (month, year) {
        (Some(m), y) => Span::Month(y.unwrap_or(this_year), m),
        (None, Some(y)) => Span::Year(y),
        (None, None) if whole_year => Span::Year(this_year),
        (None, None) => Span::Month(this_year, this_month),
    };
    Ok(Request { span, countries })
}

/// `2027-03`, `2027/03`, `03/2027`, `3-2027`.
fn year_month(word: &str) -> Option<(i64, u32)> {
    let (a, b) = word.split_once(['-', '/'])?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
    if !digits(a) || !digits(b) {
        return None;
    }
    let (y, m) = match (a.len(), b.len()) {
        (4, 1..=2) => (a, b),
        (1..=2, 4) => (b, a),
        _ => return None,
    };
    let m: u32 = m.parse().ok().filter(|m| (1..=12).contains(m))?;
    Some((y.parse().ok()?, m))
}

/// A month's name, and the year written against it (`oct`, `OCT26`, `oct2026`): two digits are this
/// century's. A zero in the name is the O it looks like (`0ct`, `n0v`). Its typos are not read here.
fn month_and_year(word: &str) -> Option<(u32, Option<i64>)> {
    let digits = word.bytes().rev().take_while(u8::is_ascii_digit).count();
    let (name, year) = word.split_at(word.len() - digits);
    let year = match year.len() {
        0 => None,
        2 => Some(2000 + year.parse::<i64>().ok()?),
        4 => Some(year_of(year)?),
        _ => return None,
    };
    let name = name.replace('0', "o");
    let found = time::month_named(&name).filter(|m| !m.typo)?;
    Some((found.month, year))
}

/// A year: four digits, 1000 to 9999.
fn year_of(word: &str) -> Option<i64> {
    (word.len() == 4 && word.bytes().all(|b| b.is_ascii_digit())).then(|| word.parse().ok()).flatten().filter(|y| *y >= 1000)
}

/// A country by its name, an alias (`USA`, `UK`, `UAE`) or its code (`IT`, `de`).
fn country(name: &str) -> Option<&'static Country> {
    if name.len() == 2 && name.bytes().all(|b| b.is_ascii_alphabetic()) {
        if let Some(c) = time::tz::countries().iter().find(|c| c.code.eq_ignore_ascii_case(name)) {
            return Some(c);
        }
    }
    time::tz::country(name)
}

/// The codes of the countries a text names (`IT,AE`, `Italy, United Arab Emirates`), for the
/// `holidays.countries` setting.
pub fn countries_of(text: &str) -> Result<Vec<&'static str>, String> {
    let mut codes: Vec<&'static str> = Vec::new();
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let found = country(part).ok_or_else(|| format!("`{part}` is not a country"))?;
        if !codes.contains(&found.code) {
            codes.push(found.code);
        }
    }
    if codes.len() > MAX_COUNTRIES {
        return Err(format!("at most {MAX_COUNTRIES} countries"));
    }
    Ok(codes)
}

// ---------------------------------------------------------------- the view

/// A calendar as shown: its months, today, the countries and their holidays.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct View {
    /// `October 2026`, `2027`.
    pub title: String,
    pub months: Vec<Month>,
    /// The ISO weekday of each column (Monday = 1), from the first day of the week.
    pub columns: [u32; 7],
    /// The weekend's ISO weekdays.
    pub weekend: Vec<u32>,
    /// Today, in days since 1970-01-01.
    pub today: i64,
    /// The countries of the holidays (`IT`, `Italy`), in their colours' order.
    pub countries: Vec<(String, String)>,
    /// The holidays, by day: each country's (its index in `countries`) and name.
    pub holidays: Vec<Mark>,
    /// Whether it is a whole year.
    pub year: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Month {
    pub year: i64,
    pub month: u32,
    /// `October 2026`.
    pub title: String,
    pub weeks: Vec<Week>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Week {
    /// Its ISO week number (of its Monday).
    pub number: u32,
    /// The day of the month in each column; 0: another month's.
    pub days: [u32; 7],
    /// The day (days since 1970-01-01) of its first column.
    pub first: i64,
}

/// A holiday in the calendar.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub day: i64,
    /// Its country, an index into `View::countries`.
    pub country: usize,
    pub name: String,
    /// The holiday's first day and how many days it lasts: the days in a row with its name in its
    /// country (the Emirates' National Day, 2 and 3 December: 2 days from the 2nd).
    pub first: i64,
    pub length: u32,
}

impl View {
    /// The countries (indexes) whose holiday a day is.
    pub fn holiday_of(&self, day: i64) -> Vec<usize> {
        let mut found: Vec<usize> = self.holidays.iter().filter(|m| m.day == day).map(|m| m.country).collect();
        found.dedup();
        found
    }
}

/// The weekdays' short names, Monday first.
pub const WEEKDAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

/// A month's weeks, from the first day of the week.
fn month(year: i64, month: u32, start: WeekStart) -> Month {
    let first = time::days_from_civil(year, month, 1);
    let last = first + i64::from(time::days_in_month(year, month)) - 1;
    let back = (i64::from(time::weekday(first)) - i64::from(start.iso())).rem_euclid(7);
    let mut weeks = Vec::new();
    let mut row = first - back;
    while row <= last {
        let mut days = [0; 7];
        for (k, d) in days.iter_mut().enumerate() {
            let day = row + k as i64;
            if (first..=last).contains(&day) {
                *d = (day - first + 1) as u32;
            }
        }
        // The row's Monday says its ISO week.
        let monday = row + (8 - i64::from(start.iso())).rem_euclid(7);
        weeks.push(Week { number: time::iso_week(monday).1, days, first: row });
        row += 7;
    }
    Month { year, month, title: format!("{} {year}", time::ENGLISH_MONTHS[month as usize - 1]), weeks }
}

/// Each holiday with its run: the days in a row that have its name (a day's two holidays apart).
fn runs(found: &[Holiday]) -> Vec<Mark> {
    let mut marks: Vec<Mark> = Vec::new();
    for h in found {
        let first = match marks.iter().rev().find(|m| m.name == h.name && m.day == h.day - 1) {
            Some(before) => before.first,
            None => h.day,
        };
        marks.push(Mark { day: h.day, country: 0, name: h.name.clone(), first, length: 1 });
    }
    // The length of each run, on all its days.
    for i in 0..marks.len() {
        let (first, name) = (marks[i].first, marks[i].name.clone());
        let length = marks.iter().filter(|m| m.first == first && m.name == name).count() as u32;
        marks[i].length = length;
    }
    marks
}

/// The holidays of a month as listed under it: a line a holiday, its days together (`2-3 Dec`), in
/// the order they begin; (the days, its country, its name, how many days).
pub fn listed(view: &View) -> Vec<(String, usize, String, u32)> {
    let mut seen: Vec<(i64, usize, &str)> = Vec::new();
    let mut out = Vec::new();
    for m in &view.holidays {
        if seen.contains(&(m.first, m.country, m.name.as_str())) {
            continue;
        }
        seen.push((m.first, m.country, m.name.as_str()));
        out.push((days_label(m.first, m.length), m.country, m.name.clone(), m.length));
    }
    out
}

/// A holiday's days: `2 Dec`, `2-3 Dec`, `30 Sep-2 Oct`.
pub fn days_label(first: i64, length: u32) -> String {
    let last = first + i64::from(length) - 1;
    let (_, m1, d1) = time::civil_from_days(first);
    let (_, m2, d2) = time::civil_from_days(last);
    let month = |m: u32| &time::ENGLISH_MONTHS[m as usize - 1][..3];
    match (length, m1 == m2) {
        (1, _) => format!("{d1} {}", month(m1)),
        (_, true) => format!("{d1}-{d2} {}", month(m1)),
        _ => format!("{d1} {}-{d2} {}", month(m1), month(m2)),
    }
}

/// What the calendar needs besides the request: the settings, the archive, the language for the
/// holidays' names, and today (days since 1970-01-01).
pub struct Context<'a> {
    pub settings: &'a crate::settings::Settings,
    pub archive: Option<&'a Archive>,
    pub language: Option<&'a str>,
    pub today: i64,
}

/// The countries whose holidays a calendar shows: those named, else the `holidays.countries`
/// setting's; none when the holidays are off. The archive reads only these (`holidays::load`).
pub fn countries(request: &Request, settings: &crate::settings::Settings) -> Vec<&'static str> {
    if !settings.holidays_enabled() {
        Vec::new()
    } else if !request.countries.is_empty() {
        request.countries.clone()
    } else {
        match settings.get("holidays.countries") {
            "auto" => device_country().and_then(|c| time::tz::countries().iter().find(|k| k.code == c)).map(|c| vec![c.code]).unwrap_or_default(),
            codes => countries_of(codes).unwrap_or_default(),
        }
    }
}

/// The calendar of `request`, and notes on its holidays (none downloaded, a year the archive does
/// not have, a country it lacks).
pub fn view(request: &Request, cx: &Context) -> (View, Vec<String>) {
    let start = week_start(cx.settings);
    let (months, title, year) = match request.span {
        Span::Month(y, m) => {
            let one = month(y, m, start);
            let title = one.title.clone();
            (vec![one], title, false)
        }
        Span::Year(y) => ((1..=12).map(|m| month(y, m, start)).collect(), y.to_string(), true),
    };
    let mut columns = [0; 7];
    for (k, c) in columns.iter_mut().enumerate() {
        *c = (start.iso() - 1 + k as u32) % 7 + 1;
    }
    let mut notes = Vec::new();
    if !cx.settings.holidays_enabled() && !request.countries.is_empty() {
        notes.push("the public holidays are off: turn on the holidays setting".to_string());
    }
    let countries = countries(request, cx.settings);
    let named = |code: &str| time::tz::countries().iter().find(|c| c.code == code).map_or(code.to_string(), |c| c.name.to_string());
    // The months' own days: the grid shows no other month's.
    let last = months.last().map_or(0, |m| time::days_from_civil(m.year, m.month, time::days_in_month(m.year, m.month)));
    let (from, to) = (time::days_from_civil(months[0].year, months[0].month, 1), last);
    let mut holidays = Vec::new();
    let mut shown = Vec::new();
    match cx.archive {
        None if !countries.is_empty() => notes.push("the public holidays are not downloaded yet: they come with the next sync".to_string()),
        None => {}
        Some(archive) => {
            let years = |y: i64| archive.years.contains(&y);
            if !months.iter().all(|m| years(m.year)) {
                notes.push(format!("public holidays only for {}", archive.years.iter().map(i64::to_string).collect::<Vec<_>>().join(", ")));
            }
            for code in &countries {
                // A month around: a holiday that began before the first day, or goes on after the last.
                match archive.of(code, from - 31, to + 31, cx.language) {
                    Some(found) => {
                        let index = shown.len();
                        shown.push((code.to_string(), named(code)));
                        holidays.extend(runs(&found).into_iter().filter(|m| (from..=to).contains(&m.day)).map(|m| Mark { country: index, ..m }));
                    }
                    None => notes.push(format!("no public holidays for {}", named(code))),
                }
            }
        }
    }
    holidays.sort_by_key(|m| (m.day, m.country));
    let weekend_country = countries.first().map(|c| c.to_string()).or_else(device_country);
    let view = View { title, months, columns, weekend: weekend_of(weekend_country.as_deref()).to_vec(), today: cx.today, countries: shown, holidays, year };
    (view, notes)
}

// ---------------------------------------------------------------- as text

/// The colours of the countries, in order, at a terminal.
pub const COLOURS: [&str; MAX_COUNTRIES] = ["\x1b[1;31m", "\x1b[1;34m", "\x1b[1;32m", "\x1b[1;35m", "\x1b[1;36m"];
const RESET: &str = "\x1b[0m";

/// How a day is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Plain,
    Weekend,
    /// A holiday of these countries (the first's colour; several: underlined too).
    Holiday { country: usize, several: bool },
}

impl View {
    /// How a day (days since 1970-01-01) of a column is drawn, and whether it is today.
    pub fn look(&self, day: i64, column: usize) -> (Look, bool) {
        let today = day == self.today;
        let of = self.holiday_of(day);
        let look = match of.first() {
            Some(&country) => Look::Holiday { country, several: of.len() > 1 },
            None if self.weekend.contains(&self.columns[column]) => Look::Weekend,
            None => Look::Plain,
        };
        (look, today)
    }
}

/// A month's lines, each `WIDTH` wide: the title, the weekdays, a line a week. `colour`: today and
/// the holidays in the terminal's colours; else the days alone, as `cal` writes them to a file.
fn month_lines(view: &View, m: &Month, colour: bool) -> Vec<String> {
    let mut lines = vec![format!("{:^WIDTH$}", m.title), format!("Wk {}", view.columns.iter().map(|&c| format!("{} ", WEEKDAYS[c as usize - 1])).collect::<String>())];
    for w in &m.weeks {
        let mut line = format!("{:>2} ", w.number);
        for (k, &d) in w.days.iter().enumerate() {
            if d == 0 {
                line.push_str("   ");
                continue;
            }
            if !colour {
                line.push_str(&format!("{d:>2} "));
                continue;
            }
            let (look, today) = view.look(w.first + k as i64, k);
            let style = match look {
                Look::Plain => String::new(),
                Look::Weekend => "\x1b[2m".to_string(),
                Look::Holiday { country, several } => format!("{}{}", COLOURS[country % MAX_COUNTRIES], if several { "\x1b[4m" } else { "" }),
            };
            let style = if today { format!("{style}\x1b[7m") } else { style };
            match style.is_empty() {
                true => line.push_str(&format!("{d:>2} ")),
                false => line.push_str(&format!("{style}{d:>2}{RESET} ")),
            }
        }
        lines.push(line);
    }
    lines
}

/// A month's width: `Wk` and 7 days of 3 characters.
pub const WIDTH: usize = 24;

/// The calendar as text: a month, or the year three months a row; the countries' colours and, for a
/// month, its holidays under it.
pub fn text(view: &View, colour: bool) -> String {
    let mut out: Vec<String> = Vec::new();
    if view.year {
        out.push(format!("{:^width$}", view.title, width = 3 * WIDTH + 4));
        for row in view.months.chunks(3) {
            out.push(String::new());
            let blocks: Vec<Vec<String>> = row.iter().map(|m| month_lines(view, m, colour)).collect();
            let height = blocks.iter().map(Vec::len).max().unwrap_or(0);
            for i in 0..height {
                let cells: Vec<String> = blocks.iter().map(|b| b.get(i).map_or(" ".repeat(WIDTH), |l| pad(l, WIDTH))).collect();
                out.push(cells.join("  "));
            }
        }
    } else {
        out.extend(month_lines(view, &view.months[0], colour));
    }
    if !view.countries.is_empty() {
        out.push(String::new());
        let legend: Vec<String> = view
            .countries
            .iter()
            .enumerate()
            .map(|(i, (code, name))| if colour { format!("{}■{RESET} {name} ({code})", COLOURS[i % MAX_COUNTRIES]) } else { format!("{code} {name}") })
            .collect();
        out.push(legend.join(if colour { "   " } else { " · " }));
        if !view.year {
            let list = listed(view);
            let width = list.iter().map(|(days, ..)| days.chars().count()).max().unwrap_or(0);
            for (days, country, name, length) in list {
                let code = &view.countries[country].0;
                let code = if colour { format!("{}{code}{RESET}", COLOURS[country % MAX_COUNTRIES]) } else { code.clone() };
                let long = if length > 1 { format!(" ({length} days)") } else { String::new() };
                out.push(format!("{days:>width$}  {code}  {name}{long}"));
            }
        }
    }
    out.iter().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
}

/// `line` padded with spaces to `width` visible characters (the colours' codes do not count).
fn pad(line: &str, width: usize) -> String {
    let mut visible = 0;
    let mut escape = false;
    for c in line.chars() {
        match (escape, c) {
            (_, '\x1b') => escape = true,
            (true, 'm') => escape = false,
            (true, _) => {}
            (false, _) => visible += 1,
        }
    }
    format!("{line}{}", " ".repeat(width.saturating_sub(visible)))
}

/// The request of the calendar `by` months (a year: years) after this one, its countries kept:
/// `cal 2026-11 BR EG PE`, `cal 2028 IT`; for swiping and the arrows.
pub fn step(view: &View, by: i64) -> String {
    let span = match view.months.first() {
        Some(m) if !view.year => {
            let total = m.year * 12 + i64::from(m.month) - 1 + by;
            format!("{:04}-{:02}", total.div_euclid(12), total.rem_euclid(12) + 1)
        }
        Some(m) => (m.year + by).to_string(),
        None => String::new(),
    };
    let codes: Vec<&str> = view.countries.iter().map(|(code, _)| code.as_str()).collect();
    format!("cal {span} {}", codes.join(" ")).trim_end().to_string()
}

// ---------------------------------------------------------------- as JSON

/// The calendar for the apps:
///
/// ```text
/// {"title":"October 2026","year":false,"today":"2026-10-26","weekdays":["Mo",…],"weekend":[5,6],
///  "countries":[{"code":"IT","name":"Italy"}],
///  "months":[{"title":"October 2026","year":2026,"month":10,"weeks":[{"week":40,"days":[0,0,0,1,2,3,4]},…]}],
///  "holidays":[{"date":"2026-11-01","day":1,"month":11,"country":0,"name":"Ognissanti",
///   "first":"2026-11-01","length":1,"days":"1 Nov"}],
///  "previous":"cal 2026-09 IT","next":"cal 2026-11 IT"}
/// ```
/// `weekend`: the columns (0 the first) that are the weekend; `previous` and `next`: the requests of
/// the month (the year) before and after.
pub fn json(view: &View) -> String {
    use crate::json::Object;
    let iso = |day: i64| {
        let (y, m, d) = time::civil_from_days(day);
        format!("{y:04}-{m:02}-{d:02}")
    };
    let weekdays: Vec<&str> = view.columns.iter().map(|&c| WEEKDAYS[c as usize - 1]).collect();
    let weekend: Vec<String> = (0..7).filter(|&k| view.weekend.contains(&view.columns[k])).map(|k| k.to_string()).collect();
    let countries: Vec<String> = view.countries.iter().map(|(code, name)| Object::new().str("code", code).str("name", name).finish()).collect();
    let months: Vec<String> = view
        .months
        .iter()
        .map(|m| {
            let weeks: Vec<String> = m
                .weeks
                .iter()
                .map(|w| format!("{{\"week\":{},\"days\":[{}]}}", w.number, w.days.iter().map(u32::to_string).collect::<Vec<_>>().join(",")))
                .collect();
            Object::new().str("title", &m.title).num("year", m.year).num("month", m.month).list("weeks", &weeks).finish()
        })
        .collect();
    let holidays: Vec<String> = view
        .holidays
        .iter()
        .map(|h| {
            let (_, month, day) = time::civil_from_days(h.day);
            Object::new()
                .str("date", &iso(h.day))
                .num("day", day)
                .num("month", month)
                .num("country", h.country)
                .str("name", &h.name)
                .str("first", &iso(h.first))
                .num("length", h.length)
                .str("days", &days_label(h.first, h.length))
                .finish()
        })
        .collect();
    Object::new()
        .str("title", &view.title)
        .num("year", view.year)
        .str("today", &iso(view.today))
        .strs("weekdays", &weekdays)
        .raw("weekend", &format!("[{}]", weekend.join(",")))
        .list("countries", &countries)
        .list("months", &months)
        .list("holidays", &holidays)
        .str("previous", &step(view, -1))
        .str("next", &step(view, 1))
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 26 Oct 2026, a Monday.
    const TODAY: (i64, u32) = (2026, 10);

    fn req(input: &str) -> Result<Request, String> {
        request(input, TODAY).expect("a calendar request")
    }

    fn codes(r: &Request) -> Vec<&str> {
        r.countries.clone()
    }

    #[test]
    fn which_month_or_year() {
        assert_eq!(req("cal").unwrap().span, Span::Month(2026, 10));
        assert_eq!(req("Calendar").unwrap().span, Span::Month(2026, 10));
        assert_eq!(req("cal march").unwrap().span, Span::Month(2026, 3), "this year's");
        assert_eq!(req("cal march 2027").unwrap().span, Span::Month(2027, 3));
        assert_eq!(req("cal 2027 march").unwrap().span, Span::Month(2027, 3));
        assert_eq!(req("cal mar").unwrap().span, Span::Month(2026, 3));
        assert_eq!(req("cal marzo").unwrap().span, Span::Month(2026, 3));
        assert_eq!(req("cal 2027-03").unwrap().span, Span::Month(2027, 3));
        assert_eq!(req("cal 3/2027").unwrap().span, Span::Month(2027, 3));
        assert_eq!(req("cal 2027").unwrap().span, Span::Year(2027));
        assert_eq!(req("cal next month").unwrap().span, Span::Month(2026, 11));
        assert_eq!(req("cal last month").unwrap().span, Span::Month(2026, 9));
        assert_eq!(request("cal next month", (2026, 12)).unwrap().unwrap().span, Span::Month(2027, 1));
        assert_eq!(request("cal last month", (2026, 1)).unwrap().unwrap().span, Span::Month(2025, 12));
        assert_eq!(req("cal next year").unwrap().span, Span::Year(2027));
        assert_eq!(req("cal this year").unwrap().span, Span::Year(2026));
        assert_eq!(req("cal december next year").unwrap().span, Span::Month(2027, 12));
        assert_eq!(req("cal in march").unwrap().span, Span::Month(2026, 3));
    }

    #[test]
    fn countries_by_name_code_or_alias() {
        assert_eq!(codes(&req("cal march UAE,USA,Italy").unwrap()), ["AE", "US", "IT"]);
        assert_eq!(codes(&req("cal march uae usa italy").unwrap()), ["AE", "US", "IT"]);
        assert_eq!(codes(&req("cal 2027 de fr").unwrap()), ["DE", "FR"]);
        assert_eq!(codes(&req("cal march united arab emirates, united states").unwrap()), ["AE", "US"]);
        assert_eq!(codes(&req("cal Japan China Russia").unwrap()), ["JP", "CN", "RU"]);
        assert_eq!(codes(&req("cal uk").unwrap()), ["GB"]);
        assert_eq!(codes(&req("cal italy italia IT").unwrap()), ["IT"], "once");
        assert_eq!(req("cal march UAE").unwrap().span, Span::Month(2026, 3));
    }

    #[test]
    fn wrong_words_and_too_many_countries() {
        assert_eq!(req("cal blorp"), Err("`blorp` is not a month, a year or a country".into()));
        assert_eq!(req("cal 2027-13"), Err("`2027-13` is not a month, a year or a country".into()));
        assert_eq!(req("cal it fr de es pt nl"), Err("at most 5 countries at a time".into()));
        assert!(req("cal it fr de es pt").is_ok());
        assert_eq!(request("10 cal J", TODAY), None, "a unit");
        assert_eq!(request("calories", TODAY), None);
        assert_eq!(countries_of("IT, United Arab Emirates,it"), Ok(vec!["IT", "AE"]));
        assert!(countries_of("Narnia").unwrap_err().contains("`Narnia` is not a country"));
    }

    #[test]
    fn week_starts_by_region() {
        assert_eq!(week_start_of("IT"), WeekStart::Monday);
        assert_eq!(week_start_of("US"), WeekStart::Sunday);
        assert_eq!(week_start_of("AE"), WeekStart::Saturday);
        assert_eq!(weekend_of(Some("SA")), [5, 6]);
        assert_eq!(weekend_of(Some("IT")), [6, 7]);
        assert_eq!(weekend_of(None), [6, 7]);
    }

    #[test]
    fn a_months_weeks() {
        // October 2026 starts on a Thursday.
        let m = month(2026, 10, WeekStart::Monday);
        assert_eq!(m.title, "October 2026");
        assert_eq!(m.weeks.len(), 5);
        assert_eq!(m.weeks[0].days, [0, 0, 0, 1, 2, 3, 4]);
        assert_eq!(m.weeks[0].number, 40);
        assert_eq!(m.weeks[4].days, [26, 27, 28, 29, 30, 31, 0]);
        assert_eq!(m.weeks[4].number, 44);
        // From Sunday: the row of Sunday 25 has the week of Monday 26.
        let m = month(2026, 10, WeekStart::Sunday);
        assert_eq!(m.weeks[0].days, [0, 0, 0, 0, 1, 2, 3]);
        assert_eq!(m.weeks[4].days, [25, 26, 27, 28, 29, 30, 31]);
        assert_eq!(m.weeks[4].number, 44);
        let m = month(2026, 10, WeekStart::Saturday);
        assert_eq!(m.weeks[0].days, [0, 0, 0, 0, 0, 1, 2]);
        // February 2027: four rows from Monday; ISO week 53 of 2026 starts January.
        assert_eq!(month(2027, 2, WeekStart::Monday).weeks.len(), 4);
        assert_eq!(month(2027, 1, WeekStart::Monday).weeks[0].number, 53);
    }

    fn settings(pairs: &[(&str, &str)]) -> crate::settings::Settings {
        let mut s = crate::settings::Settings::default();
        s.set("calendar.week_start", Some("monday")).unwrap();
        for (k, v) in pairs {
            s.set(k, Some(v)).unwrap();
        }
        s
    }

    const ARCHIVE: &str = r#"{"format":1,"built":"2026-10-26","years":[2025,2026,2027],"countries":{
        "IT":{"default":"it","days":["2026-10-31","2026-11-01","2026-12-08","2026-12-25"],"names":{"en":["Not a holiday","All Saints' Day","Immaculate Conception","Christmas Day"],"it":["Non festivo","Ognissanti","Immacolata Concezione","Natale"]}},
        "AE":{"default":"ar","days":["2026-12-02","2026-12-03"],"names":{"ar":["اليوم الوطني","اليوم الوطني"],"en":["National Day","National Day"]}},
        "US":{"default":"en","days":["2026-11-26","2026-12-25"],"names":{"en":["Thanksgiving","Christmas Day"]}}}}"#;

    fn shown(input: &str, s: &crate::settings::Settings, archive: Option<&Archive>, language: Option<&str>) -> (View, Vec<String>) {
        let today = time::days_from_civil(2026, 10, 26);
        view(&req(input).unwrap(), &Context { settings: s, archive, language, today })
    }

    #[test]
    fn holidays_of_several_countries_a_colour_each() {
        let archive = crate::holidays::parse(ARCHIVE).unwrap();
        let s = settings(&[]);
        let (v, notes) = shown("cal december UAE,USA,Italy", &s, Some(&archive), Some("it"));
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(v.countries, [("AE".into(), "United Arab Emirates".into()), ("US".into(), "United States".into()), ("IT".into(), "Italy".into())]);
        let names: Vec<(u32, usize, &str)> = v.holidays.iter().map(|h| (time::civil_from_days(h.day).2, h.country, h.name.as_str())).collect();
        assert_eq!(names, [(2, 0, "National Day"), (3, 0, "National Day"), (8, 2, "Immacolata Concezione"), (25, 1, "Christmas Day"), (25, 2, "Natale")]);
        // A month's own days only: not the next one's New Year in the grid's last week.
        let (nov, _) = shown("cal november italy", &s, Some(&archive), None);
        assert_eq!(nov.holidays.iter().map(|h| h.name.as_str()).collect::<Vec<_>>(), ["All Saints' Day"]);
        // Christmas is both the US's and Italy's: the first's colour, marked as several.
        let christmas = time::days_from_civil(2026, 12, 25);
        assert_eq!(v.look(christmas, 4), (Look::Holiday { country: 1, several: true }, false));
        // The weekend of the first country named: the Emirates', Saturday and Sunday.
        assert_eq!(v.weekend, [6, 7]);
        let text = text(&v, false);
        assert!(text.contains("2-3 Dec  AE  National Day (2 days)\n  8 Dec  IT  Immacolata Concezione\n 25 Dec  US  Christmas Day\n 25 Dec  IT  Natale"), "{text}");
        assert!(text.contains("AE United Arab Emirates · US United States · IT Italy"), "{text}");
    }

    #[test]
    fn without_cal_a_month_first() {
        assert_eq!(request("oct egypt", TODAY), Some(Ok(Request { span: Span::Month(2026, 10), countries: vec!["EG"] })));
        assert_eq!(request("march", TODAY).unwrap().unwrap().span, Span::Month(2026, 3));
        assert_eq!(request("dec 2027 italy usa", TODAY).unwrap().unwrap().span, Span::Month(2027, 12));
        assert_eq!(request("Oct, Egypt", TODAY).unwrap().unwrap().countries, ["EG"]);
        // Anything else is left to the others: a date, a date's math, a format.
        for other in ["jan 30 2025", "mar 12 2026 + 3d", "dec 25 2026", "march 2027 unix", "octx egypt", "2027", "egypt oct", "Italy"] {
            assert_eq!(request(other, TODAY), None, "{other}");
        }
    }

    #[test]
    fn a_month_and_its_year_written_any_way() {
        for written in ["oct 2026", "OCT26", "oct26", "0ct 26", "Oct 26", "oct2026", "0CT2026", "october 26", "0ctober 2026", "cal oct26", "cal 0ct 26", "cal 26 oct", "cal 2026 oct"] {
            assert_eq!(request(written, (2025, 1)).and_then(Result::ok).map(|r| r.span), Some(Span::Month(2026, 10)), "{written}");
        }
        assert_eq!(request("n0v 27 egypt", (2025, 1)).unwrap().unwrap(), Request { span: Span::Month(2027, 11), countries: vec!["EG"] });
        // Two digits alone are no year: a month is needed with them.
        assert_eq!(req("cal 26"), Err("`26` is not a month, a year or a country".into()));
        assert_eq!(req("cal oct 26 2027"), Err("a month and one year: `oct 2026`".into()));
        assert_eq!(request("oct 2026x", TODAY), None);
        assert_eq!(request("oct266", TODAY), None);
    }

    #[test]
    fn a_holidays_days_in_a_row() {
        let day = |d: u32| time::days_from_civil(2026, 12, d);
        let found: Vec<Holiday> = [(1, "A"), (2, "A"), (3, "A"), (3, "B"), (5, "A")].iter().map(|&(d, n)| Holiday { day: day(d), name: n.into() }).collect();
        let marks = runs(&found);
        let of = |i: usize| (marks[i].first, marks[i].length);
        assert_eq!([of(0), of(1), of(2)], [(day(1), 3); 3], "three days in a row");
        assert_eq!(of(3), (day(3), 1), "another name the same day: its own");
        assert_eq!(of(4), (day(5), 1), "a day apart: another");
        assert_eq!(days_label(day(2), 1), "2 Dec");
        assert_eq!(days_label(day(2), 2), "2-3 Dec");
        assert_eq!(days_label(time::days_from_civil(2026, 9, 30), 3), "30 Sep-2 Oct");
    }

    #[test]
    fn a_holiday_from_the_month_before() {
        // 30 Nov to 2 Dec: December shows its days, and the holiday as a whole.
        let archive = crate::holidays::parse(r#"{"format":1,"years":[2026],"countries":{"IT":{"default":"en","days":["2026-11-30","2026-12-01","2026-12-02"],"names":{"en":["Long Feast","Long Feast","Long Feast"]}}}}"#).unwrap();
        let (v, _) = shown("cal december italy", &settings(&[]), Some(&archive), None);
        assert_eq!(v.holidays.len(), 2);
        assert!(v.holidays.iter().all(|h| h.first == time::days_from_civil(2026, 11, 30) && h.length == 3));
        assert_eq!(listed(&v), [("30 Nov-2 Dec".to_string(), 0, "Long Feast".to_string(), 3)]);
    }

    #[test]
    fn the_settings_country_and_today() {
        let archive = crate::holidays::parse(ARCHIVE).unwrap();
        let s = settings(&[("holidays.countries", "IT")]);
        let (v, _) = shown("cal november", &s, Some(&archive), Some("en"));
        assert_eq!(v.countries.len(), 1);
        assert_eq!(v.holidays[0].name, "All Saints' Day");
        let (v, _) = shown("cal", &s, Some(&archive), None);
        let text = text(&v, false);
        assert!(text.starts_with("      October 2026\nWk Mo Tu We Th Fr Sa Su\n40           1  2  3  4"), "{text}");
        assert!(text.contains("\n44 26 27 28 29 30 31\n"), "{text}");
        assert!(calendar_text_today(&v).contains("\x1b[7m26\x1b[0m"), "today, at a terminal");
    }

    #[test]
    fn no_archive_no_holidays_and_holidays_off() {
        let s = settings(&[("holidays.countries", "IT")]);
        let (v, notes) = shown("cal december", &s, None, None);
        assert!(v.holidays.is_empty() && v.countries.is_empty());
        assert_eq!(notes, ["the public holidays are not downloaded yet: they come with the next sync"]);
        let archive = crate::holidays::parse(ARCHIVE).unwrap();
        let off = settings(&[("holidays", "false")]);
        let (v, notes) = shown("cal december italy", &off, Some(&archive), None);
        assert!(v.holidays.is_empty());
        assert_eq!(notes, ["the public holidays are off: turn on the holidays setting"]);
        let (_, notes) = shown("cal december", &off, Some(&archive), None);
        assert!(notes.is_empty(), "off and none asked: nothing to say");
    }

    #[test]
    fn years_and_countries_the_archive_lacks() {
        let archive = crate::holidays::parse(ARCHIVE).unwrap();
        let s = settings(&[]);
        let (v, notes) = shown("cal 2030 italy", &s, Some(&archive), None);
        assert!(v.holidays.is_empty());
        assert_eq!(notes, ["public holidays only for 2025, 2026, 2027"]);
        let (_, notes) = shown("cal december france", &s, Some(&archive), None);
        assert_eq!(notes, ["no public holidays for France"]);
    }

    fn calendar_text_today(v: &View) -> String {
        text(v, true)
    }

    #[test]
    fn a_year_three_months_a_row() {
        let archive = crate::holidays::parse(ARCHIVE).unwrap();
        let (v, _) = shown("cal 2026 italy", &settings(&[]), Some(&archive), None);
        assert_eq!(v.months.len(), 12);
        let text = text(&v, false);
        assert!(text.lines().any(|l| l.contains("January 2026") && l.contains("February 2026") && l.contains("March 2026")), "{text}");
        assert!(!text.contains("Natale") && text.contains("IT Italy"), "the year marks, the names are for a month: {text}");
        assert!(text.lines().all(|l| l.chars().count() <= 3 * WIDTH + 4), "{text}");
    }

    #[test]
    fn colours_at_a_terminal() {
        let archive = crate::holidays::parse(ARCHIVE).unwrap();
        let (v, _) = shown("cal december italy usa", &settings(&[]), Some(&archive), None);
        let text = text(&v, true);
        assert!(text.contains("\x1b[1;31m 8\x1b[0m"), "Italy's colour: {text:?}");
        assert!(text.contains("\x1b[1;31m\x1b[4m25\x1b[0m"), "two countries: underlined: {text:?}");
        assert_eq!(pad("\x1b[1m12\x1b[0m", 4), "\x1b[1m12\x1b[0m  ");
    }

    #[test]
    fn the_month_before_and_after() {
        let archive = crate::holidays::parse(ARCHIVE).unwrap();
        let (v, _) = shown("cal december italy usa", &settings(&[]), Some(&archive), None);
        assert_eq!((step(&v, 1).as_str(), step(&v, -1).as_str()), ("cal 2027-01 IT US", "cal 2026-11 IT US"));
        let (v, _) = shown("cal 2026", &settings(&[("holidays", "false")]), None, None);
        assert_eq!((step(&v, 1).as_str(), step(&v, -1).as_str()), ("cal 2027", "cal 2025"));
        // The request it gives is the calendar asked.
        assert_eq!(req(&step(&v, 1)).unwrap().span, Span::Year(2027));
        assert_eq!(req("cal 2027-01 IT US").unwrap(), Request { span: Span::Month(2027, 1), countries: vec!["IT", "US"] });
    }

    #[test]
    fn as_json() {
        let archive = crate::holidays::parse(ARCHIVE).unwrap();
        let (v, _) = shown("cal december italy", &settings(&[]), Some(&archive), Some("it"));
        let doc = crate::json::parse(&json(&v)).unwrap();
        assert_eq!(doc.get("title").and_then(|t| t.str()), Some("December 2026"));
        assert_eq!(doc.get("today").and_then(|t| t.str()), Some("2026-10-26"));
        assert_eq!(doc.get("weekdays").unwrap().strings(), ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]);
        assert_eq!(doc.get("weekend").unwrap().array().iter().filter_map(|n| n.number()).collect::<Vec<_>>(), [5.0, 6.0]);
        let month = &doc.get("months").unwrap().array()[0];
        assert_eq!(month.get("weeks").unwrap().array()[0].get("days").unwrap().array().len(), 7);
        let first = &doc.get("holidays").unwrap().array()[0];
        assert_eq!(first.get("name").and_then(|n| n.str()), Some("Immacolata Concezione"));
        assert_eq!(first.get("day").and_then(|n| n.number()), Some(8.0));
    }
}
