//! Dates and times end to end: the real binary with the time zone and locale pinned (`TZ`,
//! `LC_ALL=C`), so the results do not depend on the machine.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

fn run(tz: &str, args: &str) -> (Result<String, String>, i32) {
    let out = Command::new(BIN).arg("-v").args(args.split_whitespace()).env("TZ", tz).env("LC_ALL", "C").output().unwrap();
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8(out.stdout).unwrap().trim_end().to_string();
    let stderr = String::from_utf8(out.stderr).unwrap().trim_end().to_string();
    (if code == 0 { Ok(stdout) } else { Err(stderr) }, code)
}

fn utc(args: &str) -> String {
    run("UTC", args).0.unwrap_or_else(|e| panic!("{args}: {e}"))
}

fn rome(args: &str) -> String {
    run("Europe/Rome", args).0.unwrap_or_else(|e| panic!("{args}: {e}"))
}

/// The converted side of `<date> iso = <value> <format>` (or the date of `<date> iso`).
fn value(out: String) -> String {
    let out = out.rsplit(" = ").next().unwrap().to_string();
    out.rsplit_once(' ').map_or(out.clone(), |(v, _)| v.to_string())
}

#[test]
fn reported_conversion_shows_both_sides() {
    // The user's example: the date in ISO and the converted value, each named.
    assert_eq!(rome("2026/04/12 date unix"), "2026-04-12T00:00+02:00 iso = 1775944800 unix");
    assert_eq!(rome("1775944800 unix iso-week"), "2026-04-12T00:00+02:00 iso = 2026-W15-7 iso-week");
    assert_eq!(rome("1775944800 unix local"), "2026-04-12T00:00+02:00 iso = Sun Apr 12 00:00:00 2026 local");
    assert_eq!(rome("2026/04/12 date iso-utc"), "2026-04-11T22:00Z iso-utc");
    // The iso target (or none): one side.
    assert_eq!(rome("2026/04/12 date"), "2026-04-12T00:00+02:00 iso = 2026/04/12 YYYY/MM/DD");
    assert_eq!(rome("2026/04/12 date iso"), "2026-04-12T00:00+02:00 iso");
}

/// stdout and stderr of a successful run.
fn run_both(tz: &str, args: &str) -> (String, String) {
    let out = Command::new(BIN).arg("-v").args(args.split_whitespace()).env("TZ", tz).env("LC_ALL", "C").output().unwrap();
    assert!(out.status.success(), "{args}");
    (String::from_utf8(out.stdout).unwrap().trim_end().into(), String::from_utf8(out.stderr).unwrap().trim_end().into())
}

#[test]
fn reported_guess_shows_both_sides_and_how_it_was_read() {
    // One format given (the target): the date is read with a guessed order, and both sides are shown
    // so it can be checked; stderr says how it was read. 13/04/26 has one reading only
    // (`12/04/26` has three: see tests/ambiguous_dates.rs).
    let (out, err) = run_both("Europe/Rome", "04/13/26 3:20PM unix");
    assert_eq!(out, "2026-04-13T15:20+02:00 iso = 1776086400 unix");
    assert_eq!(err, "aequalis: note: read `04/13/26 3:20PM` as MM/DD/YY-12h (the locale's order)");
    // Only one valid order: that one, said so.
    let (out, err) = run_both("Europe/Rome", "25/12/2026 18:00 iso");
    assert_eq!(out, "2026-12-25T18:00+01:00 iso");
    assert!(err.ends_with("as DD/MM/YYYY-24h (the only valid order)"), "{err}");
    // -s: no note.
    let (_, err) = run_both("Europe/Rome", "-s 04/13/26 3:20PM unix");
    assert_eq!(err, "");
}

