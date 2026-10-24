//! Bug 018: `now New York Tokyo` showed the time in Tokyo only: the first city was read as the zone
//! the request is written in, as in `10:00AM New York Tokyo`. With `now` there is no time written
//! in a zone, so every city is a target: the time now in each, one line per city.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

fn run(args: &str) -> (String, String, i32) {
    let out = Command::new(BIN).args(["--now", "1792222391"]).args(args.split_whitespace()).env("LC_ALL", "C").output().unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

#[test]
fn every_city_is_a_target() {
    let (out, err, code) = run("now New York Tokyo");
    assert_eq!(code, 0, "{err}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "{out}");
    assert!(lines[0].ends_with("GMT-4 New York"), "{out}");
    assert!(lines[1].ends_with("GMT+9 Tokyo"), "{out}");
}

#[test]
fn any_number_of_cities_each_once() {
    assert_eq!(run("now New York Tokyo Paris").0.lines().count(), 3);
    assert_eq!(run("now Tokyo Tokyo").0, run("now Tokyo").0);
}

#[test]
fn a_chosen_zone_of_a_country_and_another_country() {
    // `now usa japan` asks which US zone; each choice is a request of this kind.
    let (out, err, code) = run("now New York Japan");
    assert_eq!(code, 0, "{err}");
    assert_eq!(out.lines().count(), 2, "{out}");
    assert!(out.contains("New York") && out.contains("Tokyo"), "{out}");
}

#[test]
fn to_between_cities_changes_nothing() {
    assert_eq!(run("now New York to Tokyo").0, run("now New York Tokyo").0);
    assert_eq!(run("now New York -> Tokyo").0, run("now New York Tokyo").0);
}

#[test]
fn a_time_of_day_still_has_a_source() {
    let (out, err, code) = run("10:00AM New York Tokyo");
    assert_eq!(code, 0, "{err}");
    assert_eq!(out.lines().count(), 1, "{out}");
    assert!(out.ends_with("Tokyo"), "{out}");
}
