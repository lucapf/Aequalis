//! The `offline` setting (no network at all), `--online`, the refresh policies and the online
//! data (`~/.config/aequalis/online.json`, `currencies.json`), through the binary. Each test has its
//! own config directory with prepared files, so no test syncs for real (the data is fresh, the
//! policy is manual, or the mode is offline).

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// Exchange rates synced at `t`.
fn rates(t: u64, ok: bool) -> String {
    format!(
        r#"{{"format":1,"synced_at":{t},"attempted_at":{t},"ok":{ok},"values":{{"rates date":{{"value":{t},"at":{t}}},"USD/EUR":{{"value":0.9,"at":{t}}}}}}}"#
    )
}

/// A config directory for one test: `online.json` holding `live` (none if empty) and fresh
/// exchange rates (`rates`, none if empty).
fn config(name: &str, live: &str, rates: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aequalis-network-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("aequalis")).unwrap();
    if !live.is_empty() {
        std::fs::write(dir.join("aequalis/online.json"), live).unwrap();
    }
    if !rates.is_empty() {
        std::fs::write(dir.join("aequalis/currencies.json"), rates).unwrap();
    }
    dir
}

/// Fresh data (synced now): no sync is needed.
fn fresh(ok: bool) -> String {
    let t = now();
    format!(
        r#"{{"format":1,"synced_at":{t},"attempted_at":{t},"ok":{ok},"values":{{"Voyager 1":{{"value":180.5,"at":{t}}},"Starlink":{{"value":12345,"at":{t}}}}}}}"#
    )
}

/// The binary, with `settings` written first (`offline` false unless given).
fn aequalis(settings: &[(&str, &str)], dir: &PathBuf, args: &[&str]) -> Output {
    let mut all = settings.to_vec();
    if !all.iter().any(|(k, _)| *k == "offline") {
        all.push(("offline", "false"));
    }
    let body: Vec<String> = all.iter().map(|(k, v)| format!("\"{k}\": \"{v}\"")).collect();
    std::fs::write(dir.join("aequalis/settings.json"), format!("{{{}}}", body.join(", "))).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_aequalis"));
    cmd.args(args).env("LC_ALL", "C").env("XDG_CONFIG_HOME", dir).stdin(Stdio::null());
    cmd.output().expect("run aequalis")
}

const OFFLINE: &[(&str, &str)] = &[("offline", "true")];

fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}

#[test]
fn invalid_settings_are_usage_errors() {
    let dir = config("invalid", &fresh(true), &rates(now(), true));
    let out = aequalis(&[("offline", "sometimes")], &dir, &["1", "km", "m"]);
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(err.contains("`offline` must be true or false (it is `sometimes`)"), "{err}");
    // Any case: conversions work as usual.
    assert_eq!(text(&aequalis(&[("offline", "TRUE")], &dir, &["1", "km", "m"])), "1000 m");
    // An interval too.
    let out = aequalis(&[("rates.interval", "soon")], &dir, &["1", "km", "m"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("`rates.interval` must be a duration"));
}

#[test]
fn forced_offline() {
    let dir = config("forced", "", "");
    assert_eq!(text(&aequalis(OFFLINE, &dir, &["--online"])), "offline (forced)");
    let json = text(&aequalis(OFFLINE, &dir, &["--json", "--online"]));
    assert!(json.starts_with(r#"{"ok":true,"result":"offline (forced)""#), "{json}");
    // The trivia: no network, estimates at once; no sync started (no lock, no file).
    let started = Instant::now();
    let out = text(&aequalis(OFFLINE, &dir, &["--trivia"]));
    assert!(started.elapsed().as_secs() < 3, "no download: {:?}", started.elapsed());
    assert!(out.contains("Voyager 1 = ") && out.contains("(estimated, not synced yet)"), "{out}");
    assert!(!dir.join("aequalis/online.lock").exists() && !dir.join("aequalis/online.json").exists());
    assert_eq!(String::from_utf8_lossy(&aequalis(OFFLINE, &dir, &["--sync"]).stderr).trim_end(), "aequalis: the offline setting is on: no sync");
}

#[test]
fn fresh_data_is_used_offline() {
    // Online, but the data is fresh (and the live values are manual): nothing is synced.
    let dir = config("fresh", &fresh(true), &rates(now(), true));
    let before = std::fs::read_to_string(dir.join("aequalis/online.json")).unwrap();
    let out = text(&aequalis(&[], &dir, &["--trivia"]));
    assert!(out.contains("Starlink = 12345 (synced ") && out.contains(" from CelesTrak)"), "{out}");
    assert!(out.contains("Voyager 1 = ") && out.contains(" from JPL Horizons)"), "{out}");
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!dir.join("aequalis/online.lock").exists(), "no sync started");
    assert_eq!(std::fs::read_to_string(dir.join("aequalis/online.json")).unwrap(), before, "the file is untouched");
    assert_eq!(text(&aequalis(&[], &dir, &["--online"])), "online");
    // The last try (the newest of the files) reached no source, and was just made: offline, no
    // retry yet.
    let t = now();
    let failed =
        format!(r#"{{"format":1,"synced_at":{old},"attempted_at":{t},"ok":false,"values":{{"Starlink":{{"value":12345,"at":{old}}}}}}}"#, old = t - 100);
    let dir = config("failed", &failed, &rates(t - 1000, true));
    assert_eq!(text(&aequalis(&[], &dir, &["--online"])), "offline");
}

#[test]
fn notices_under_the_notify_policy() {
    // Out-of-date exchange rates, under `notify`: a notice, nothing downloaded. (The cities are
    // off here: their archive's own notice is tested below.)
    let old = now() - 3 * 86_400;
    let dir = config("notify", &fresh(true), &rates(old, true));
    let notify = [("rates.refresh", "notify"), ("cities.enabled", "false")];
    let out = aequalis(&notify, &dir, &["1", "km", "m"]);
    assert_eq!(text(&out), "1000 m");
    // In a script (stderr not a terminal): nothing on stderr; in --json, always.
    assert!(String::from_utf8_lossy(&out.stderr).is_empty());
    let json = text(&aequalis(&notify, &dir, &["--json", "1", "km", "m"]));
    assert!(json.contains(r#""notices":["the exchange rates are out of date"]"#), "{json}");
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!dir.join("aequalis/online.lock").exists(), "nothing downloaded");
    // Manual: nothing said.
    assert!(!text(&aequalis(&[("rates.refresh", "manual"), ("cities.enabled", "false")], &dir, &["--json", "1", "km", "m"])).contains("notices"));
    // The city archive never downloaded: said whatever the policy, with the online state too
    // (the widget shows it).
    let json = text(&aequalis(&[("rates.refresh", "manual")], &dir, &["--json", "--online"]));
    assert!(json.contains(r#""notices":["the city archive is not downloaded yet"]"#), "{json}");
}
