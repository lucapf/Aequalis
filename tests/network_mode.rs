//! `TALLY_MODE` (online by default, or offline: no network) and `--online`, through the binary.

use std::process::{Command, Output, Stdio};
use std::time::Instant;

fn tally(mode: Option<&str>, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tally"));
    cmd.args(args).env("LC_ALL", "C").env_remove("TALLY_MODE").stdin(Stdio::null());
    if let Some(m) = mode {
        cmd.env("TALLY_MODE", m);
    }
    cmd.output().expect("run tally")
}

fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}

#[test]
fn an_invalid_mode_is_a_usage_error() {
    let out = tally(Some("sometimes"), &["1", "km", "m"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&out.stderr).trim_end(), "tally: error: TALLY_MODE must be online or offline (it is `sometimes`)");
    // Valid, any case: conversions work as usual.
    assert_eq!(text(&tally(Some("OFFLINE"), &["1", "km", "m"])), "1000 m");
    assert_eq!(text(&tally(Some("online"), &["1", "km", "m"])), "1000 m");
}

#[test]
fn forced_offline() {
    assert_eq!(text(&tally(Some("offline"), &["--online"])), "offline (forced)");
    let json = text(&tally(Some("offline"), &["--json", "--online"]));
    assert!(json.starts_with(r#"{"ok":true,"result":"offline (forced)""#), "{json}");
    // The trivia: no network, estimates at once.
    let started = Instant::now();
    let out = text(&tally(Some("offline"), &["--trivia"]));
    assert!(started.elapsed().as_secs() < 3, "no download: {:?}", started.elapsed());
    assert!(out.contains("Voyager 1 = ") && out.contains("(estimated: offline)"), "{out}");
}

#[test]
fn online_status_is_a_word() {
    // Whatever the network here: one of the two.
    let out = text(&tally(None, &["--online"]));
    assert!(out == "online" || out == "offline", "{out}");
}
