//! Settings: everything an operator configures, in one place — `~/.config/aequalis/settings.json`
//! (`$XDG_CONFIG_HOME/aequalis`), changed with `aequalis --set key=value`, shown with `aequalis --settings`,
//! or in the interactive mode's Settings window. Nothing is read from the environment.
//!
//! ```json
//! {"offline": false, "cities.enabled": true, "cities.refresh": "manual", "precision": 4}
//! ```
//! Keys not in the file have their default; an unknown key or a bad value is an error naming it.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

/// How a group of online data is kept up to date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refresh {
    /// Downloaded in the background when it is older than its interval.
    Silent,
    /// Not downloaded: a notice says a newer version is available; the operator downloads it.
    Notify,
    /// Only when the operator asks (`aequalis --sync`, Settings).
    Manual,
}

/// The unit of a distance between two places when none is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceUnit {
    /// The device's measurement system: miles where it is imperial, else kilometres.
    Auto,
    Km,
    Mi,
}

/// What a setting holds, to check a value and to edit it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Bool,
    /// One of these words.
    Choice(&'static [&'static str]),
    /// A duration: `24h`, `90m`, `30d`.
    Duration,
    /// An integer from 0 to 15.
    Precision,
    /// A currency code, or `auto`.
    Currency,
    /// A date format pattern.
    Format,
    /// An address: `https://…`, `file://…`.
    Url,
    /// Names with their hits (`USD:3,EUR:1`; empty: the defaults), kept by the interactive mode.
    Favourites,
}

/// A setting: its key, default, kind, and what it does.
pub struct Key {
    pub key: &'static str,
    pub default: &'static str,
    pub kind: Kind,
    pub about: &'static str,
}

const REFRESH: &[&str] = &["silent", "notify", "manual"];

/// Every setting, in the order they are shown.
pub const KEYS: &[Key] = &[
    Key { key: "offline", default: "false", kind: Kind::Bool, about: "no network at all: online data stays as last downloaded" },
    Key { key: "cities.enabled", default: "true", kind: Kind::Bool, about: "distances between any city (the city archive), not only capitals" },
    Key {
        key: "cities.refresh",
        default: "manual",
        kind: Kind::Choice(REFRESH),
        about: "the city archive: silent, notify (a notice when a new one is out) or manual",
    },
    Key { key: "cities.interval", default: "30d", kind: Kind::Duration, about: "how often the city archive is checked (silent, notify)" },
    Key { key: "cities.url", default: crate::cities::URL, kind: Kind::Url, about: "where the city archive is downloaded from (a mirror, a file://)" },
    Key { key: "rates.refresh", default: "silent", kind: Kind::Choice(REFRESH), about: "the exchange rates: silent, notify or manual" },
    Key { key: "rates.interval", default: "24h", kind: Kind::Duration, about: "how old the exchange rates may be" },
    Key {
        key: "live.refresh",
        default: "manual",
        kind: Kind::Choice(REFRESH),
        about: "the Trivia's live values (Voyagers, satellites): silent, notify or manual",
    },
    Key { key: "live.interval", default: "24h", kind: Kind::Duration, about: "how old the live values may be" },
    Key { key: "currency.local", default: "auto", kind: Kind::Currency, about: "the local currency, for `12 usd` (auto: the time zone's country)" },
    Key {
        key: "distance.unit",
        default: "auto",
        kind: Kind::Choice(&["auto", "km", "mi"]),
        about: "the unit of `Rome Paris` (auto: miles where the device is imperial)",
    },
    Key { key: "precision", default: "4", kind: Kind::Precision, about: "decimal places of numbers, 0-15 (--precision for one request)" },
    Key { key: "format.date", default: crate::time::HUMAN_DATE, kind: Kind::Format, about: "how dates are shown" },
    Key { key: "format.datetime", default: crate::time::HUMAN_DATETIME, kind: Kind::Format, about: "how dates with a time are shown" },
    // Only the interactive mode's: the app (`ffi::settings`) doesn't list the `tui.` keys.
    Key {
        key: "tui.keyboard",
        default: "false",
        kind: Kind::Bool,
        about: "the interactive mode starts with its keypad on screen (F9 shows, changes and hides it)",
    },
    Key {
        key: "tui.favourite_units",
        default: "",
        kind: Kind::Favourites,
        about: "the units of the keypad's shortcuts (Alt+1…5) with their hits; kept as you use them (empty: the defaults)",
    },
    Key {
        key: "tui.favourite_currencies",
        default: "",
        kind: Kind::Favourites,
        about: "the currencies of the keypad's shortcuts (Alt+6…0) with their hits; kept as you use them (empty: the defaults)",
    },
];