#[test]
fn explicit_patterns() {
    // Two formats: the first reads the date exactly, the second writes it.
    assert_eq!(rome("12/04/26 3:20PM DD/MM/YY-12h unix"), "2026-04-12T15:20+02:00 iso = 1776000000 unix");
    assert_eq!(rome("12/04/26 15:20 +00:00 DD/MM/YY-24h-tz iso-utc"), "2026-04-12T15:20Z iso-utc");
    assert_eq!(run("Europe/Rome", "12/04/26 3:20PM DD/MM/YY-24h unix").1, 2, "a 12-hour time for a 24h pattern");
    // As outputs.
    assert_eq!(rome("1776000000 unix DD/MM/YYYY-12h-tz"), "2026-04-12T15:20+02:00 iso = 12/04/2026 03:20:00 PM +02:00 DD/MM/YYYY-12h-tz");
    assert_eq!(value(rome("1776000000 unix YY/MM/DD")), "26/04/12");
    assert_eq!(rome("1776000000 unix MM/DD/YYYY-24h"), "2026-04-12T15:20+02:00 iso = 04/12/2026 15:20:00 MM/DD/YYYY-24h");
}

#[test]
fn date_word() {
    assert_eq!(rome("2026.4.12 15:30 date iso-utc"), "2026-04-12T13:30Z iso-utc");
    assert_eq!(rome("2026-4-12T9:05 date"), "2026-04-12T09:05+02:00 iso = 2026/04/12 09:05:00 YYYY/MM/DD-24h");
    assert_eq!(rome("2026/04/12 15:30Z date unix"), "2026-04-12T15:30Z iso = 1776007800 unix");
    // Day and month both up to 12: ambiguous, the operator chooses (tests/ambiguous_dates.rs).
    assert_eq!(run("Europe/Rome", "12/04/2026 date iso-date").1, 2);
    assert_eq!(rome("13/04/2026 date iso-date"), "2026-04-13T00:00+02:00 iso = 2026-04-13 iso-date");
    let (err, code) = run("UTC", "2026/13/40 date unix");
    assert_eq!(code, 2);
    assert!(err.unwrap_err().contains("is not a date (year/month/day, or month/day/year here)"));
}

#[test]
fn durations() {
    assert_eq!(utc("90 min h"), "1.5 h");
    assert_eq!(utc("1 yr ns"), "31557600000000000 ns");
    assert_eq!(utc("1 yr d"), "365.25 d");
    assert_eq!(utc("12 mo yr"), "1 yr");
    assert_eq!(utc("1 wk h"), "168 h");
    assert_eq!(utc("1500 ms s"), "1.5 s");
    assert_eq!(utc("2 hours minutes"), "120 min");
}

#[test]
fn iso_inputs_and_outputs() {
    for input in ["2026-10-10T15:00Z", "20261010T150000Z", "2026-283T15:00Z", "2026-W41-6T15:00Z", "2026-10-10T17:00+02:00"] {
        assert_eq!(value(utc(&format!("{input} unix"))), "1791644400", "{input}");
    }
    assert_eq!(utc("2026-10-10T15:00Z"), "2026-10-10T15:00Z iso-utc");
    assert_eq!(utc("2026-10-10T15:00Z iso-basic"), "2026-10-10T15:00Z iso = 20261010T1500Z iso-basic");
    assert_eq!(value(utc("2026-10-10T15:00Z iso-ordinal")), "2026-283");
    assert_eq!(value(utc("2026-10-10T15:00Z iso-week")), "2026-W41-6");
    assert_eq!(value(utc("2026-10-10T15:00Z iso-date")), "2026-10-10");
    assert_eq!(value(utc("2026-10-10T15:00Z iso-time")), "15:00Z");
    assert_eq!(value(utc("2026-10-10T15:00:00Z iso-time")), "15:00:00Z", "seconds written");
    assert_eq!(value(utc("2026-10-10T15:00:00.123456789Z unix-ns")), "1791644400123456789");
    // Ambiguous basic forms need the `iso` word (else they are numbers).
    assert_eq!(value(utc("20261010 iso iso-week")), "2026-W41-6");
    assert_eq!(utc("20261010 km m"), "20261010000 m");
}

#[test]
fn local_zone_and_dst() {
    // Without a zone, an input is local time; output is local with its offset.
    assert_eq!(rome("2026-10-10T17:00 unix"), "2026-10-10T17:00+02:00 iso = 1791644400 unix");
    assert_eq!(rome("1791644400 unix"), "2026-10-10T17:00+02:00 iso = 1791644400 unix");
    assert_eq!(value(rome("1791644400 unix iso-utc")), "2026-10-10T15:00Z");
    assert_eq!(rome("2026-01-10T12:00 iso"), "2026-01-10T12:00+01:00 iso");
    // A Z input keeps its zone.
    assert_eq!(rome("2026-10-10T15:00Z iso"), "2026-10-10T15:00Z iso");
    // DST starts on 2026-03-29 in Rome: a calendar day keeps the wall time (23 hours), 24h doesn't.
    assert_eq!(rome("2026-03-28T12:00 + 1d"), "2026-03-29T12:00+02:00 iso");
    assert_eq!(rome("2026-03-28T12:00 + 24h"), "2026-03-29T13:00+02:00 iso");
    assert_eq!(rome("2026-03-29T12:00 - 2026-03-28T12:00"), "23h");
}

