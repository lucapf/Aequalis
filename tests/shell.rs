//! `aequalis --shell`: a prompt where each line is a request, answered as the whole expression. Piped, it
//! prints the session as a transcript, which is how the examples of the website's typed terminal are made.

mod common;

use std::io::Write;
use std::process::{Command, Stdio};

/// Rates per US dollar, as in `currencies.rs`, and the day of the test.
const RATES: &str = r#"{"format":1,"synced_at":1791800000,"attempted_at":1791800000,"ok":true,"values":{
    "rates date":{"value":1791763351,"at":1791800000},"USD/EUR":{"value":0.8945,"at":1791800000},
    "USD/AED":{"value":3.6725,"at":1791800000},"USD/THB":{"value":33.7,"at":1791800000}}}"#;
const NOW: &str = "--now=1791800000";

/// `aequalis --shell <args>` with `input` piped: (stdout, stderr, exit code).
fn shell(dir: &std::path::PathBuf, args: &[&str], input: &str) -> (String, String, i32) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aequalis"))
        .arg(NOW)
        .arg("--shell")
        .args(args)
        .env("XDG_CONFIG_HOME", dir)
        .env("LC_ALL", "C")
        .env("TZ", "Europe/Rome")
        .env("AEQUALIS_MACHINE", "plain")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run aequalis");
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    (String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string(), out.status.code().unwrap_or(-1))
}

#[test]
fn a_session_is_printed_as_a_transcript() {
    let dir = common::config("shell-transcript", &[]);
    let (stdout, _, code) = shell(&dir, &[], "12+4/2\n10 km mi\n\n20% of 150\n");
    assert_eq!(code, 0);
    assert_eq!(stdout, "aequalis> 12+4/2\n12 + (4 / 2) = 14\naequalis> 10 km mi\n10 km = 6.2137 mi\naequalis> 20% of 150\n20% of 150 = 30\n");
}

#[test]
fn exit_and_quit_leave_and_the_last_line_needs_no_newline() {
    let dir = common::config("shell-exit", &[]);
    assert_eq!(shell(&dir, &[], "1+1\nexit\n2+2\n").0, "aequalis> 1+1\n1+1 = 2\n");
    assert_eq!(shell(&dir, &[], "1+1\nquit\n2+2\n").0, "aequalis> 1+1\n1+1 = 2\n");
    assert_eq!(shell(&dir, &[], "1+1").0, "aequalis> 1+1\n1+1 = 2\n");
}

#[test]
fn a_mistake_is_reported_on_stderr_and_the_session_goes_on() {
    let dir = common::config("shell-error", &[]);
    let (stdout, stderr, code) = shell(&dir, &[], "1+1\nfoo bar\n2+2\n");
    assert_eq!(stdout, "aequalis> 1+1\n1+1 = 2\naequalis> foo bar\naequalis> 2+2\n2+2 = 4\n");
    assert!(stderr.contains("aequalis: error:"), "{stderr}");
    assert_eq!(code, 2, "piped, the most severe outcome is the exit code");
}

#[test]
fn the_options_hold_for_every_request() {
    let dir = common::config("shell-options", &[]);
    assert_eq!(shell(&dir, &["--precision", "2"], "10 km mi\n").0, "aequalis> 10 km mi\n10 km = 6.21 mi\n");
}

#[test]
fn a_currency_is_the_whole_expression_and_silent_drops_the_notes() {
    let dir = common::config("shell-money", &[]);
    std::fs::write(dir.join("aequalis/currencies.json"), RATES).unwrap();
    let (stdout, stderr, _) = shell(&dir, &[], "12 usd eur\n2400 Thailand usd\n");
    assert_eq!(stdout, "aequalis> 12 usd eur\n12 USD = 10.73 EUR\naequalis> 2400 Thailand usd\n2400 THB = 71.22 USD\n");
    assert!(stderr.contains("note:"), "the notes are on stderr: {stderr}");
    let (_, quiet, _) = shell(&dir, &["-s"], "12 usd eur\n2400 Thailand usd\n");
    assert_eq!(quiet, "", "-s: no notes");
}

#[test]
fn a_table_stays_a_table() {
    let dir = common::config("shell-table", &[]);
    let (stdout, _, _) = shell(&dir, &["-s"], "shoes 42\n");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "aequalis> shoes 42");
    assert!(lines.len() > 2 && !lines[1].contains(" = "), "{stdout}");
}