/// The settings in effect: every key with its value (as written in the file, or the default).
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    values: Vec<(&'static str, String)>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { values: KEYS.iter().map(|k| (k.key, k.default.to_string())).collect() }
    }
}

impl Settings {
    /// The value of `key`, as text.
    pub fn get(&self, key: &str) -> &str {
        self.values.iter().find(|(k, _)| *k == key).map(|(_, v)| v.as_str()).unwrap_or_else(|| panic!("no setting {key}"))
    }

    /// Changes `key` (checked first); `None` puts back the default.
    pub fn set(&mut self, key: &str, value: Option<&str>) -> Result<(), String> {
        let spec = spec(key)?;
        let value = match value {
            None => spec.default.to_string(),
            Some(v) => check(spec, v)?,
        };
        if let Some((_, v)) = self.values.iter_mut().find(|(k, _)| *k == key) {
            *v = value;
        }
        Ok(())
    }

    pub fn offline(&self) -> bool {
        self.get("offline") == "true"
    }

    pub fn cities_enabled(&self) -> bool {
        self.get("cities.enabled") == "true"
    }

    /// The refresh policy of a group: `cities`, `rates`, `live`.
    pub fn refresh(&self, group: &str) -> Refresh {
        match self.get(&format!("{group}.refresh")) {
            "silent" => Refresh::Silent,
            "notify" => Refresh::Notify,
            _ => Refresh::Manual,
        }
    }

    /// The interval of a group, in seconds.
    pub fn interval(&self, group: &str) -> i64 {
        let key = format!("{group}.interval");
        duration(self.get(&key)).unwrap_or_else(|| duration(spec(&key).map(|s| s.default).unwrap_or("24h")).unwrap_or(86_400))
    }

    /// The local currency's code, when set (not `auto`).
    pub fn currency(&self) -> Option<&str> {
        Some(self.get("currency.local")).filter(|c| *c != "auto")
    }

    pub fn distance_unit(&self) -> DistanceUnit {
        match self.get("distance.unit") {
            "km" => DistanceUnit::Km,
            "mi" => DistanceUnit::Mi,
            _ => DistanceUnit::Auto,
        }
    }

    pub fn precision(&self) -> usize {
        self.get("precision").parse().unwrap_or(4)
    }

    pub fn format_date(&self) -> &str {
        self.get("format.date")
    }

    pub fn format_datetime(&self) -> &str {
        self.get("format.datetime")
    }

    /// The interactive mode starts with its keypad on screen (off unless asked).
    pub fn keyboard(&self) -> bool {
        self.get("tui.keyboard") == "true"
    }

    /// The stored shortcuts: the units, or the currencies (empty when nothing is stored).
    pub fn favourites(&self, currencies: bool) -> &str {
        self.get(if currencies { "tui.favourite_currencies" } else { "tui.favourite_units" })
    }

    /// The file's content: every key, one per line, in `KEYS` order.
    pub fn to_json(&self) -> String {
        let lines: Vec<String> = KEYS
            .iter()
            .map(|k| {
                let v = self.get(k.key);
                let v = match k.kind {
                    Kind::Bool => v.to_string(),
                    Kind::Precision => v.to_string(),
                    _ => crate::json::string(v),
                };
                format!("  {}: {v}", crate::json::string(k.key))
            })
            .collect();
        format!("{{\n{}\n}}\n", lines.join(",\n"))
    }

