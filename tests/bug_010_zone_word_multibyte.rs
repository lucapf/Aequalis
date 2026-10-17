//! Bug 010: `gmt+1 sin(30°)`, `utc+5 °C`, `GMT+2 €` crashed tally (a panic; in the Android app,
//! the app closed). Found by the fuzzer (`fuzz/`), minimized to `GMT+؋*`.
//!
//! When no stage reads a request, tally checks whether a place is being typed, and asks whether
//! all its words are a reserved word; `gmt_zone`, written for one word (`GMT+0530`), took the last
//! two *bytes* after the sign as the minutes, and cut inside a character of two or more bytes
//! (`°`, `€`). Fixed: only ASCII text is cut there; anything else is no zone.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_tally");

fn run(args: &str) -> (String, String, i32) {
    let out = Command::new(BIN).args(args.split_whitespace()).env("TZ", "Europe/Rome").env("LC_ALL", "C").output().unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

#[test]
fn reported() {
    for request in ["gmt+1 sin(30°)", "utc+5 °C", "GMT+2 €", "GMT+؋*", "UTC-1 🙂🙂", "GMT+12€"] {
        let (_, err, code) = run(request);
        assert_eq!(code, 2, "{request}: {err}");
        assert!(err.starts_with("tally: error: "), "{request}: {err}");
    }
    // Crashed too, and is a request: the time in São Paulo.
    let (out, err, code) = run("GMT+2 São Paulo");
    assert_eq!(code, 0, "{err}");
    assert!(out.ends_with("São Paulo"), "{out}");
}

#[test]
fn zone_words_unchanged() {
    assert_eq!(run("2026/12/04 15:30 GMT+2 unix").0, "1796391000 unix");
    assert_eq!(run("2026/12/04 15:30 GMT+0530 unix").0, "1796378400 unix");
    assert_eq!(run("2026/12/04 15:30 UTC-05:30 unix").0, "1796418000 unix");
    assert_eq!(run("2026/12/04 15:30 GMT+abc unix").2, 2, "a malformed zone is still an error");
}
