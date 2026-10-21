//! A unit alone converts to the device's measurement system, as `12 usd` to the local currency:
//! `28 in` gives centimetres on a metric machine, `10 km` miles on an imperial one. A unit of the
//! device's own system still waits for its target. The machine is simulated (`AEQUALIS_MACHINE`,
//! `machine::Mock`): the same results on every laptop. Debug builds only: a release binary ignores
//! `AEQUALIS_MACHINE` and reads the real machine.
#![cfg(debug_assertions)]

use std::process::{Command, Stdio};

fn run(machine: &str, request: &str) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_aequalis")).arg(request).env("AEQUALIS_MACHINE", machine).stdin(Stdio::null()).output().expect("run aequalis");
    let text = if out.status.success() { out.stdout } else { out.stderr };
    (out.status.success(), String::from_utf8(text).unwrap().trim_end().to_string())
}

fn converted(machine: &str, request: &str) -> String {
    let (ok, text) = run(machine, request);
    assert!(ok, "{request} ({machine}): {text}");
    text
}

#[test]
fn imperial_to_metric_on_a_metric_machine() {
    assert_eq!(converted("italy", "28 in"), "71.12 cm");
    assert_eq!(converted("switzerland", "5 ft"), "1.524 m");
    assert_eq!(converted("italy", "3 lb"), "1.3608 kg");
    assert_eq!(converted("italy", "70 F"), "21.1111 C", "°F: Celsius, not kelvin");
    assert_eq!(converted("italy", "60 mph"), "96.5606 km/h");
    assert_eq!(converted("italy", "1 gal-us"), "3.7854 l");
}

#[test]
fn metric_to_imperial_on_an_imperial_machine() {
    assert_eq!(converted("us", "10 km"), "6.2137 mi");
    assert_eq!(converted("us", "100 km/h"), "62.1371 mph", "km/h: miles an hour, not feet a second");
    assert_eq!(converted("us", "20 C"), "68 F");
    assert_eq!(converted("us", "1000 ml"), "33.814 floz-us", "the US units, not the UK ones");
}

#[test]
fn the_device_system_waits_for_a_target() {
    let (ok, text) = run("italy", "10 km");
    assert!(!ok && text.contains("now the length unit to convert to"), "{text}");
    let (ok, text) = run("italy", "28 in to");
    assert!(!ok && text.contains("now the length unit to convert to"), "a target announced: {text}");
    // Angles and durations have no metric and imperial.
    assert!(!run("italy", "30 deg").0);
    assert!(!run("us", "10 min").0);
    // Written out, the target is the one written.
    assert_eq!(converted("italy", "28 in mm"), "711.2 mm");
}

#[test]
fn the_date_order_is_the_machines() {
    // A day and a month that could be either way round: the machine's order is offered first.
    let first = |machine: &str| run(machine, "03/04/2026 iso").1;
    assert!(first("us").contains("ambiguous: MM/DD/YYYY (2026-03-04) or DD/MM/YYYY"), "{}", first("us"));
    assert!(first("italy").contains("ambiguous: DD/MM/YYYY (2026-04-03) or MM/DD/YYYY"), "{}", first("italy"));
    assert!(first("japan").contains("ambiguous: DD/MM/YYYY"), "year first: day/month after it, {}", first("japan"));
}

#[test]
fn each_run_its_own_folder() {
    let out = Command::new(env!("CARGO_BIN_EXE_aequalis")).arg("--settings").env("AEQUALIS_MACHINE", "japan").stdin(Stdio::null()).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("aequalis-machine-") && text.contains("-japan-"), "a fresh folder, not the laptop's: {text}");
}
