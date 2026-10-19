//! The Settings window (`O`, or Settings in the top bar): every setting (`settings.rs`) with its
//! value, changed in place and saved at once, and the downloads by hand.
//!
//! ↑↓ (j k) choose; a yes/no or a choice changes with Enter, Space or ← →; the others (durations,
//! numbers, formats, the currency, the address) are typed in an edit line, Enter saves, Esc
//! cancels; `d` puts back the default. The last row downloads all the online data in the
//! background (`aequalis --sync`).

use aequalis::settings::{self, Kind, Settings, KEYS};
use std::path::PathBuf;

/// The window's state.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    /// The row chosen: a setting (`KEYS`), then the download.
    pub selected: usize,
    /// The value being typed, for a setting typed in.
    pub edit: Option<String>,
    /// What the last change did (saved, an error, a download started).
    pub message: Option<String>,
    /// The settings file changed (the one in use), and its settings as last read or saved.
    pub file: PathBuf,
    pub settings: Settings,
}

impl View {
    /// The window on the settings file at `file`.
    pub fn on(file: PathBuf) -> View {
        let settings = settings::load_from(&file).unwrap_or_default();
        View { selected: 0, edit: None, message: None, file, settings }
    }
}

impl Default for View {
    /// The window on the settings in use.
    fn default() -> View {
        View::on(settings::path().unwrap_or_default())
    }
}

/// The row after the settings: everything online, downloaded again.
pub const DOWNLOAD: &str = "Download everything now";

/// How many rows: the settings, then the download.
pub fn rows() -> usize {
    KEYS.len() + 1
}

/// What a key does; `false` when the window closes (Esc).
pub fn key(view: &mut View, key: crate::tui::keys::Key) -> bool {
    use crate::tui::keys::Key;
    // Typing a value.
    if let Some(text) = &mut view.edit {
        match key {
            Key::Esc => view.edit = None,
            Key::Enter => {
                let value = text.clone();
                view.edit = None;
                save(view, Some(&value));
            }
            Key::Backspace => {
                text.pop();
            }
            Key::Char(c) => text.push(c),
            _ => {}
        }
        return true;
    }
    match key {
        Key::Esc | Key::Char('q') => return false,
        Key::Up | Key::Char('k') => view.selected = (view.selected + rows() - 1) % rows(),
        Key::Down | Key::Char('j') | Key::Tab => view.selected = (view.selected + 1) % rows(),
        Key::Left | Key::Char('h') => step(view, false),
        Key::Right | Key::Char('l') | Key::Char(' ') => step(view, true),
        Key::Enter => match KEYS.get(view.selected) {
            Some(k) if matches!(k.kind, Kind::Bool | Kind::Choice(_)) => step(view, true),
            Some(k) => view.edit = Some(view.settings.get(k.key).to_string()),
            None => download(view),
        },
        // `d`: back to the default.
        Key::Char('d') if view.selected < KEYS.len() => save(view, None),
        _ => {}
    }
    true
}

/// A yes/no or a choice: the next (or previous) value.
fn step(view: &mut View, forward: bool) {
    let Some(k) = KEYS.get(view.selected) else { return };
    let value = view.settings.get(k.key);
    let next = match k.kind {
        Kind::Bool => if value == "true" { "false" } else { "true" }.to_string(),
        Kind::Choice(options) => {
            let i = options.iter().position(|o| *o == value).unwrap_or(0);
            let n = options.len();
            options[if forward { (i + 1) % n } else { (i + n - 1) % n }].to_string()
        }
        _ => return,
    };
    save(view, Some(&next));
}

fn save(view: &mut View, value: Option<&str>) {
    let Some(k) = KEYS.get(view.selected) else { return };
    view.message = Some(match settings::change_in(&view.file, k.key, value) {
        Ok(saved) => {
            view.settings = saved;
            match k.key {
                "precision" => format!("saved: {} = {} (for new requests: restart the interactive mode)", k.key, view.settings.get(k.key)),
                _ => format!("saved: {} = {}", k.key, view.settings.get(k.key)),
            }
        }
        Err(e) => e,
    });
}

