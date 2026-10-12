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
    // The exchange rates too: without them a sync starts at once.
    let rates: String = "USD JPY CZK DKK GBP HUF PLN RON SEK CHF ISK NOK TRY AUD BRL CAD CNY HKD IDR ILS INR KRW MXN MYR NZD PHP SGD THB ZAR"
        .split(' ')
        .map(|c| format!(r#","EUR/{c}":{{"value":1.5,"at":{t}}}"#))
        .collect();
    format!(r#"{{"format":1,"synced_at":{t},"attempted_at":{t},"ok":{ok},"values":{{"Voyager 1":{{"value":180.5,"at":{t}}},"Starlink":{{"value":12345,"at":{t}}},"ECB rates date":{{"value":{day},"at":{t}}}{rates}}}}}"#, day = t - t % 86_400)
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

#[test]
fn currencies_from_the_synced_rates() {
    // Every rate is 1.5 per euro in the fixture: 100 USD is 66.67 EUR, 100 USD is 100 GBP.
    let dir = config("currency", &fresh(true));
    assert_eq!(text(&tally(None, &dir, &["100", "usd", "eur"])), "66.67 EUR");
    assert_eq!(text(&tally(None, &dir, &["100", "USD", "to", "Gbp"])), "100.00 GBP");
    // The amounts on stdin, like `tally km mi`.
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tally"));
    cmd.args(["usd", "eur"]).env("XDG_CONFIG_HOME", &dir).env_remove("TALLY_MODE").stdin(Stdio::piped()).stdout(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), b"100\n300\n").unwrap();
    assert_eq!(String::from_utf8_lossy(&child.wait_with_output().unwrap().stdout), "66.67 EUR\n200.00 EUR\n");
    let out = tally(None, &dir, &["100", "usd", "eur", "gbp"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("one amount and two currencies"));
    // Never synced, offline: no rates, exit 1.
    let dir = config("currency-none", "");
    let out = tally(Some("offline"), &dir, &["1", "usd", "eur"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("no exchange rates: they were never downloaded"), "{:?}", out);
}
