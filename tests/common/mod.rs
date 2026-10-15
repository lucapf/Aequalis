//! For the tests that run the binary: a config directory of their own, with `settings.json`
//! holding the settings they need (offline unless they say otherwise: no test uses the network).
#![allow(dead_code)]

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

/// `$XDG_CONFIG_HOME` for one test: `tally/settings.json` with `settings` (and `offline: true`
/// unless given).
pub fn config(name: &str, settings: &[(&str, &str)]) -> PathBuf {
    // Unique: tests run in parallel.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tally-test-{}-{name}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("tally")).unwrap();
    let mut all: Vec<(&str, &str)> = settings.to_vec();
    if !all.iter().any(|(k, _)| *k == "offline") {
        all.push(("offline", "true"));
    }
    let body: Vec<String> = all.iter().map(|(k, v)| format!("  \"{k}\": \"{v}\"")).collect();
    std::fs::write(dir.join("tally/settings.json"), format!("{{\n{}\n}}\n", body.join(",\n"))).unwrap();
    dir
}

/// The binary with that config directory, the C locale, no input; and a proxy nothing answers,
/// so that a sync online (`--sync` downloads everything) fails at once for the network sources
/// (`file://` archives still work).
pub fn tally(dir: &PathBuf, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tally"));
    for var in ["http_proxy", "https_proxy", "HTTPS_PROXY", "ALL_PROXY", "all_proxy"] {
        command.env(var, "http://127.0.0.1:9");
    }
    command.args(args).env("XDG_CONFIG_HOME", dir).env("LC_ALL", "C").stdin(Stdio::null()).output().expect("run tally")
}

/// `tally`, on a device in the time zone `zone` (its country orders the cities offered).
pub fn tally_in(zone: &str, dir: &PathBuf, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tally"));
    for var in ["http_proxy", "https_proxy", "HTTPS_PROXY", "ALL_PROXY", "all_proxy"] {
        command.env(var, "http://127.0.0.1:9");
    }
    command.args(args).env("XDG_CONFIG_HOME", dir).env("LC_ALL", "C").env("TZ", zone).stdin(Stdio::null()).output().expect("run tally")
}

pub fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).trim_end().to_string()
}

pub fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).trim_end().to_string()
}
