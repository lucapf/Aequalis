//! Bug 019: `18000000unix` (the number against its format) and `18E6 unix` (a number in scientific
//! notation) were no dates; `12USD->EUR` (an arrow against its words) no request. A number is
//! accepted written as `xEy` (x times 10^y) where a plain one is, and joined to the word after it.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

fn run(args: &str) -> (String, String, i32) {
    let out = Command::new(BIN).args(args.split_whitespace()).env("LC_ALL", "C").env("TZ", "Asia/Dubai").output().unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

#[test]
fn unix_stamps_in_any_notation() {
    let (plain, err, code) = run("18000000 unix");
    assert_eq!(code, 0, "{err}");
    assert_eq!(plain, "28 Jul 70 12:00 PM GMT+4");
    for form in ["18000000unix", "18E6 unix", "18E6unix", "18e6unix", "1.8e7 unix", "1.8E7unix", "180000e2 unix"] {
        assert_eq!(run(form).0, plain, "{form}");
    }
}

#[test]
fn negative_exponents_and_other_formats() {
    assert_eq!(run("18E6 unix iso").0, run("18000000 unix iso").0);
    assert_eq!(run("18000000unix iso").0, run("18000000 unix iso").0);
    assert_eq!(run("1e-3 unix").0, run("0.001 unix").0);
}

#[test]
fn an_arrow_against_its_words() {
    assert_eq!(run("5km->mi").0, "3.1069 mi");
    assert_eq!(run("5 km->mi").0, "3.1069 mi");
    assert_eq!(run("5km ->mi").0, "3.1069 mi");
    assert_eq!(run("5km->mi").0, run("5 km -> mi").0);
}

#[test]
fn labels_keep_their_number() {
    // Not a date format: the number belongs to the word (`2XL` is a size).
    assert!(run("2XL").0.contains("XXL"));
}

#[test]
fn scientific_notation_is_a_product() {
    assert_eq!(run("18E6").0, "18000000");
    assert_eq!(run("2.5E-3 * 4").0, "0.01");
    assert_eq!(run("1.5E3 + 1").0, "1501");
}
