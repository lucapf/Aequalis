//! Exchange rates of a past day through the binary: `100 usd to eur 2025-01-30`, `on 30 Jan 2025`,
//! `yesterday`, `3 months ago`. The days' files (`currencies-YYMMDD.json`) are kept in the config
//! directory, or downloaded from a `file://` archive (`rates.history_url`); no date is the latest
//! rates, without a question.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The latest rates, as synced from ExchangeRate-API, dated 12 Oct 2026.
const LATEST: &str = r#"{"format":1,"synced_at":1791800000,"attempted_at":1791800000,"ok":true,"values":{
    "rates date":{"value":1791763351,"at":1791800000},"USD/EUR":{"value":0.8945,"at":1791800000},
    "USD/AED":{"value":3.6725,"at":1791800000},"USD/JPY":{"value":158.6,"at":1791800000}}}"#;

/// The day of the tests: 12 Oct 2026.
const NOW: i64 = 1_791_800_000;

/// 2026-10-12 as days since 1970-01-01.
const TODAY: i64 = 20_738;

/// A day's file, in ExchangeRate-API's answer format: rates per dollar, as of `rates_day` (days).
fn day_file(rates_day: i64, eur: f64, jpy: f64) -> String {
    format!(r#"{{"result":"success","time_last_update_unix":{},"rates":{{"USD":1,"EUR":{eur},"JPY":{jpy}}}}}"#, rates_day * 86_400)
}

/// A config directory with the latest rates, and `files` kept in `rates/`: (name, content).
fn config(name: &str, files: &[(&str, String)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aequalis-history-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("aequalis/rates")).unwrap();
    std::fs::write(dir.join("aequalis/currencies.json"), LATEST).unwrap();
    for (file, body) in files {
        std::fs::write(dir.join("aequalis/rates").join(file), body).unwrap();
    }
    dir
}

/// A folder serving as the archive (`rates.history_url`): the same files.
fn archive(name: &str, files: &[(&str, String)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aequalis-history-archive-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, body) in files {
        std::fs::write(dir.join(file), body).unwrap();
    }
    dir
}

fn settings(dir: &Path, offline: bool, url: Option<&Path>) {
    let url = url.map_or(String::new(), |u| format!(r#","rates.history_url":"{}""#, common::file_url(u)));
    let text = format!(r#"{{"offline":{offline},"rates.refresh":"manual","cities.refresh":"manual"{url}}}"#);
    std::fs::write(dir.join("aequalis/settings.json"), text).unwrap();
}

/// The binary on 12 Oct 2026 in Rome, with a proxy nothing answers: only `file://` is reachable.
fn aequalis(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_aequalis"));
    for var in ["http_proxy", "https_proxy", "HTTPS_PROXY", "ALL_PROXY", "all_proxy"] {
        command.env(var, "http://127.0.0.1:9");
    }
    command
        .arg(format!("--now={NOW}"))
        .args(args)
        .env("XDG_CONFIG_HOME", dir)
        .env("TZ", "Europe/Rome")
        .env("LC_ALL", "C")
        .env("AEQUALIS_MACHINE", "plain")
        .stdin(Stdio::null())
        .output()
        .expect("run aequalis")
}

fn run(dir: &Path, request: &str) -> (Output, String, String) {
    let args: Vec<&str> = request.split(' ').collect();
    let out = aequalis(dir, &args);
    let (stdout, stderr) = (String::from_utf8_lossy(&out.stdout).trim().to_string(), String::from_utf8_lossy(&out.stderr).trim().to_string());
    (out, stdout, stderr)
}

/// The answer of a request that works.
fn ok(dir: &Path, request: &str) -> String {
    let (out, stdout, stderr) = run(dir, request);
    assert!(out.status.success(), "{request}: {stderr}");
    stdout
}

/// The error of a request that fails: (exit code, message).
fn fails(dir: &Path, request: &str) -> (i32, String) {
    let (out, stdout, stderr) = run(dir, request);
    assert!(!out.status.success(), "{request}: {stdout}");
    (out.status.code().unwrap(), stderr)
}

/// Files for the days the tests ask about: 30 Jan 2025 (EUR 0.96126), yesterday, 12 Jul 2026.
fn files() -> Vec<(&'static str, String)> {
    vec![
        ("currencies-250130.json", day_file(20_118, 0.96126, 154.11)),
        ("currencies-261011.json", day_file(TODAY - 1, 0.8934, 157.0)),
        ("currencies-260712.json", day_file(20_646, 0.85, 150.0)),
    ]
}

#[test]
fn a_date_at_the_end_or_the_start() {
    let dir = config("dates", &files());
    settings(&dir, true, None);
    assert_eq!(ok(&dir, "100 usd to eur 2025-01-30"), "96.13 EUR");
    assert_eq!(ok(&dir, "100 usd to eur on 30 Jan 2025"), "96.13 EUR");
    assert_eq!(ok(&dir, "100 usd to eur 30 January 2025"), "96.13 EUR");
    assert_eq!(ok(&dir, "on 2025-01-30 100 usd to eur"), "96.13 EUR");
    assert_eq!(ok(&dir, "2025-01-30 100 usd eur"), "96.13 EUR");
    // Currencies in other words, and the other direction.
    assert_eq!(ok(&dir, "100 usd to euros 2025-01-30"), "96.13 EUR");
    assert_eq!(ok(&dir, "100 eur to jpy 2025-01-30"), "16032.08 JPY");
}

#[test]
fn relative_dates() {
    let dir = config("relative", &files());
    settings(&dir, true, None);
    assert_eq!(ok(&dir, "100 usd to eur yesterday"), "89.34 EUR");
    assert_eq!(ok(&dir, "100 usd to eur 3 months ago"), "85.00 EUR");
    assert_eq!(ok(&dir, "100 usd to eur 1 day ago"), "89.34 EUR");
    assert_eq!(ok(&dir, "100 usd to eur 92 days ago"), "85.00 EUR");
}

#[test]
fn no_date_is_the_latest_rates_and_never_asked() {
    let dir = config("latest", &files());
    settings(&dir, true, None);
    assert_eq!(ok(&dir, "100 usd to eur"), "89.45 EUR");
    // Today is the latest rates too, with no file for it.
    assert_eq!(ok(&dir, "100 usd to eur today"), "89.45 EUR");
    assert_eq!(ok(&dir, "100 usd to eur 2026-10-12"), "89.45 EUR");
    // The latest rates warn when old (a past day's do not): here they are fresh.
    let (_, _, stderr) = run(&dir, "-v 100 usd to eur");
    assert!(stderr.contains("rates of 12 Oct 26 from ExchangeRate-API"), "{stderr}");
}

#[test]
fn the_note_says_which_day_and_where_from() {
    let dir = config("note", &files());
    settings(&dir, true, None);
    let (out, stdout, stderr) = run(&dir, "-v 100 usd to eur 2025-01-30");
    assert!(out.status.success());
    assert_eq!(stdout, "96.13 EUR");
    assert!(stderr.contains("1 USD = 0.96126 EUR, rates of 30 Jan 25 from ExchangeRate-API"), "{stderr}");
    // No warning that the rates are old: the operator asked for that day.
    assert!(!stderr.contains("not updated since"), "{stderr}");
}

#[test]
fn a_file_holding_an_earlier_days_rates_says_which_day() {
    // The file of 2 Feb 2025 holds the rates of 31 Jan (the source had not updated yet): the note says so.
    let dir = config("weekend", &[("currencies-250202.json", day_file(20_119, 0.96219, 155.0))]);
    settings(&dir, true, None);
    let (out, stdout, stderr) = run(&dir, "-v 100 usd to eur 2025-02-02");
    assert!(out.status.success(), "{stderr}");
    assert_eq!(stdout, "96.22 EUR");
    assert!(stderr.contains("rates of 31 Jan 25"), "{stderr}");
}

#[test]
fn an_ambiguous_date_asks_day_or_month() {
    let dir = config("ambiguous", &files());
    settings(&dir, true, None);
    let (code, message) = fails(&dir, "100 usd to eur 03/04/25");
    assert_eq!(code, 2);
    assert!(message.contains("`03/04/25` is ambiguous"), "{message}");
    assert!(message.contains("`100 usd to eur 2025-03-04`") && message.contains("2025-04-03"), "{message}");
    // The choices are plain requests.
    let json = String::from_utf8(aequalis(&dir, &["--json", "100", "usd", "to", "eur", "03/04/25"]).stdout).unwrap();
    assert!(json.contains(r#""kind":"date""#) && json.contains(r#""request":"100 usd to eur 2025-03-04""#), "{json}");
    assert_eq!(ok(&dir, "100 usd to eur 2025-01-30"), "96.13 EUR");
    // A written month is not ambiguous.
    assert_eq!(ok(&dir, "100 usd to eur 30/Jan/2025"), "96.13 EUR");
}

#[test]
fn days_that_cannot_have_rates() {
    let dir = config("range", &files());
    settings(&dir, true, None);
    let (code, message) = fails(&dir, "100 usd to eur tomorrow");
    assert_eq!(code, 1);
    assert!(message.contains("no exchange rates of 13 Oct 26: that day has not come yet"), "{message}");
    assert!(fails(&dir, "100 usd to eur 2026-12-01").1.contains("has not come yet"));
    let (_, message) = fails(&dir, "100 usd to eur 2024-01-01");
    assert!(message.contains("no exchange rates of 01 Jan 24: the history keeps 750 days"), "{message}");
    // 749 days ago is the oldest day: it asks for its file (offline: not there); 750 is out.
    assert!(fails(&dir, "100 usd to eur 749 days ago").1.contains("offline"));
    assert!(fails(&dir, "100 usd to eur 750 days ago").1.contains("keeps 750 days"));
}

#[test]
fn a_currency_a_days_file_lacks_is_named_with_the_day() {
    let dir = config("lacks", &files());
    settings(&dir, true, None);
    let (code, message) = fails(&dir, "100 usd to aed 2025-01-30");
    assert_eq!(code, 1);
    assert!(message.contains("no exchange rate for the UAE dirham (AED) on 30 Jan 25"), "{message}");
    // The latest rates have it.
    assert_eq!(ok(&dir, "100 usd to aed"), "367.25 AED");
}

#[test]
fn offline_uses_the_files_kept_and_says_when_one_is_missing() {
    let dir = config("offline", &files());
    settings(&dir, true, None);
    assert_eq!(ok(&dir, "100 usd to eur 2025-01-30"), "96.13 EUR");
    let (code, message) = fails(&dir, "100 usd to eur 2025-01-29");
    assert_eq!(code, 1);
    assert!(message.contains("no exchange rates of 29 Jan 25 on this device, and the offline setting is on"), "{message}");
}

#[test]
fn a_day_is_downloaded_once_and_kept() {
    let dir = config("download", &[]);
    let from = archive("download", &files());
    settings(&dir, false, Some(&from));
    assert!(!dir.join("aequalis/rates/currencies-250130.json").exists());
    assert_eq!(ok(&dir, "100 usd to eur 2025-01-30"), "96.13 EUR");
    assert!(dir.join("aequalis/rates/currencies-250130.json").exists(), "kept");
    // The archive gone: the kept file answers, nothing is downloaded again.
    std::fs::remove_dir_all(&from).unwrap();
    assert_eq!(ok(&dir, "100 usd to eur 2025-01-30"), "96.13 EUR");
}

#[test]
fn a_day_the_archive_lacks_or_a_broken_file_is_an_error() {
    let dir = config("missing", &[]);
    let from = archive("missing", &[("currencies-250131.json", "not rates".to_string())]);
    settings(&dir, false, Some(&from));
    let (code, message) = fails(&dir, "100 usd to eur 2025-01-30");
    assert_eq!(code, 1);
    assert!(message.contains("no exchange rates of 30 Jan 25: they could not be downloaded"), "{message}");
    let (_, message) = fails(&dir, "100 usd to eur 2025-01-31");
    assert!(message.contains("no exchange rates of 31 Jan 25: the archive has none"), "{message}");
    assert!(!dir.join("aequalis/rates/currencies-250131.json").exists(), "a broken file is not kept");
}

#[test]
fn a_sync_deletes_the_days_older_than_750() {
    // 749 days ago is 2024-09-23: kept; 750 days ago (2024-09-22) and older: deleted.
    let old = [("currencies-240922.json", day_file(1, 0.9, 150.0)), ("currencies-200101.json", day_file(1, 0.9, 150.0))];
    let kept = [("currencies-240923.json", day_file(1, 0.9, 150.0)), ("currencies-261011.json", day_file(1, 0.9, 150.0))];
    let all: Vec<(&str, String)> = old.iter().chain(kept.iter()).map(|(n, b)| (*n, b.clone())).collect();
    let dir = config("prune", &all);
    settings(&dir, false, None);
    let _ = aequalis(&dir, &["--sync"]);
    let rates = dir.join("aequalis/rates");
    assert!(!rates.join("currencies-240922.json").exists() && !rates.join("currencies-200101.json").exists());
    assert!(rates.join("currencies-240923.json").exists() && rates.join("currencies-261011.json").exists());
}

#[test]
fn requests_that_only_look_like_they_have_a_date_are_not_touched() {
    let dir = config("others", &files());
    settings(&dir, true, None);
    // Times and units with a date or a number: the other converters.
    assert_eq!(ok(&dir, "5 km to m"), "5000 m");
    let (_, _, stderr) = run(&dir, "10:00 Tokyo 2025-01-30");
    assert!(!stderr.contains("exchange rates"), "{stderr}");
    // A date with no currency request: the time converter's usual error, not a rates error.
    let (_, message) = fails(&dir, "2025-01-30 yesterday");
    assert!(!message.contains("exchange rates"), "{message}");
}

/// The settings with `rates.history` set.
fn history_setting(dir: &Path, on: bool, url: Option<&Path>) {
    let url = url.map_or(String::new(), |u| format!(r#","rates.history_url":"{}""#, common::file_url(u)));
    let text = format!(r#"{{"offline":false,"rates.history":{on},"rates.refresh":"manual","cities.refresh":"manual"{url}}}"#);
    std::fs::write(dir.join("aequalis/settings.json"), text).unwrap();
}

#[test]
fn history_is_on_by_default() {
    let dir = config("default-on", &files());
    std::fs::remove_file(dir.join("aequalis/settings.json")).ok();
    let out = aequalis(&dir, &["--settings"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("rates.history") && text.contains("true"), "{text}");
}

#[test]
fn off_there_are_no_dated_requests_and_no_downloads() {
    let dir = config("off", &files());
    let from = archive("off", &[("currencies-261012.json", day_file(TODAY, 0.8945, 158.6))]);
    history_setting(&dir, false, Some(&from));
    let (code, message) = fails(&dir, "100 usd to eur 2025-01-30");
    assert_eq!(code, 1);
    assert!(message.contains("past days are off: turn on the rates.history setting"), "{message}");
    assert!(fails(&dir, "100 usd to eur yesterday").1.contains("are off"));
    // The latest rates work as ever, and so does today.
    assert_eq!(ok(&dir, "100 usd to eur"), "89.45 EUR");
    assert_eq!(ok(&dir, "100 usd to eur today"), "89.45 EUR");
    // A sync downloads no past day.
    let _ = aequalis(&dir, &["--sync"]);
    assert!(!dir.join("aequalis/rates/currencies-261012.json").exists());
    // The kept files stay, and come back when it is on again.
    history_setting(&dir, true, Some(&from));
    assert_eq!(ok(&dir, "100 usd to eur 2025-01-30"), "96.13 EUR");
}

#[test]
fn a_sync_downloads_the_missing_days_and_stops_at_the_archives_start() {
    // The archive began on 8 Oct, and lacks 9 Oct (a run missed): 12, 11, 10 and 8 Oct are there,
    // 3 Oct too, beyond three days it lacks in a row (7, 6, 5).
    let names = ["261012", "261011", "261010", "261008", "261003"];
    let made: Vec<(String, String)> = names.iter().map(|d| (format!("currencies-{d}.json"), day_file(TODAY, 0.9, 150.0))).collect();
    let from = archive("sync", &made.iter().map(|(n, b)| (n.as_str(), b.clone())).collect::<Vec<_>>());
    let dir = config("sync", &[("currencies-261011.json", day_file(TODAY - 1, 0.8934, 157.0))]);
    history_setting(&dir, true, Some(&from));
    let _ = aequalis(&dir, &["--sync"]);
    let rates = dir.join("aequalis/rates");
    for held in ["261012", "261011", "261010", "261008"] {
        assert!(rates.join(format!("currencies-{held}.json")).exists(), "{held}");
    }
    assert!(!rates.join("currencies-261003.json").exists(), "past three days in a row without a file");
    // Again: nothing new, nothing lost.
    let _ = aequalis(&dir, &["--sync"]);
    assert_eq!(std::fs::read_dir(&rates).unwrap().count(), 4);
}

#[test]
fn the_days_held_are_shown() {
    let dir = config("count", &[]);
    history_setting(&dir, true, None);
    let json = |args: &[&str]| String::from_utf8(aequalis(&dir, args).stdout).unwrap();
    // None yet.
    assert!(json(&["--online", "--json"]).contains(r#""history":"none yet""#));
    let rates = dir.join("aequalis/rates");
    for name in ["currencies-261010.json", "currencies-261011.json", "currencies-261012.json", "currencies-190101.json"] {
        std::fs::write(rates.join(name), day_file(TODAY, 0.9, 150.0)).unwrap();
    }
    // Three days (the file of 2019 is out of the 750 days), the oldest named.
    assert!(json(&["--online", "--json"]).contains(r#""history":"3 days, since 10 Oct 26""#), "{}", json(&["--online", "--json"]));
    assert!(json(&["--about"]).contains("3 days, since 10 Oct 26"), "{}", json(&["--about"]));
    assert!(json(&["--settings"]).contains("rates.history"));
    history_setting(&dir, false, None);
    assert!(json(&["--online", "--json"]).contains(r#""history":"off""#));
    assert!(json(&["--about"]).contains("Rates of past days") && json(&["--about"]).contains("off"));
}
