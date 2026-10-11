//! `TALLY_SCALE`: the decimal places of numbers (default 4); `--precision` overrides it.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_tally");

fn run(scale: Option<&str>, args: &str) -> (String, String, i32) {
    let mut cmd = Command::new(BIN);
    cmd.args(args.split_whitespace()).env_remove("TALLY_SCALE").env("LC_ALL", "C");
    if let Some(scale) = scale {
        cmd.env("TALLY_SCALE", scale);
    }
    let out = cmd.output().unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

#[test]
fn default_and_variable() {
    assert_eq!(run(None, "1/3").0, "0.3333", "default 4");
    assert_eq!(run(Some(""), "1/3").0, "0.3333", "empty: the default");
    assert_eq!(run(Some("2"), "1/3").0, "0.33");
    assert_eq!(run(Some("0"), "2/3").0, "1");
    assert_eq!(run(Some("8"), "10 km mi").0, "6.21371192 mi");
    assert_eq!(run(Some(" 6 "), "1/7").0, "0.142857", "spaces aside");
    // Trailing zeros are still trimmed.
    assert_eq!(run(Some("6"), "1/4").0, "0.25");
    // Durations of date differences too.
    assert_eq!(run(Some("1"), "2026-12-25T00:00Z - 2026-10-10T15:00Z d").0, "75.4 d");
}

#[test]
fn precision_overrides_it() {
    assert_eq!(run(Some("8"), "1/3 --precision 2").0, "0.33");
    assert_eq!(run(Some("abc"), "1/3 --precision 2").2, 2, "an invalid variable is still reported");
}

#[test]
fn invalid_values() {
    for bad in ["abc", "-1", "16", "2.5"] {
        let (_, err, code) = run(Some(bad), "1/3");
        assert_eq!(code, 2, "{bad}");
        assert_eq!(err, format!("tally: error: TALLY_SCALE must be an integer from 0 to 15 (it is `{bad}`)"));
    }
}
