//! The city archive through the binary: downloaded with `tally --sync` (from a local copy,
//! `cities.url` = file://…, so no network), then distances between any two cities, choices,
//! About, and the default unit.

mod common;

use std::path::PathBuf;
use std::process::Command;

/// A config dir whose `cities.url` points at the sample archive (gzipped next to its version),
/// online but with every refresh manual: nothing starts by itself.
fn config(name: &str) -> PathBuf {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cities-sample.json");
    let serve = std::env::temp_dir().join(format!("tally-archive-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&serve);
    std::fs::create_dir_all(&serve).unwrap();
    let gz = std::fs::File::create(serve.join("cities.json.gz")).unwrap();
    assert!(Command::new("gzip").arg("-c").arg(&fixture).stdout(gz).status().unwrap().success());
    std::fs::write(serve.join("cities.version"), "1791877958 sample\n").unwrap();
    let url = format!("file://{}", serve.join("cities.json.gz").display());
    common::config(
        name,
        &[("offline", "false"), ("cities.refresh", "manual"), ("rates.refresh", "manual"), ("live.refresh", "manual"), ("cities.url", &url), ("distance.unit", "km")],
    )
}

#[test]
fn download_then_any_city() {
    let dir = config("download");
    // Before: the built-in places only.
    let o = common::tally(&dir, &["Springfield, Illinois", "Chicago"]);
    assert_ne!(o.status.code(), Some(0), "{}", common::out(&o));
    // Everything: the archive (a local copy), the rates and live values (no network: none).
    let o = common::tally(&dir, &["--sync"]);
    assert_eq!(common::out(&o), "9 cities\n0 exchange rates\n0 live values", "{}", common::err(&o));
    assert!(dir.join("tally/cities.json").exists());
    // One place so far (an archive city, alone or before `to`): what comes next, not "not a number".
    for typed in [&["Springfield,", "Illinois"][..], &["Springfield,", "Illinois", "to"], &["distance", "Springfield,", "Illinois"]] {
        let o = common::tally(&dir, typed);
        assert_eq!(common::err(&o), "tally: error: now the place to go to: `Springfield, Illinois to Paris`", "{typed:?}");
    }
    // Two parameters: the default unit; three: the given one.
    assert_eq!(common::out(&common::tally(&dir, &["Springfield,", "Illinois", "Chicago"])), "283 km");
    assert_eq!(common::out(&common::tally(&dir, &["Springfield,", "Illinois", "Chicago", "mi"])), "176 mi");
    assert_eq!(common::out(&common::tally(&dir, &["distance", "Рим", "to", "Chicago"])), "7765 km");
    // Several: asked (exit 2 in scripts, choices in --json), ordered: the exact name, then the
    // device's country (its time zone), then by population.
    let o = common::tally_in("Europe/Rome", &dir, &["Chicago", "Paris"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(common::err(&o).contains("`Paris` is several places: did you mean Paris - France or Paris - United States?"), "{}", common::err(&o));
    let o = common::tally_in("America/Chicago", &dir, &["Chicago", "Paris"]);
    assert!(common::err(&o).contains("did you mean Paris - United States or Paris - France?"), "in the US: {}", common::err(&o));
    let json = common::out(&common::tally(&dir, &["--json", "Chicago", "Paris"]));
    assert!(json.contains(r#""kind":"place""#) && json.contains("google.com/maps?q="), "{json}");
    // -v: where they are.
    let o = common::tally(&dir, &["-v", "Springfield,", "Illinois", "Chicago"]);
    assert!(common::err(&o).contains("Chicago, Illinois, United States: 2746388 inhabitants, https://www.google.com/maps?q=41.85003,-87.65005"), "{}", common::err(&o));
    // About: how many, from GeoNames.
    let about = common::out(&common::tally(&dir, &["--about"]));
    assert!(about.contains("Cities: 9 of 13 Oct 26, GeoNames (https://www.geonames.org), CC BY 4.0 (refresh: manual)"), "{about}");
    // Off: back to the built-in places.
    std::fs::write(dir.join("tally/settings.json"), std::fs::read_to_string(dir.join("tally/settings.json")).unwrap().replace("\"offline\"", "\"cities.enabled\": \"false\",\n  \"offline\"")).unwrap();
    assert_ne!(common::tally(&dir, &["Springfield,", "Illinois", "Chicago"]).status.code(), Some(0));
    assert_eq!(common::out(&common::tally(&dir, &["Rome", "Paris"])), "1107 km");
}

#[test]
fn the_default_unit_follows_the_device() {
    let dir = common::config("unit", &[]);
    let run = |locale: &str| {
        let o = Command::new(env!("CARGO_BIN_EXE_tally")).args(["Rome", "Paris"]).env("XDG_CONFIG_HOME", &dir).env("LC_ALL", locale).output().unwrap();
        common::out(&o)
    };
    assert_eq!(run("C"), "1107 km");
    assert_eq!(run("en_US.UTF-8"), "688 mi");
    assert_eq!(run("it_IT.UTF-8"), "1107 km");
}

#[test]
fn offline_no_download() {
    let dir = common::config("offline", &[]);
    let o = common::tally(&dir, &["--sync"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(common::err(&o), "tally: the offline setting is on: no sync");
    // No group: everything or nothing.
    let o = common::tally(&dir, &["--sync", "cities"]);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(common::err(&o), "tally: error: `--sync` downloads everything: no `cities` after it");
}
