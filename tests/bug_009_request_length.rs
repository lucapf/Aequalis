//! Bug 009: a request longer than 512 characters was not always refused, nor refused first.
//!
//! The limit was checked in one place only, after the request's words had been split and joined
//! again by one space: a request made long by its spaces passed (`1`, 2,000 spaces, `+1` gave `2`),
//! the app's grouping then parsed a request already refused (its reply carried the grouping of
//! 600 characters), the suggestions had no limit at all, and a 10 MB request took 0.9 s to be
//! refused. Fixed: `check_length`, the first thing every entry point does, counts every character
//! of the request as given, spaces too, and looks at no more than 513 of them; the C interface
//! and the JNI bridge refuse more than 2,048 bytes (512 characters of 4 bytes) unread.

use std::io::Write;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_tally");
const TOO_LONG: &str = "tally: error: the request is too long: at most 512 characters";

fn run(args: &[&str], stdin: Option<&str>) -> (String, String, i32) {
    let mut child = Command::new(BIN)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = stdin {
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

/// `1+1+…+1`, `n` ones: `2n - 1` characters.
fn ones(n: usize) -> String {
    vec!["1"; n].join("+")
}

#[test]
fn reported_spaces_count() {
    // 512 characters: `1+1`, then 509 spaces.
    let fits = format!("1+1{}", " ".repeat(509));
    assert_eq!(run(&[&fits], None), ("2".into(), String::new(), 0));
    // 513 characters: refused, though `1+1` is all it says.
    let long = format!("1+1{}", " ".repeat(510));
    assert_eq!(run(&[&long], None), (String::new(), TOO_LONG.into(), 2));
    let reported = format!("1{}+1", " ".repeat(2000));
    assert_eq!(run(&[&reported], None).2, 2);
}

#[test]
fn up_to_512_characters() {
    assert_eq!(run(&[&ones(256)], None).0, "256", "511 characters");
    // 512: `1+…+1` with 255 ones (509 characters), then `+00`.
    let exact = format!("{}+00", ones(255));
    assert_eq!(exact.chars().count(), 512);
    assert_eq!(run(&[&exact], None).0, "255");
    let long = format!("{exact}0");
    assert_eq!(run(&[&long], None), (String::new(), TOO_LONG.into(), 2), "513 characters");
}

#[test]
fn the_spaces_between_words_count() {
    // The command line's words are joined by one space: `1 + 1 + … + 1`.
    let words = |n: usize| -> Vec<&str> { ["1", "+"].into_iter().cycle().take(n).collect() };
    assert_eq!(words(255).join(" ").chars().count(), 509);
    assert_eq!(run(&words(255), None).0, "128");
    assert_eq!(words(257).join(" ").chars().count(), 513);
    assert_eq!(run(&words(257), None).1, TOO_LONG);
}

#[test]
fn characters_not_bytes() {
    // 512 characters of up to 4 bytes each are a request (here an error of its own kind, not
    // the length); one more is too long.
    let euros = "€".repeat(512);
    let (_, err, _) = run(&[&euros], None);
    assert_ne!(err, TOO_LONG, "512 characters, 1,536 bytes");
    let emoji = "🙂".repeat(512);
    assert_ne!(run(&[&emoji], None).1, TOO_LONG, "512 characters, 2,048 bytes");
    assert_eq!(run(&[&format!("{emoji}🙂")], None).1, TOO_LONG);
}

#[test]
fn each_line_on_stdin() {
    // A long line is refused, the others still answered.
    let input = format!("1+1\n{}\n2+2\n", ones(300));
    let (out, err, code) = run(&[], Some(&input));
    assert_eq!(out, "2\n4");
    assert_eq!(err, TOO_LONG);
    assert_eq!(code, 2);
    // The line's own spaces count, before its words are split.
    let spaced = format!("1+1{}\n", " ".repeat(600));
    assert_eq!(run(&[], Some(&spaced)).1, TOO_LONG);
    // A line and the arguments' units together are one request; the options are not part of it.
    let line = |n: usize| format!("{}\n", ones(n));
    assert_eq!(run(&["km", "mi", "--precision", "2"], Some(&line(253))).0, "157.21 mi", "505 + 6 = 511 characters");
    assert_eq!(run(&["km", "mi", "--precision", "2"], Some(&line(254))).1, TOO_LONG, "507 + 6 = 513 characters");
}

#[test]
fn json_replies() {
    let (out, _, code) = run(&["--json", &ones(300)], None);
    assert!(out.starts_with(r#"{"ok":false,"exit":2,"error":"the request is too long: at most 512 characters""#), "{out}");
    assert_eq!(code, 2);
    // A huge request is refused at once.
    let huge = "Rome ".repeat(200_000);
    let started = std::time::Instant::now();
    let (_, err, code) = run(&[], Some(&format!("{huge}\n")));
    assert_eq!((err.as_str(), code), (TOO_LONG, 2));
    assert!(started.elapsed().as_secs() < 5, "1 MB refused in {:?}", started.elapsed());
}
