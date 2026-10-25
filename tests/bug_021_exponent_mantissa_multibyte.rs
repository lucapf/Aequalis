//! Bug 021: `€6388e-6 unix` crashed aequalis (a panic; in the Android app, the app closed). Found by
//! the fuzzer (`fuzz/`) in an 8 h run, 16 times, from `€{6.388e-6 unix`.
//!
//! A timestamp with an exponent (`18E6 unix`) has its decimal point moved by `time::shifted`, which
//! cut the mantissa by byte position *before* anyone checked that it was digits; a character of two
//! or more bytes (`€`, `°`) was cut in the middle. Fixed: the mantissa is ASCII digits and a point,
//! or the timestamp is not a time.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

fn run(args: &str) -> (String, String, i32) {
    let out = Command::new(BIN).args(args.split_whitespace()).env("LC_ALL", "C").env("TZ", "Asia/Dubai").output().unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

#[test]
fn reported() {
    for request in [
        "€6388e-6 unix",
        "€6.388e-6 unix",
        "€{6.388e-6 unix",
        "€1e2 unix",
        "1€e2 unix",
        "1.€e1 unix",
        "°5e1 unix",
        "🙂5e-1 unix",
        "5e€ unix",
    ] {
        let (_, err, code) = run(request);
        assert_ne!(code, 101, "{request}: {err}");
        assert!(!err.contains("panicked"), "{request}: {err}");
    }
}

#[test]
fn exponents_still_read() {
    let plain = run("18000000 unix");
    assert_eq!(plain.2, 0, "{}", plain.1);
    for request in ["18E6 unix", "1.8e7 unix", "180e5 unix"] {
        assert_eq!(run(request), plain, "{request}");
    }
}
