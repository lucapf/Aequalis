//! Calculations with units and currencies through the binary (`quantity.rs`): the dimensions
//! checked, the result in the target, the local currency, the first value's unit or the units
//! written; each value as converted noted. Prepared exchange rates, offline, in Rome's time zone.

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

/// Rates per US dollar, dated 12 Oct 2026 (as `tests/currencies.rs`).
const RATES: &str = r#"{"format":1,"synced_at":1791800000,"attempted_at":1791800000,"ok":true,"values":{
    "rates date":{"value":1791763351,"at":1791800000},"USD/EUR":{"value":0.8945,"at":1791800000},
    "USD/AED":{"value":3.6725,"at":1791800000},"USD/GBP":{"value":0.7556,"at":1791800000}}}"#;

const NOW: i64 = 1_791_800_000;

fn config(name: &str, rates: bool) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aequalis-expressions-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("aequalis")).unwrap();
    if rates {
        std::fs::write(dir.join("aequalis/currencies.json"), RATES).unwrap();
    }
    std::fs::write(dir.join("aequalis/settings.json"), r#"{"offline": true}"#).unwrap();
    dir
}

fn aequalis(dir: &PathBuf, request: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aequalis"))
        .arg(format!("--now={NOW}"))
        .arg(request)
        .env("XDG_CONFIG_HOME", dir)
        .env("TZ", "Europe/Rome")
        .env("LC_ALL", "C")
        .env("AEQUALIS_MACHINE", "plain")
        .stdin(Stdio::null())
        .output()
        .expect("run aequalis")
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).trim_end().to_string()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).trim_end().to_string()
}

#[test]
fn currencies_added_and_subtracted() {
    let dir = config("money", true);
    let o = aequalis(&dir, "100 EUR + 2000 AED - 200 USD to USD");
    assert_eq!(out(&o), "456.38 USD");
    assert_eq!(err(&o), "aequalis: note: 100 EUR = 111.79 USD\naequalis: note: 2000 AED = 544.59 USD");
    // No target: the local currency (Rome: EUR).
    let o = aequalis(&dir, "100 EUR + 2000 AED - 200 USD");
    assert_eq!(out(&o), "408.23 EUR");
    assert_eq!(err(&o), "aequalis: note: 2000 AED = 487.13 EUR\naequalis: note: 200 USD = 178.90 EUR");
    for (request, answer) in [
        ("12 usd + 3 eur in gbp", "11.60 GBP"),
        ("100 EUR + 10%", "110.00 EUR"),
        ("100 EUR / 8 h", "12.5 EUR/h"),
        ("1 kWh * 0.25 EUR/kWh", "0.25 EUR"),
        ("1.85 EUR/l to USD/gal", "7.829 USD/gal-us"),
        ("100 EUR / 50 USD", "2.2359"),
    ] {
        assert_eq!(out(&aequalis(&dir, request)), answer, "{request}");
    }
}

#[test]
fn money_details() {
    let dir = config("money-details", true);
    assert_eq!(out(&aequalis(&dir, "100 EUR - 10%")), "90.00 EUR");
    // No local currency (UTC): the first one written.
    let o = Command::new(env!("CARGO_BIN_EXE_aequalis"))
        .args([format!("--now={NOW}"), "100 EUR + 20 USD".to_string()])
        .env("XDG_CONFIG_HOME", &dir)
        .env("TZ", "UTC")
        .env("LC_ALL", "C")
        .env("AEQUALIS_MACHINE", "plain")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out(&o), "117.89 EUR");
    // Rates 12 days old: the warning, as for a conversion.
    let o = Command::new(env!("CARGO_BIN_EXE_aequalis"))
        .args([format!("--now={}", NOW + 1_000_000), "100 EUR + 20 USD".to_string()])
        .env("XDG_CONFIG_HOME", &dir)
        .env("TZ", "Europe/Rome")
        .env("LC_ALL", "C")
        .env("AEQUALIS_MACHINE", "plain")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(err(&o).contains("warning: exchange rates of 12 Oct 26: not updated since"), "{}", err(&o));
    // `--json`: the rates' age.
    let o = Command::new(env!("CARGO_BIN_EXE_aequalis"))
        .args([format!("--now={NOW}"), "--json".to_string(), "100 EUR + 20 USD".to_string()])
        .env("XDG_CONFIG_HOME", &dir)
        .env("TZ", "Europe/Rome")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out(&o).contains(r#""result":"117.89 EUR","notes":["20 USD = 17.89 EUR"]"#) && out(&o).contains(r#""rates_age":"10h""#), "{}", out(&o));
    // A past day's rates not on the device (offline): said so.
    let o = aequalis(&dir, "100 EUR + 20 USD to GBP 2025-01-30");
    assert!(err(&o).contains("no exchange rates of 30 Jan 25"), "{}", err(&o));
}

