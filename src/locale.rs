//! The measurement system configured on the device, used to open the unit keys on the right tab.
//!
//! - **Linux** (and other Unix): the locale, as the C library reads it: `LC_ALL`, else
//!   `LC_MEASUREMENT`, else `LANG`. Like glibc, the territory `US` is imperial (`en_US`, `es_US`…),
//!   everything else metric.
//! - **macOS**: the system setting (`defaults read -g AppleMetricUnits`, then
//!   `AppleMeasurementUnits`, then `AppleLocale`), and the environment as above if those fail.
//! - **Windows**: the Region settings (`LOCALE_IMEASURE`), unless the environment names a locale.
//!
//! When nothing is known: metric.

use crate::config::System;

pub fn device_system() -> System {
    if let Some(m) = crate::machine::mocked() {
        return m.system;
    }
    if let Some(&system) = DEVICE_SYSTEM.get() {
        return system;
    }
    #[cfg(target_os = "macos")]
    if let Some(system) = macos_setting() {
        return system;
    }
    let var = |name| std::env::var(name).ok().filter(|v| !v.is_empty());
    // Windows: the Region settings, unless the environment names a locale (the tests).
    #[cfg(windows)]
    if var("LC_ALL").or_else(|| var("LC_MEASUREMENT")).or_else(|| var("LANG")).is_none() {
        if let Some(system) = crate::windows::measurement() {
            return system;
        }
    }
    var("LC_ALL").or_else(|| var("LC_MEASUREMENT")).or_else(|| var("LANG")).and_then(|locale| from_locale(&locale)).unwrap_or(System::Metric)
}

static DEVICE_SYSTEM: std::sync::OnceLock<System> = std::sync::OnceLock::new();

/// The device's system as the app embedding aequalis knows it (the Android app: from its `Locale`); set
/// once, before use.
pub fn set_device_system(system: System) {
    let _ = DEVICE_SYSTEM.set(system);
}

static DEVICE_LANGUAGE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The device's language as the app embedding aequalis knows it (`it`); set once, before use.
pub fn set_device_language(language: &str) {
    let _ = DEVICE_LANGUAGE.set(language.to_ascii_lowercase());
}

/// The device's language, its first part (`it` for `it_IT.UTF-8`): the app's, else the locale's
/// (`LC_ALL`, `LC_MESSAGES`, `LANG`); `None` for `C`, `POSIX` or nothing.
pub fn device_language() -> Option<String> {
    if let Some(language) = DEVICE_LANGUAGE.get() {
        return Some(language.clone());
    }
    let var = |name| std::env::var(name).ok().filter(|v| !v.is_empty());
    var("LC_ALL").or_else(|| var("LC_MESSAGES")).or_else(|| var("LANG")).and_then(|l| language_of(&l))
}

/// `it_IT.UTF-8` → `it`, `pt-BR` → `pt`; `C`, `POSIX` → none.
pub fn language_of(locale: &str) -> Option<String> {
    let name = locale.split(['.', '@']).next()?;
    let language = name.split(['_', '-']).next()?.to_ascii_lowercase();
    (language.len() >= 2 && language.bytes().all(|b| b.is_ascii_lowercase()) && language != "posix").then_some(language)
}

/// The territory of the locale (`IT` for `it_IT.UTF-8`), from `LC_ALL`, `LC_TIME`, `LANG`.
pub fn territory() -> Option<String> {
    let var = |name| std::env::var(name).ok().filter(|v| !v.is_empty());
    let locale = var("LC_ALL").or_else(|| var("LC_TIME")).or_else(|| var("LANG"))?;
    let name = locale.split(['.', '@']).next()?;
    let territory = name.split_once(['_', '-'])?.1;
    (territory.len() == 2).then(|| territory.to_ascii_uppercase())
}

/// `en_US.UTF-8` → imperial, `it_IT.UTF-8` → metric; `C` / `POSIX` / no territory → unknown.
pub fn from_locale(locale: &str) -> Option<System> {
    let name = locale.split(['.', '@']).next()?;
    let territory = name.split_once(['_', '-'])?.1;
    Some(if territory.eq_ignore_ascii_case("US") { System::Imperial } else { System::Metric })
}

/// Output of `defaults read -g <key>` for the macOS measurement keys.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn from_macos_defaults(key: &str, value: &str) -> Option<System> {
    match (key, value.trim()) {
        ("AppleMetricUnits", "1") | ("AppleMeasurementUnits", "Centimeters") => Some(System::Metric),
        ("AppleMetricUnits", "0") | ("AppleMeasurementUnits", "Inches") => Some(System::Imperial),
        ("AppleLocale", locale) => from_locale(locale),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn macos_setting() -> Option<System> {
    ["AppleMetricUnits", "AppleMeasurementUnits", "AppleLocale"].iter().find_map(|key| {
        let out = std::process::Command::new("/usr/bin/defaults").args(["read", "-g", key]).output().ok()?;
        if !out.status.success() {
            return None;
        }
        from_macos_defaults(key, &String::from_utf8_lossy(&out.stdout))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use System::*;

    #[test]
    fn locales() {
        assert_eq!(from_locale("en_US.UTF-8"), Some(Imperial));
        assert_eq!(from_locale("es_US"), Some(Imperial));
        assert_eq!(from_locale("en-US"), Some(Imperial));
        assert_eq!(from_locale("it_IT.UTF-8"), Some(Metric));
        assert_eq!(from_locale("en_GB.UTF-8@euro"), Some(Metric));
        assert_eq!(from_locale("my_MM"), Some(Metric), "glibc: metric");
        assert_eq!(from_locale("C"), None);
        assert_eq!(from_locale("POSIX"), None);
        assert_eq!(from_locale("C.UTF-8"), None);
    }

    #[test]
    fn macos_defaults() {
        assert_eq!(from_macos_defaults("AppleMetricUnits", "1\n"), Some(Metric));
        assert_eq!(from_macos_defaults("AppleMetricUnits", "0\n"), Some(Imperial));
        assert_eq!(from_macos_defaults("AppleMeasurementUnits", "Inches\n"), Some(Imperial));
        assert_eq!(from_macos_defaults("AppleLocale", "it_IT\n"), Some(Metric));
        assert_eq!(from_macos_defaults("AppleLocale", "en_US\n"), Some(Imperial));
        assert_eq!(from_macos_defaults("AppleMetricUnits", "garbage"), None);
    }
}
