//! The Settings window (`O`, or Settings in the top bar): the few settings most people use, with
//! their value, changed in place and saved at once; `a` (or its row) shows the advanced ones too;
//! the standard settings come back with Reset, and the downloads by hand.
//!
//! ↑↓ (j k) choose; a yes/no changes with Enter, Space or ← →; a choice with ← →, or Enter opens
//! its list (the currency's too: typing filters it, Enter saves); the others (durations, numbers,
//! formats, the address) are typed in an edit line, Enter saves, Esc cancels; `d` puts back the default of one. `R` is the Reset row (Enter twice). The last row
//! downloads all the online data in the background (`aequalis --sync`).

use aequalis::currency;
use aequalis::settings::{self, Key as Setting, Kind, Settings, KEYS};
use std::path::PathBuf;

/// The window's state.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    /// The row chosen (see `rows`).
    pub selected: usize,
    /// The value being typed, for a setting typed in.
    pub edit: Option<String>,
    /// What the last change did (saved, an error, a download started).
    pub message: Option<String>,
    /// The settings file changed (the one in use), and its settings as last read or saved.
    pub file: PathBuf,
    pub settings: Settings,
    /// The advanced settings are listed too.
    pub advanced: bool,
    /// Reset was asked for once: the next Enter does it.
    pub confirm_reset: bool,
    /// The list of a choice or of the currencies, open under its row.
    pub list: Option<List>,
    /// A sync by hand running (since then), and its progress as last read (`online::read_progress`).
    pub syncing: Option<std::time::Instant>,
    pub progress: Option<aequalis::online::Syncing>,
    /// How the last sync by hand ended: everything synced (`true`), or something failed.
    pub outcome: Option<bool>,
}

/// The values to pick from, filtered by what is typed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct List {
    pub query: String,
    /// Into `View::choices`.
    pub selected: usize,
}

/// A row of the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Row {
    Setting(&'static Setting),
    /// Shows or hides the advanced settings.
    Advanced,
    /// Every setting back to its default.
    Reset,
    Download,
}

impl View {
    /// The window on the settings file at `file`.
    pub fn on(file: PathBuf) -> View {
        let settings = settings::load_from(&file).unwrap_or_default();
        // A sync by hand still running (the window closed and opened again): its progress shown.
        let running = aequalis::online::read_progress().is_some_and(|p| p.done.is_none() && p.age < 5);
        let syncing = running.then(std::time::Instant::now);
        View {
            selected: 0,
            edit: None,
            message: None,
            file,
            settings,
            advanced: false,
            confirm_reset: false,
            list: None,
            syncing,
            progress: None,
            outcome: None,
        }
    }

    /// The window on the setting `key` (the advanced ones listed when it is one).
    pub fn at(key: &str) -> View {
        let mut view = View::default();
        view.advanced = KEYS.iter().any(|k| k.key == key && k.advanced);
        view.selected = view.rows().iter().position(|r| matches!(r, Row::Setting(k) if k.key == key)).unwrap_or(0);
        view
    }

    /// The rows: the settings shown, then show / hide the advanced ones, Reset and the download.
    pub fn rows(&self) -> Vec<Row> {
        let settings = KEYS.iter().filter(|k| k.here() && (self.advanced || !k.advanced)).map(Row::Setting);
        settings.chain([Row::Advanced, Row::Reset, Row::Download]).collect()
    }

    pub fn row(&self) -> Option<Row> {
        self.rows().get(self.selected).copied()
    }

    /// The list open: (value, label), `auto` as it stands (`auto (km)`); for the currency, `auto`
    /// then every currency, ranked by what is typed (code, name, country).
    pub fn choices(&self) -> Vec<(String, String)> {
        let (Some(Row::Setting(k)), Some(list)) = (self.row(), &self.list) else { return Vec::new() };
        let typed = list.query.trim().to_lowercase();
        let shown = |o: &str| (o.to_string(), settings::shown(k.key, o));
        let matches = |(_, label): &(String, String)| label.to_lowercase().contains(&typed);
        match k.kind {
            Kind::Choice(options) => options.iter().map(|o| shown(o)).filter(matches).collect(),
            Kind::Currency => {
                let all = currency::search(&typed).into_iter().map(|c| (c.code.to_string(), format!("{}  {}", c.code, c.name)));
                Some(shown("auto")).filter(matches).into_iter().chain(all).collect()
            }
            _ => Vec::new(),
        }
    }