#[test]
fn unix_inputs() {
    assert_eq!(utc("1791644400 unix"), "2026-10-10T15:00+00:00 iso = 1791644400 unix");
    assert_eq!(value(utc("1791644400500 unix-ms iso-utc")), "2026-10-10T15:00:00.5Z");
    assert_eq!(utc("1791644400.25 unix unix-us"), "2026-10-10T15:00:00.25+00:00 iso = 1791644400250000 unix-us");
    assert_eq!(value(utc("-1 unix iso-utc")), "1969-12-31T23:59:59Z");
    assert_eq!(utc("0 unix local"), "1970-01-01T00:00+00:00 iso = Thu Jan  1 00:00:00 1970 local");
}

#[test]
fn date_math() {
    assert_eq!(value(utc("2026-01-31 + 1mo iso-date")), "2026-02-28");
    assert_eq!(value(utc("2024-02-29 + 1yr iso-date")), "2025-02-28");
    assert_eq!(utc("2026-10-10T15:00Z + 3d - 90min"), "2026-10-13T13:30Z iso-utc");
    assert_eq!(value(utc("2026-10-10T15:00Z+1wk+2h iso-utc")), "2026-10-17T17:00Z");
    assert_eq!(utc("2026-10-10T15:00Z + 1 day"), "2026-10-11T15:00Z iso-utc");
    assert_eq!(utc("2026-10-10T15:00Z + 1500 ms"), "2026-10-10T15:00:01.5Z iso-utc");
    assert_eq!(utc("2026-10-10T15:00Z + 1d unix"), "2026-10-11T15:00Z iso = 1791730800 unix", "date math, then the conversion");
}

#[test]
fn differences() {
    assert_eq!(utc("2026-12-25T00:00Z - 2026-10-10T15:00Z"), "75d 9h");
    assert_eq!(utc("2026-12-25T00:00Z - 2026-10-10T15:00Z d"), "75.375 d");
    assert_eq!(utc("2026-12-25T00:00Z - 2026-10-10T15:00Z h"), "1809 h");
    assert_eq!(utc("2026-10-10T15:00Z - 2026-12-25T00:00Z"), "-75d 9h");
}

#[test]
fn now_and_today() {
    let secs: i128 = value(utc("now unix")).split('.').next().unwrap().parse().unwrap();
    let real = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i128;
    assert!((secs - real).abs() <= 1, "now = {secs}, clock = {real}");
    assert!(utc("today iso").ends_with("T00:00+00:00 iso"));
    assert!(value(utc("now + 1d iso-date")) > value(utc("now iso-date")));
    let gap: f64 = utc("today + 1d - now h").split(' ').next().unwrap().parse().unwrap();
    assert!((0.0..=24.0).contains(&gap));
}

#[test]
fn errors() {
    assert_eq!(run("UTC", "2026-10-10 + 3").1, 2, "a duration needs a unit");
    assert_eq!(run("UTC", "2026-10-10 + 3 km").1, 2);
    assert_eq!(run("UTC", "2026-10-10 bogus").1, 2);
    assert_eq!(run("UTC", "2026-10-10 + 1.5mo").1, 2, "calendar steps need whole numbers");
    assert_eq!(run("UTC", "9999-12-31 + 1yr").1, 1, "out of range");
    let (err, code) = run("UTC", "2026-13-01 iso");
    assert_eq!(code, 2, "{err:?}");
}

#[test]
fn stdin_requests() {
    use std::io::Write;
    let mut child =
        Command::new(BIN).arg("-v").arg("iso-date").env("TZ", "UTC").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"2026-10-10T15:00Z\n1791644400 unix\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "2026-10-10T15:00Z iso = 2026-10-10 iso-date\n2026-10-10T15:00+00:00 iso = 2026-10-10 iso-date\n");
}