#[test]
fn no_rates() {
    let dir = config("no-rates", false);
    let o = aequalis(&dir, "100 EUR + 20 USD");
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("no exchange rates"), "{}", err(&o));
    // Units alone need none.
    assert_eq!(out(&aequalis(&dir, "10 km + 5 mi")), "18.0467 km");
}

#[test]
fn units_and_dimensions() {
    let dir = config("units", false);
    for (request, answer) in [
        ("100 km/h / 20 mi/h", "3.1069"),
        ("100km/h / 20mi/h", "3.1069"),
        ("10 km + 5 mi", "18.0467 km"),
        ("5 mi + 10 km", "11.2137 mi"),
        ("10 km + 5 mi to m", "18046.72 m"),
        ("3 m * 2 m", "6 m^2"),
        ("2 m x 3 m", "6 m^2"),
        ("2 m * 3 m to ft2", "64.5835 ft2"),
        ("3 m2 + 2 m * 1 m", "5 m2"),
        ("2 m + 3 m * 2", "8 m"),
        ("(2 m)^2", "4 m^2"),
        ("sqrt(16 m2)", "4 m"),
        ("10 km * 3", "30 km"),
        ("100 km/h * 2 h", "200 km"),
        ("1 m/s^2 * 2 s", "2 m/s"),
        ("1 h + 30 min to min", "90 min"),
        ("10 ft + 3 in in cm", "312.42 cm"),
        ("2 ft * 3 m", "19.685 ft^2"),
        ("1 m / 2 s / 3 s", "0.1667 m/s^2"),
        ("(2 m)^3 / 2 m", "4 m^2"),
        ("(2 m)^0", "1"),
        ("-(2 m) + 3 m", "1 m"),
        ("sqrt(4) m + 1 m", "3 m"),
        ("root(27, 3) m + 1 m", "4 m"),
        ("sin(30°) m + 1 m", "1.5 m"),
    ] {
        assert_eq!(out(&aequalis(&dir, request)), answer, "{request}");
    }
    let o = aequalis(&dir, "10 km + 5 mi to");
    assert_eq!((err(&o).as_str(), o.status.code()), ("aequalis: error: now the unit of the result", Some(2)));
    let o = aequalis(&dir, "100 km/h / 20 mi/h");
    assert_eq!(err(&o), "aequalis: note: no dimension\naequalis: note: 20 mi/h = 32.1869 km/h");
}

#[test]
fn conversions_stay_conversions() {
    let dir = config("conversions", true);
    for (request, answer) in
        [("10 km mi", "6.2137 mi"), ("(10+5) km mi", "9.3206 mi"), ("100 + 20 sqft m2", "11.1484 m2"), ("12 usd eur", "10.73 EUR"), ("12+4/2", "14")]
    {
        assert_eq!(out(&aequalis(&dir, request)), answer, "{request}");
    }
}

#[test]
fn errors() {
    let dir = config("errors", true);
    for (request, error) in [
        ("10 km + 5 kg", "can't add `kg` to `km`: different kinds of quantity"),
        ("10 km - 5", "can't subtract a number from `km`: different kinds of quantity"),
        ("10 km + 5 mi to kg", "cannot convert `km` to `kg`: different kinds of unit"),
        ("20 C + 5 C", "`C` can't be part of a calculation: convert it alone (`20 C …`)"),
        ("sin(2 m)", "`sin` takes a plain number, not `m`"),
        ("2 m ^ 0.5", "`m` can be raised to a whole power only, not 0.5"),
        ("2 ^ (3 m)", "a power takes a plain number, not `m`"),
        ("(2 m)^200", "the powers of the units are too large (at most 127)"),
        ("(1 m)^100 * (1 m)^100", "the powers of the units are too large (at most 127)"),
        ("sqrt(4 m) + 1 m", "`sqrt` takes a plain number, not `m`"),
        ("1 / 2 s + 1", "can't add a number to `1/s`: different kinds of quantity"),
        ("1 l/100km + 1 m", "`l/100km` can't be part of a calculation: convert it alone (`20 l/100km …`)"),
        ("5 + 1 m to s", "cannot convert `m` to `s`: different kinds of unit"),
        ("10 m / 0 s", "undefined"),
        ("1e308 m * 10 km", "result is out of range"),
        // Found by the fuzzer: a value too large to store crashed the note that writes it.
        ("5bar/1e386pascal", "result is out of range"),
        ("10 km + 5 mi foo bar", "expected a calculation with units, then the unit of the result: `10 km + 5 mi to km`"),
    ] {
        let o = aequalis(&dir, request);
        assert_eq!(err(&o), format!("aequalis: error: {error}"), "{request}");
        assert_ne!(o.status.code(), Some(0), "{request}");
    }
}