    /// How many advanced settings are not at their default, while they are hidden.
    pub fn hidden_changes(&self) -> usize {
        if self.advanced {
            return 0;
        }
        KEYS.iter().filter(|k| k.advanced && self.settings.get(k.key) != k.default).count()
    }
}

impl Default for View {
    /// The window on the settings in use.
    fn default() -> View {
        View::on(settings::path().unwrap_or_default())
    }
}

/// The row that downloads everything.
pub const DOWNLOAD: &str = "Sync";
pub const RESET: &str = "Reset";

/// What a key does; `false` when the window closes (Esc).
pub fn key(view: &mut View, key: crate::tui::keys::Key) -> bool {
    use crate::tui::keys::Key;
    if view.edit.is_some() {
        edit_key(view, key);
        return true;
    }
    if view.list.is_some() {
        list_key(view, key);
        return true;
    }
    // Reset asks once more; anything but Enter on it cancels.
    let asked = std::mem::take(&mut view.confirm_reset);
    let rows = view.rows().len();
    match key {
        Key::Esc | Key::Char('q') => return false,
        Key::Up | Key::Char('k') => view.selected = (view.selected + rows - 1) % rows,
        Key::Down | Key::Char('j') | Key::Tab => view.selected = (view.selected + 1) % rows,
        Key::Left | Key::Char('h') => step(view, false),
        Key::Right | Key::Char('l') | Key::Char(' ') => step(view, true),
        Key::Enter => match view.row() {
            Some(Row::Setting(k)) if k.kind == Kind::Bool => step(view, true),
            Some(Row::Setting(k)) if matches!(k.kind, Kind::Choice(_) | Kind::Currency) => open_list(view),
            Some(Row::Setting(k)) => view.edit = Some(view.settings.get(k.key).to_string()),
            Some(Row::Advanced) => toggle_advanced(view),
            Some(Row::Reset) if asked => reset(view),
            Some(Row::Reset) => {
                view.confirm_reset = true;
                view.message = Some("Enter again to reset everything".into());
            }
            Some(Row::Download) => download(view),
            None => {}
        },
        Key::Char('a') => toggle_advanced(view),
        Key::Char('R') => view.selected = rows - 2,
        // `d`: back to the default.
        Key::Char('d') if matches!(view.row(), Some(Row::Setting(_))) => save(view, None),
        _ => {}
    }
    true
}

/// Typing a value: Enter saves it, Esc cancels.
fn edit_key(view: &mut View, key: crate::tui::keys::Key) {
    use crate::tui::keys::Key;
    let Some(text) = &mut view.edit else { return };
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
}

/// Picking from a list: typing filters it, ↑ ↓ choose, Enter saves the value chosen, Esc closes it.
fn list_key(view: &mut View, key: crate::tui::keys::Key) {
    use crate::tui::keys::Key;
    let Some(list) = &mut view.list else { return };
    match key {
        Key::Esc => view.list = None,
        Key::Up => list.selected = list.selected.saturating_sub(1),
        Key::Down | Key::Tab => list.selected += 1,
        Key::Backspace => {
            list.query.pop();
            list.selected = 0;
        }
        Key::Char(c) => {
            list.query.push(c);
            list.selected = 0;
        }
        Key::Enter => {
            let selected = list.selected;
            let picked = view.choices().get(selected).map(|(value, _)| value.clone());
            view.list = None;
            if let Some(value) = picked {
                save(view, Some(&value));
            }
            return;
        }
        _ => {}
    }
    let last = view.choices().len().saturating_sub(1);
    if let Some(list) = &mut view.list {
        list.selected = list.selected.min(last);
    }
}

/// The list of the setting chosen, on its value.
fn open_list(view: &mut View) {
    let Some(Row::Setting(k)) = view.row() else { return };
    view.list = Some(List::default());
    let value = view.settings.get(k.key).to_string();
    let at = view.choices().iter().position(|(v, _)| *v == value).unwrap_or(0);
    view.list = Some(List { query: String::new(), selected: at });
}

