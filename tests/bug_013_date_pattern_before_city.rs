//! Bug 013: `12/04/2026 10:00 Paris` asked which reading was meant, and its choices asked again:
//! `12/04/2026 10:00 MM/DD/YYYY-24h Paris` was still ambiguous, and suggested the pattern twice.
//! In the app, a tap on a choice asked the same question forever. Found by the fuzzer (`fuzz/`,
//! its check that a choice never asks the same question).
//!
//! A pattern after a written date was read only with a word after it, the target format; with a
//! city at the end, the city (the target) is taken out first, so the pattern was the last word and
//! was not seen. Fixed: with a city, the pattern may come last.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

fn run(args: &[&str]) -> (String, i32) {
    let out = Command::new(BIN).args(args).env("TZ", "Europe/Rome").env("LC_ALL", "C").output().unwrap();
    (String::from_utf8(out.stdout).unwrap().trim_end().to_string(), out.status.code().unwrap_or(-1))
}

fn words(request: &str) -> Vec<&str> {
    request.split_whitespace().collect()
}

#[test]
fn reported() {
    assert_eq!(run(&words("12/04/2026 10:00 MM/DD/YYYY-24h Paris")).0, "04 Dec 26 10:00 AM GMT+1 Paris");
    assert_eq!(run(&words("12/04/2026 10:00 DD/MM/YYYY-24h Paris")).0, "12 Apr 26 10:00 AM GMT+2 Paris");
    assert_eq!(run(&words("12/04/2026 MM/DD/YYYY Paris")).0, "04 Dec 26 12:00 AM GMT+1 Paris");
}

/// Every choice of the question answers.
#[test]
fn the_choices_answer() {
    let (reply, code) = run(&["--json", "12/04/2026", "10:00", "Paris"]);
    assert_eq!(code, 2);
    let requests: Vec<&str> = reply.split("\"request\":\"").skip(1).map(|r| &r[..r.find('"').unwrap()]).collect();
    assert_eq!(requests.len(), 2, "{reply}");
    for request in requests {
        let (out, code) = run(&words(request));
        assert_eq!(code, 0, "{request}: {out}");
        assert!(out.ends_with("Paris"), "{request}: {out}");
    }
}

#[test]
fn a_pattern_without_a_city_unchanged() {
    assert_eq!(run(&words("12/04/2026 MM/DD/YYYY unix")).0, "1796338800 unix");
    assert_eq!(run(&words("12/04/2026 MM/DD/YYYY")).1, 2, "the pattern is the target: still asked");
}
