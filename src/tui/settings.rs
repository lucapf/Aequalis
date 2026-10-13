//! The Settings window (`O`, or Settings in the top bar): every setting (`settings.rs`) with its
//! value, changed in place and saved at once, and the downloads by hand.
//!
//! ↑↓ (j k) choose; a yes/no or a choice changes with Enter, Space or ← →; the others (durations,
//! numbers, formats, the currency, the address) are typed in an edit line, Enter saves, Esc
//! cancels. The last rows start a download in the background (`tally --sync <group>`).

use crate::online::Group;
use crate::settings::{self, Kind, KEYS};

/// The window's state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct View {
    /// The row chosen: a setting (`KEYS`), then the downloads.
    pub selected: usize,
    /// The value being typed, for a setting typed in.
    pub edit: Option<String>,
    /// What the last change did (saved, an error, a download started).
    pub message: Option<String>,
}

/// The downloads offered after the settings.
pub const DOWNLOADS: [(Group, &str); 3] = [
    (Group::Cities, "Download the city archive now"),
    (Group::Rates, "Download the exchange rates now"),
    (Group::Live, "Download the live values now"),
];

/// How many rows: the settings, then the downloads.
pub fn rows() -> usize {
    KEYS.len() + DOWNLOADS.len()
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
            Some(k) => view.edit = Some(settings::current().get(k.key).to_string()),
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
    let current = settings::current();
    let value = current.get(k.key);
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
    view.message = Some(match settings::change(k.key, value) {
        Ok(s) => match k.key {
            "precision" => format!("saved: {} = {} (for new requests: restart the interactive mode)", k.key, s.get(k.key)),
            _ => format!("saved: {} = {}", k.key, s.get(k.key)),
        },
        Err(e) => e,
    });
}

/// Starts the download of the chosen row in the background.
fn download(view: &mut View) {
    let Some(&(group, _)) = DOWNLOADS.get(view.selected - KEYS.len()) else { return };
    if settings::current().offline() {
        view.message = Some("the offline setting is on: no download".into());
        return;
    }
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    let started = std::env::current_exe().ok().and_then(|exe| {
        Command::new(exe)
            .args(["--sync", group.name()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .ok()
    });
    view.message = Some(match started {
        Some(_) => format!("downloading {} in the background", group.title()),
        None => "could not start the download".into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::keys::Key;

    #[test]
    fn moving_around_and_typing() {
        let mut v = View::default();
        assert!(key(&mut v, Key::Up));
        assert_eq!(v.selected, rows() - 1, "circular");
        key(&mut v, Key::Down);
        assert_eq!(v.selected, 0);
        // A typed setting: Enter opens the edit line with the value, Esc cancels.
        v.selected = KEYS.iter().position(|k| k.key == "rates.interval").unwrap();
        key(&mut v, Key::Enter);
        assert!(v.edit.is_some());
        key(&mut v, Key::Backspace);
        key(&mut v, Key::Esc);
        assert_eq!(v.edit, None);
        assert!(!key(&mut v, Key::Esc), "Esc closes");
    }
}
