//! Bug 007: in the TUI, `2026/12/24 - 12d` previewed as "· now the target unit", with unit
//! suggestions (`Tab: d day days`), and picking the `d` on the unit keys did not complete it.
//!
//! The preview's "target unit" prompt and the suggestions read the expression as a number
//! followed by one unit (`d`). Fixed: a date request is complete (its result previews; no prompt,
//! no suggestions; the unit keys switch back to input mode). Checked in a real pty session; the
//! CLI result (target YYYY/MM/DD, the input's format) is checked too.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

#[test]
fn reported_case_on_the_command_line() {
    let out = Command::new(BIN).arg("-v").args(["2026/12/24", "-", "12d"]).env("TZ", "Europe/Rome").env("LC_ALL", "C").output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "2026/12/12 YYYY/MM/DD\n");
}

#[cfg(all(target_os = "linux", feature = "tui"))]
#[test]
fn reported_case_in_the_tui() {
    use std::io::{Read, Write};
    use std::process::Stdio;
    if Command::new("script").arg("--version").output().is_err() {
        return;
    }
    let mut child = Command::new("script")
        .args(["-qec", &format!("stty rows 24 cols 80; exec '{BIN}'"), "/dev/null"])
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
        let mut out = Vec::new();
        stdout.read_to_end(&mut out).map(|_| out)
    });
    std::thread::sleep(std::time::Duration::from_millis(500));
    stdin.write_all(b"2026/12/24 - 12d").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(400));
    stdin.write_all(b"\r").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(400));
    stdin.write_all(b"\x03").unwrap();
    drop(stdin);
    child.wait().unwrap();
    let out = String::from_utf8_lossy(&reader.join().unwrap().unwrap()).into_owned();
    assert!(!out.contains("now the target unit"), "no target-unit prompt");
    assert!(!out.contains("Tab:"), "no unit suggestions");
    assert!(out.contains("= 2026/12/12 YYYY/MM/DD") && !out.contains(" iso = "), "previewed and evaluated");
}