#[test]
fn gmt_time_zones() {
    // A GMT±N (or UTC±N) word is the zone the date is written in, and the zone of the result.
    assert_eq!(rome("2026/12/04 15:30 GMT+2 unix"), rome("2026-12-04T15:30+02:00 unix"));
    assert_eq!(rome("2026/12/04 15:30 GMT+2 unix"), "2026-12-04T15:30+02:00 iso = 1796391000 unix");
    assert_eq!(rome("2026/12/04 15:30GMT+2 iso"), "2026-12-04T15:30+02:00 iso", "attached to the time");
    assert_eq!(rome("2026/12/04 3:20PMUTC-5 iso"), "2026-12-04T15:20-05:00 iso");
    assert_eq!(rome("2026/12/04 15:30 GMT+05:30 iso"), "2026-12-04T15:30+05:30 iso");
    assert_eq!(rome("2026/12/04 15:30 gmt-0330 iso"), "2026-12-04T15:30-03:30 iso");
    assert_eq!(rome("2026/12/04 15:30 GMT iso-utc"), "2026-12-04T15:30Z iso-utc");
    assert_eq!(rome("GMT-5 2026/12/04 15:30 unix"), "2026-12-04T15:30-05:00 iso = 1796416200 unix", "anywhere");
    // A zone marks the request as a date: the written date alone, in its own format.
    assert_eq!(rome("2026/12/04 GMT+2"), "2026-12-04T00:00+02:00 iso = 2026/12/04 YYYY/MM/DD");
    // ISO without an offset, Unix timestamps and date math are in the zone too.
    assert_eq!(rome("2026-12-04T15:30 GMT-5"), "2026-12-04T15:30-05:00 iso");
    assert_eq!(rome("1700000000 unix iso GMT+2"), "2023-11-15T00:13:20+02:00 iso");
    assert_eq!(rome("2026/12/04 - 12d GMT+2"), "2026/11/22 YYYY/MM/DD");
    // now and today are shown in the zone; today starts there.
    assert!(rome("now GMT+2").ends_with("+02:00 iso"));
    assert!(rome("GMT+2").ends_with("+02:00 iso"), "the zone alone: now there");
    assert!(rome("today UTC+05:30").ends_with("T00:00+05:30 iso"));
    // An ambiguous date: the suggested requests keep the zone.
    let (out, code) = run("Europe/Rome", "12/04/2026 GMT+2");
    assert_eq!(code, 2);
    assert!(out.unwrap_err().contains("`12/04/2026 MM/DD/YYYY iso GMT+2`"));
    assert_eq!(rome("12/04/2026 MM/DD/YYYY iso GMT+2"), "2026-12-04T00:00+02:00 iso");
}

#[test]
fn gmt_time_zone_errors() {
    for (args, message) in [
        ("2026-12-04T15:30Z GMT+2", "has its own time zone"),
        ("2026/12/04 15:30+01:00 GMT+2", "has its own time zone"),
        ("2026/12/04 GMT+1 UTC+2", "two time zones"),
        ("2026/12/04 GMT+15", "GMT-14 to GMT+14"),
        ("2026/12/04 GMT+5:75", "GMT-14 to GMT+14"),
        ("2026/12/04 GMT+abc", "is not a time zone"),
        ("10/10/2026 local GMT+2", "device's time zone"),
    ] {
        let (out, code) = run("Europe/Rome", args);
        assert_eq!(code, 2, "{args}");
        let err = out.unwrap_err();
        assert!(err.contains(message), "{args}: {err}");
    }
    // Words that only contain UTC are not zones.
    assert_eq!(rome("2026/04/12 date iso-utc"), "2026-04-11T22:00Z iso-utc");
}