    /// Reads a file's content: keys not in it keep their default.
    pub fn from_json(text: &str) -> Result<Settings, String> {
        let doc = crate::json::parse(text).map_err(|e| format!("settings: {e}"))?;
        let mut settings = Settings::default();
        for (key, value) in doc.members() {
            let text = match (value.str(), value.number(), value.boolean()) {
                (Some(s), _, _) => s.to_string(),
                (_, Some(n), _) => format!("{n}"),
                (_, _, Some(b)) => b.to_string(),
                _ => return Err(format!("settings: `{key}` must be a string, a number or true/false")),
            };
            // The file holds every key, so the archive's address of when it was published
            // compressed stays in it: today's address instead.
            let text = if key == "cities.url" && text == crate::cities::LEGACY_URL { crate::cities::URL.to_string() } else { text };
            settings.set(key, Some(&text)).map_err(|e| format!("settings ({}): {e}", path_text()))?;
        }
        Ok(settings)
    }
}

/// The setting called `key`.
pub fn spec(key: &str) -> Result<&'static Key, String> {
    KEYS.iter().find(|k| k.key == key).ok_or_else(|| {
        let keys: Vec<&str> = KEYS.iter().map(|k| k.key).collect();
        format!("unknown setting `{key}` (one of: {})", keys.join(", "))
    })
}

/// `value` for the setting `spec`, normalised (`TRUE` → `true`, `Usd` → `USD`), or why it's wrong.
pub fn check(spec: &Key, value: &str) -> Result<String, String> {
    let v = value.trim();
    let wrong = |what: &str| Err(format!("`{}` must be {what} (it is `{v}`)", spec.key));
    match spec.kind {
        Kind::Bool => match v.to_ascii_lowercase().as_str() {
            "true" | "on" | "yes" | "1" => Ok("true".into()),
            "false" | "off" | "no" | "0" => Ok("false".into()),
            _ => wrong("true or false"),
        },
        Kind::Choice(options) => match options.iter().find(|o| o.eq_ignore_ascii_case(v)) {
            Some(o) => Ok(o.to_string()),
            None => wrong(&format!("one of {}", options.join(", "))),
        },
        Kind::Duration => match duration(v) {
            Some(_) => Ok(v.to_ascii_lowercase()),
            None => wrong("a duration of a minute or more, such as 24h, 90m or 30d"),
        },
        Kind::Precision => match v.parse::<usize>() {
            Ok(p) if p <= 15 => Ok(p.to_string()),
            _ => wrong("an integer from 0 to 15"),
        },
        Kind::Currency => {
            if v.eq_ignore_ascii_case("auto") {
                return Ok("auto".into());
            }
            match crate::currency::exact(v).as_deref() {
                Some(&[code]) => Ok(code.to_string()),
                _ => wrong("auto or one currency, such as EUR or USD"),
            }
        }
        Kind::Url => match v.contains("://") {
            true => Ok(v.to_string()),
            false => wrong("an address, such as https://… or file://…"),
        },
        Kind::Favourites => crate::favourites::Favourites::check(v).map_err(|e| format!("`{}`: {e}", spec.key)),
        Kind::Format => match v.is_empty() {
            true => wrong("a date pattern, such as DD MMM YY"),
            false => Ok(v.to_string()),
        },
    }
}

/// `24h`, `90m`, `30d`, `45min`, or seconds (`3600`); a minute at least.
pub fn duration(text: &str) -> Option<i64> {
    let text = text.trim().to_ascii_lowercase();
    let digits = text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len());
    let (number, unit) = text.split_at(digits);
    let n: i64 = number.parse().ok()?;
    let seconds = match unit.trim() {
        "" | "s" => n,
        "m" | "min" => n * 60,
        "h" => n * 3600,
        "d" => n * 86_400,
        _ => return None,
    };
    (seconds >= 60).then_some(seconds)
}

// ---------------------------------------------------------------- the file

/// `~/.config/aequalis/settings.json`.
pub fn path() -> Option<PathBuf> {
    crate::files::dir().map(|d| d.join("settings.json"))
}

fn path_text() -> String {
    path().map_or("settings.json".into(), |p| p.display().to_string())
}

