//! `TALLY_MODE` (online by default, or offline: no network), `--online`, and the online data
//! (`~/.config/tally/online.json`), through the binary. Each test has its own config directory with
//! a prepared file, so no test syncs for real (the data is fresh, or the mode is offline).

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// A config directory for one test, with `online.json` holding `json` (none if empty).
fn config(name: &str, json: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tally-network-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("tally")).unwrap();
    if !json.is_empty() {
        std::fs::write(dir.join("tally/online.json"), json).unwrap();
    }
    dir
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// Fresh data (synced now): no sync is needed.
fn fresh(ok: bool) -> String {
    let t = now();
    format!(r#"{{"format":1,"synced_at":{t},"attempted_at":{t},"ok":{ok},"values":{{"Voyager 1":{{"value":180.5,"at":{t}}},"Starlink":{{"value":12345,"at":{t}}}}}}}"#)
}

fn tally(mode: Option<&str>, dir: &PathBuf, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tally"));
    cmd.args(args).env("LC_ALL", "C").env("XDG_CONFIG_HOME", dir).env_remove("TALLY_MODE").stdin(Stdio::null());
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
    let dir = config("invalid", &fresh(true));
    let out = tally(Some("sometimes"), &dir, &["1", "km", "m"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&out.stderr).trim_end(), "tally: error: TALLY_MODE must be online or offline (it is `sometimes`)");
    // Valid, any case: conversions work as usual.
    assert_eq!(text(&tally(Some("OFFLINE"), &dir, &["1", "km", "m"])), "1000 m");
    assert_eq!(text(&tally(Some("online"), &dir, &["1", "km", "m"])), "1000 m");
    // The refresh interval too.
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tally"));
    let out = cmd.args(["1", "km", "m"]).env("XDG_CONFIG_HOME", &dir).env("TALLY_ONLINE_REFRESH_INTERVAL", "soon").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("TALLY_ONLINE_REFRESH_INTERVAL must be a duration"));
}

#[test]
fn forced_offline() {
    let dir = config("forced", "");
    assert_eq!(text(&tally(Some("offline"), &dir, &["--online"])), "offline (forced)");
    let json = text(&tally(Some("offline"), &dir, &["--json", "--online"]));
    assert!(json.starts_with(r#"{"ok":true,"result":"offline (forced)""#), "{json}");
    // The trivia: no network, estimates at once; no sync started (no lock, no file).
    let started = Instant::now();
    let out = text(&tally(Some("offline"), &dir, &["--trivia"]));
    assert!(started.elapsed().as_secs() < 3, "no download: {:?}", started.elapsed());
    assert!(out.contains("Voyager 1 = ") && out.contains("(estimated, not synced yet)"), "{out}");
    assert!(!dir.join("tally/online.lock").exists() && !dir.join("tally/online.json").exists());
    assert_eq!(String::from_utf8_lossy(&tally(Some("offline"), &dir, &["--sync"]).stderr).trim_end(), "tally: TALLY_MODE=offline: no sync");
}

#[test]
fn fresh_data_is_used_offline() {
    // Online mode, but the data is fresh: its values are shown, nothing is synced.
    let dir = config("fresh", &fresh(true));
    let before = std::fs::read_to_string(dir.join("tally/online.json")).unwrap();
    let out = text(&tally(None, &dir, &["--trivia"]));
    assert!(out.contains("Starlink = 12345 (synced ") && out.contains(" from CelesTrak)"), "{out}");
    assert!(out.contains("Voyager 1 = ") && out.contains(" from JPL Horizons)"), "{out}");
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!dir.join("tally/online.lock").exists(), "no sync started");
    assert_eq!(std::fs::read_to_string(dir.join("tally/online.json")).unwrap(), before, "the file is untouched");
    assert_eq!(text(&tally(None, &dir, &["--online"])), "online");
    // The last try reached no source (its values are older), and was just made: offline, no
    // retry yet.
    let t = now();
    let failed = format!(r#"{{"format":1,"synced_at":{old},"attempted_at":{t},"ok":false,"values":{{"Starlink":{{"value":12345,"at":{old}}}}}}}"#, old = t - 100);
    let dir = config("failed", &failed);
    assert_eq!(text(&tally(None, &dir, &["--online"])), "offline");
}
