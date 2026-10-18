//! The device the program runs on, as this program reads it (part of the `aequalis` command, not of
//! the library): its time zone, read at start and, in the interactive mode, every hour, and handed to
//! the library (`time::set_device_zone`), which does not look for it. An app embedding the library
//! (Android) gives it the same way. On Windows, the zones' rules are the system's too (`windows`).

use aequalis::time;

#[cfg(windows)]
mod windows;

/// How often a long-running mode reads the device's zone again (it may change: a trip, a setting).
#[cfg(feature = "tui")]
const CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(3600);

/// At start, before anything else: the zones' rules (Windows: the system's), then the device's zone.
pub fn start() {
    #[cfg(windows)]
    windows::use_system_rules();
    check();
}

/// Reads the device's zone again every hour, in the background (the interactive mode, which runs
/// for hours).
#[cfg(feature = "tui")]
pub fn watch() {
    std::thread::spawn(|| loop {
        std::thread::sleep(CHECK_EVERY);
        check();
    });
}

/// Hands the device's zone to the library, whose rules it reads again. A zone the library does not
/// know (a POSIX rule in `TZ`, such as `CET-1CEST`): it keeps the zone it has, else the C library's.
fn check() {
    if let Some(name) = zone() {
        let _ = time::set_device_zone(&name);
    }
}

/// The device's zone: `TZ` when set (`Europe/Rome`, `:Europe/Rome`, a file under a zoneinfo
/// folder), else the system's.
fn zone() -> Option<String> {
    match std::env::var("TZ").ok().filter(|t| !t.is_empty()) {
        Some(tz) => Some(after_zoneinfo(tz.trim_start_matches(':'))),
        None => system_zone(),
    }
}

/// Where /etc/localtime points (`/usr/share/zoneinfo/Europe/Rome`, macOS
/// `/var/db/timezone/zoneinfo/Europe/Rome`).
#[cfg(not(windows))]
fn system_zone() -> Option<String> {
    let link = std::fs::read_link("/etc/localtime").ok()?;
    Some(after_zoneinfo(&link.to_string_lossy()))
}

/// The zone in the Windows settings.
#[cfg(windows)]
fn system_zone() -> Option<String> {
    windows::device_zone()
}

/// The zone's name in a path to its file: the part after `zoneinfo/` (the whole text if none).
fn after_zoneinfo(path: &str) -> String {
    path.split_once("zoneinfo/").map_or(path, |(_, zone)| zone).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_names_in_paths() {
        assert_eq!(after_zoneinfo("/usr/share/zoneinfo/Europe/Rome"), "Europe/Rome");
        assert_eq!(after_zoneinfo("/var/db/timezone/zoneinfo/America/New_York"), "America/New_York");
        assert_eq!(after_zoneinfo("Asia/Dubai"), "Asia/Dubai");
        assert_eq!(after_zoneinfo("UTC"), "UTC");
    }
}