pub fn load_from(path: &Path) -> Result<Settings, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Settings::from_json(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// Writes the settings (a new file renamed over the old one).
pub fn save_to(path: &Path, settings: &Settings) -> std::io::Result<()> {
    crate::files::write_atomic(path, settings.to_json().as_bytes())
}

fn cell() -> &'static RwLock<Result<Arc<Settings>, String>> {
    static CURRENT: OnceLock<RwLock<Result<Arc<Settings>, String>>> = OnceLock::new();
    CURRENT.get_or_init(|| RwLock::new(path().map_or(Ok(Settings::default()), |p| load_from(&p)).map(Arc::new)))
}

/// The file's problem, if it has one (the command line reports it; the settings are then the
/// defaults).
pub fn error() -> Option<String> {
    cell().read().ok().and_then(|s| s.as_ref().err().cloned())
}

/// The settings in effect (the defaults when the file has a problem); shared, not copied.
pub fn current() -> Arc<Settings> {
    cell().read().ok().and_then(|s| s.as_ref().ok().cloned()).unwrap_or_default()
}

/// Changes one setting and saves the file; `None` puts back its default.
pub fn change(key: &str, value: Option<&str>) -> Result<Settings, String> {
    change_in(&path().ok_or("no home directory")?, key, value)
}

/// `change`, in the settings file at `path`; the settings in effect follow when it is the one in use.
pub fn change_in(path: &Path, key: &str, value: Option<&str>) -> Result<Settings, String> {
    let mut settings = load_from(path)?;
    settings.set(key, value)?;
    save_to(path, &settings).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    if self::path().as_deref() == Some(path) {
        if let Ok(mut cell) = cell().write() {
            *cell = Ok(Arc::new(settings.clone()));
        }
    }
    Ok(settings)
}

/// `aequalis --settings --json`: every setting with its value, default, kind (`bool`, `choice` and its
/// options, or `text`: typed) and what it does, for the widget's Settings section.
pub fn json(settings: &Settings) -> String {
    json_where(settings, |_| true)
}

/// `json`, with only the settings `keep` keeps (the app leaves out the interactive mode's).
pub fn json_where(settings: &Settings, keep: impl Fn(&Key) -> bool) -> String {
    let items: Vec<String> = KEYS
        .iter()
        .filter(|k| keep(k))
        .map(|k| {
            let (kind, options): (&str, &[&str]) = match k.kind {
                Kind::Bool => ("bool", &["true", "false"]),
                Kind::Choice(options) => ("choice", options),
                _ => ("text", &[]),
            };
            crate::json::Object::new()
                .str("key", k.key)
                .str("value", settings.get(k.key))
                .str("default", k.default)
                .str("kind", kind)
                .strs("options", &options.iter().map(|o| o.to_string()).collect::<Vec<_>>())
                .str("about", k.about)
                .finish()
        })
        .collect();
    crate::json::Object::new().str("file", &path_text()).list("settings", &items).finish()
}

