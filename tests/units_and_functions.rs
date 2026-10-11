//! Speed, pressure, fuel economy, energy and power; compound units (`km/h`, `m/s^2`, `Mb/s`); and the
//! calculator's functions and constants (`sqrt`, `log`, `ln`, `sin`…, `e`, `pi`). Run through the
//! real binary, with stdin not a terminal (as from the bar widget), so a function name first is
//! a value and stdin is not read.

use std::process::{Command, Output, Stdio};

fn tally(args: &[&str]) -> Output {
    // The plain number format, whatever the machine's locale.
    Command::new(env!("CARGO_BIN_EXE_tally")).args(args).env("LC_ALL", "C").stdin(Stdio::null()).output().expect("run tally")
}

fn out(args: &[&str]) -> String {
    let out = tally(args);
    assert!(out.status.success(), "{args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim_end().to_string()
}

fn err(args: &[&str]) -> (i32, String) {
    let out = tally(args);
    (out.status.code().unwrap(), String::from_utf8(out.stderr).unwrap().trim_end().to_string())
}

#[test]
fn new_categories() {
    assert_eq!(out(&["100", "km/h", "mph"]), "62.1371 mph");
    assert_eq!(out(&["100km/h", "m/s"]), "27.7778 m/s");
    assert_eq!(out(&["30", "knots", "km/h"]), "55.56 km/h");
    assert_eq!(out(&["1", "bar", "psi"]), "14.5038 psi");
    assert_eq!(out(&["1013.25", "hPa", "atm"]), "1 atm");
    assert_eq!(out(&["5", "l/100km", "mpg"]), "47.0429 mpg");
    assert_eq!(out(&["40", "mpg", "l/100km"]), "5.8804 l/100km");
    assert_eq!(out(&["1", "kWh", "MJ"]), "3.6 MJ");
    assert_eq!(out(&["100", "hp", "kW"]), "74.57 kW");
    // Heart rate is not supported.
    assert_eq!(err(&["60", "bpm", "Hz"]).0, 1);
}

#[test]
fn compound_units() {
    assert_eq!(out(&["100", "kilometers/hour", "mi/h"]), "62.1371 mph");
    assert_eq!(out(&["9.81", "m/s^2", "ft/s2"]), "32.185 ft/s2");
    assert_eq!(out(&["100", "Mb/s", "MB/s"]), "12.5 MB/s");
    assert_eq!(out(&["20", "km/l", "mi/gal"]), "47.0429 mpg");
    // Piped, like any request.
    let mut child = Command::new(env!("CARGO_BIN_EXE_tally"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child.stdin.take().unwrap().write_all(b"100 km/h mph\n").unwrap();
    assert_eq!(String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap(), "62.1371 mph\n");
}

#[test]
fn unit_errors() {
    assert_eq!(err(&["5", "foo/h", "mph"]), (1, "tally: error: unknown unit `foo` (see `tally --list`)".into()));
    assert_eq!(err(&["10", "C/s", "K/s"]), (1, "tally: error: `C` can't be part of a compound unit".into()));
    assert_eq!(err(&["1", "ha", "l/100km"]).0, 1);
    assert_eq!(err(&["0", "mpg", "l/100km"]), (1, "tally: error: division by zero".into()));
}

#[test]
fn functions_and_constants() {
    assert_eq!(out(&["sqrt(16)"]), "4");
    assert_eq!(out(&["sqrt", "16"]), "4");
    assert_eq!(out(&["log(1000)"]), "3");
    assert_eq!(out(&["log10(1000)"]), "3");
    assert_eq!(out(&["ln(e)"]), "1");
    assert_eq!(out(&["loge(e^3)"]), "3");
    assert_eq!(out(&["e"]), "2.7183");
    assert_eq!(out(&["2", "x", "pi"]), "6.2832");
    assert_eq!(out(&["sin(30°)"]), "0.5");
    assert_eq!(out(&["cos(pi)"]), "-1");
    assert_eq!(out(&["tan(45deg)"]), "1");
    assert_eq!(out(&["sinh(1)", "--precision", "6"]), "1.175201");
    assert_eq!(out(&["cosh(1)"]), "1.5431");
    assert_eq!(out(&["tanh(1)"]), "0.7616");
    // With units.
    assert_eq!(out(&["sqrt(2)", "km", "mi"]), "0.8788 mi");
    // The JSON mode the widget uses.
    let json = String::from_utf8(tally(&["--json", "sqrt(16)"]).stdout).unwrap();
    assert!(json.starts_with(r#"{"ok":true,"result":"4","#), "{json}");
}

#[test]
fn function_errors() {
    assert_eq!(err(&["sqrt(-1)"]), (1, "tally: error: `sqrt` of a negative number (-1)".into()));
    assert_eq!(err(&["log(0)"]), (1, "tally: error: `log` of a number that is not positive (0)".into()));
}

#[test]
fn places_without_a_time_are_a_distance() {
    // Two places and nothing else: the distance along the Earth's surface.
    // Whole kilometres (or miles).
    assert_eq!(out(&["Rome", "to", "Doha"]), "4019 km");
    assert_eq!(out(&["Rome", "to", "Doha", "mi"]), "2497 mi");
    assert_eq!(out(&["Rome", "to", "Doha", "in", "mi"]), "2497 mi");
    assert_eq!(out(&["distance", "Rome", "to", "Doha"]), "4019 km");
    // With a time (or now): the time in Doha when it is that time in Rome, with AM / PM.
    assert!(out(&["10:00", "Rome", "to", "Doha"]).ends_with(" 11:00 AM GMT+3 Doha"));
    assert!(out(&["10:00PM", "Rome", "to", "Doha"]).ends_with(" 11:00 PM GMT+3 Doha"));
    assert!(out(&["now", "Rome", "to", "Doha"]).ends_with(" GMT+3 Doha"));
    let now = out(&["now", "Rome", "to", "Doha"]);
    assert!(now.contains(" AM ") || now.contains(" PM "), "{now}");
}

#[test]
fn distances_use_the_desktop_thousands_separator() {
    // Only where the locale is installed (`locale -a`).
    let installed = Command::new("locale").arg("-a").output().map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase()).unwrap_or_default();
    if !installed.contains("en_us.utf8") {
        return;
    }
    let run = |locale: &str, args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_tally")).args(args).env("LC_ALL", locale).stdin(Stdio::null()).output().unwrap();
        String::from_utf8(out.stdout).unwrap().trim_end().to_string()
    };
    assert_eq!(run("en_US.UTF-8", &["Rome", "to", "Doha"]), "4,019 km");
    assert_eq!(run("en_US.UTF-8", &["Rome", "to", "Tokyo"]), "9,875 km");
    assert_eq!(run("C", &["Rome", "to", "Tokyo"]), "9875 km");
}
