//! Exchange rates of a past day: `100 usd to eur 2025-01-30`, `… on 30 Jan 2025`, `… yesterday`,
//! `… 3 months ago`; the date ends the request or starts it. No date: the latest rates, never asked.
//!
//! The rates of each day are a file, `currencies-YYMMDD.json` (`scripts/build-rates.py`: the latest
//! rates of ExchangeRate-API, as the app uses them), added every day and kept `DAYS` days: the
//! archive starts the day it was set up, so earlier days have no file. A day is
//! downloaded when a request needs it (5 s at most), kept in `rates/` beside the latest rates, and
//! never downloaded again: the past does not change. `prune` deletes the files older than `DAYS`.

use crate::config::Config;
use crate::currency::{self, Error};
use crate::online::Data;
use crate::time::{self, Ambiguity, AmbiguityKind, Choice, Order};
use std::path::PathBuf;

/// Where the files are (also a setting, `rates.history_url`, for mirrors and tests).
pub const URL: &str = "https://github.com/lucapf/aequalis/releases/download/rates";
/// How many days back, today included.
pub const DAYS: i64 = 750;
/// Seconds to download a day's rates.
const SECONDS: u32 = 5;
const DAY: i64 = 86_400;
/// A date is this many words at most (`3 months ago`, `30 January 2025`).
const MAX_WORDS: usize = 4;

// ---------------------------------------------------------------- the date in a request

/// How a date was read.
enum Found {
    Day(i64),
    /// Written in a way that is more than one date (`03/04/25`): the pattern, year, month, day of each.
    Ambiguous(Vec<(String, i64, u32, u32)>),
}

/// A request with a date: the request without it, and the day (days since 1970-01-01); `None` for
/// the latest rates (today). `None` too when there is no date after all: the request is not
/// a currency request, or is one as written. `today` is in days.
pub fn split(config: &Config, input: &str, today: i64) -> Option<Result<(String, Option<i64>), Error>> {
    let words: Vec<&str> = input.split_whitespace().collect();
    if words.len() < 3 || currency::is_request(config, input) {
        return None;
    }
    for n in (1..=MAX_WORDS.min(words.len() - 2)).rev() {
        // The date last (`… on 30 Jan 2025`), or first (`on 30 Jan 2025 100 usd eur`).
        let last = (&words[words.len() - n..], &words[..words.len() - n], true);
        let first = (&words[..n], &words[n..], false);
        for (date, rest, ends) in [last, first] {
            let Some(found) = read(&date.join(" "), today) else { continue };
            let mut rest = rest.to_vec();
            if ends && rest.last() == Some(&"on") {
                rest.pop();
            }
            let rest = rest.join(" ");
            if !currency::is_request(config, &rest) {
                continue;
            }
            return Some(finish(found, &date.join(" "), rest, today));
        }
    }
    // `on 30 Jan 2025 100 usd eur`: `on` before a date at the start.
    if words.first() == Some(&"on") {
        return split(config, &words[1..].join(" "), today);
    }
    None
}

fn finish(found: Found, text: &str, rest: String, today: i64) -> Result<(String, Option<i64>), Error> {
    match found {
        Found::Ambiguous(readings) => {
            let choices = readings
                .into_iter()
                .map(|(pattern, y, m, d)| {
                    let date = format!("{y:04}-{m:02}-{d:02}");
                    Choice { pattern, request: format!("{rest} {date}"), date }
                })
                .collect();
            Err(Error::Ambiguous(Ambiguity { text: text.to_string(), choices, kind: AmbiguityKind::Date, note: String::new() }))
        }
        Found::Day(day) if day == today => Ok((rest, None)),
        Found::Day(day) if day > today => Err(Error::NoRates(format!("no exchange rates of {}: that day has not come yet", shown(day)))),
        Found::Day(day) if today - day >= DAYS => Err(Error::NoRates(format!("no exchange rates of {}: the history keeps {DAYS} days", shown(day)))),
        Found::Day(day) => Ok((rest, Some(day))),
    }
}

/// A day for people, in the `format.date` setting.
pub fn shown(day: i64) -> String {
    time::human_day(day * DAY)
}

