//! Shoe sizes through the binary: every phase 1 case of `docs/SPEC_SIZES.md` §9. A size alone runs
//! on a simulated machine (`AEQUALIS_MACHINE`: `italy`, `us`), the same on every laptop; a currency
//! that could be a size (`42 eu us`) needs exchange rates, so it runs as the currency tests do:
//! prepared rates, offline, a fixed day, the region from `TZ`. Debug builds only (the machine).
#![cfg(debug_assertions)]

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const BRAND: &str = "aequalis: note: sizes vary by brand: check the brand's chart";

fn on(machine: &str, request: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aequalis")).args(request.split(' ')).env("AEQUALIS_MACHINE", machine).stdin(Stdio::null()).output().expect("run aequalis")
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).trim_end().to_string()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).trim_end().to_string()
}

/// The size on both machines (the target is typed: the region does not matter).
fn size(request: &str) -> String {
    let (a, b) = (on("italy", request), on("us", request));
    assert!(a.status.success(), "{request}: {}", err(&a));
    assert_eq!(out(&a), out(&b), "{request}: the same on every machine");
    out(&a)
}

/// The notes of a size on the `italy` machine.
fn notes(request: &str) -> Vec<String> {
    err(&on("italy", request)).lines().map(String::from).collect()
}

#[test]
fn one_person_both_charts_named_where_they_differ() {
    assert_eq!(size("42 eu men shoes us"), "US 8.5 (Nike), 9 (Clarks)");
    assert_eq!(notes("42 eu men shoes us"), ["aequalis: note: UK 7.5 · EU 42 · JP 26.5 cm · CN 42", "aequalis: note: men's shoes (details: Nike)", BRAND]);
    assert_eq!(size("41 eu men shoes us"), "US 8", "the charts agree");
    assert_eq!(size("42 eu donna scarpe us"), "US 10 (Nike), 10.5 (Clarks)");
    assert_eq!(size("uk 6 men shoes us"), "US 6.5–7 (Nike), 7 (Clarks)", "UK 6 twice in Nike's chart: a range");
    assert_eq!(size("42⅔ eu men shoes us"), "US 9 (Nike), 9.5 (Clarks)", "an Adidas label");
    assert_eq!(size("50 eu men shoes us"), "US 15.5 (Clarks)", "past Nike's last row");
}

#[test]
fn between_rows() {
    assert_eq!(size("42.2 eu men shoes us"), "US 8.5 (Nike), 9 (Clarks)");
    assert!(notes("42.2 eu men shoes us").contains(&"aequalis: note: between EU 42 and 42.5".to_string()));
    assert_eq!(size("40.5 eu men shoes us"), "US 7.5 (Nike), 8 (Clarks)");
    assert!(notes("40.5 eu men shoes us").contains(&"aequalis: note: between EU 40 and 41 (Clarks)".to_string()));
    assert_eq!(size("41.75 eu men shoes us"), "US 8.5 (Nike), 9 (Clarks)");
    let n = notes("41.75 eu men shoes us");
    for note in ["between EU 41 and 42 (Nike)", "between EU 41.5 and 42 (Clarks)"] {
        assert!(n.contains(&format!("aequalis: note: {note}")), "{n:?}");
    }
}

#[test]
fn no_target_the_regions_system() {
    let italy = on("italy", "scarpe uomo 42");
    assert_eq!(out(&italy), "EU 42");
    assert!(err(&italy).starts_with("aequalis: note: US 8.5 · UK 7.5 · JP 26.5 cm · CN 42"), "{}", err(&italy));
    assert_eq!(out(&on("italy", "men shoes 9 us")), "EU 42.5 (Nike), 42 (Clarks)");
    assert_eq!(out(&on("us", "men shoes 42 eu")), "US 8.5 (Nike), 9 (Clarks)");
    assert_eq!(out(&on("italy", "zapatos mujer 38")), "EU 38");
    assert_eq!(out(&on("us", "shoes kids 30")), "US 12.5C");
}

#[test]
fn from_the_foot() {
    assert_eq!(out(&on("italy", "27 cm men foot")), "EU 44 (Nike), 43 (Clarks)");
    assert_eq!(out(&on("us", "11 in men foot")), "US 11", "27.94 cm: both charts' US 11");
    // `foot` alone is the unit.
    assert_eq!(out(&on("italy", "27 cm foot")), "0.8858 ft");
}

#[test]
fn no_person_every_person() {
    assert_eq!(size("42 eu shoes us"), "US 8.5 men, US 10 women");
    assert_eq!(size("30 eu shoes us"), "US 12.5C kids");
    assert_eq!(size("50 eu shoes us"), "US 15.5 men (Clarks)");
    assert_eq!(size("uk 4.5 kids shoes us"), "US 5C toddler, US 5Y (Nike), 5.5Y (Clarks) youth");
    assert_eq!(size("42 eu shoes"), "         US    UK   EU     JP   CN\nmen     8.5   7.5   42   26.5   42\nwomen  10     7.5   42   27     42");
}