#[test]
fn working_days() {
    // The user's example: Monday 5/10/2026 + 5 working days = Monday 12/10/2026.
    assert_eq!(rome("5/10/2026 DD/MM/YYYY +5wd"), "12/10/2026 DD/MM/YYYY");
    assert_eq!(value(rome("2026/10/05 + 5wd")), "2026/10/12");
    assert_eq!(value(rome("2026/10/09 + 1wd")), "2026/10/12", "Friday → Monday");
    assert_eq!(value(rome("2026/10/10 + 1 wd")), "2026/10/12", "Saturday → Monday");
    assert_eq!(value(rome("2026/10/10 - 1wd")), "2026/10/09", "Saturday → Friday");
    assert_eq!(value(rome("2026/10/05 - 5 workdays")), "2026/09/28");
    assert_eq!(value(rome("2026/10/05 + 2 WEEKDAYS")), "2026/10/07");
    assert_eq!(value(rome("2026/10/05 + 23wd")), "2026/11/05", "across the DST change");
    // Mixed with other steps; the time of day is kept.
    assert_eq!(rome("2026-10-09T15:30Z + 1wd + 2h iso"), "2026-10-12T17:30Z iso");
    // Differences: working days between two dates.
    assert_eq!(rome("2026-10-12 - 2026-10-05 wd"), "5 wd");
    assert_eq!(rome("2026-10-05 - 2026-10-12 wd"), "-5 wd");
    assert_eq!(rome("2026-12-25 - 2026-10-10 wd"), "55 wd");
    // Whole numbers only; the ambiguous written date is still asked.
    assert_eq!(run("Europe/Rome", "2026/10/05 + 1.5wd").1, 2);
    let (out, code) = run("Europe/Rome", "5/10/2026 +5wd");
    assert_eq!(code, 2);
    let err = out.unwrap_err();
    assert!(err.contains("DD/MM/YYYY (2026-10-05)") && err.contains("+5wd`"), "{err}");
}

#[test]
fn month_names() {
    // The user's examples, then each language: no ambiguity, a note says how the month was read.
    assert_eq!(rome("12 Sept 2026"), "2026-09-12T00:00+02:00 iso = 12/09/2026 DD/MM/YYYY");
    assert_eq!(rome("12 September 2026 unix"), "2026-09-12T00:00+02:00 iso = 1789164000 unix");
    for (text, iso) in [
        ("12 Sptember 2026", "2026-09-12"),
        ("September 12, 2026", "2026-09-12"),
        ("12 de septiembre de 2026", "2026-09-12"),
        ("1er janvier 2026", "2026-01-01"),
        ("12 févr. 2026", "2026-02-12"),
        ("12 dicembre 2026", "2026-12-12"),
        ("12 gen 2026", "2026-01-12"),
        ("12. März 2026", "2026-03-12"),
        ("12 Okt 2026", "2026-10-12"),
        ("12 de março de 2026", "2026-03-12"),
        ("12 out 2026", "2026-10-12"),
        ("2026 Sep 12", "2026-09-12"),
    ] {
        assert_eq!(
            rome(&format!("{text} iso-date")),
            format!("{iso}T00:00{} iso = {iso} iso-date", if !("2026-03-29"..="2026-10-25").contains(&iso) { "+01:00" } else { "+02:00" }),
            "{text}"
        );
    }
    // Times, zones, date math, differences.
    assert_eq!(rome("12 Sept 2026 15:30 unix"), "2026-09-12T15:30+02:00 iso = 1789219800 unix");
    assert_eq!(rome("September 12, 2026 3:20 PM iso"), "2026-09-12T15:20+02:00 iso");
    assert_eq!(rome("12 Sept 2026 15:30 GMT+5 iso"), "2026-09-12T15:30+05:00 iso");
    assert_eq!(rome("12 Sept 2026 + 5wd"), "18/09/2026 DD/MM/YYYY");
    assert_eq!(rome("12 Dec 2026 - 12 Sept 2026 wd"), "65 wd");
    assert_eq!(rome("2026-12-25 - 12 Sept 2026 d"), "104.0417 d");
    assert_eq!(rome("12 Sept 2026 DD/MM/YYYY iso"), "2026-09-12T00:00+02:00 iso");
}