/// Starts the download of everything in the background.
fn download(view: &mut View) {
    if view.settings.offline() {
        view.message = Some("the offline setting is on: no download".into());
        return;
    }
    #[cfg(windows)]
    use aequalis::windows::CommandExt;
    #[cfg(unix)]
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    let started = std::env::current_exe().ok().and_then(|exe| {
        Command::new(exe)
            .arg("--sync")
            .args(aequalis::time::fixed_now().map(|t| format!("--now={t}")))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .ok()
    });
    view.message = Some(match started {
        Some(_) => "downloading everything in the background".to_string(),
        None => "could not start the download".into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::keys::Key;

    /// The window on a settings file of its own (`offline` on: no download starts).
    fn view(name: &str) -> View {
        let dir = std::env::temp_dir().join(format!("aequalis-settings-window-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("settings.json");
        settings::change_in(&file, "offline", Some("true")).unwrap();
        View::on(file)
    }

    fn row(key: &str) -> usize {
        KEYS.iter().position(|k| k.key == key).unwrap()
    }

    fn saved(v: &View, key: &str) -> String {
        settings::load_from(&v.file).unwrap().get(key).to_string()
    }

    #[test]
    fn moving_around() {
        let mut v = view("moving");
        assert!(key(&mut v, Key::Up));
        assert_eq!(v.selected, rows() - 1, "circular");
        key(&mut v, Key::Down);
        assert_eq!(v.selected, 0);
        assert!(!key(&mut v, Key::Esc), "Esc closes");
    }

    #[test]
    fn yes_no_and_choices_change_in_place() {
        let mut v = view("choices");
        v.selected = row("cities.enabled");
        key(&mut v, Key::Enter);
        assert_eq!((saved(&v, "cities.enabled").as_str(), v.message.as_deref()), ("false", Some("saved: cities.enabled = false")));
        key(&mut v, Key::Char(' '));
        assert_eq!(saved(&v, "cities.enabled"), "true");
        // A choice: forward and back, circular.
        v.selected = row("cities.refresh");
        key(&mut v, Key::Right);
        assert_eq!(saved(&v, "cities.refresh"), "silent", "manual → silent (circular)");
        key(&mut v, Key::Left);
        key(&mut v, Key::Left);
        assert_eq!(saved(&v, "cities.refresh"), "notify");
        // `d`: the default.
        key(&mut v, Key::Char('d'));
        assert_eq!(saved(&v, "cities.refresh"), "manual");
    }

    #[test]
    fn typed_values_are_checked() {
        let mut v = view("typed");
        v.selected = row("rates.interval");
        key(&mut v, Key::Enter);
        assert_eq!(v.edit.as_deref(), Some("24h"), "the value to edit");
        for _ in 0..3 {
            key(&mut v, Key::Backspace);
        }
        for c in "90m".chars() {
            key(&mut v, Key::Char(c));
        }
        key(&mut v, Key::Enter);
        assert_eq!((v.edit.as_deref(), saved(&v, "rates.interval").as_str()), (None, "90m"));
        // A bad value: the error, nothing saved.
        key(&mut v, Key::Enter);
        key(&mut v, Key::Char('x'));
        key(&mut v, Key::Enter);
        assert!(v.message.as_deref().is_some_and(|m| m.contains("`rates.interval` must be a duration")), "{:?}", v.message);
        assert_eq!(saved(&v, "rates.interval"), "90m");
        // Esc cancels the edit.
        key(&mut v, Key::Enter);
        key(&mut v, Key::Char('7'));
        key(&mut v, Key::Esc);
        assert_eq!((v.edit.as_deref(), saved(&v, "rates.interval").as_str()), (None, "90m"));
        // The precision says when it applies.
        v.selected = row("precision");
        key(&mut v, Key::Enter);
        key(&mut v, Key::Backspace);
        key(&mut v, Key::Char('2'));
        key(&mut v, Key::Enter);
        assert!(v.message.as_deref().is_some_and(|m| m.contains("restart the interactive mode")));
    }

    #[test]
    fn no_download_offline() {
        let mut v = view("download");
        v.selected = KEYS.len();
        key(&mut v, Key::Enter);
        assert_eq!(v.message.as_deref(), Some("the offline setting is on: no download"));
    }
}
