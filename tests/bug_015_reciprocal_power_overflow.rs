//! Bug 015: `1 m^-121 m^-128` crashed the fuzz build (`attempt to negate with overflow`, in
//! `convert`); a release build wrapped around silently. Found by the fuzzer (`fuzz/`).
//!
//! To tell a fuel economy unit from its reciprocal (`mpg` ↔ `l/100km`), `convert` negated the
//! target's dimensions; a power of −128 (allowed: the dimensions hold −128 to 127) has no
//! opposite in an i8. Fixed: compared in i16.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_tally");

fn run(args: &str) -> (String, String, i32) {
    let out = Command::new(BIN).args(args.split_whitespace()).env("LC_ALL", "C").output().unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

#[test]
fn reported() {
    let (out, err, code) = run("1 m^-121 m^-128");
    assert_eq!((out.as_str(), code), ("", 1));
    assert_eq!(err, "tally: error: cannot convert `m^-121` to `m^-128`: different kinds of unit");
    for request in ["1 m^-128 m^-128", "1 m^-128 l/100km", "1 mpg m^-128", "1 m^127 m^-128"] {
        let (_, err, code) = run(request);
        assert_ne!(code, 101, "{request}: {err}");
        assert!(!err.contains("panicked"), "{request}: {err}");
    }
}

#[test]
fn reciprocal_units_unchanged() {
    assert_eq!(run("1 m^-128 m^-128").0, "1 m^-128");
    assert_eq!(run("30 mpg l/100km").0, "7.8405 l/100km");
}
