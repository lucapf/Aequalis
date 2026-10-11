//! Ambiguous written dates (`12/04/2026`: 12 April or 4 December?) are never guessed: at a
//! terminal the operator is asked; in scripts and pipes the request fails (exit 2) with the
//! readings and the pattern to write. `TZ` and `LC_ALL=C` (month first) are pinned.

use std::io::Write;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_tally");

fn run(args: &str) -> (String, String, i32) {
    let out = Command::new(BIN).arg("-v")
        .args(args.split_whitespace())
        .env("TZ", "Europe/Rome")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

#[test]
fn reported_example_without_a_terminal() {
    let (out, err, code) = run("12/04/2026 unix");
    assert_eq!((out.as_str(), code), ("", 2));
    assert_eq!(
        err,
        "tally: error: `12/04/2026` is ambiguous: MM/DD/YYYY (2026-12-04) or DD/MM/YYYY (2026-04-12); \
         write the pattern: `12/04/2026 MM/DD/YYYY unix`"
    );
    // The suggested request works, and so does the other reading.
    assert_eq!(run("12/04/2026 MM/DD/YYYY unix").0, "2026-12-04T00:00+01:00 iso = 1796338800 unix");
    assert_eq!(run("12/04/2026 DD/MM/YYYY unix").0, "2026-04-12T00:00+02:00 iso = 1775944800 unix");
}

#[test]
fn every_ambiguous_shape() {
    // Two-digit years: up to three readings.
    let (_, err, code) = run("10/04/12 iso");
    assert_eq!(code, 2);
    assert!(err.contains("MM/DD/YY (2012-10-04) or DD/MM/YY (2012-04-10) or YY/MM/DD (2010-04-12)"), "{err}");
    // With a time, the pattern keeps its clock.
    let (_, err, _) = run("12/04/2026 3:20PM unix");
    assert!(err.contains("`12/04/2026 3:20PM MM/DD/YYYY-12h unix`"), "{err}");
    // The `date` word: the pattern takes its place.
    let (_, err, _) = run("12/04/2026 date unix");
    assert!(err.ends_with("`12/04/2026 MM/DD/YYYY unix`"), "{err}");
}

#[test]
fn not_ambiguous() {
    for (args, want) in [
        ("04/04/2026 unix", "2026-04-04T00:00+02:00 iso = 1775253600 unix"),
        ("25/12/2026 unix", "2026-12-25T00:00+01:00 iso = 1798153200 unix"),
        ("2026/12/04 unix", "2026-12-04T00:00+01:00 iso = 1796338800 unix"),
        ("12/04/2026 DD/MM/YYYY unix", "2026-04-12T00:00+02:00 iso = 1775944800 unix"),
    ] {
        let (out, err, code) = run(args);
        assert_eq!((out.as_str(), code), (want, 0), "{args}: {err}");
    }
}

#[test]
fn piped_requests_are_not_asked() {
    let mut child = Command::new(BIN).arg("-v")
        .arg("unix")
        .env("TZ", "Europe/Rome")
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"12/04/2026\n2026/12/04\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "2026-12-04T00:00+01:00 iso = 1796338800 unix\n", "the other line still runs");
    assert!(String::from_utf8(out.stderr).unwrap().contains("is ambiguous"));
}

/// At a terminal (a pty through util-linux `script`): the readings are listed and the answer
/// chooses; Enter takes the first, the locale's order.
#[cfg(target_os = "linux")]
#[test]
fn asked_at_a_terminal() {
    use std::io::Read;
    if Command::new("script").arg("--version").output().is_err() {
        return;
    }
    let ask = |answer: &str| -> String {
        let mut child = Command::new("script")
            .args(["-qec", &format!("'{BIN}' -v 12/04/2026 unix"), "/dev/null"])
            .env("TZ", "Europe/Rome")
            .env("LC_ALL", "C")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let reader = std::thread::spawn(move || {
            let mut out = String::new();
            stdout.read_to_string(&mut out).map(|_| out)
        });
        std::thread::sleep(std::time::Duration::from_millis(400));
        stdin.write_all(format!("{answer}\r").as_bytes()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(400));
        drop(stdin);
        child.wait().unwrap();
        reader.join().unwrap().unwrap().replace('\r', "")
    };
    let out = ask("2");
    assert!(out.contains("`12/04/2026` is ambiguous:\n  1) MM/DD/YYYY   2026-12-04\n  2) DD/MM/YYYY   2026-04-12\n"), "{out}");
    assert!(out.contains("Which one? [1-2, Enter = 1]: "));
    assert!(out.contains("2026-04-12T00:00+02:00 iso = 1775944800 unix"), "{out}");
    let out = ask("");
    assert!(out.contains("2026-12-04T00:00+01:00 iso = 1796338800 unix"), "Enter: the first: {out}");
}