/// The advanced settings shown or hidden; the row chosen stays the same one when it can.
fn toggle_advanced(view: &mut View) {
    let chosen = view.row();
    view.advanced = !view.advanced;
    view.selected = chosen.and_then(|c| view.rows().iter().position(|r| *r == c)).unwrap_or(view.rows().len() - 3);
}

fn reset(view: &mut View) {
    view.message = Some(match settings::reset_in(&view.file) {
        Ok(saved) => {
            view.settings = saved;
            "reset: every setting is back to its default".to_string()
        }
        Err(e) => e,
    });
}

/// A yes/no or a choice: the next (or previous) value.
fn step(view: &mut View, forward: bool) {
    let Some(Row::Setting(k)) = view.row() else { return };
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
    let Some(Row::Setting(k)) = view.row() else { return };
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
        view.message = Some("offline setting on: no sync".into());
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
    view.message = match started {
        Some(_) => None,
        None => Some("could not start the download".into()),
    };
    view.syncing = started.is_some().then(std::time::Instant::now);
    (view.progress, view.outcome) = (None, None);
}

/// A sync by hand from this window: its progress read again (the bar, the speed); at its end, its
/// report as the message. `true` while it runs: the frame is drawn again (the bar moves).
pub fn poll_sync(view: &mut View) -> bool {
    let Some(started) = view.syncing else { return false };
    match aequalis::online::read_progress() {
        Some(aequalis::online::Syncing { done: Some((done, failed)), .. }) => {
            view.outcome = Some(failed.is_empty());
            let all: Vec<String> = done.into_iter().chain(failed).collect();
            view.message = Some(if all.is_empty() { "synced".to_string() } else { all.join(", ") });
            (view.syncing, view.progress) = (None, None);
            if let Some(path) = aequalis::online::progress_path() {
                let _ = std::fs::remove_file(path);
            }
        }
        // It stopped writing, or never started: it died.
        Some(p) if p.age > 30 => {
            (view.syncing, view.progress, view.message, view.outcome) = (None, None, Some("sync stopped".into()), Some(false));
        }
        None if started.elapsed().as_secs() > 10 => {
            (view.syncing, view.progress, view.message, view.outcome) = (None, None, Some("sync stopped".into()), Some(false));
        }
        p => view.progress = p,
    }
    true
}

/// The Sync row's label: `Sync`; while a sync runs `Syncing ████░░░░░░ 40%`, or a block moving
/// across while there is no percentage (installing); at its end `Synced`, or `Sync failed`.
pub fn sync_label(view: &View) -> String {
    match (sync_bar(view), view.outcome) {
        (Some(bar), _) => bar,
        (None, Some(true)) => "Synced".to_string(),
        (None, Some(false)) => "Sync failed".to_string(),
        (None, None) => DOWNLOAD.to_string(),
    }
}

/// What a running sync says under the window: its step, and the speed while downloading
/// (`downloading the city archive · 400 kB/s`).
pub fn sync_status(view: &View) -> Option<String> {
    view.syncing?;
    let p = view.progress.clone().unwrap_or_default();
    let step = if p.phase.is_empty() { "starting".to_string() } else { p.phase };
    Some(match step.starts_with("downloading") {
        true => format!("{step} · {}", aequalis::online::speed_text(p.speed)),
        false => step,
    })
}