/// `text` as a date: relative (`yesterday`, `3 months ago`) or written (`2025-01-30`, `30 Jan 2025`).
fn read(text: &str, today: i64) -> Option<Found> {
    if let Some(day) = relative(text, today) {
        return Some(Found::Day(day));
    }
    // Numbers alone are no dates: a separator or a month's name makes one.
    let reading = time::read_date(text, None, Order::of_locale())?;
    if reading.month_name.is_none() && !text.contains(['-', '/', '.']) {
        return None;
    }
    if reading.readings.len() > 1 {
        return Some(Found::Ambiguous(reading.readings.iter().map(|(p, y, m, d)| (p.name(), *y, *m, *d)).collect()));
    }
    let c = reading.civil;
    Some(Found::Day(time::days_from_civil(c.year, c.month, c.day)))
}

/// `yesterday`, `today`, `tomorrow`; a count back, `3 days ago`, `3 d ago`, `3dago`, `a week ago`,
/// or `-3d`: in days (`d`), working days (`wd`, Monday to Friday), weeks (`w`), months (`m`, `mo`),
/// years (`y`).
fn relative(text: &str, today: i64) -> Option<i64> {
    let lower = text.to_lowercase();
    match lower.as_str() {
        "yesterday" => return Some(today - 1),
        "today" => return Some(today),
        "tomorrow" => return Some(today + 1),
        _ => {}
    }
    // The spaces do not matter: `3 d ago`, `3d ago`, `3 dago` are one.
    let joined: String = lower.split_whitespace().collect();
    let count = match joined.strip_prefix('-') {
        Some(count) => count,
        None => joined.strip_suffix("ago")?,
    };
    let digits = count.bytes().take_while(u8::is_ascii_digit).count();
    let (n, unit) = match digits {
        1..=6 => (count[..digits].parse().ok()?, &count[digits..]),
        0 => (1, ["one", "an", "a"].iter().find_map(|word| count.strip_prefix(word))?),
        _ => return None,
    };
    match unit.strip_suffix('s').unwrap_or(unit) {
        "d" | "day" => Some(today - n),
        "wd" | "workday" | "workingday" => Some(working_days_back(today, n)),
        "w" | "wk" | "week" => Some(today - 7 * n),
        "m" | "mo" | "month" => Some(months_back(today, n)),
        "y" | "yr" | "year" => Some(months_back(today, 12 * n)),
        _ => None,
    }
}

/// `n` working days (Monday to Friday) before the day (days): from a Saturday or a Sunday, one
/// working day back is the Friday.
fn working_days_back(day: i64, n: i64) -> i64 {
    // Past the history either way: a working day is at least a day.
    let mut left = n.min(DAYS);
    let mut day = day;
    while left > 0 {
        day -= 1;
        // 1970-01-01 was a Thursday: 0 is Monday.
        if (day + 3).rem_euclid(7) < 5 {
            left -= 1;
        }
    }
    day
}

