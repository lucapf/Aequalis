//! Shoe and clothing sizes through the binary: every case of `docs/SPEC_SIZES.md` §9 (phase 1) and
//! `docs/SPEC_CLOTHES.md` §9, each clothes case also in every phrasing that must give the same answer
//! (§8.1). A size alone runs on a simulated machine (`AEQUALIS_MACHINE`: `italy`, `us`), the same on
//! every laptop; a currency that could be a size (`42 eu us`) needs exchange rates, so it runs as the
//! currency tests do: prepared rates, offline, a fixed day, the region from `TZ`. Debug builds only
//! (the machine).
#![cfg(debug_assertions)]

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const BRAND: &str = "aequalis: note: sizes vary by brand: check the brand's chart";

fn on(machine: &str, request: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aequalis"))
        .args(request.split(' ').filter(|w| !w.is_empty()))
        .env("AEQUALIS_MACHINE", machine)
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
fn copy_every_person() {
    // The UIs' reply (`--as-typed`): Copy and the expression give every person's size, a line each.
    let view = |request: &str| out(&on("italy", &format!("--json --as-typed {request}")));
    let shoes = view("42 eu shoes us ");
    assert!(shoes.contains(r#""value":"Men US 8.5\nWomen US 10""#) && shoes.contains(r#""expression":"Men US 8.5\nWomen US 10""#), "{shoes}");
    let table = view("shoes 42 ");
    assert!(table.contains(r#""value":"Men US 8.5 · UK 7.5 · EU 42 · JP 26.5 · CN 42\nWomen US 10 · UK 7.5 · EU 42 · JP 27 · CN 42""#), "{table}");
    // One person: the first chart's value, as before; --json's result unchanged.
    assert!(view("42 eu men shoes us ").contains(r#""value":"8.5""#));
    assert!(out(&on("italy", "--json 42 eu shoes us")).contains(r#""result":"US 8.5 men""#));
}

#[test]
fn errors_and_what_stays_as_it_was() {
    let o = on("italy", "60 eu men shoes us");
    assert_eq!((o.status.code(), err(&o)), (Some(1), "aequalis: error: no EU 60 in men's shoes (EU 38–51)".to_string()));
    for request in ["34 bra eu", "7 hat us"] {
        assert_eq!(err(&on("italy", request)), "aequalis: error: only shoes and clothes for now", "{request}");
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
    assert_eq!(
        rome("list", "42 eu us"),
        "- 42 EUR = 46.95 USD\n- shoes: 42 eu = US 8.5 men, US 10 women\n- clothes: 42 eu = US 12 women, US 26 men's trousers, US 16.5 men's shirt"
    );
    assert_eq!(rome("women-kids", "36 eu us"), "- 36 EUR = 40.25 USD\n- shoes: 36 eu = US 5.5 women, US 4Y kids\n- clothes: 36 eu = US 6 women, US 14 men's shirt");
    assert_eq!(rome("kids", "30 eu us"), "- 30 EUR = 33.54 USD\n- shoes: 30 eu = US 12.5C kids\n- clothes: 30 eu = US 0 women");
    assert_eq!(rome("men", "47 eu us"), "- 47 EUR = 52.54 USD\n- shoes: 47 eu = US 12.5 men\n- clothes: 47 eu = US 18 women, US 38 men's suit, US 31 men's trousers");
    assert_eq!(rome("clarks", "50 eu us"), "- 50 EUR = 55.90 USD\n- shoes: 50 eu = US 15.5 men (Clarks)\n- clothes: 50 eu = US 40 men's suit, US 34 men's trousers");
    // No line: an amount in no chart, a code that is no size system.
    assert_eq!(rome("none", "100 eu us"), "111.79 USD");
    assert_eq!(rome("yen", "42 eu jpy"), "7446.84 JPY");
    // A code that is no currency: only sizes answer, they are the answer (SPEC_CLOTHES §3.4).
    assert_eq!(rome("fr", "42 eu fr"), "- clothes: 42 eu = FR 44 women");
    // Programs: the currency is the result, the sizes the extra ones.
    let json = rome("json", "--json 42 eu us");
    assert!(
        json.starts_with(
            r#"{"ok":true,"result":"46.95 USD","extra":[{"type":"shoes","results":["US 8.5 men","US 10 women"],"from":"42 eu"},{"type":"clothes","results":["US 12 women","US 26 men's trousers","US 16.5 men's shirt"],"from":"42 eu"}],"#
        ),
        "{json}"
    );
}

#[test]
fn no_target_the_local_currency_and_the_regions_sizes() {
    // New York: dollars, US sizes.
    let o = money("new-york", "America/New_York", "42 eu");
    assert_eq!(
        out(&o),
        "- 42 EUR = 46.95 USD\n- shoes: 42 eu = US 8.5 men, US 10 women\n- clothes: 42 eu = US 12 women, US 26 men's trousers, US 16.5 men's shirt"
    );
    // Rome: the euro is local (a hint); EU shoe sizes already, the women's clothes in IT sizes.
    let o = money("rome", "Europe/Rome", "42 eu");
    assert_eq!((out(&o), err(&o)), ("- clothes: 42 eu = IT 48 women".to_string(), "aequalis: error: now the currency to convert to: `42 EUR USD`".to_string()));
    // Dublin: the euro is local, UK sizes: the hint and the lines.
    let o = money("dublin", "Europe/Dublin", "42 eu");
    assert_eq!(out(&o), "- shoes: 42 eu = UK 7.5 men, UK 7.5 women\n- clothes: 42 eu = UK 16 women, UK 26 men's trousers, UK 16.5 men's shirt");
    let json = out(&money("dublin-json", "Europe/Dublin", "--json 42 eu"));
    assert!(json.contains(r#""incomplete":true,"extra":[{"type":"shoes","results":["UK 7.5 men","UK 7.5 women"],"from":"42 eu"},{"type":"clothes""#), "{json}");
}

#[test]
fn words_that_mean_something_else() {
    // KID, the Kiribati dollar (no rate for it here): a currency, no sizes.
    let kid = money("kid", "Europe/Rome", "42 kid us");
    assert!(err(&kid).contains("Kiribati dollar") && out(&kid).is_empty(), "{}", err(&kid));
    // Beside another size word, `kid` is the kids' word.
    assert_eq!(size("30 eu kid shoes us"), size("30 eu kids shoes us"));
}

// ---------------------------------------------------------------- clothes (SPEC_CLOTHES.md)

/// The adjectives and country names a system is also written as (SPEC_CLOTHES §2.4), a few per
/// language.
fn names_of(system: &str) -> &'static [&'static str] {
    match system {
        "it" => &["italian", "italiano", "italiana", "italien", "italienisch", "Italy", "italia", "włoski"],
        "us" => &["american", "americano", "USA", "amerikanisch", "américain", "amerika"],
        "eu" => &["european", "europeo", "Europe", "europäisch"],
        "uk" => &["british", "inglese", "britannique", "english"],
        "fr" => &["french", "francese", "français", "France"],
        "de" => &["german", "tedesco", "deutsch", "Germany"],
        "int" => &["international", "letters", "internazionale"],
        _ => &[],
    }
}

/// Every phrasing of `<size> <system> <item> <target>` that must give the same answer (§8.1): the
/// orders, the connectors, case, the attached system, the item's translations, fillers and
/// punctuation, the systems as countries and adjectives.
fn phrasings(size: &str, system: &str, items: &[&str], target: &str) -> Vec<String> {
    let item = items[0];
    let mut all = Vec::new();
    for c in ["", "to ", "in ", "-> ", "→ ", "= "] {
        all.push(format!("{size} {system} {item} {c}{target}"));
        all.push(format!("{item} {size} {system} {c}{target}"));
        all.push(format!("{system} {size} {item} {c}{target}"));
        all.push(format!("{item} {system}{size} {c}{target}"));
        all.push(format!("{size}{system} {item} {c}{target}"));
    }
    all.push(format!("{size} {} {} {}", system.to_uppercase(), item.to_uppercase(), target.to_uppercase()));
    all.extend(items.iter().map(|i| format!("{size} {system} {i} {target}")));
    all.push(format!("size {size} {system} {item} in {target}"));
    all.push(format!("taglia {size} {system} {item} {target}"));
    all.push(format!("my {item} size is {size} {system}, what is it in {target}?"));
    all.push(format!("what is {size} {system} {item} in {target}"));
    all.extend(names_of(system).iter().map(|n| format!("{item} {size} {n} to {target}")));
    all.extend(names_of(target).iter().map(|n| format!("{size} {system} {item} to {n}")));
    all
}

/// Every phrasing gives `expected`, on both machines.
fn same_answer(phrasings: &[String], expected: &str) {
    for p in phrasings {
        assert_eq!(size(p), expected, "{p}");
    }
}

#[test]
fn a_dress_in_every_phrasing() {
    let dress = ["dress", "vestito", "vestido", "robe", "Kleid", "sukienka", "elbise", "váy", "vay", "gaun", "abito", "dresses"];
    same_answer(&phrasings("44", "it", &dress, "us"), "US 8");
    assert_eq!(notes("44 it dress us"), ["aequalis: note: UK 12 · DE 38 · FR 40 · IT 44 · JP 11 · M · CN 165/92A", "aequalis: note: women's clothes", BRAND]);
    same_answer(&phrasings("8", "us", &["dress", "vestito"], "it"), "IT 44");
    same_answer(&phrasings("12", "uk", &["dress", "robe"], "fr"), "FR 40");
    // `eu` for women is DE, with a note.
    same_answer(&phrasings("38", "eu", &["dress"], "us"), "US 8");
    assert!(notes("38 eu dress us").contains(&"aequalis: note: EU sizes as in DE".to_string()));
}

#[test]
fn men_in_every_phrasing() {
    same_answer(&phrasings("40", "us", &["suit", "jacket", "blazer", "giacca", "Anzug", "veste", "marynarka"], "eu"), "EU 50");
    same_answer(&phrasings("40", "us", &["suit"], "int"), "L");
    same_answer(&phrasings("50", "it", &["suit"], "us"), "US 40");
    assert_eq!(size("40 us suit jp"), "JP L");
    assert_eq!(size("44 us suit int"), "XXL", "ASOS's chest ranges: 44 is XXL");
    // A person alone: the men's charts, a line each, by priority.
    assert_eq!(size("40 us man eu"), "EU 50 men's suit\nEU 56 men's trousers");
    assert_eq!(size("40 us uomo eu"), size("40 us man eu"));
}

#[test]
fn letters() {
    for p in ["M women top it", "women top M it", "m women top it", "M donna maglietta it", "top donna M in it", "M women top to italian", "women M top IT"] {
        assert_eq!(size(p), "IT 44", "{p}");
    }
    assert_eq!(size("clothes M it"), "IT 44 women\nIT 48 men's suit\nIT 48–49 men's trousers");
    assert_eq!(size("XL men shirt eu"), "EU 52", "a shirt's letter: the men's letters");
    assert_eq!(size("2XL dress us"), size("XXL dress us"));
    assert_eq!(size("XXL dress us"), "US 16–18", "a letter on two rows: a range");
    assert_eq!(size("LL jp suit eu"), "EU 54–56", "JP's LL");
    assert_eq!(size("S/M dress us"), "US 6", "the women's S/M row");
}

#[test]
fn ranges_cn_labels_and_waists() {
    assert_eq!(size("8-10 us dress it"), "IT 44–46");
    assert_eq!(size("8/10 us dress it"), "IT 44–46");
    assert_eq!(size("S-M women top it"), "IT 40–44");
    for p in ["160/92A dress us", "165/92a dress us", "dress cn 165/92A us", "165/92 dress to us"] {
        assert_eq!(size(p), "US 8", "{p}");
    }
    for p in ["W32 L34 trousers eu", "32x34 pants eu", "32/34 trousers eu", "32W trousers eu", "trousers 32 waist eu", "W32 pantaloni eu"] {
        assert_eq!(size(p), "EU 48", "{p}");
    }
    assert!(notes("W32 L34 trousers eu").contains(&"aequalis: note: inseam L34 has no EU size".to_string()));
    assert_eq!(size("32x34 pants it"), "IT 48");
}

#[test]
fn body_measures() {
    assert_eq!(out(&on("us", "92 cm bust")), "US 8");
    assert_eq!(out(&on("us", "bust 92 cm")), "US 8");
    assert_eq!(out(&on("us", "92cm bust")), "US 8");
    assert_eq!(out(&on("italy", "100 cm chest")), "IT 50");
    assert!(notes("100 cm chest").contains(&"aequalis: note: between US 38 and 40".to_string()));
    assert_eq!(out(&on("italy", "40 in chest")), "IT 50");
    assert_eq!(out(&on("italy", "40\" chest")), "IT 50");
    assert_eq!(out(&on("us", "81 cm waist")), "US 10 women\nUS 32 men's trousers");
    assert_eq!(out(&on("italy", "70 cm taille")), "IT 42 women\nIT 44 men's trousers", "`taille`: French for waist beside a measure");
    assert_eq!(out(&on("us", "39 cm collar")), "US 15.5");
    assert_eq!(size("15.5 in neck eu"), "EU 39");
    assert_eq!(size("39 cm collo us"), "US 15.5");
    assert_eq!(err(&on("us", "92 cm bust 75 cm waist")), "aequalis: error: one measure at a time");
    let beyond = notes("150 cm bust");
    assert!(beyond.contains(&"aequalis: note: beyond the chart".to_string()), "{beyond:?}");
}

#[test]
fn no_target_and_no_system() {
    assert_eq!(out(&on("italy", "vestito 44")), "IT 44");
    assert_eq!(err(&on("us", "vestito 44")), "aequalis: error: which system: it, fr, de");
    assert_eq!(out(&on("italy", "us 8 dress")), "IT 44");
    assert_eq!(out(&on("us", "dress M")), "US 8");
    assert_eq!(
        out(&on("italy", "clothes 44 it")),
        "women           US 8 · UK 12 · DE 38 · FR 40 · IT 44 · JP 11 · M · CN 165/92A\nmen's suit      US 34 · EU 44 · JP S · XS\nmen's trousers  US 28 · EU 44 · XS"
    );
}

#[test]
fn every_reading_by_priority() {
    // A person alone: clothes first, then the shoes.
    assert_eq!(size("12 us woman eu"), "- US 12 = EU 42\n- shoes: US 12 = EU 44.5 women");
    assert_eq!(size("42 eu women us"), "- EU 42 = US 12\n- shoes: EU 42 = US 10 women");
    // `size`: shoes first, then the clothes.
    assert_eq!(size("size 42 eu us"), "- EU 42 = US 8.5 men, US 10 women\n- clothes: EU 42 = US 12 women, US 26 men's trousers, US 16.5 men's shirt");
    assert_eq!(size("taglia 42 eu us"), size("size 42 eu us"));
    // No size word, nothing else answers: the sizes are the answer.
    assert_eq!(size("44 it us"), "- shoes: 44 it = US 10 men, US 11.5 women\n- clothes: 44 it = US 8 women, US 34 men's suit, US 28 men's trousers");
    assert_eq!(size("M it"), "- clothes: M = IT 44 women, IT 48 men's suit, IT 48–49 men's trousers");
    assert_eq!(size("44it us"), size("44 it us"));
}

#[test]
fn clothes_clashes_and_errors() {
    // Without a size word, everything keeps its meaning.
    assert_eq!(out(&on("italy", "160/84")), "1.9048");
    assert_eq!(err(&on("italy", "10 m")), "aequalis: error: now the length unit to convert to: in, ft, yd…");
    assert_eq!(out(&on("italy", "3 s ms")), "3000 ms");
    let o = on("italy", "clothes 128 kids");
    assert_eq!((o.status.code(), err(&o)), (Some(1), "aequalis: error: kids' clothes: not yet".to_string()));
    assert_eq!(
        err(&on("italy", "clothes 70 it")),
        "aequalis: error: no IT 70 in clothes (IT 36–54 women, IT 44–60 men's suit, IT 42–56 men's trousers)"
    );
    assert_eq!(err(&on("italy", "clothes")), "aequalis: error: now the size: 44 it, M, 8 us…");
    assert_eq!(err(&on("italy", "42 eu 44 shoes")), "aequalis: error: one size at a time");
    assert_eq!(err(&on("italy", "M shoes eu")), "aequalis: error: shoe sizes are numbers: 42 eu, 9.5 us, 4C");
    for request in ["44 it dress us", "clothes M it", "92 cm bust", "W32 trousers eu"] {
        assert!(notes(request).contains(&BRAND.to_string()), "{request}: the brand note on every answer");
    }
}

#[test]
fn clothes_json_and_copy() {
    let json = out(&on("italy", "--json clothes M it"));
    assert!(
        json.starts_with(r#"{"ok":true,"result":"IT 44 women","extra":[{"type":"clothes men's suit","results":["IT 48"]},{"type":"clothes men's trousers","results":["IT 48–49"]}],"#),
        "{json}"
    );
    let view = out(&on("italy", "--json --as-typed clothes M it "));
    assert!(view.contains(r#""value":"Women IT 44\nMen's suit IT 48\nMen's trousers IT 48–49""#), "{view}");
    assert!(out(&on("italy", "--json --as-typed 44 it dress us ")).contains(r#""value":"8""#));
}
