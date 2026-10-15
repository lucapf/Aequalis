//! Bug 008: `tally 2026/12/04 10:00 PM` and `tally 2026-12-04 10:00 PM` failed ("unexpected `1`",
//! "unexpected `PM`", "unknown date format `10:00`").
//!
//! A written date followed by a time, with no target, was not a date request (the calculator got
//! `2026/12/04 10:00 PM`), and with `-` the first word was taken for an ISO date (`2026-12-04`),
//! leaving the time behind. Fixed: a written date with a time is read whole, before its date part
//! can be taken for an ISO date, and shown in ISO (year first with `-`: ISO only; otherwise both
//! sides). The time being typed (`2026-12-04 10`) gets its own message.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_tally");

fn run(args: &str) -> (String, i32) {
    let out = Command::new(BIN)
        .arg("-v")
        .args(args.split_whitespace())
        .env("TZ", "Europe/Rome")
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    let code = out.status.code().unwrap_or(-1);
    (if code == 0 { text(out.stdout) } else { text(out.stderr) }, code)
}

#[test]
fn reported_slash_date_with_a_12_hour_time() {
    assert_eq!(run("2026/12/04 10:00 PM"), ("2026-12-04T22:00+01:00 iso = 2026/12/04 10:00:00 PM YYYY/MM/DD-12h".into(), 0));
    assert_eq!(run("2026/12/04 10:00PM").0, "2026-12-04T22:00+01:00 iso = 2026/12/04 10:00:00 PM YYYY/MM/DD-12h");
    assert_eq!(run("2026/12/04 10:00").0, "2026-12-04T10:00+01:00 iso = 2026/12/04 10:00:00 YYYY/MM/DD-24h");
}

#[test]
fn reported_dash_date_with_a_time() {
    // Year first with `-` is ISO-like: shown in ISO only.
    assert_eq!(run("2026-12-04 10:00 PM"), ("2026-12-04T22:00+01:00 iso".into(), 0));
    assert_eq!(run("2026-12-04 22:00").0, "2026-12-04T22:00+01:00 iso");
    assert_eq!(run("2026-12-04T10:00PM").0, "2026-12-04T22:00+01:00 iso");
    assert_eq!(run("2026-12-04").0, "2026-12-04T00:00+01:00 iso = 2026-12-04 iso-date", "the date alone");
}

#[test]
fn with_targets_math_and_zones() {
    assert_eq!(run("2026-12-04 10:00 PM unix").0, "2026-12-04T22:00+01:00 iso = 1796418000 unix");
    assert_eq!(run("2026-12-04 10:00 PM + 1d").0, "2026-12-05T22:00+01:00 iso");
    assert_eq!(run("2026/12/04 10:00 PM + 1d").0, "2026/12/05 10:00:00 PM YYYY/MM/DD-12h");
    assert_eq!(run("2026/12/04 10:00 PM GMT+2").0, "2026-12-04T22:00+02:00 iso = 2026/12/04 10:00:00 PM YYYY/MM/DD-12h");
    assert_eq!(run("September 12, 2026 3:20 PM").0, "2026-09-12T15:20+02:00 iso = 09/12/2026 03:20:00 PM MM/DD/YYYY-12h");
    // An ISO literal keeps its own format.
    assert_eq!(run("2026-12-04T22:00Z").0, "2026-12-04T22:00Z iso-utc");
}

#[test]
fn ambiguous_and_partial() {
    let (out, code) = run("12/04/2026 10:00 PM");
    assert_eq!(code, 2);
    assert!(out.contains("is ambiguous") && out.contains("`12/04/2026 10:00 PM MM/DD/YYYY-12h iso`"), "{out}");
    // While typing the time.
    for partial in ["2026-12-04 10", "2026-12-04 10:"] {
        let (out, code) = run(partial);
        assert_eq!(code, 2);
        assert!(out.contains("write the time as H:MM"), "{partial}: {out}");
    }
    // Not dates: arithmetic and conversions stay as they were.
    assert_eq!(run("10 km mi").0, "6.2137 mi");
    assert_eq!(run("-5 C F").0, "23 F");
}