#[test]
fn piped_an_ambiguous_request_is_an_error_not_a_question() {
    // At a terminal it asks; piped, the next line is a request, not an answer.
    let dir = common::config("shell-ambiguous", &[]);
    let (stdout, stderr, _) = shell(&dir, &[], "12/04/2026 unix\n1+1\n");
    assert!(stderr.contains("ambiguous"), "{stderr}");
    assert!(stdout.ends_with("aequalis> 1+1\n1+1 = 2\n"), "{stdout}");
}

#[test]
fn it_takes_no_requests_as_arguments_and_no_other_command() {
    let dir = common::config("shell-arguments", &[]);
    let (_, stderr, code) = shell(&dir, &["12", "km", "mi"], "");
    assert_eq!(code, 2);
    assert!(stderr.contains("not as arguments"), "{stderr}");
    let (_, stderr, code) = shell(&dir, &["--list"], "");
    assert_eq!(code, 2);
    assert!(stderr.contains("not with another command"), "{stderr}");
}

#[test]
fn json_gives_each_answers_reply() {
    let dir = common::config("shell-json", &[]);
    let (stdout, _, _) = shell(&dir, &["--json"], "1+1\n2+2\n");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "no prompt, no echo: {stdout}");
    assert!(lines[0].starts_with(r#"{"ok":true,"result":"2""#) && lines[1].starts_with(r#"{"ok":true,"result":"4""#), "{stdout}");
}

/// At a terminal (a pty through util-linux `script`, Linux only; skipped when `script` is missing).
#[cfg(target_os = "linux")]
mod at_a_terminal {
    use super::common;
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    /// Runs `aequalis --shell` in a pty, types each of `keys` (one write each, with pauses); gives what it
    /// printed and whether it left by itself within 10 seconds of the last key.
    fn session(keys: &[&str]) -> Option<(String, bool)> {
        Command::new("script").arg("--version").output().ok()?;
        let bin = env!("CARGO_BIN_EXE_aequalis");
        let mut child = Command::new("script")
            .args(["-qec", &format!("'{bin}' --shell"), "/dev/null"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .env("LC_ALL", "C")
            .env("TZ", "Europe/Rome")
            .env("AEQUALIS_MACHINE", "plain")
            .env("XDG_CONFIG_HOME", common::config("shell-pty", &[]))
            .spawn()
            .ok()?;
        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let reader = std::thread::spawn(move || {
            let mut out = Vec::new();
            stdout.read_to_end(&mut out).map(|_| out)
        });
        sleep(Duration::from_millis(500));
        for k in keys {
            stdin.write_all(k.as_bytes()).unwrap();
            stdin.flush().unwrap();
            sleep(Duration::from_millis(300));
        }
        // The program has to leave by itself (Ctrl-D, `exit`): a hang is a failure, not a stuck test.
        let started = Instant::now();
        let left = loop {
            if child.try_wait().unwrap().is_some() {
                break true;
            }
            if started.elapsed() > Duration::from_secs(10) {
                let _ = child.kill();
                break false;
            }
            sleep(Duration::from_millis(100));
        };
        drop(stdin);
        let out = reader.join().unwrap().unwrap();
        Some((String::from_utf8_lossy(&out).into_owned(), left))
    }

    #[test]
    fn it_prompts_asks_about_an_ambiguous_request_and_leaves_on_ctrl_d() {
        // The answer is read from the same input as the lines: a lock held across the loop would hang here.
        let Some((out, left)) = session(&["12+4/2\r", "12/04/2026 unix\r", "2\r", "10 km mi\r", "\x04"]) else {
            return;
        };
        assert!(left, "it left on Ctrl-D: {out}");
        assert!(out.starts_with("aequalis> "), "{out}");
        assert!(out.contains("12 + (4 / 2) = 14"), "{out}");
        assert!(out.contains("Which one? [1-2, Enter = 1]:"), "{out}");
        // The reading chosen (2: day first) is in the expression.
        assert!(out.contains("12/04/2026 DD/MM/YYYY unix = "), "{out}");
        assert!(out.contains("10 km = 6.2137 mi"), "the session went on after the answer: {out}");
    }

    #[test]
    fn exit_leaves() {
        let Some((out, left)) = session(&["1+1\r", "exit\r"]) else {
            return;
        };
        assert!(left && out.contains("1+1 = 2"), "{out}");
    }
}
