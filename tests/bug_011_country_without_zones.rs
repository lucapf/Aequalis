//! Bug 011: `USA` (any country with several time zones) crashed aequalis when the zone files can't
//! be read (a Linux system or container without them, a wrong `TZDIR`). Found reading the code,
//! confirmed by the fuzzer (`fuzz/`, the configuration without zone files: `Indonesia`).
//!
//! The question lists one choice per zone, leaving out the zones that don't load; with none, it
//! had no choice, and its message took the first. Fixed: a country none of whose zones loads is
//! its first city, which then says which zone is missing, as `now Tokyo` does; and a question's
//! message never takes a choice that isn't there.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

fn run(tzdir: Option<&std::path::Path>, args: &str) -> (String, i32) {
    let mut c = Command::new(BIN);
    c.args(args.split_whitespace()).env("TZ", "UTC").env("LC_ALL", "C");
    if let Some(dir) = tzdir {
        c.env("TZDIR", dir);
    }
    let out = c.output().unwrap();
    (String::from_utf8(out.stderr).unwrap().trim_end().to_string(), out.status.code().unwrap_or(-1))
}

#[test]
fn no_zone_files() {
    let empty = std::env::temp_dir().join(format!("aequalis-bug-011-{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    for request in ["USA", "now USA", "10:00 USA to Japan", "Indonesia", "now Russia"] {
        let (err, code) = run(Some(&empty), request);
        assert_eq!(code, 2, "{request}: {err}");
        assert!(err.contains("is not installed"), "{request}: {err}");
    }
    std::fs::remove_dir_all(&empty).unwrap();
}

#[test]
fn with_zone_files_still_asked() {
    if !std::path::Path::new("/usr/share/zoneinfo/America/New_York").exists() {
        return;
    }
    let (err, code) = run(None, "now USA");
    assert_eq!(code, 2);
    assert!(err.contains("`USA` has several time zones"), "{err}");
}
