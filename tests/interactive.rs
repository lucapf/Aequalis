//! Interactive mode, driven in a real pseudo-terminal through util-linux `script`
//! (Linux only; skipped when `script` is missing). Keys are sent with pauses, as typed.

#![cfg(all(target_os = "linux", feature = "tui"))]

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

/// Runs `aequalis` with no arguments in a pty, types `keys` (one write per item) and returns
/// everything it printed, escapes included.
fn session(keys: &[&str]) -> Option<String> {
    Command::new("script").arg("--version").output().ok()?;
    let mut child = Command::new("script")
        .args(["-qec", &format!("'{BIN}' -v"), "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        // Independent of this machine's settings: C locale = metric tabs, plain number format.
        .env("LC_ALL", "C")
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take().unwrap();
    // Read the output while typing, as a terminal does: otherwise the pipe fills up, `script`
    // stops relaying keys, and the session hangs.
    let mut stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        stdout.read_to_end(&mut out).map(|_| out)
    });
    sleep(Duration::from_millis(400));
    for k in keys {
        stdin.write_all(k.as_bytes()).unwrap();
        stdin.flush().unwrap();
        sleep(Duration::from_millis(250));
    }
    sleep(Duration::from_millis(300));
    drop(stdin);
    assert!(child.wait().unwrap().success());
    let out = reader.join().unwrap().unwrap();
    Some(String::from_utf8_lossy(&out).into_owned())
}

#[test]
fn evaluate_recall_edit_and_quit() {
    // ↑ highlights the entry, Enter writes it into the bar, then it is edited and evaluated.
    let Some(out) = session(&["10 km mi\r", "\x1b[A", "\r", "\x7f\x7fft\r", "(10+12-3)/4Km Mi\r", "\x04"]) else {
        return;
    };
    assert!(out.contains("\x1b[?1049h"), "alternate screen entered");
    assert!(out.contains("6.2137 mi"));
    assert!(out.contains("32808.399 ft"), "recalled entry edited and evaluated again");
    // Unquoted brackets work: the shell is not involved.
    assert!(out.contains("2.9515 mi"));
    assert!(out.contains("evaluated as ((10 + 12) - 3) / 4"));
    assert!(out.trim_end().ends_with("\x1b[?1049l\x1b[?25h"), "terminal restored on exit");
}

#[test]
fn tab_completes_units() {
    let Some(out) = session(&["100 sqft m", "\t\t", "\r", "\x03"]) else {
        return;
    };
    assert!(out.contains("100 sqft m2"));
    assert!(out.contains("9.2903 m2"));
}