/// `aequalis --settings`: every setting, its value, and what it does.
pub fn text(settings: &Settings) -> String {
    let mut out = format!("Settings ({}; change with `aequalis --set key=value`, `aequalis --set key` for the default)\n", path_text());
    for k in KEYS {
        let v = settings.get(k.key);
        let mark = if v == k.default { String::new() } else { format!("  (default: {})", k.default) };
        out.push_str(&format!("  {:<16} {v}{mark}\n  {:<16} {}\n", k.key, "", k.about));
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favourites_are_stored_checked_and_cleared() {
        let mut s = Settings::default();
        assert_eq!((s.favourites(true), s.favourites(false)), ("", ""), "nothing stored: the defaults");
        s.set("tui.favourite_currencies", Some(" usd:2 , EUR ")).unwrap();
        assert_eq!(s.favourites(true), "usd:2,EUR:0", "normalised");
        assert!(s.set("tui.favourite_units", Some("km:many")).is_err());
        let back = Settings::from_json(&s.to_json()).unwrap();
        assert_eq!(back.favourites(true), "usd:2,EUR:0", "kept in the file");
        s.set("tui.favourite_currencies", None).unwrap();
        assert_eq!(s.favourites(true), "", "cleared: the defaults again");
    }

    #[test]
    fn defaults() {
        let s = Settings::default();
        assert!(!s.offline() && s.cities_enabled());
        assert_eq!((s.refresh("cities"), s.refresh("rates"), s.refresh("live")), (Refresh::Manual, Refresh::Silent, Refresh::Manual));
        assert_eq!((s.interval("cities"), s.interval("rates")), (30 * 86_400, 86_400));
        assert_eq!((s.currency(), s.distance_unit(), s.precision()), (None, DistanceUnit::Auto, 4));
        assert_eq!((s.format_date(), s.format_datetime()), ("DD MMM YY", "DD MMM YY h:mm AM tz"));
        // Every accessor reads a key by name (`get` panics on a wrong one): each is called here.
        assert_eq!((s.interval("live"), s.keyboard()), (86_400, false));
    }

    #[test]
    fn values_are_checked_and_normalised() {
        let mut s = Settings::default();
        s.set("offline", Some("ON")).unwrap();
        assert!(s.offline());
        s.set("cities.refresh", Some("Notify")).unwrap();
        assert_eq!(s.refresh("cities"), Refresh::Notify);
        s.set("currency.local", Some("swiss franc")).unwrap();
        assert_eq!(s.currency(), Some("CHF"));
        s.set("rates.interval", Some("90M")).unwrap();
        assert_eq!(s.interval("rates"), 5_400);
        for (text, seconds) in [("24h", 86_400), ("2d", 172_800), ("3600", 3_600), ("45 MIN", 2_700)] {
            assert_eq!(duration(text), Some(seconds), "{text}");
        }
        assert!(duration("30s").is_none() && duration("soon").is_none());
        s.set("precision", None).unwrap();
        assert_eq!(s.precision(), 4);
        for (key, bad) in [
            ("offline", "maybe"),
            ("cities.refresh", "often"),
            ("rates.interval", "30s"),
            ("precision", "16"),
            ("currency.local", "dollar"),
            ("distance.unit", "ly"),
            ("cities.url", "nowhere"),
        ] {
            let e = s.set(key, Some(bad)).unwrap_err();
            assert!(e.starts_with(&format!("`{key}` must be")), "{e}");
        }
        assert!(s.set("bogus", Some("1")).unwrap_err().starts_with("unknown setting `bogus`"));
    }

    #[test]
    fn the_file_round_trips() {
        let mut s = Settings::default();
        s.set("offline", Some("true")).unwrap();
        s.set("format.date", Some("YYYY-MM-DD")).unwrap();
        s.set("precision", Some("2")).unwrap();
        let text = s.to_json();
        assert!(text.contains("\"offline\": true") && text.contains("\"precision\": 2"), "{text}");
        assert_eq!(Settings::from_json(&text).unwrap(), s);
        // Missing keys: defaults; numbers and booleans as written by hand.
        let hand = Settings::from_json(r#"{"precision": 6, "cities.enabled": false}"#).unwrap();
        assert_eq!((hand.precision(), hand.cities_enabled(), hand.offline()), (6, false, false));
        assert!(Settings::from_json(r#"{"precision": "x"}"#).unwrap_err().contains("`precision` must be"));
        assert!(Settings::from_json("{not json").is_err());
    }

    /// A file written when the city archive was published compressed holds its old address (every
    /// key is saved): read as today's; a mirror stays as it is.
    #[test]
    fn the_old_archive_address_becomes_today_s() {
        let old = Settings::from_json(&format!(r#"{{"cities.url": "{}"}}"#, crate::cities::LEGACY_URL)).unwrap();
        assert_eq!(old.get("cities.url"), crate::cities::URL);
        assert!(!old.to_json().contains("cities.json.gz"), "saved with today's address");
        let mirror = Settings::from_json(r#"{"cities.url": "https://example.org/cities.json"}"#).unwrap();
        assert_eq!(mirror.get("cities.url"), "https://example.org/cities.json");
    }

    #[test]
    fn saved_atomically() {
        let dir = std::env::temp_dir().join(format!("aequalis-settings-{}", std::process::id()));
        let path = dir.join("settings.json");
        let mut s = Settings::default();
        s.set("live.refresh", Some("silent")).unwrap();
        save_to(&path, &s).unwrap();
        assert_eq!(load_from(&path).unwrap(), s);
        assert_eq!(load_from(&dir.join("none.json")).unwrap(), Settings::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