/// `months` months before the day (days), the day of the month kept as far as the month has it.
fn months_back(day: i64, months: i64) -> i64 {
    let (y, m, d) = time::civil_from_days(day);
    let total = y * 12 + i64::from(m) - 1 - months;
    let (year, month) = (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32);
    let (next_year, next_month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    let length = time::days_from_civil(next_year, next_month, 1) - time::days_from_civil(year, month, 1);
    time::days_from_civil(year, month, d.min(length as u32))
}

// ---------------------------------------------------------------- the files

/// The file's name: `currencies-250130.json`.
fn file_name(day: i64) -> String {
    let (y, m, d) = time::civil_from_days(day);
    format!("currencies-{:02}{m:02}{d:02}.json", y.rem_euclid(100))
}

/// The day (days) of a file's name.
fn file_day(name: &str) -> Option<i64> {
    let digits = name.strip_prefix("currencies-")?.strip_suffix(".json")?;
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let part = |i: usize| digits[i..i + 2].parse::<u32>().ok();
    Some(time::days_from_civil(2000 + i64::from(part(0)?), part(2)?, part(4)?))
}

/// `~/.config/aequalis/rates`.
fn folder() -> Option<PathBuf> {
    crate::files::dir().map(|d| d.join("rates"))
}

/// The rates of `day` (days): the file kept, else downloaded and kept. The reason in words when
/// there are none.
pub fn load(day: i64, now: i64) -> Result<Data, String> {
    let shown = shown(day);
    let path = folder().ok_or("no data folder")?.join(file_name(day));
    let kept = std::fs::read_to_string(&path).ok().and_then(|text| currency::parse(&text));
    let rates = match kept {
        Some(rates) => rates,
        None => download(day, &path).map_err(|failure| failure.reason(&shown))?,
    };
    let mut data = Data::default();
    for (name, value) in rates {
        data.set(&name, value, now);
    }
    Ok(data)
}

/// Why a day could not be downloaded.
enum Failure {
    /// The `offline` setting.
    Offline,
    /// Nothing answered.
    NoConnection,
    /// The archive answered without the day (or with a broken file).
    Lacks,
}

impl Failure {
    fn reason(&self, shown: &str) -> String {
        match self {
            Failure::Offline => format!("no exchange rates of {shown} on this device, and the offline setting is on"),
            Failure::NoConnection => format!("no exchange rates of {shown}: they could not be downloaded (no connection)"),
            Failure::Lacks => format!("no exchange rates of {shown}: the archive has none (it holds the days since it began)"),
        }
    }
}

fn download(day: i64, path: &std::path::Path) -> Result<Vec<(String, f64)>, Failure> {
    if !crate::network::allowed() {
        return Err(Failure::Offline);
    }
    let settings = crate::settings::current();
    let url = format!("{}/{}", settings.get("rates.history_url").trim_end_matches('/'), file_name(day));
    let Some(body) = crate::http::get(&url, SECONDS, true) else {
        return Err(if crate::http::answers(&url, SECONDS) { Failure::Lacks } else { Failure::NoConnection });
    };
    let text = String::from_utf8_lossy(&body);
    let rates = currency::parse(&text).ok_or(Failure::Lacks)?;
    let _ = crate::files::write_atomic(path, text.as_bytes());
    Ok(rates)
}

/// The days of rates kept: how many (within the last `DAYS`), and the oldest.
pub struct Held {
    pub days: usize,
    pub since: Option<i64>,
}

pub fn held(today: i64) -> Held {
    let kept = folder().and_then(|f| std::fs::read_dir(f).ok());
    let days: Vec<i64> = kept
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str().and_then(file_day))
        .filter(|day| *day <= today && today - day < DAYS)
        .collect();
    Held { days: days.len(), since: days.iter().copied().min() }
}

/// What is kept, for people: `42 days, since 14 Sep 26`, `none yet`, or `off`.
pub fn summary(enabled: bool) -> String {
    if !enabled {
        return "off".to_string();
    }
    let held = held(time::unix_now().div_euclid(DAY));
    match held.since {
        None => "none yet".to_string(),
        Some(since) => format!("{} day{}, since {}", held.days, if held.days == 1 { "" } else { "s" }, shown(since)),
    }
}

/// Downloads the days not kept yet, from today back (the archive has the days since it began, and
/// a day may be missing): a day already kept is skipped, and three days in a row the archive lacks
/// end it, as does no connection. How many were downloaded; none when the network is off.
pub fn sync(today: i64) -> usize {
    let Some(folder) = folder() else { return 0 };
    let (mut got, mut misses) = (0, 0);
    for back in 0..DAYS {
        let day = today - back;
        let path = folder.join(file_name(day));
        if path.exists() {
            misses = 0;
            continue;
        }
        match download(day, &path) {
            Ok(_) => (got, misses) = (got + 1, 0),
            // No day can come.
            Err(Failure::Offline | Failure::NoConnection) => break,
            Err(Failure::Lacks) => {
                misses += 1;
                if misses >= 3 {
                    break;
                }
            }
        }
    }
    got
}

