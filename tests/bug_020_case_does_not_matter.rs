//! Bug 020: a request is the same in any case (`18E6UNIX`, `10:00 PARIS TO NEW YORK`, `SQRT 16`,
//! `20% OF 150`) except where the case tells two things apart (the data units: `MB` and `Mb`), which
//! asks when it is neither.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

fn run(args: &str) -> (String, String, i32) {
    let out = Command::new(BIN).args(args.split_whitespace()).env("LC_ALL", "C").env("TZ", "Asia/Dubai").output().unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

#[test]
fn format_words_and_connectors_in_any_case() {
    for (loud, quiet) in [
        ("18E6UNIX", "18E6unix"),
        ("18000000UNIX", "18000000unix"),
        ("18E6 Unix", "18E6 unix"),
        ("10:00 PARIS TO NEW YORK", "10:00 paris to new york"),
        ("10:00AM Paris To Tokyo", "10:00AM paris to tokyo"),
        ("5 KM TO MI", "5 km to mi"),
    ] {
        let (stdout, err, code) = run(loud);
        assert_eq!(code, 0, "{loud}: {err}");
        assert_eq!(stdout, run(quiet).0, "{loud}");
    }
}

#[test]
fn functions_and_constants_in_any_case() {
    for (loud, quiet) in [
        ("SQRT 16", "sqrt 16"),
        ("Sqrt(16)", "sqrt(16)"),
        ("LOG(100)", "log(100)"),
        ("ROOT(27, 3)", "root(27, 3)"),
        ("CBRT(27)", "cbrt(27)"),
        ("PI", "pi"),
        ("Pi * 2", "pi * 2"),
        ("20% OF 150", "20% of 150"),
    ] {
        assert_eq!(run(loud).0, run(quiet).0, "{loud}");
    }
    // `E` is the exponent, not Euler's number.
    assert_eq!(run("2E5").0, "200000");
}

#[test]
fn data_units_ask_when_the_case_does_not_say() {
    let (_, err, code) = run("1 mb kB");
    assert_eq!(code, 2);
    assert!(err.contains("`mb` is several units") && err.contains("megabyte") && err.contains("megabit"), "{err}");
    assert!(err.contains("`1 MB kB`"), "{err}");
    let (_, err, _) = run("1 kib kB");
    assert!(err.contains("kibibyte") && err.contains("kibibit"), "{err}");
    // Exactly written, nothing to ask.
    assert_eq!(run("1 Mb kb").0, "1000 kb");
}
