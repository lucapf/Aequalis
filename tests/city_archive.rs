//! The city archive through the binary: downloaded with `aequalis --sync` (from a local copy,
//! `cities.url` = file://…, so no network), then distances between any two cities, choices,
//! About, and the default unit.

mod common;

use std::path::PathBuf;
use std::process::Command;

/// A config dir whose `cities.url` points at the sample archive (next to its version), online but
/// with every refresh manual: nothing starts by itself.
fn config(name: &str) -> PathBuf {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cities-sample.json");
    let serve = std::env::temp_dir().join(format!("aequalis-archive-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&serve);
    std::fs::create_dir_all(&serve).unwrap();
    std::fs::copy(&fixture, serve.join("cities.json")).unwrap();
    std::fs::write(serve.join("cities.version"), "1791877958 sample\n").unwrap();
    let url = common::file_url(&serve.join("cities.json"));
    common::config(
        name,
        &[
            ("offline", "false"),
            ("cities.refresh", "manual"),
            ("rates.refresh", "manual"),
            ("live.refresh", "manual"),
            ("holidays", "false"),
            ("cities.url", &url),
            ("distance.unit", "km"),
        ],
    )
}

// The device's region: the C locale's on every machine (`AEQUALIS_MACHINE=plain`, debug builds
// only; a Mac reads its own setting, not `LC_ALL`).
#[cfg(debug_assertions)]
#[test]
fn download_then_any_city() {
    let dir = config("download");
    // Before: the built-in places only.
    let o = common::aequalis(&dir, &["Springfield, Illinois", "Chicago"]);
    assert_ne!(o.status.code(), Some(0), "{}", common::out(&o));
    // Everything: the archive (a local copy), the rates and live values (no network: none).
    let o = common::aequalis(&dir, &["--sync"]);
    assert_eq!(common::out(&o), "0 exchange rates\n0 live values\nholidays: off (the holidays setting)\n9 cities", "{}", common::err(&o));
    assert!(dir.join("aequalis/cities.json").exists());
    // One place so far (an archive city, alone or before `to`): what comes next, not "not a number".
    for typed in [&["Springfield,", "Illinois"][..], &["Springfield,", "Illinois", "to"], &["distance", "Springfield,", "Illinois"]] {
        let o = common::aequalis(&dir, typed);
        assert_eq!(common::err(&o), "aequalis: error: now the place to go to: `Springfield, Illinois to Paris`", "{typed:?}");
    }
    // Two parameters: the default unit; three: the given one.
    assert_eq!(common::out(&common::aequalis(&dir, &["Springfield,", "Illinois", "Chicago"])), "283 km");
    assert_eq!(common::out(&common::aequalis(&dir, &["Springfield,", "Illinois", "Chicago", "mi"])), "176 mi");
    assert_eq!(common::out(&common::aequalis(&dir, &["distance", "Рим", "to", "Chicago"])), "7765 km");
    // Several: asked (exit 2 in scripts, choices in --json), ordered: the capital (the default
    // choice), the exact name, the device's country (its time zone), then by population.
    let o = common::aequalis_in("Europe/Rome", &dir, &["Chicago", "Paris"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(common::err(&o).contains("`Paris` is several places: did you mean Paris - France or Paris - United States?"), "{}", common::err(&o));
    let o = common::aequalis_in("America/Chicago", &dir, &["Chicago", "Paris"]);
    assert!(common::err(&o).contains("did you mean Paris - France or Paris - United States?"), "in the US, the capital first: {}", common::err(&o));
    let json = common::out(&common::aequalis(&dir, &["--json", "Chicago", "Paris"]));
    assert!(json.contains(r#""kind":"place""#) && json.contains("google.com/maps?q="), "{json}");
    // -v: where they are.
    let o = common::aequalis(&dir, &["-v", "Springfield,", "Illinois", "Chicago"]);
    assert!(
        common::err(&o).contains("Chicago, Illinois, United States: 2746388 inhabitants, https://www.google.com/maps?q=41.85003,-87.65005"),
        "{}",
        common::err(&o)
    );
    // About: how many, from GeoNames.
    let about = common::out(&common::aequalis(&dir, &["--about"]));
    assert!(about.contains("Cities: 9 of 13 Oct 26, GeoNames (https://www.geonames.org), CC BY 4.0 (refresh: manual)"), "{about}");
    // Off: back to the built-in places.
    std::fs::write(
        dir.join("aequalis/settings.json"),
        std::fs::read_to_string(dir.join("aequalis/settings.json")).unwrap().replace("\"offline\"", "\"cities.enabled\": \"false\",\n  \"offline\""),
    )
    .unwrap();
    assert_ne!(common::aequalis(&dir, &["Springfield,", "Illinois", "Chicago"]).status.code(), Some(0));
    assert_eq!(common::out(&common::aequalis(&dir, &["Rome", "Paris"])), "1107 km");
}

// The locale (`LC_ALL`) is the device on Linux and Windows; a Mac reads its own setting
// (`locale::tests::macos_defaults`).
#[cfg(not(target_os = "macos"))]
#[test]
fn the_default_unit_follows_the_device() {
    let dir = common::config("unit", &[]);
    let run = |locale: &str| {
        let o = Command::new(env!("CARGO_BIN_EXE_aequalis")).args(["Rome", "Paris"]).env("XDG_CONFIG_HOME", &dir).env("LC_ALL", locale).output().unwrap();
        common::out(&o)
    };
    assert_eq!(run("C"), "1107 km");
    assert_eq!(run("en_US.UTF-8"), "688 mi");
    assert_eq!(run("it_IT.UTF-8"), "1107 km");
}

/// A group that fails does not keep the others from their turn: the archive can't be downloaded,
/// the exchange rates and live values are still synced; the failure is said, and the exit code 1.
#[test]
fn a_failed_download_stops_nothing_else() {
    let url = common::file_url(&std::env::temp_dir().join("nowhere").join("cities.json"));
    let dir = common::config(
        "failed",
        &[("offline", "false"), ("cities.refresh", "manual"), ("rates.refresh", "manual"), ("live.refresh", "manual"), ("holidays", "false"), ("cities.url", &url)],
    );
    let o = common::aequalis(&dir, &["--sync"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(common::out(&o), "0 exchange rates\n0 live values\nholidays: off (the holidays setting)");
    assert_eq!(common::err(&o), format!("aequalis: cities: could not download the city archive ({url})"));
    let json = common::out(&common::aequalis(&dir, &["--json", "--sync"]));
    assert!(json.starts_with(r#"{"ok":true,"result":"0 exchange rates\n0 live values\nholidays: off (the holidays setting)","notes":[],"warnings":["cities: could not download"#), "{json}");
}

#[test]
fn offline_no_download() {
    let dir = common::config("offline", &[]);
    let o = common::aequalis(&dir, &["--sync"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(common::err(&o), "aequalis: the offline setting is on: no sync");
    // No group: everything or nothing.
    let o = common::aequalis(&dir, &["--sync", "cities"]);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(common::err(&o), "aequalis: error: `--sync` downloads everything: no `cities` after it");
}