/// The Sync row while a sync runs: `Syncing ████░░░░░░ 40%`, or a block moving across while
/// there is no percentage yet.
pub fn sync_bar(view: &View) -> Option<String> {
    const CELLS: usize = 10;
    let started = view.syncing?;
    let bar = match view.progress.as_ref().and_then(|p| p.fraction) {
        Some(f) => {
            let filled = ((f * CELLS as f64).round() as usize).min(CELLS);
            format!("{}{} {:.0}%", "█".repeat(filled), "░".repeat(CELLS - filled), f * 100.0)
        }
        None => {
            let at = (started.elapsed().as_millis() / 150) as usize % (CELLS + 2);
            (0..CELLS).map(|i| if i + 2 >= at && i < at { '█' } else { '░' }).collect()
        }
    };
    Some(format!("Syncing {bar}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sync_row_while_it_runs() {
        let mut v = View::default();
        assert_eq!(sync_bar(&v), None, "Sync, when none runs");
        v.syncing = Some(std::time::Instant::now());
        assert_eq!(sync_bar(&v).unwrap(), "Syncing ░░░░░░░░░░", "no percentage yet: the block not in yet");
        v.progress = Some(aequalis::online::Syncing { fraction: Some(0.0), ..Default::default() });
        assert_eq!(sync_bar(&v).unwrap(), "Syncing ░░░░░░░░░░ 0%");
        v.progress = Some(aequalis::online::Syncing { fraction: Some(0.4), speed: 400_000, ..Default::default() });
        assert_eq!(sync_bar(&v).unwrap(), "Syncing ████░░░░░░ 40%");
        // What it is doing, and the speed while downloading.
        v.progress =
            Some(aequalis::online::Syncing { fraction: Some(0.4), speed: 400_000, phase: "downloading the city archive".into(), ..Default::default() });
        assert_eq!(sync_status(&v).as_deref(), Some("downloading the city archive · 400 kB/s"));
        v.progress = Some(aequalis::online::Syncing { phase: "installing the city archive".into(), ..Default::default() });
        assert_eq!(sync_status(&v).as_deref(), Some("installing the city archive"));
        // Its end: the report as the message, the row green (everything synced) or orange.
        let _m = aequalis::machine::Mock::italy().install();
        let path = aequalis::online::progress_path().unwrap();
        std::fs::write(&path, r#"{"speed":0,"phase":"","done":["9 cities","166 exchange rates"],"failed":[]}"#).unwrap();
        assert!(poll_sync(&mut v));
        assert_eq!((v.syncing, v.message.as_deref(), sync_label(&v).as_str()), (None, Some("9 cities, 166 exchange rates"), "Synced"));
        assert!(!path.exists(), "read once");
        v.syncing = Some(std::time::Instant::now());
        std::fs::write(&path, r#"{"speed":0,"phase":"","done":["9 cities"],"failed":["rates: offline"]}"#).unwrap();
        poll_sync(&mut v);
        assert_eq!((v.outcome, sync_label(&v).as_str()), (Some(false), "Sync failed"));
    }
    use crate::tui::keys::Key;

    /// The window on a settings file of its own (`offline` on: no download starts).
    fn view(name: &str) -> View {
        let dir = std::env::temp_dir().join(format!("aequalis-settings-window-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("settings.json");
        settings::change_in(&file, "offline", Some("true")).unwrap();
        View::on(file)
    }

    /// Selects the row of the setting `key` (the advanced ones shown).
    fn row(v: &mut View, key: &str) {
        v.advanced = true;
        v.selected = v.rows().iter().position(|r| matches!(r, Row::Setting(k) if k.key == key)).unwrap();
    }

    fn saved(v: &View, key: &str) -> String {
        settings::load_from(&v.file).unwrap().get(key).to_string()
    }

    #[test]
    fn moving_around() {
        let mut v = view("moving");
        assert!(key(&mut v, Key::Up));
        assert_eq!(v.selected, v.rows().len() - 1, "circular");
        key(&mut v, Key::Down);
        assert_eq!(v.selected, 0);
        assert!(!key(&mut v, Key::Esc), "Esc closes");
    }

    #[test]
    fn yes_no_and_choices_change_in_place() {
        let mut v = view("choices");
        row(&mut v, "cities.enabled");
        key(&mut v, Key::Enter);
        assert_eq!((saved(&v, "cities.enabled").as_str(), v.message.as_deref()), ("false", Some("saved: cities.enabled = false")));
        key(&mut v, Key::Char(' '));
        assert_eq!(saved(&v, "cities.enabled"), "true");
        // A choice: forward and back, circular.
        row(&mut v, "cities.refresh");
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
    fn lists_for_the_currency_and_the_choices() {
        let mut v = view("lists");
        // The currency: Enter opens the list on its value (auto, as it stands), typing filters it.
        row(&mut v, "currency.local");
        key(&mut v, Key::Enter);
        assert_eq!((v.list.as_ref().map(|l| l.selected), v.choices()[0].0.as_str()), (Some(0), "auto"));
        assert!(v.choices()[0].1.starts_with("auto"), "{:?}", v.choices()[0]);
        for c in "dirham".chars() {
            key(&mut v, Key::Char(c));
        }
        assert_eq!(v.choices()[0].0, "AED", "{:?}", v.choices());
        key(&mut v, Key::Enter);
        assert_eq!((v.list.is_none(), saved(&v, "currency.local").as_str()), (true, "AED"));
        // Esc closes it, nothing saved.
        key(&mut v, Key::Enter);
        key(&mut v, Key::Down);
        key(&mut v, Key::Esc);
        assert_eq!((v.list.is_none(), saved(&v, "currency.local").as_str()), (true, "AED"));
        // A choice: its values, `auto` as it stands; ← → still step without the list.
        row(&mut v, "distance.unit");
        key(&mut v, Key::Enter);
        let values: Vec<String> = v.choices().into_iter().map(|(value, _)| value).collect();
        assert_eq!(values, ["auto", "km", "mi"]);
        key(&mut v, Key::Down);
        key(&mut v, Key::Enter);
        assert_eq!(saved(&v, "distance.unit"), "km");
        key(&mut v, Key::Right);
        assert_eq!(saved(&v, "distance.unit"), "mi");
    }

    #[test]
    fn typed_values_are_checked() {
        let mut v = view("typed");
        row(&mut v, "rates.interval");
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
        row(&mut v, "precision");
        key(&mut v, Key::Enter);
        key(&mut v, Key::Backspace);
        key(&mut v, Key::Char('2'));
        key(&mut v, Key::Enter);
        assert!(v.message.as_deref().is_some_and(|m| m.contains("restart the interactive mode")));
    }

    #[test]
    fn no_download_offline() {
        let mut v = view("download");
        v.selected = v.rows().len() - 1;
        key(&mut v, Key::Enter);
        assert_eq!(v.message.as_deref(), Some("offline setting on: no sync"));
    }

    #[test]
    fn it_opens_on_the_few_and_shows_the_rest_on_request() {
        let mut v = view("advanced");
        let keys = |v: &View| v.rows().iter().filter(|r| matches!(r, Row::Setting(_))).count();
        assert_eq!((keys(&v), v.rows().len()), (5, 8), "5 settings, Advanced, Reset, Download");
        v.selected = v.rows().len() - 3;
        key(&mut v, Key::Enter);
        assert_eq!((v.advanced, keys(&v)), (true, KEYS.iter().filter(|k| k.here()).count()));
        assert!(matches!(v.row(), Some(Row::Advanced)), "the row stays chosen");
        key(&mut v, Key::Char('a'));
        assert_eq!((v.advanced, keys(&v)), (false, 5));
        // Ctrl+K on an advanced setting lists them.
        assert!(View::at("rates.interval").advanced && !View::at("precision").advanced);
        assert!(matches!(View::at("rates.interval").row(), Some(Row::Setting(k)) if k.key == "rates.interval"));
    }

    #[test]
    fn reset_asks_twice_and_puts_every_default_back() {
        let mut v = view("reset");
        row(&mut v, "rates.interval");
        key(&mut v, Key::Char('x'));
        key(&mut v, Key::Char('R'));
        assert!(matches!(v.row(), Some(Row::Reset)));
        key(&mut v, Key::Enter);
        assert!(v.confirm_reset && v.message.as_deref().is_some_and(|m| m.contains("Enter again")));
        // Another key cancels.
        key(&mut v, Key::Up);
        key(&mut v, Key::Down);
        key(&mut v, Key::Enter);
        assert!(v.confirm_reset, "asked again, not done");
        assert_eq!(saved(&v, "offline"), "true");
        key(&mut v, Key::Enter);
        assert_eq!((saved(&v, "offline").as_str(), v.settings.get("offline")), ("false", "false"));
        // The hidden ones changed are told.
        let mut v = view("hidden");
        settings::change_in(&v.file, "cities.refresh", Some("silent")).unwrap();
        v.settings = settings::load_from(&v.file).unwrap();
        assert_eq!(v.hidden_changes(), 1);
    }
}
