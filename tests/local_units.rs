//! A unit alone converts to the device's measurement system, as `12 usd` to the local currency:
//! `28 in` gives centimetres on a metric machine, `10 km` miles on an imperial one. A unit of the
//! device's own system still waits for its target. The system is the locale's (`LC_ALL`: the US
//! is imperial, everything else metric).

use std::process::{Command, Stdio};

fn run(locale: &str, request: &str) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_aequalis")).arg(request).env("LC_ALL", locale).stdin(Stdio::null()).output().expect("run aequalis");
    let text = if out.status.success() { out.stdout } else { out.stderr };
    (out.status.success(), String::from_utf8(text).unwrap().trim_end().to_string())
}

fn converted(locale: &str, request: &str) -> String {
    let (ok, text) = run(locale, request);
    assert!(ok, "{request} ({locale}): {text}");
    text
}

#[test]
fn imperial_to_metric_on_a_metric_machine() {
    assert_eq!(converted("C", "28 in"), "71.12 cm");
    assert_eq!(converted("it_IT.UTF-8", "5 ft"), "1.524 m");
    assert_eq!(converted("C", "3 lb"), "1.3608 kg");
    assert_eq!(converted("C", "70 F"), "21.1111 C", "°F: Celsius, not kelvin");
    assert_eq!(converted("C", "60 mph"), "96.5606 km/h");
    assert_eq!(converted("C", "1 gal-us"), "3.7854 l");
}

#[test]
fn metric_to_imperial_on_an_imperial_machine() {
    assert_eq!(converted("en_US.UTF-8", "10 km"), "6.2137 mi");
    assert_eq!(converted("en_US.UTF-8", "100 km/h"), "62.1371 mph", "km/h: miles an hour, not feet a second");
    assert_eq!(converted("en_US.UTF-8", "20 C"), "68 F");
    assert_eq!(converted("en_US.UTF-8", "1000 ml"), "33.814 floz-us", "the US units, not the UK ones");
}

#[test]
fn the_device_system_waits_for_a_target() {
    let (ok, text) = run("C", "10 km");
    assert!(!ok && text.contains("now the length unit to convert to"), "{text}");
    let (ok, text) = run("C", "28 in to");
    assert!(!ok && text.contains("now the length unit to convert to"), "a target announced: {text}");
    // Angles and durations have no metric and imperial.
    assert!(!run("C", "30 deg").0);
    assert!(!run("en_US.UTF-8", "10 min").0);
    // Written out, the target is the one written.
    assert_eq!(converted("C", "28 in mm"), "711.2 mm");
}
