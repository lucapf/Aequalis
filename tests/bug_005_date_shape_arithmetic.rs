//! Bug 005: `aequalis 2026-12-02/12` failed with "unknown date format `/12`".
//!
//! A leading ISO date shape always made a date request, so arithmetic on it was impossible. Fixed:
//! an ISO shape followed by `/`, `*`, `^`, `x` or `(` is arithmetic (date math only uses `+` and
//! `-`), and the `num` target (`n`, `number`, any case) forces the number reading of any
//! request. Dates without such an operator are unchanged.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

fn run(args: &str) -> (String, String, i32) {
    let out = Command::new(BIN).arg("-v").args(args.split_whitespace()).env("TZ", "Europe/Rome").env("LC_ALL", "C").output().unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

#[test]
fn reported_division_is_a_number() {
    let (out, err, code) = run("2026-12-02/12");
    assert_eq!((out.as_str(), code), ("2013.8333", 0), "{err}");
    assert_eq!(err, "aequalis: note: evaluated as (2026 - 12) - (2 / 12)");
}

#[test]
fn other_operators_and_spacing() {
    assert_eq!(run("2026-12-02 / 12").0, "2013.8333");
    assert_eq!(run("2026-12-02*2").0, "2010");
    assert_eq!(run("2026-12-02^1").0, "2012");
    assert_eq!(run("2026-12-02x2").0, "2010");
    assert_eq!(run("2026-12-02/12 km m").0, "2013833.3333 m", "a number, so units apply");
}

#[test]
fn num_forces_a_number() {
    for word in ["num", "NUM", "n", "N", "number", "Number"] {
        assert_eq!(run(&format!("2026-12-02 {word}")).0, "2012", "{word}");
    }
    assert_eq!(run("12/04/2026 num").0, "0.0015", "a written date as a division");
    // A full datetime is not a number expression: the arithmetic stops at `T15:00Z`, an unknown unit.
    let (_, err, code) = run("2026-10-10T15:00Z num");
    assert_eq!(code, 1);
    assert!(err.contains("unknown unit `T15:00Z`"), "{err}");
}

#[test]
fn dates_unchanged() {
    assert_eq!(run("2026-12-02 iso").0, "2026-12-02T00:00+01:00 iso");
    assert_eq!(run("2026-12-02 unix").0, "2026-12-02T00:00+01:00 iso = 1796166000 unix");
    assert_eq!(run("2026-12-02 + 1d").0, "2026-12-03 iso-date");
    assert_eq!(run("2026-12-02-2026-12-01 d").0, "1 d", "with a unit, minus is a date difference");
    assert_eq!(run("2026-12-02").0, "2012", "alone, a number expression");
    assert_eq!(run("2026-12-02-2026-12-01").0, "-27", "alone, a number expression");
    assert_eq!(run("2026/12/04 unix").0, "2026-12-04T00:00+01:00 iso = 1796338800 unix");
}
