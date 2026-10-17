//! Bug 012: `1 m^64*m^64 m^-128` answered `1 m^-128`: metres to the 128th read as metres to the
//! −128th. Found by the fuzzer (`fuzz/`, whose build checks every overflow: there it was a
//! crash), minimized to `1W·square-metre99`.
//!
//! A unit's dimensions are small integers (−128 to 127); a power (`^99`) and the terms of a
//! compound unit multiplied and added them without a check, and in a release build they wrapped
//! around. Fixed: checked, and past the range an error.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_tally");

fn run(args: &str) -> (String, String, i32) {
    let out = Command::new(BIN).args(args.split_whitespace()).env("LC_ALL", "C").output().unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

#[test]
fn reported() {
    let (out, err, code) = run("1 m^64*m^64 m^-128");
    assert_eq!((out.as_str(), code), ("", 1));
    assert_eq!(err, "tally: error: the powers in `m^64*m^64` are too large (at most 127)");
    for request in ["1 W·square-metre^99 W", "1 m^99*m^99 m", "4 kt^99/oz*kWh*t/s^99 m", "1 m^127*m km", "1 m^-128/m km"] {
        let (_, err, code) = run(request);
        assert_eq!(code, 1, "{request}: {err}");
        assert!(err.contains("are too large"), "{request}: {err}");
    }
}

#[test]
fn powers_in_range_unchanged() {
    assert_eq!(run("1 m^99 m^99").0, "1 m^99");
    assert_eq!(run("9.81 m/s^2 ft/s2").0, "32.185 ft/s2");
    assert_eq!(run("1 km^2 m2").0, "1000000 m2");
    assert_eq!(run("1 m^127 m^127").0, "1 m^127");
}
