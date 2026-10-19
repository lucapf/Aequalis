//! Formatting: the output formats (ISO, Unix, patterns, `local`), dates for people, durations.

use super::*;

/// `text` (digits, optional `.` fraction, optional leading `-`) times `scale`, rounded to an integer.
pub(super) fn scaled(text: &str, scale: i128) -> Option<i128> {
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
        let doubled = num.checked_mul(scale)?.checked_mul(2)?.checked_add(den)?;
        value = value.checked_add(doubled / (2 * den))?;
    }
    Some(sign * value)
}

/// `value / 10^digits` as a decimal, trailing zeros trimmed.
pub(super) fn decimal(value: i128, digits: u32) -> String {
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
pub(super) fn unix_scale(word: &str) -> Option<i128> {
    match word {
        "unix" => Some(NS),
        "unix-ms" => Some(1_000_000),
        "unix-us" => Some(1_000),
        "unix-ns" => Some(1),
        _ => None,
    }
}

pub(super) fn year(y: i64) -> String {
    if y < 0 {
        format!("-{:04}", -y)
    } else {
        format!("{y:04}")
    }
}

pub(super) fn fraction(nano: u32) -> String {
    match nano {
        0 => String::new(),
        n => format!(".{}", format!("{n:09}").trim_end_matches('0')),
    }
}

pub(super) fn zone_suffix(instant: &Instant, extended: bool) -> String {
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
pub(super) fn format_as(instant: &Instant, word: &str, seconds: bool) -> Result<String, TimeError> {
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
    let time = if short { format!("{:02}:{:02}", c.hour, c.minute) } else { format!("{:02}:{:02}:{:02}{}", c.hour, c.minute, c.second, fraction(c.nano)) };
    Ok(match word {
        "iso" => format!("{date}T{time}{}", zone_suffix(instant, true)),
        "iso-basic" => format!("{}{:02}{:02}T{}{}", year(c.year), c.month, c.day, time.replace(':', ""), zone_suffix(instant, false)),
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
pub(super) const MONTH_ABBREVIATIONS_EN: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sept", "Oct", "Nov", "Dec"];
pub(super) const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// The tokens of a human format, longest first (so `MMMM` is not read as `MM` twice).
pub(super) const HUMAN_TOKENS: [&str; 22] =
    ["YYYY", "MMMM", "dddd", "H24", "H12", "MMM", "ddd", "YY", "MM", "DD", "HH", "hh", "mm", "ss", "tz", "ZZ", "AM", "D", "M", "H", "h", "A"];

/// A date for people: `format` with its tokens replaced, the rest kept (`[text]`: literal).
///
/// `YYYY` 2026 · `YY` 26 · `MMMM` September · `MMM` Sept · `MM` 09 · `M` 9 · `DD` 05 · `D` 5 ·
/// `dddd` Saturday · `ddd` Sat · `H24` / `HH` 20 (24-hour) · `H` 20 · `H12` / `hh` 08 · `h` 8 ·
/// `mm` 05 · `ss` 07 · `AM` / `A` PM · `tz` GMT+2 (GMT+5:30, GMT) · `ZZ` +02:00.
/// A day for people, in the local zone: `3 Jan 2027`.
/// Now, in Unix seconds.
pub fn unix_now() -> i64 {
    now().ns.div_euclid(NS) as i64
}

/// A day (the date only, UTC: a day of data, not a moment) for people, in the `format.date`
/// setting: `12 Oct 26`.
pub fn human_day(unix_seconds: i64) -> String {
    human(&Instant { ns: i128::from(unix_seconds) * NS, zone: Zone::Fixed(0) }, true)
}

/// A moment (Unix seconds) for people, in local time and the `format.datetime` setting.
pub fn human_moment(unix_seconds: i64) -> String {
    human(&Instant { ns: i128::from(unix_seconds) * NS, zone: Zone::Local }, false)
}

pub fn day_for_people(unix_seconds: f64) -> String {
    format_human(&Instant { ns: unix_seconds as i128 * NS, zone: Zone::Local }, "D MMM YYYY")
}

pub fn format_human(instant: &Instant, format: &str) -> String {
    let c = instant.civil();
    let offset = instant.offset();
    let h12 = if c.hour.is_multiple_of(12) { 12 } else { c.hour % 12 };
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
