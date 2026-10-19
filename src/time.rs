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
//!
//! The parts, under `time/`: `calendar` (days and instants), `iso` (ISO 8601), `written` (patterns,
//! month names, zone words), `format`, `math` (date math), `request` (what a date request is and its
//! answer), `tz` (zones, cities, countries), `sys` (the C library). This file holds the types they
//! share and the tests.

mod sys;
pub(crate) mod tz;

pub use tz::{set_source as set_zone_source, Rules as ZoneRules, Source as ZoneSource};

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
    /// Not finished yet: what comes next (`10:00 Paris to`: the city to convert to).
    Incomplete(String),
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
        // Every question has choices; the message must not take the process down if one hasn't.
        let Some(first) = self.choices.first() else { return format!("`{}` is ambiguous", self.text) };
        match self.kind {
            AmbiguityKind::City | AmbiguityKind::Currency | AmbiguityKind::Place => {
                return format!("{}: did you mean {}? (`{}`)", self.note, names.join(" or "), first.request);
            }
            AmbiguityKind::Country => {
                let zones: Vec<String> = self.choices.iter().map(|c| format!("{} ({})", c.pattern, c.date.split(" · ").next().unwrap_or(""))).collect();
                return format!("`{}` has several time zones: {}; write a city: `{}`", self.text, zones.join(", "), first.request);
            }
            AmbiguityKind::Date => {}
        }
        let readings: Vec<String> = self.choices.iter().map(|c| format!("{} ({})", c.pattern, c.date)).collect();
        format!("`{}` is ambiguous: {}; write the pattern: `{}`", self.text, readings.join(" or "), first.request)
    }
}

fn usage<T>(msg: impl Into<String>) -> Result<T, TimeError> {
    Err(TimeError::Usage(msg.into()))
}

mod calendar;
mod format;
mod iso;
mod math;
mod request;
mod written;

pub use calendar::*;
pub use format::*;
pub use iso::*;
use math::*;
pub use request::*;
pub use written::*;

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
        for s in
            ["2026-13-01", "2026-02-29", "2026-W54-1", "2026-W41-8", "2026-367", "2026-10-10T25:00", "2026-10-10T12:60", "2026-10-10x", "2026-10", "202610"]
        {
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
        assert!(req("20261010T15Z iso iso-ordinal").is_err(), "iso input word takes a single literal");
    }

    #[test]
    fn loose_dates() {
        let d = |s: &str, o| {
            read_date(s, None, o).map(|r| (r.civil.year, r.civil.month, r.civil.day, r.civil.hour, r.civil.minute, r.civil.second, r.civil.nano, r.offset))
        };
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
        assert_eq!(
            a.choices.iter().map(|c| c.pattern.as_str()).collect::<Vec<_>>(),
            ["New York", "Chicago", "Denver", "Phoenix", "Los Angeles", "Anchorage", "Honolulu"]
        );
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
        // Calculations and connectors are no misspelt places (`pi - pi`: not Dili; `mpg in`: not Spain).
        for input in ["pi - pi", "pi-pi", "42 mpg in km/l"] {
            assert!(!matches!(request(&config, input, 4, false), Some(Err(TimeError::Ambiguous(_)))), "{input}");
        }
        // A misspelt city with 24:00 (bug: no correction offered, 24:00 was not a time).
        assert_eq!(ask("24:00 Mecico City to Croatia").choices[0].request, "24:00 Mexico City to Zagreb");
    }

    #[test]
    fn end_of_day_with_a_city() {
        let config = Config::default();
        let at = |s: &str| request(&config, s, 4, false).unwrap();
        // 24:00 is the next day's 00:00, as in ISO; not 24:30, not 24:00 PM.
        let (midnight, end) = (at("00:00 Paris").unwrap().0, at("24:00 Paris").unwrap().0);
        assert!(end.contains("12:00 AM") && end != midnight, "{end}, {midnight}");
        assert!(at("24:00 Mexico City to Croatia").is_ok());
        assert!(at("24:30 Paris").is_err());
        assert!(at("24:00 PM Paris").is_err());
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
        let r =
            |s: &str| read_date(s, None, Order::MonthFirst).map(|r| (r.civil.year, r.civil.month, r.civil.day, r.civil.hour, r.civil.minute, r.pattern.name()));
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
            ("2026-10-09", 1, "2026-10-12"),  // Friday → Monday
            ("2026-10-10", 1, "2026-10-12"),  // Saturday → Monday
            ("2026-10-11", 5, "2026-10-16"),  // Sunday → Friday
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
        for yes in [
            "now",
            "now+3d",
            "today iso",
            "2026-10-10",
            "2026-W41-6 unix",
            "1791644400 unix",
            "20261010 iso",
            "10/10/2026 local",
            "2026/04/12 date unix",
            "12/04/2026 15:00 date",
            "12/04/26 3:20PM unix",
            "12/04/26 DD/MM/YY iso",
            "2026/04/12 iso-week",
            "2026/12/04 GMT+2",
            "GMT+2",
            "2026/12/04 15:30UTC-5 unix",
            "2026/12/04 GMT+99",
        ] {
            assert!(is_date_request(yes), "{yes}");
        }
        // Numbers, subtractions and conversions stay numeric.
        for no in [
            "10 km mi",
            "20261010",
            "2026-283",
            "2026-10",
            "100 + 20",
            "nowhere",
            "-5 local",
            "",
            "12/04/26",
            "2026-12-02/12",
            "2026-12-02 / 12",
            "2026-12-02*2",
            "2026-12-02 num",
            "now N",
            "2026/12/04 NUMBER",
        ] {
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
