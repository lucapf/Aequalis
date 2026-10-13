//! Notices: online data that is out of date, or a new city archive, under the `notify` refresh
//! policy (`settings.rs`). Shown by the command line (a note on stderr, once a day at most), the
//! interactive mode (the status bar), `--json` (`"notices"`, for the widget and the Mac app), and
//! once per new city archive as a desktop notification.
//!
//! What was checked and shown is kept in `~/.config/tally/state.json`.

use crate::online::Group;
use crate::settings::{Refresh, Settings};
use std::path::PathBuf;

/// What Tally remembers between runs about the notices.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    /// When the published city archive's version was last checked.
    pub cities_checked_at: i64,
    /// The newest published version seen (`cities.version`).
    pub cities_available: String,
    /// The version a desktop notification was shown for.
    pub cities_notified: String,
    /// When the command line last printed the notices.
    pub shown_at: i64,
}

pub fn path() -> Option<PathBuf> {
    crate::online::dir().map(|d| d.join("state.json"))
}

pub fn load() -> State {
    let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else { return State::default() };
    let Ok(doc) = crate::json::parse(&text) else { return State::default() };
    let n = |k: &str| doc.get(k).and_then(|v| v.number()).unwrap_or(0.0) as i64;
    let s = |k: &str| doc.get(k).and_then(|v| v.str()).unwrap_or_default().to_string();
    State { cities_checked_at: n("cities_checked_at"), cities_available: s("cities_available"), cities_notified: s("cities_notified"), shown_at: n("shown_at") }
}

pub fn save(state: &State) {
    let Some(path) = path() else { return };
    let text = format!(
        "{{\"cities_checked_at\":{},\"cities_available\":{},\"cities_notified\":{},\"shown_at\":{}}}\n",
        state.cities_checked_at,
        crate::json::string(&state.cities_available),
        crate::json::string(&state.cities_notified),
        state.shown_at
    );
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// The notices now: each group under `notify` that has something new or is out of date.
pub fn notices(settings: &Settings, state: &State, cities_version: Option<&str>, stale: &dyn Fn(Group) -> bool) -> Vec<String> {
    if settings.offline() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for group in Group::ALL {
        if settings.refresh(group.name()) != Refresh::Notify {
            continue;
        }
        match group {
            Group::Cities => {
                let newer = !state.cities_available.is_empty() && Some(state.cities_available.as_str()) != cities_version;
                if settings.cities_enabled() && newer {
                    out.push(format!("a new city archive is available: tally --sync cities"));
                }
            }
            _ if stale(group) => out.push(format!("{} are out of date: tally --sync {}", group.title(), group.name())),
            _ => {}
        }
    }
    out
}

/// The notices for this machine now.
pub fn current() -> Vec<String> {
    let now = crate::online::now();
    let settings = crate::settings::current();
    let version = crate::cities::meta().map(|m| m.version);
    notices(&settings, &load(), version.as_deref(), &|g| crate::online::stale(g, &settings, now))
}

/// A desktop notification (`notify-send` on Linux, Notification Center on macOS); nothing if
/// neither is there.
pub fn desktop(title: &str, body: &str) {
    use std::process::{Command, Stdio};
    let quiet = |c: &mut Command| {
        let _ = c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    };
    if cfg!(target_os = "macos") {
        let escape = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        quiet(Command::new("osascript").args(["-e", &format!("display notification \"{}\" with title \"{}\"", escape(body), escape(title))]));
    } else {
        quiet(Command::new("notify-send").args(["--app-name=Tally", title, body]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_follow_the_policies() {
        let mut s = Settings::default();
        let state = State { cities_available: "2 bbb".into(), ..State::default() };
        let all_stale = |_: Group| true;
        // The defaults: cities manual, rates silent, live manual: nothing to say.
        assert!(notices(&s, &state, Some("1 aaa"), &all_stale).is_empty());
        s.set("cities.refresh", Some("notify")).unwrap();
        s.set("rates.refresh", Some("notify")).unwrap();
        assert_eq!(
            notices(&s, &state, Some("1 aaa"), &all_stale),
            ["a new city archive is available: tally --sync cities", "the exchange rates are out of date: tally --sync rates"]
        );
        // Up to date: nothing.
        assert_eq!(notices(&s, &state, Some("2 bbb"), &|_| false), Vec::<String>::new());
        // Never checked: nothing to say about the archive.
        assert!(notices(&s, &State::default(), None, &|_| false).is_empty());
        // Offline, or the cities off: no notice about them.
        s.set("cities.enabled", Some("false")).unwrap();
        assert_eq!(notices(&s, &state, Some("1 aaa"), &|_| false), Vec::<String>::new());
        s.set("offline", Some("true")).unwrap();
        assert!(notices(&s, &state, Some("1 aaa"), &all_stale).is_empty());
    }
}
