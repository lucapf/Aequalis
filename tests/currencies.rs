//! Currency conversions through the binary: a config directory with prepared exchange rates
//! (`currencies.json`), offline, the local currency from `TZ` or the `currency.local` setting.

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

/// Rates per US dollar, as synced from ExchangeRate-API, dated 12 Oct 2026.
const RATES: &str = r#"{"format":1,"synced_at":1791800000,"attempted_at":1791800000,"ok":true,"values":{
    "rates date":{"value":1791763351,"at":1791800000},"USD/EUR":{"value":0.8945,"at":1791800000},
    "USD/AED":{"value":3.6725,"at":1791800000},"USD/CNY":{"value":7.1243,"at":1791800000},
    "USD/JPY":{"value":158.6,"at":1791800000},"USD/GBP":{"value":0.7556,"at":1791800000},
    "USD/CAD":{"value":1.4262,"at":1791800000},"USD/CHF":{"value":0.8312,"at":1791800000}}}"#;

/// The day of the tests: the rates' day, a few hours later.
const NOW: i64 = 1_791_800_000;

fn config(name: &str, rates: bool) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tally-currencies-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("tally")).unwrap();
    if rates {
        std::fs::write(dir.join("tally/currencies.json"), RATES).unwrap();
    }
    dir
}

/// The binary: offline, in Rome's time zone, on 12 Oct 2026 (`--now`: the rates are a few hours old,
/// whatever the day the tests run); `env` sets settings (`currency.local`) and `TZ`.
fn tally(dir: &PathBuf, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut settings = vec!["\"offline\": true".to_string()];
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tally"));
    cmd.arg(format!("--now={NOW}")).args(args).env("XDG_CONFIG_HOME", dir).env("TZ", "Europe/Rome").env("LC_ALL", "C").stdin(Stdio::null());
    for (k, v) in env {
        match *k {
            "TZ" => {
                cmd.env(k, v);
            }
            setting => settings.push(format!("\"{setting}\": \"{v}\"")),
        }
    }
    std::fs::write(dir.join("tally/settings.json"), format!("{{{}}}", settings.join(", "))).unwrap();
    cmd.output().expect("run tally")
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).trim_end().to_string()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).trim_end().to_string()
}

#[test]
fn names_countries_symbols() {
    let dir = config("names", true);
    for (request, answer) in [
        ("12 usd euro", "10.73 EUR"),
        ("12 USA Europe", "10.73 EUR"),
        ("12 USD UAE", "44.07 AED"),
        ("12 China Euro", "1.51 EUR"),
        ("12 US dollars to euros", "10.73 EUR"),
        ("12€ usd", "13.42 USD"),
        ("12dhs usd", "3.27 USD"),
        ("12 GBP in japan", "2518.79 JPY"),
        // Countries with several time zones: money has one currency, no zone is asked.
        ("12 USA UAE", "44.07 AED"),
        ("12 USA", "10.73 EUR"),
        ("12 Canada to USA", "8.41 USD"),
    ] {
        let words: Vec<&str> = request.split(' ').collect();
        assert_eq!(out(&tally(&dir, &words, &[])), answer, "{request}");
    }
    // Countries: the currencies used, always noted (stderr), to check.
    let o = tally(&dir, &["12", "USA", "UAE"], &[]);
    assert_eq!(out(&o), "44.07 AED");
    assert_eq!(err(&o), "tally: note: United States: US dollar (USD, US$) → United Arab Emirates: UAE dirham (AED, د.إ)");
    assert!(err(&tally(&dir, &["-s", "12", "USA", "UAE"], &[])).is_empty(), "silent");
    assert!(err(&tally(&dir, &["12", "usd", "eur"], &[])).is_empty(), "no country, no note");
    // With -v, the rate and its source.
    let o = tally(&dir, &["-v", "12", "usd", "eur"], &[]);
    assert!(err(&o).contains("1 USD = 0.8945 EUR, rates of 12 Oct 26 from ExchangeRate-API (open.er-api.com)"), "{}", err(&o));
}

#[test]
fn one_currency_goes_to_the_local_one() {
    let dir = config("local", true);
    assert_eq!(out(&tally(&dir, &["12", "usd"], &[])), "10.73 EUR");
    assert_eq!(out(&tally(&dir, &["12", "usd"], &[("TZ", "Asia/Tokyo")])), "1903.20 JPY");
    assert_eq!(out(&tally(&dir, &["12", "usd"], &[("currency.local", "chf")])), "9.97 CHF");
    let o = tally(&dir, &["12", "eur"], &[]);
    assert_eq!(o.status.code(), Some(2));
    assert!(err(&o).contains("now the currency to convert to: `12 EUR USD`"), "{}", err(&o));
    let o = tally(&dir, &["1", "km", "m"], &[("currency.local", "bogus")]);
    assert_eq!(o.status.code(), Some(2));
    assert!(err(&o).contains("`currency.local` must be auto or one currency"), "{}", err(&o));
}

#[test]
fn shared_and_misspelt_names_are_asked() {
    let dir = config("ask", true);
    // In a script nobody can choose: exit 2 with the choices.
    let o = tally(&dir, &["12", "dollar", "euro"], &[]);
    assert_eq!(o.status.code(), Some(2));
    assert!(err(&o).contains("`dollar` is several currencies: did you mean US dollar or Canadian dollar"), "{}", err(&o));
    let o = tally(&dir, &["12", "dolar", "eur"], &[]);
    assert!(err(&o).contains("`dolar` is not a currency: did you mean US dollar"), "{}", err(&o));
    // --json: the choices, to show as buttons.
    let json = out(&tally(&dir, &["--json", "12", "dollar", "euro"], &[]));
    assert!(json.contains(r#""kind":"currency""#) && json.contains(r#""request":"12 USD euro""#), "{json}");
}

#[test]
fn other_requests_are_unchanged() {
    let dir = config("other", true);
    assert_eq!(out(&tally(&dir, &["12", "pounds", "kg"], &[])), "5.4431 kg");
    assert_eq!(out(&tally(&dir, &["1", "cup", "ml"], &[])), out(&tally(&dir, &["1", "cup", "ml"], &[])));
    assert!(tally(&dir, &["1", "cup", "ml"], &[]).status.success());
    assert!(out(&tally(&dir, &["10:00AM", "USA", "to", "Japan"], &[])).is_empty(), "USA has several zones: asked");
    assert!(out(&tally(&dir, &["10:00AM", "Italy", "Japan"], &[])).ends_with(" Tokyo"));
    // A time with a country of several zones: which zone (the time request, unchanged).
    let o = tally(&dir, &["10:00", "USA", "UAE"], &[]);
    assert!(err(&o).contains("`USA` has several time zones"), "{}", err(&o));
    assert!(out(&tally(&dir, &["Italy", "Japan"], &[])).ends_with(" km"), "a distance");
    assert_eq!(out(&tally(&dir, &["10", "km", "mi"], &[])), "6.2137 mi");
}

#[test]
fn no_rates() {
    let dir = config("none", false);
    let o = tally(&dir, &["12", "usd", "eur"], &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("no exchange rates: they were never downloaded, and the offline setting is on"), "{}", err(&o));
}
