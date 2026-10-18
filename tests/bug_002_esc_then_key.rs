//! Bug 002: in the interactive mode, Esc followed by a command key in the same read (typed quickly,
//! or bundled by ssh) was decoded as Alt+key and dropped, so `Esc q` did not quit and `Esc L` did
//! not open the unit picker. Fixed: ESC followed by a key that does not start an escape sequence is
//! the Esc key, then that key (as in Vim).

#![cfg(all(target_os = "linux", feature = "tui"))]

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

/// Runs `aequalis` in a pty via util-linux `script`, sends each item as ONE write, and returns
/// the output, or `None` when `script` is missing. Fails if the program does not exit by itself.
fn session(writes: &[&[u8]]) -> Option<String> {
    Command::new("script").arg("--version").output().ok()?;
    let mut child = Command::new("script")
        .args(["-qec", &format!("'{BIN}'"), "/dev/null"])
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
    for w in writes {
        stdin.write_all(w).unwrap();
        stdin.flush().unwrap();
        sleep(Duration::from_millis(250));
    }
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if started.elapsed() > Duration::from_secs(3) {
            child.kill().unwrap();
            panic!("aequalis did not quit");
        }
        sleep(Duration::from_millis(50));
    }
    drop(stdin);
    let out = reader.join().unwrap().unwrap();
    Some(String::from_utf8_lossy(&out).into_owned())
}

#[test]
fn reported_esc_q_in_one_write_quits() {
    let Some(out) = session(&[b"1+1\r", b"\x1bq"]) else { return };
    assert!(out.trim_end().ends_with("\x1b[?1049l\x1b[?25h"), "terminal restored");
}

#[test]
fn esc_then_shortcut_opens_the_picker() {
    // Esc M (one write) opens the Measures; L the Length unit keys (mm cm m km au ly pc): l l l
    // Enter writes km; they stay open, so Tab (imperial: in ft yd mi), l l l Enter writes the
    // target. That completes the expression (back to input mode): Enter evaluates, Esc to
    // command mode, q quits.
    let writes: &[&[u8]] = &[b"10", b"\x1bM", b"L", b"lll\r", b"\t", b"lll\r", b"\r", b"\x1b", b"q"];
    let Some(out) = session(writes) else { return };
    assert!(out.contains("Measures") && out.contains("Length units"), "picker opened");
    assert!(out.contains("10 km mi"));
    assert!(out.contains("6.2137 mi"), "evaluated after picking");
}

#[test]
fn arrow_keys_still_decode() {
    let Some(out) = session(&[b"10 km mi\r", b"\x1b[A", b"\r", b"\x1b", b"q"]) else { return };
    assert!(out.matches("6.2137 mi").count() >= 2, "recalled with ↑ and evaluated again");
}
