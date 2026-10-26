//! The calendar through the binary: `cal`, `cal march 2027`, `cal 2027`, `cal december UAE,USA,Italy`,
//! the public holidays downloaded by the sync from a `file://` archive (`holidays.url`), named in the
//! locale's language; none before the first sync; `--json` for the apps.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// 12 Oct 2026, a Monday.
const NOW: i64 = 1_791_800_000;

const ARCHIVE: &str = r#"{"format":1,"built":"2026-10-12","years":[2025,2026,2027],"countries":{
    "IT":{"default":"it","days":["2026-11-01","2026-12-08","2026-12-25","2026-12-26"],
          "names":{"en":["All Saints' Day","Immaculate Conception","Christmas Day","Saint Stephen's Day"],
                   "it":["Ognissanti","Immacolata Concezione","Natale","Santo Stefano"]}},
    "AE":{"default":"ar","days":["2026-12-02","2026-12-03"],"names":{"ar":["اليوم الوطني","اليوم الوطني"],"en":["National Day","National Day"]}},
    "US":{"default":"en","days":["2026-11-26","2026-12-25"],"names":{"en":["Thanksgiving Day","Christmas Day"]}}}}"#;

/// A config directory whose settings point the holidays at a `file://` copy of `ARCHIVE`, the other
/// groups manual.
fn config(name: &str, extra: &[(&str, &str)]) -> PathBuf {
    let served = std::env::temp_dir().join(format!("aequalis-calendar-archive-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&served);
    std::fs::create_dir_all(&served).unwrap();
    std::fs::write(served.join("holidays.json"), ARCHIVE).unwrap();
    let url = common::file_url(&served.join("holidays.json"));
    let mut settings = vec![
        ("offline", "false"),
        ("rates.refresh", "manual"),
        ("live.refresh", "manual"),
        ("cities.refresh", "manual"),
        ("cities.enabled", "false"),
        ("holidays.refresh", "manual"),
        ("holidays.url", url.as_str()),
    ];
    settings.extend_from_slice(extra);
    common::config(name, &settings)
}

/// The binary on 12 Oct 2026 in Rome, in `locale`.
fn run(dir: &Path, locale: &str, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_aequalis"));
    for var in ["http_proxy", "https_proxy", "HTTPS_PROXY", "ALL_PROXY", "all_proxy"] {
        command.env(var, "http://127.0.0.1:9");
    }
    command
        .arg(format!("--now={NOW}"))
        .args(args)
        .env("XDG_CONFIG_HOME", dir)
        .env("TZ", "Europe/Rome")
        .env("LC_ALL", locale)
        .stdin(Stdio::null())
        .output()
        .expect("run aequalis")
}

fn ok(dir: &Path, locale: &str, request: &str) -> String {
    let o = run(dir, locale, &request.split(' ').collect::<Vec<_>>());
    assert!(o.status.success(), "{request}: {}", common::err(&o));
    common::out(&o)
}

#[test]
fn a_month_like_cal() {
    let dir = config("month", &[]);
    // Rome: the week starts on Monday; this month, with its ISO weeks.
    let month = ok(&dir, "C", "cal");
    assert!(month.starts_with("      October 2026\nWk Mo Tu We Th Fr Sa Su\n40           1  2  3  4\n41  5  6  7  8  9 10 11\n"), "{month}");
    assert_eq!(ok(&dir, "C", "calendar"), month);
    assert!(ok(&dir, "C", "cal march 2027").starts_with("       March 2027\nWk Mo Tu We Th Fr Sa Su\n 9  1  2  3  4  5  6  7\n"));
    assert_eq!(ok(&dir, "C", "cal 2027-03"), ok(&dir, "C", "cal march 2027"));
    assert!(ok(&dir, "C", "cal next month").starts_with("     November 2026"));
    // The US's region: from Sunday.
    assert!(ok(&dir, "en_US.UTF-8", "cal march 2027").contains("\nWk Su Mo Tu We Th Fr Sa\n"));
    // The setting wins.
    let dir = config("month-saturday", &[("calendar.week_start", "saturday")]);
    assert!(ok(&dir, "C", "cal march 2027").contains("\nWk Sa Su Mo Tu We Th Fr\n"));
}

#[test]
fn a_year_three_months_a_row() {
    let dir = config("year", &[]);
    let year = ok(&dir, "C", "cal 2027");
    let lines: Vec<&str> = year.lines().collect();
    assert_eq!(lines[0].trim(), "2027");
    assert!(lines.iter().any(|l| l.contains("January 2027") && l.contains("February 2027") && l.contains("March 2027")), "{year}");
    assert!(lines.iter().any(|l| l.contains("October 2027") && l.contains("December 2027")), "{year}");
}

#[test]
fn holidays_come_with_the_sync() {
    let dir = config("sync", &[("holidays.countries", "IT")]);
    // Before the first sync: none, and only `-v` says why.
    let before = ok(&dir, "C", "cal december");
    assert!(!before.contains("Italy"), "{before}");
    let o = run(&dir, "C", &["-v", "cal", "december"]);
    assert!(common::err(&o).contains("the public holidays are not downloaded yet"), "{}", common::err(&o));
    let o = run(&dir, "C", &["--sync"]);
    assert!(common::out(&o).contains("3 countries' holidays, 2025-2027"), "{}", common::out(&o));
    assert!(dir.join("aequalis/holidays.json").exists());
    // The setting's country, named in English in the C locale, in Italian in Italy's.
    let december = ok(&dir, "C", "cal december");
    assert!(december.contains("IT Italy\n 8 Dec  IT  Immaculate Conception\n25 Dec  IT  Christmas Day\n26 Dec  IT  Saint Stephen's Day"), "{december}");
    // Without `cal`: a month first, then the countries.
    assert_eq!(ok(&dir, "C", "dec"), december);
    assert_eq!(ok(&dir, "C", "december italy"), ok(&dir, "C", "cal december italy"));
    assert!(ok(&dir, "it_IT.UTF-8", "cal december").contains("25 Dec  IT  Natale"));
    // The settings window says what is held.
    assert!(common::out(&run(&dir, "C", &["--json", "--settings"])).contains(r#""key":"holidays","value":"true""#));
}

#[test]
fn several_countries_a_colour_each() {
    let dir = config("several", &[]);
    run(&dir, "C", &["--sync"]);
    let december = ok(&dir, "C", "cal december UAE,USA,Italy");
    assert!(december.contains("AE United Arab Emirates · US United States · IT Italy"), "{december}");
    // A holiday of several days in a row: one line, its days together.
    assert!(december.contains("2-3 Dec  AE  National Day (2 days)\n  8 Dec  IT  Immaculate Conception\n 25 Dec  US  Christmas Day\n 25 Dec  IT  Christmas Day"), "{december}");
    // Spaces, codes and names alike.
    assert_eq!(ok(&dir, "C", "cal december uae usa italy"), december);
    assert_eq!(ok(&dir, "C", "cal december ae us it"), december);
    assert_eq!(ok(&dir, "C", "cal december united arab emirates, united states, italy"), december);
    // A country the archive lacks: no holidays for it, `-v` says so.
    let o = run(&dir, "C", &["-v", "cal", "december", "france"]);
    assert!(common::err(&o).contains("no public holidays for France"), "{}", common::err(&o));
}

#[test]
fn for_the_apps() {
    let dir = config("json", &[]);
    run(&dir, "C", &["--sync"]);
    let json = common::out(&run(&dir, "C", &["--json", "cal", "december", "italy"]));
    assert!(json.starts_with(r#"{"ok":true,"result":"     December 2026\n"#), "{json}");
    assert!(json.contains(r#""calendar":{"title":"December 2026","year":false,"today":"2026-10-12","weekdays":["Mo","Tu","We","Th","Fr","Sa","Su"],"weekend":[5,6],"countries":[{"code":"IT","name":"Italy"}]"#), "{json}");
    assert!(json.contains(r#"{"date":"2026-12-25","day":25,"month":12,"country":0,"name":"Christmas Day","first":"2026-12-25","length":1,"days":"25 Dec"}"#), "{json}");
    let json = common::out(&run(&dir, "C", &["--json", "dec", "uae"]));
    assert!(json.contains(r#"{"date":"2026-12-03","day":3,"month":12,"country":0,"name":"National Day","first":"2026-12-02","length":2,"days":"2-3 Dec"}"#), "{json}");
}

#[test]
fn off_and_wrong_words() {
    let dir = config("off", &[("holidays", "false")]);
    assert!(common::out(&run(&dir, "C", &["--sync"])).contains("holidays: off (the holidays setting)"));
    assert!(!dir.join("aequalis/holidays.json").exists(), "off: nothing downloaded");
    let o = run(&dir, "C", &["-v", "cal", "december", "italy"]);
    assert!(common::err(&o).contains("the public holidays are off: turn on the holidays setting"), "{}", common::err(&o));
    let o = run(&dir, "C", &["cal", "blorp"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(common::err(&o).contains("`blorp` is not a month, a year or a country"), "{}", common::err(&o));
    let o = run(&dir, "C", &["cal", "it", "fr", "de", "es", "pt", "nl"]);
    assert!(common::err(&o).contains("at most 5 countries at a time"));
    // The countries' setting: checked, written as codes.
    assert!(common::err(&run(&dir, "C", &["--set", "holidays.countries=Narnia"])).contains("`Narnia` is not a country"));
    assert!(run(&dir, "C", &["--set", "holidays.countries=Italy, UAE"]).status.success());
    assert!(std::fs::read_to_string(dir.join("aequalis/settings.json")).unwrap().contains(r#""holidays.countries": "IT,AE""#));
}

#[test]
fn a_calorie_is_still_a_unit() {
    let dir = config("calorie", &[]);
    assert_eq!(ok(&dir, "C", "1 kcal cal"), "1000 cal");
    let o = Command::new(env!("CARGO_BIN_EXE_aequalis"))
        .args(["cal", "J"])
        .env("XDG_CONFIG_HOME", &dir)
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(b"1\n")?;
            child.wait_with_output()
        })
        .unwrap();
    assert_eq!(common::out(&o), "4.184 J", "a value on stdin: a unit, not a calendar");
}

#[test]
fn a_day_and_month_without_a_year_are_this_years() {
    let dir = config("no-year", &[]);
    // 12 Oct 2026: `26 oct` is 26 Oct 2026, and date math works on it.
    assert_eq!(ok(&dir, "C", "26 oct"), ok(&dir, "C", "26 oct 2026"));
    assert_eq!(ok(&dir, "C", "26 oct + 3wd"), "29/10/2026 DD/MM/YYYY");
    assert_eq!(ok(&dir, "C", "26 October - 2d"), "24/10/2026 DD/MM/YYYY");
    assert_eq!(ok(&dir, "C", "oct 26th + 3wd"), "10/29/2026 MM/DD/YYYY");
    assert_eq!(ok(&dir, "C", "1st of may + 2d"), "03/05/2026 DD/MM/YYYY");
    // With a year, as before; `oct 26`, month first and alone, is the calendar.
    assert_eq!(ok(&dir, "C", "12 Sept 2026 + 5wd"), "18/09/2026 DD/MM/YYYY");
    assert!(ok(&dir, "C", "oct 26").contains("October 2026\nWk "));
}
