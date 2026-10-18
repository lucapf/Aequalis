//! Windows: the zones' rules from the registry (no `TZDIR`), the ones the system clock follows, kept
//! up to date by Windows Update. ICU's own data, in the same Windows, can be years older: these
//! zones changed since 2022, and the answers are the registry's.

#![cfg(windows)]

use std::process::Command;

/// The request's answer, with the zone database the system's (no `TZDIR`).
fn run(request: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_aequalis"))
        .args(request.split_whitespace())
        .env_remove("TZDIR")
        .env("TZ", "Europe/Rome")
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    assert!(out.status.success(), "{request}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim_end().to_string()
}

#[test]
fn cities_by_the_systems_rules() {
    assert_eq!(run("2026-07-01T10:00 New York to Paris"), "2026-07-01T10:00-04:00 New York = 2026-07-01T16:00+02:00 Paris");
    assert_eq!(run("2026-12-04T10:00 New York to Paris"), "2026-12-04T10:00-05:00 New York = 2026-12-04T16:00+01:00 Paris");
    // Egypt has DST again since 2023.
    let cairo = run("2025-07-15T12:00 Rome to Cairo");
    assert!(cairo.ends_with("2025-07-15T13:00+03:00 Cairo"), "{cairo}");
    // Paraguay keeps UTC-03:00 all year since October 2024.
    let asuncion = run("2025-07-15T12:00 Rome to Asuncion");
    assert!(asuncion.contains("2025-07-15T07:00-03:00 "), "{asuncion}");
}
