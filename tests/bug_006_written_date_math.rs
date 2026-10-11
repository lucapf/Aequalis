//! Bug 006: `tally 2026/12/04 - 12d` failed with "expected <value> <from> <to>…".
//!
//! Only ISO dates, `now` and written dates followed by a target format were dates, so a written
//! date followed by date math went to the calculator (2026 / 12 / 4 - 12, then the unit `d`).
//! Fixed: as soon as `+` / `-` is followed by a duration (`12d`, `3 hours`), the value before it
//! is a date, read like any written date (year first, the locale's order, ambiguity asked).

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_tally");

fn run(args: &str) -> (String, String, i32) {
    let out = Command::new(BIN).arg("-v")
        .args(args.split_whitespace())
        .env("TZ", "Europe/Rome")
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

#[test]
fn reported_subtracting_days() {
    let (out, err, code) = run("2026/12/04 - 12d");
    assert_eq!((out.as_str(), code), ("2026/11/22 YYYY/MM/DD", 0), "{err}");
    assert_eq!(err, "tally: note: read `2026/12/04` as YYYY/MM/DD (year first)");
}

#[test]
fn spacing_targets_and_several_steps() {
    assert_eq!(run("2026/12/04-12d").0, "2026/11/22 YYYY/MM/DD");
    assert_eq!(run("2026/12/04 - 12d unix").0, "2026-11-22T00:00+01:00 iso = 1795302000 unix");
    assert_eq!(run("2026/12/04 - 12 days + 3h iso-utc").0, "2026-11-22T02:00Z iso-utc");
    assert_eq!(run("2026-4-12 + 1mo").0, "2026/05/12 YYYY/MM/DD");
    assert_eq!(run("2026.12.4 15:30 + 90min").0, "2026/12/04 17:00:00 YYYY/MM/DD-24h");
    assert_eq!(run("25/12/2026 - 1wk iso-date").0, "2026-12-18T00:00+01:00 iso = 2026-12-18 iso-date");
}

#[test]
fn ambiguous_dates_are_still_asked() {
    let (_, err, code) = run("12/04/2026 - 12d");
    assert_eq!(code, 2);
    assert!(err.ends_with("write the pattern: `12/04/2026 MM/DD/YYYY - 12d`"), "{err}");
    assert_eq!(run("12/04/2026 DD/MM/YYYY - 12d").0, "31/03/2026 DD/MM/YYYY");
}

#[test]
fn numbers_stay_numbers() {
    // No duration after the operator: arithmetic, as before.
    assert_eq!(run("12/4 - 3").0, "0");
    assert_eq!(run("10 - 5 min s").0, "300 s");
    assert_eq!(run("2026/12/04 - 12 num").0, "30.2083");
}
