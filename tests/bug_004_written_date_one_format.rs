//! Bug 004: `tally 2026/12/04 unix` failed with "`2026/12/04` is not a unix timestamp".
//!
//! A value followed by `unix` (or `iso`) was always taken as a timestamp (or an ISO date) in that
//! input format, so a written date followed by its *target* format never reached the guessing.
//! Fixed: the word is the input format only when the value is a number (an ISO literal for
//! `iso`); otherwise the value is a written date, read with a guessed order, and the word is
//! the target. Without a zone the date is in the device's time zone, with that date's own offset
//! (DST: CET +01:00 in December, CEST +02:00 in April for Europe/Rome).

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_tally");

fn run(tz: &str, args: &str) -> (String, String, i32) {
    let out = Command::new(BIN).arg("-v").args(args.split_whitespace()).env("TZ", tz).env("LC_ALL", "C").output().unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

#[test]
fn reported_written_date_to_unix() {
    let (out, err, code) = run("Europe/Rome", "2026/12/04 unix");
    assert_eq!(code, 0, "{err}");
    // 4 December: winter time in Rome, +01:00; midnight there is 2026-12-03T23:00Z.
    assert_eq!(out, "2026-12-04T00:00+01:00 iso = 1796338800 unix");
    assert_eq!(err, "tally: note: read `2026/12/04` as YYYY/MM/DD (year first)");
}

#[test]
fn the_dates_own_offset() {
    assert_eq!(run("Europe/Rome", "2026/04/12 unix").0, "2026-04-12T00:00+02:00 iso = 1775944800 unix", "summer: +02:00");
    assert_eq!(run("UTC", "2026/12/04 unix").0, "2026-12-04T00:00+00:00 iso = 1796342400 unix");
    assert_eq!(run("America/New_York", "2026/12/04 unix").0, "2026-12-04T00:00-05:00 iso = 1796360400 unix");
}

#[test]
fn other_targets_too() {
    assert_eq!(run("Europe/Rome", "2026/12/04 iso").0, "2026-12-04T00:00+01:00 iso");
    assert_eq!(run("Europe/Rome", "2026/12/04 iso-week").0, "2026-12-04T00:00+01:00 iso = 2026-W49-5 iso-week");
    assert_eq!(run("Europe/Rome", "2026/12/04 unix-ms").0, "2026-12-04T00:00+01:00 iso = 1796338800000 unix-ms");
    assert_eq!(run("Europe/Rome", "2026.12.4 unix").0, "2026-12-04T00:00+01:00 iso = 1796338800 unix");
}

#[test]
fn timestamps_and_iso_inputs_unchanged() {
    // A number before `unix` is still a timestamp, an ISO literal before `iso` an ISO date.
    assert_eq!(run("Europe/Rome", "1796338800 unix").0, "2026-12-04T00:00+01:00 iso = 1796338800 unix");
    assert_eq!(run("Europe/Rome", "1796338800500 unix-ms iso-utc").0, "2026-12-03T23:00:00.5Z iso-utc");
    assert_eq!(run("Europe/Rome", "20261204 iso unix").0, "2026-12-04T00:00+01:00 iso = 1796338800 unix");
    assert_eq!(run("Europe/Rome", "2026-12-04 unix").0, "2026-12-04T00:00+01:00 iso = 1796338800 unix");
    // Neither a number nor a date: still an error.
    assert_eq!(run("Europe/Rome", "abc unix").2, 2);
}