/// Deletes the kept files older than `DAYS` (`today` in days).
pub fn prune(today: i64) {
    let Some(entries) = folder().and_then(|f| std::fs::read_dir(f).ok()) else { return };
    for entry in entries.flatten() {
        let old = entry.file_name().to_str().and_then(file_day).is_some_and(|day| today - day >= DAYS);
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-12.
    const TODAY: i64 = 20_738;

    fn date(y: i64, m: u32, d: u32) -> i64 {
        time::days_from_civil(y, m, d)
    }

    fn split_of(input: &str) -> Option<Result<(String, Option<i64>), Error>> {
        split(&Config::default(), input, TODAY)
    }

    #[test]
    fn today_is_the_civil_day() {
        assert_eq!(TODAY, date(2026, 10, 12));
    }

    #[test]
    fn relative_dates() {
        assert_eq!(relative("yesterday", TODAY), Some(TODAY - 1));
        assert_eq!(relative("Today", TODAY), Some(TODAY));
        assert_eq!(relative("3 days ago", TODAY), Some(TODAY - 3));
        assert_eq!(relative("a week ago", TODAY), Some(TODAY - 7));
        assert_eq!(relative("2 weeks ago", TODAY), Some(TODAY - 14));
        assert_eq!(relative("3 months ago", TODAY), Some(date(2026, 7, 12)));
        assert_eq!(relative("1 year ago", TODAY), Some(date(2025, 10, 12)));
        // The month is shorter: its last day.
        assert_eq!(months_back(date(2026, 3, 31), 1), date(2026, 2, 28));
        assert_eq!(months_back(date(2026, 1, 15), 2), date(2025, 11, 15));
        assert_eq!(relative("3 fortnights ago", TODAY), None);
        assert_eq!(relative("x days ago", TODAY), None);
    }

    #[test]
    fn short_relative_dates() {
        for text in ["-1d", "- 1 d", "1 d ago", "1d ago", "1 dago", "1dago", "-1day", "1 day ago"] {
            assert_eq!(relative(text, TODAY), Some(TODAY - 1), "{text}");
        }
        assert_eq!(relative("3 d ago", TODAY), Some(TODAY - 3));
        assert_eq!(relative("3 dago", TODAY), Some(TODAY - 3));
        assert_eq!(relative("-2w", TODAY), Some(TODAY - 14));
        for text in ["-3m", "-3mo", "3 m ago", "3mo ago", "3 months ago"] {
            assert_eq!(relative(text, TODAY), Some(date(2026, 7, 12)), "{text}");
        }
        for text in ["-1y", "1 y ago", "1yago", "-1yr"] {
            assert_eq!(relative(text, TODAY), Some(date(2025, 10, 12)), "{text}");
        }
        // A count with neither `-` nor `ago` is no date, nor both, nor an unknown unit.
        for text in ["1d", "3 m", "-1d ago", "-d", "-1x", "-1234567d", "1 wdd ago"] {
            assert_eq!(relative(text, TODAY), None, "{text}");
        }
    }

    #[test]
    fn working_days_skip_the_weekend() {
        // TODAY is a Monday: one working day back is the Friday before.
        assert_eq!(relative("-1wd", TODAY), Some(date(2026, 10, 9)));
        assert_eq!(relative("1 wd ago", TODAY), Some(date(2026, 10, 9)));
        assert_eq!(relative("1wdago", TODAY), Some(date(2026, 10, 9)));
        assert_eq!(relative("-5wd", TODAY), Some(date(2026, 10, 5)));
        assert_eq!(relative("-6wd", TODAY), Some(date(2026, 10, 2)));
        assert_eq!(relative("3 working days ago", TODAY), Some(date(2026, 10, 7)));
        // From a weekend: Friday, then Thursday.
        assert_eq!(working_days_back(date(2026, 10, 11), 1), date(2026, 10, 9));
        assert_eq!(working_days_back(date(2026, 10, 10), 2), date(2026, 10, 8));
        // Midweek, no weekend crossed.
        assert_eq!(working_days_back(date(2026, 10, 15), 2), date(2026, 10, 13));
        // A count far past the history stays past it.
        assert!(TODAY - working_days_back(TODAY, 999_999) >= DAYS);
    }

    #[test]
    fn short_dates_after_a_one_currency_request() {
        for (input, day) in [
            ("12 eur -1d", TODAY - 1),
            ("12 eur -1wd", date(2026, 10, 9)),
            ("12 eur yesterday", TODAY - 1),
            ("12 eur 3 d ago", TODAY - 3),
            ("12 eur 3 dago", TODAY - 3),
            ("12 eur -2m", date(2026, 8, 12)),
            ("12 eur -1y", date(2025, 10, 12)),
            ("12 usd to eur -1wd", date(2026, 10, 9)),
        ] {
            let (rest, found) = split_of(input).unwrap().unwrap();
            assert_eq!(found, Some(day), "{input}");
            assert!(input.starts_with(&rest), "{input}: {rest}");
        }
        // `qw` is no number: no currency request, so no date either.
        assert!(split_of("qw eur 3 dago").is_none());
        assert!(matches!(split_of("12 eur -3y"), Some(Err(Error::NoRates(m))) if m.contains("keeps 750 days")));
    }

    #[test]
    fn dates_written_and_asked() {
        let (rest, day) = split_of("100 usd to eur 2025-01-30").unwrap().unwrap();
        assert_eq!((rest.as_str(), day), ("100 usd to eur", Some(date(2025, 1, 30))));
        let (rest, day) = split_of("100 usd to eur on 30 Jan 2025").unwrap().unwrap();
        assert_eq!((rest.as_str(), day), ("100 usd to eur", Some(date(2025, 1, 30))));
        let (rest, day) = split_of("100 usd to eur yesterday").unwrap().unwrap();
        assert_eq!((rest.as_str(), day), ("100 usd to eur", Some(TODAY - 1)));
        let (rest, day) = split_of("100 usd to eur 3 months ago").unwrap().unwrap();
        assert_eq!((rest.as_str(), day), ("100 usd to eur", Some(date(2026, 7, 12))));
        // First.
        let (rest, day) = split_of("on 2025-01-30 100 usd to eur").unwrap().unwrap();
        assert_eq!((rest.as_str(), day), ("100 usd to eur", Some(date(2025, 1, 30))));
        // Today: the latest rates.
        let (rest, day) = split_of("100 usd to eur today").unwrap().unwrap();
        assert_eq!((rest.as_str(), day), ("100 usd to eur", None));
        let (_, day) = split_of("100 usd to eur 2026-10-12").unwrap().unwrap();
        assert_eq!(day, None);
    }

    #[test]
    fn requests_without_a_date_are_left_alone() {
        for input in ["100 usd to eur", "100 usd eur", "12 usd", "10:00 Tokyo 2025-01-30", "10:00 Rome to Tokyo", "5 km to mi", "2025-01-30 to unix"] {
            assert!(split_of(input).is_none(), "{input}");
        }
    }

    #[test]
    fn days_without_rates() {
        let message = |input: &str| match split_of(input) {
            Some(Err(Error::NoRates(m))) => m,
            other => panic!("{input}: {other:?}"),
        };
        assert!(message("100 usd to eur tomorrow").contains("has not come yet"));
        assert!(message("100 usd to eur 2024-01-01").contains("keeps 750 days"));
        // The oldest day kept is 749 days ago; one more is gone.
        assert_eq!(split_of("100 usd to eur 749 days ago").unwrap().unwrap().1, Some(TODAY - 749));
        assert!(message("100 usd to eur 750 days ago").contains("keeps 750 days"));
    }

    #[test]
    fn ambiguous_dates_ask() {
        let Some(Err(Error::Ambiguous(a))) = split_of("100 usd to eur 03/04/25") else { panic!("not asked") };
        assert_eq!(a.kind, AmbiguityKind::Date);
        assert!(a.choices.len() >= 2);
        assert!(a.choices.iter().all(|c| c.request.starts_with("100 usd to eur 20")), "{:?}", a.choices);
        assert!(a.choices.iter().any(|c| c.request == "100 usd to eur 2025-03-04"));
        assert!(a.choices.iter().any(|c| c.request == "100 usd to eur 2025-04-03"));
    }

    #[test]
    fn files_are_named_by_day() {
        let day = date(2025, 1, 30);
        assert_eq!(file_name(day), "currencies-250130.json");
        assert_eq!(file_day("currencies-250130.json"), Some(day));
        assert_eq!(file_day("currencies-2501.json"), None);
        assert_eq!(file_day("currencies.json"), None);
        assert_eq!(file_day("currencies-250130.json.tmp"), None);
    }
}