#[test]
fn month_name_notes_and_errors() {
    let out = Command::new(BIN).arg("-v").args(["12 Sptember 2026", "iso-date"]).env("TZ", "Europe/Rome").output().unwrap();
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("read `12 Sptember 2026` as 2026-09-12 (`Sptember`: September, read as a misspelling)"), "{err}");
    for (args, message) in [
        ("31 Feb 2026", "`31 Feb 2026` is not a date (no such day in that month)"),
        ("31 Feb 2026 iso", "`31 Feb 2026` is not a date"),
        ("12 Sept 2026 MM/DD/YYYY iso", "not a date in the MM/DD/YYYY format"),
    ] {
        let (out, code) = run("Europe/Rome", args);
        assert_eq!(code, 2, "{args}");
        assert!(out.unwrap_err().contains(message), "{args}");
    }
    // Quoted as one argument, with the month first: still a request, not a wait on stdin.
    let out =
        Command::new(BIN).arg("-v").args(["September 12, 2026", "iso-date"]).env("TZ", "Europe/Rome").stdin(std::process::Stdio::null()).output().unwrap();
    assert!(String::from_utf8(out.stdout).unwrap().ends_with("2026-09-12 iso-date\n"));
}

#[test]
fn date_math_shows_only_the_result() {
    // Date math without a target: only the result, in the format the date was written in.
    assert_eq!(rome("2026/12/04 + 5wd"), "2026/12/11 YYYY/MM/DD");
    assert_eq!(rome("12/10/2026 DD/MM/YYYY + 1d"), "13/10/2026 DD/MM/YYYY");
    assert_eq!(rome("12 Sept 2026 + 5wd"), "18/09/2026 DD/MM/YYYY");
    assert_eq!(rome("2026-10-05 + 5wd"), "2026-10-12 iso-date");
    assert_eq!(rome("1791644400 unix + 1h"), "1791648000 unix");
    assert_eq!(rome("now + 3d").matches(" iso").count(), 1, "now: ISO only");
    // A date alone is still read into ISO (its own format would repeat it), and an explicit target
    // shows both sides.
    assert_eq!(rome("1791644400 unix"), "2026-10-10T17:00+02:00 iso = 1791644400 unix");
    assert_eq!(rome("12 Sept 2026"), "2026-09-12T00:00+02:00 iso = 12/09/2026 DD/MM/YYYY");
    assert_eq!(rome("2026/12/04 + 5wd unix"), "2026-12-11T00:00+01:00 iso = 1796943600 unix");
}

#[test]
fn iso_shows_seconds_only_when_needed() {
    // ISO results show HH:MM and the zone, unless the input wrote seconds or they are not zero.
    assert_eq!(rome("2026/12/04 10:00 PM iso"), "2026-12-04T22:00+01:00 iso");
    assert_eq!(rome("2026/12/04 10:00:00 PM iso"), "2026-12-04T22:00:00+01:00 iso", "written");
    assert_eq!(rome("2026/12/04 10:00:30 PM iso"), "2026-12-04T22:00:30+01:00 iso");
    assert_eq!(rome("2026-12-04T22:00 iso-utc"), "2026-12-04T21:00Z iso-utc");
    assert_eq!(rome("2026-12-04T22:00 + 90s iso"), "2026-12-04T22:01:30+01:00 iso", "not zero");
    assert_eq!(rome("1796338800 unix"), "2026-12-04T00:00+01:00 iso = 1796338800 unix");
    assert_eq!(rome("today"), rome("today").replace(":00:00", ":00"), "today: HH:MM");
    assert!(rome("now").matches(':').count() >= 3, "now: its seconds");
    assert_eq!(rome("2026-12-04T22:00Z iso-basic"), "2026-12-04T22:00Z iso = 20261204T2200Z iso-basic");
    assert_eq!(rome("20261204T220000Z iso-basic"), "2026-12-04T22:00:00Z iso = 20261204T220000Z iso-basic", "seconds written");
}

/// `--now` (not in the help) freezes "now" for the whole run, for tests whose data has a date:
/// `now`, `today` and date math from them use it; a value that is not Unix seconds is refused.
#[test]
fn now_can_be_frozen() {
    assert_eq!(value(utc("--now 1791800000 now unix")), "1791800000");
    assert_eq!(value(utc("--now=1791800000 now unix")), "1791800000");
    assert_eq!(value(utc("--now 1791800000 today iso-date")), "2026-10-12");
    assert_eq!(value(utc("--now 1791800000 now + 1d unix")), "1791886400");
    let (out, code) = run("UTC", "--now soon now unix");
    assert_eq!((out.unwrap_err(), code), ("aequalis: error: --now requires Unix seconds (--now 1791800000)".to_string(), 2));
}