#[test]
fn json_and_its_extra_results() {
    let json = |request: &str| out(&on("italy", &format!("--json {request}")));
    assert!(
        json("42 eu shoes us").starts_with(r#"{"ok":true,"result":"US 8.5 men","extra":[{"type":"shoes","results":["US 10 women"]}],"#),
        "{}",
        json("42 eu shoes us")
    );
    assert!(
        json("42 eu shoes").contains(r#""extra":[{"type":"shoes men","results":["US 8.5","UK 7.5","EU 42","JP 26.5","CN 42"]},{"type":"shoes women","results":["US 10","UK 7.5","EU 42","JP 27","CN 42"]}]"#),
        "{}",
        json("42 eu shoes")
    );
}

#[test]
fn errors_and_what_stays_as_it_was() {
    let o = on("italy", "60 eu men shoes us");
    assert_eq!((o.status.code(), err(&o)), (Some(1), "aequalis: error: no EU 60 in men's shoes (EU 38–51)".to_string()));
    for request in ["12 us woman eu", "44 it dress us"] {
        assert_eq!(err(&on("italy", request)), "aequalis: error: only shoes for now", "{request}");
    }
    assert!(err(&on("italy", "42 eu nike shoes us")).starts_with("aequalis: error: expected <value>"), "brands are not words");
    // `in` with no size word: inches (an imperial machine asks for the target).
    assert_eq!(err(&on("us", "42 in")), "aequalis: error: now the length unit to convert to: mm, cm, m…");
    let it = notes("42 it men shoes us");
    assert!(it.contains(&"aequalis: note: Italian shoe sizes are EU sizes".to_string()), "{it:?}");
    for request in ["42 eu men shoes us", "42 eu shoes us", "42 eu shoes", "27 cm men foot"] {
        assert!(notes(request).contains(&BRAND.to_string()), "{request}: the brand note on every answer");
    }
}

// ---------------------------------------------------------------- beside a currency

/// Rates per US dollar, dated 12 Oct 2026.
const RATES: &str = r#"{"format":1,"synced_at":1791800000,"attempted_at":1791800000,"ok":true,"values":{
    "rates date":{"value":1791763351,"at":1791800000},"USD/EUR":{"value":0.8945,"at":1791800000},
    "USD/GBP":{"value":0.7556,"at":1791800000},"USD/JPY":{"value":158.6,"at":1791800000}}}"#;

fn config(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aequalis-sizes-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("aequalis")).unwrap();
    std::fs::write(dir.join("aequalis/currencies.json"), RATES).unwrap();
    std::fs::write(dir.join("aequalis/settings.json"), r#"{"offline": true}"#).unwrap();
    dir
}

/// The binary with the rates, offline, a few hours after them, in the time zone `zone`.
fn money(name: &str, zone: &str, request: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aequalis"))
        .arg("--now=1791800000")
        .args(request.split(' '))
        .env("XDG_CONFIG_HOME", config(name))
        .env("TZ", zone)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .expect("run aequalis")
}

#[test]
fn the_shoes_line_beside_a_currency() {
    let rome = |name: &str, request: &str| out(&money(name, "Europe/Rome", request));
    assert_eq!(rome("list", "42 eu us"), "- 42 EUR = 46.95 USD\n- shoes: 42 eu = US 8.5 men, US 10 women");
    assert_eq!(rome("between", "42.2 eu us"), "- 42.2 EUR = 47.18 USD\n- shoes: 42.2 eu = US 8.5 men, US 10 women");
    assert_eq!(rome("women-kids", "36 eu us"), "- 36 EUR = 40.25 USD\n- shoes: 36 eu = US 5.5 women, US 4Y kids");
    assert_eq!(rome("kids", "30 eu us"), "- 30 EUR = 33.54 USD\n- shoes: 30 eu = US 12.5C kids");
    assert_eq!(rome("men", "47 eu us"), "- 47 EUR = 52.54 USD\n- shoes: 47 eu = US 12.5 men");
    assert_eq!(rome("clarks", "50 eu us"), "- 50 EUR = 55.90 USD\n- shoes: 50 eu = US 15.5 men (Clarks)");
    // No line: an amount in no chart, a code that is no size system, one that is no currency.
    assert_eq!(rome("none", "100 eu us"), "111.79 USD");
    assert_eq!(rome("yen", "42 eu jpy"), "7446.84 JPY");
    assert!(err(&money("fr", "Europe/Rome", "42 eu fr")).contains("unknown unit"));
    // Programs: the currency is the result, the sizes the extra ones.
    let json = rome("json", "--json 42 eu us");
    assert!(json.starts_with(r#"{"ok":true,"result":"46.95 USD","extra":[{"type":"shoes","results":["US 8.5 men","US 10 women"]}],"#), "{json}");
}

#[test]
fn no_target_the_local_currency_and_the_regions_sizes() {
    // New York: dollars, US sizes.
    let o = money("new-york", "America/New_York", "42 eu");
    assert_eq!(out(&o), "- 42 EUR = 46.95 USD\n- shoes: 42 eu = US 8.5 men, US 10 women");
    // Rome: the euro is local (a hint), EU sizes already: no line.
    let o = money("rome", "Europe/Rome", "42 eu");
    assert_eq!((out(&o), err(&o)), (String::new(), "aequalis: error: now the currency to convert to: `42 EUR USD`".to_string()));
    // Dublin: the euro is local, UK sizes: the hint and the line.
    let o = money("dublin", "Europe/Dublin", "42 eu");
    assert_eq!(out(&o), "- shoes: 42 eu = UK 7.5 men, UK 7.5 women");
    let json = out(&money("dublin-json", "Europe/Dublin", "--json 42 eu"));
    assert!(json.contains(r#""incomplete":true,"extra":[{"type":"shoes","results":["UK 7.5 men","UK 7.5 women"]}]"#), "{json}");
}

#[test]
fn words_that_mean_something_else() {
    // KID, the Kiribati dollar (no rate for it here): a currency, no sizes.
    let kid = money("kid", "Europe/Rome", "42 kid us");
    assert!(err(&kid).contains("Kiribati dollar") && out(&kid).is_empty(), "{}", err(&kid));
    // Beside another size word, `kid` is the kids' word.
    assert_eq!(size("30 eu kid shoes us"), size("30 eu kids shoes us"));
}
