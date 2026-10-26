//! The online data, in three groups, each with its refresh policy and interval in the settings
//! (`settings.rs`): the Trivia's live values (`live`: the Voyagers' distances, the satellite counts)
//! in `~/.config/aequalis/online.json`, the exchange rates (`rates`, `currency.rs`) in
//! `currencies.json`, the city archive (`cities`, `cities.rs`). Aequalis reads only these files: no
//! request ever waits for the network. Under the `silent` policy, any run of aequalis starts a sync
//! in the background (`aequalis --sync --auto`, a separate process) when a group is out of date,
//! unless one is running (`online.lock`) or the `offline` setting is on; under `notify` it says so
//! (`notify.rs`); under `manual` only `aequalis --sync` (or Settings) downloads.
//!
//! The sync may be interrupted at any time and the file stays consistent: it is never edited in
//! place, a new version is written next to it and renamed over it (atomic), after every 10 values
//! or every 20 seconds, and at the end. An interrupted sync is resumed by the next one, which only
//! fetches the values still older than the interval.
//!
//! ```json
//! {"format":1,"synced_at":1791718112,"attempted_at":1791718112,"ok":true,
//!  "values":{"Voyager 1":{"value":172.4223,"at":1791718110},"Starlink":{"value":11129,"at":1791718112}}}
//! ```

use crate::json::{self, Value};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// After a failed sync (offline), the next try waits this long (or the interval, if shorter).
const RETRY_AFTER: i64 = 3600;
/// A new version of the file every this many values, or this many seconds.
pub const BATCH: usize = 10;
pub const BATCH_SECONDS: u64 = 20;
/// A lock older than this is from a sync that died: taken over.
const STALE_LOCK: i64 = 15 * 60;
/// Real seconds between two background syncs started by runs of aequalis: the time a sync has to
/// take its lock, while the runs around it start none.
const SPAWN_GAP: u64 = 60;

/// A value and when it was synced (Unix seconds).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stored {
    pub value: f64,
    pub at: i64,
}

/// The file's content.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Data {
    /// When the last complete sync ended: every value fetched (or still fresh).
    pub synced_at: Option<i64>,
    /// When the last sync started, and whether every source answered.
    pub attempted_at: Option<i64>,
    pub ok: bool,
    /// When the last sync run (by hand or in the background) ended having synced something, even
    /// if a source did not answer: the data's age for people.
    pub run_at: Option<i64>,
    /// By Trivia entry name.
    pub values: Vec<(String, Stored)>,
}

impl Data {
    /// When the data was last synced, for people: the last sync run that synced something, or the
    /// last complete sync, whichever is later; else the newest value (a sync that did not reach
    /// every source, before runs were recorded).
    pub fn last_sync(&self) -> Option<i64> {
        self.synced_at.max(self.run_at).or_else(|| self.values.iter().map(|(_, s)| s.at).max())
    }

    pub fn get(&self, name: &str) -> Option<Stored> {
        self.values.iter().find(|(n, _)| n == name).map(|(_, s)| *s)
    }

    pub fn set(&mut self, name: &str, value: f64, at: i64) {
        match self.values.iter_mut().find(|(n, _)| n == name) {
            Some((_, s)) => *s = Stored { value, at },
            None => self.values.push((name.to_string(), Stored { value, at })),
        }
    }

    pub fn to_json(&self) -> String {
        let values = self.values.iter().fold(json::Object::new(), |o, (n, s)| o.raw(n, &json::Object::new().num("value", s.value).num("at", s.at).finish()));
        json::Object::new()
            .num("format", 1)
            .opt("synced_at", self.synced_at)
            .opt("attempted_at", self.attempted_at)
            .num("ok", self.ok)
            .opt("run_at", self.run_at)
            .raw("values", &values.finish())
            .finish()
            + "\n"
    }

    pub fn from_json(text: &str) -> Option<Data> {
        let doc = json::parse(text).ok()?;
        let time = |k: &str| doc.get(k).and_then(Value::number).map(|n| n as i64);
        Some(Data {
            synced_at: time("synced_at"),
            attempted_at: time("attempted_at"),
            ok: doc.get("ok") == Some(&Value::Bool(true)),
            run_at: time("run_at"),
            values: doc
                .get("values")
                .map(Value::members)
                .unwrap_or_default()
                .iter()
                .filter_map(|(n, v)| Some((n.clone(), Stored { value: v.get("value")?.number()?, at: v.get("at")?.number()? as i64 })))
                .collect(),
        })
    }
}

// ---------------------------------------------------------------- the file

pub fn path() -> Option<PathBuf> {
    crate::files::dir().map(|d| d.join("online.json"))
}

/// The stored data; empty when there is no file (or it cannot be read).
pub fn load() -> Data {
    path().map(|p| load_from(&p)).unwrap_or_default()
}

pub fn load_from(path: &Path) -> Data {
    std::fs::read_to_string(path).ok().and_then(|t| Data::from_json(&t)).unwrap_or_default()
}

/// Writes a new version next to the file, then renames it over the file: readers see the old
/// version or the new one, never a part.
pub fn save_to(path: &Path, data: &Data) -> std::io::Result<()> {
    crate::files::write_atomic(path, data.to_json().as_bytes())
}

// ---------------------------------------------------------------- when

/// Whether the data needs a sync at `now`: never synced, or synced longer ago than `interval`;
/// after a failed try, not before `RETRY_AFTER` (or the interval) has passed.
pub fn expired(data: &Data, now: i64, interval: i64) -> bool {
    if let (Some(tried), false) = (data.attempted_at, data.ok) {
        if now - tried < RETRY_AFTER.min(interval) {
            return false;
        }
    }
    data.synced_at.is_none_or(|t| now - t >= interval)
}

// ---------------------------------------------------------------- the background sync

/// A group of online data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Live,
    Rates,
    Holidays,
    Cities,
}

impl Group {
    /// In the order they are synced: the exchange rates first, the city archive (the big one) last.
    pub const ALL: [Group; 4] = [Group::Rates, Group::Live, Group::Holidays, Group::Cities];

    /// Its name in the settings (`rates.refresh`).
    pub fn name(self) -> &'static str {
        match self {
            Group::Live => "live",
            Group::Rates => "rates",
            Group::Holidays => "holidays",
            Group::Cities => "cities",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Group::Live => "the Trivia's live values",
            Group::Rates => "the exchange rates",
            Group::Holidays => "the public holidays",
            Group::Cities => "the city archive",
        }
    }
}

/// Whether a group's data is older than its interval (or missing). For the city archive: whether
/// it is time to look for a new one.
pub fn stale(group: Group, settings: &crate::settings::Settings, now: i64) -> bool {
    let interval = settings.interval(group.name());
    match group {
        Group::Live => expired(&load(), now, interval),
        Group::Rates => expired(&crate::currency::load(), now, interval),
        Group::Holidays => holidays_stale(crate::holidays::downloaded_at(), crate::notify::load().holidays_failed_at, now, interval),
        Group::Cities => {
            let checked = crate::cities::meta().map_or(0, |m| m.downloaded_at).max(crate::notify::load().cities_checked_at);
            now - checked >= interval
        }
    }
}

/// Whether the public holidays are out of date: never downloaded, or `interval` ago; but after a
/// failed download (`failed`, 0: none), not before `RETRY_AFTER` (or the interval) has passed, else
/// every run of aequalis would start a sync for them.
fn holidays_stale(downloaded: Option<i64>, failed: i64, now: i64, interval: i64) -> bool {
    let waiting = failed > 0 && now - failed < RETRY_AFTER.min(interval);
    !waiting && downloaded.is_none_or(|at| now - at >= interval)
}

/// The groups a background sync has work for: out of date under `silent`; the city archive's
/// check under `notify`.
pub fn due(settings: &crate::settings::Settings, now: i64) -> Vec<Group> {
    due_when(settings, &|g| stale(g, settings, now))
}

/// `due`, with `stale` saying which groups are out of date.
fn due_when(settings: &crate::settings::Settings, stale: &dyn Fn(Group) -> bool) -> Vec<Group> {
    use crate::settings::Refresh;
    Group::ALL
        .into_iter()
        .filter(|&g| g != Group::Cities || settings.cities_enabled())
        .filter(|&g| g != Group::Holidays || settings.holidays_enabled())
        .filter(|&g| match settings.refresh(g.name()) {
            Refresh::Silent => stale(g),
            Refresh::Notify => g == Group::Cities && stale(g),
            Refresh::Manual => false,
        })
        .collect()
}

/// Starts `aequalis --sync --auto` in the background when a group has work (and the network may be
/// used, and no sync is running). One per user, however many runs ask at once (`spawn_gate`).
/// Returns the process, for a long-running caller to reap.
pub fn ensure_fresh() -> Option<std::process::Child> {
    let settings = crate::settings::current();
    if settings.offline() || lock_held() || due(&settings, crate::time::unix_now()).is_empty() {
        return None;
    }
    // Held until the sync has started.
    let _gate = spawn_gate(&crate::files::dir()?.join("online.spawn"), std::time::SystemTime::now())?;
    #[cfg(windows)]
    use crate::windows::CommandExt;
    #[cfg(unix)]
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    let exe = std::env::current_exe().ok()?;
    Command::new(exe)
        .args(["--sync", "--auto"])
        // A frozen "now" (`--now`, tests) holds for the sync too.
        .args(crate::time::fixed_now().map(|t| format!("--now={t}")))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Its own process group: Ctrl-C in the terminal does not stop it.
        .process_group(0)
        .spawn()
        .ok()
}

/// The right to start a background sync, `now`: the file `path` locked (the system's lock, let go
/// when the process ends, even killed), holding when the last one was started (Unix seconds). `None`
/// while another run holds it, or a sync was started less than `SPAWN_GAP` ago (it has not taken
/// its own lock yet); else the time is written and the file stays locked until dropped. Runs that
/// ask at once start one sync between them, and a sync that keeps failing at most one a minute.
fn spawn_gate(path: &Path, now: std::time::SystemTime) -> Option<std::fs::File> {
    use std::io::{Read, Seek, Write};
    std::fs::create_dir_all(path.parent()?).ok()?;
    let mut file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path).ok()?;
    file.try_lock().ok()?;
    let now = now.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    let mut text = String::new();
    let _ = file.read_to_string(&mut text);
    // A time ahead of now (the clock put back): not trusted.
    if text.trim().parse::<u64>().is_ok_and(|last| last <= now && now - last < SPAWN_GAP) {
        return None;
    }
    file.set_len(0).ok()?;
    file.rewind().ok()?;
    file.write_all(now.to_string().as_bytes()).ok()?;
    Some(file)
}

fn lock_path() -> Option<PathBuf> {
    crate::files::dir().map(|d| d.join("online.lock"))
}

/// Whether a sync is running (a lock that is not stale).
fn lock_held() -> bool {
    let Some(path) = lock_path() else { return false };
    std::fs::metadata(&path).ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|age| (age.as_secs() as i64) < STALE_LOCK)
}

/// The lock, removed when dropped.
struct Lock(PathBuf);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn take_lock() -> Result<Lock, String> {
    let path = lock_path().ok_or("no home directory")?;
    std::fs::create_dir_all(path.parent().unwrap_or(&path)).map_err(|e| e.to_string())?;
    if !lock_held() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map(|mut f| {
            use std::io::Write;
            let _ = writeln!(f, "{}", std::process::id());
            Lock(path)
        })
        .map_err(|_| "a sync is already running".to_string())
}

/// A source of live values: its values' names, and how to fetch them (`None`: it did not answer).
pub struct Source {
    pub names: Vec<String>,
    pub fetch: Fetch,
}

/// How a source is fetched: its values by name, `None` when it did not answer.
pub type Fetch = Box<dyn Fn() -> Option<Vec<(String, f64)>>>;

/// `aequalis --sync` (`auto`: the background sync, `--auto`): under the lock, by hand every group
/// is downloaded again; the background sync does what the policies say (`due`). Returns a line
/// per group.
/// What a sync did: a line for each group that managed (`12 exchange rates`) and for each that did
/// not (`cities: could not download the city archive (…)`). Each group is synced on its own: one
/// that fails does not keep the others from their turn.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    pub done: Vec<String>,
    pub failed: Vec<String>,
}

/// Syncs every group (`auto`: those the refresh settings say are due). `Err`: no sync at all (the
/// offline setting, another sync running).
pub fn sync(auto: bool) -> Result<Report, String> {
    let settings = crate::settings::current();
    if settings.offline() {
        return Err("the offline setting is on: no sync".into());
    }
    let _lock = take_lock()?;
    let now = crate::time::unix_now();
    let groups: Vec<Group> = if auto { due(&settings, now) } else { Group::ALL.to_vec() };
    let no_home = || "no home directory".to_string();
    let mut report = Report::default();
    for group in groups {
        phase(match group {
            Group::Live => "downloading the live values",
            Group::Rates => "downloading the exchange rates",
            Group::Holidays => "downloading the public holidays",
            Group::Cities => "checking the city archive",
        });
        let interval = if auto { settings.interval(group.name()) } else { 0 };
        let line = match group {
            Group::Live => path().ok_or_else(no_home).map(|path| {
                let n = sync_into(&path, crate::trivia::sources(), interval, &crate::time::unix_now);
                format!("{n} live value{}", if n == 1 { "" } else { "s" })
            }),
            Group::Rates => crate::currency::path().ok_or_else(no_home).map(|path| {
                let n = sync_into(&path, vec![crate::currency::source()], interval, &crate::time::unix_now);
                let today = crate::time::unix_now().div_euclid(86_400);
                crate::history::prune(today);
                let past = if settings.history_enabled() {
                    phase("downloading the exchange rates of past days");
                    crate::history::sync(today)
                } else {
                    0
                };
                let days = if past > 0 { format!(", {past} past day{}", if past == 1 { "" } else { "s" }) } else { String::new() };
                format!("{n} exchange rate{}{days}", if n == 1 { "" } else { "s" })
            }),
            Group::Holidays => {
                let result = crate::holidays::sync(&settings);
                let mut state = crate::notify::load();
                state.holidays_failed_at = if result.is_err() { now } else { 0 };
                crate::notify::save(&state);
                result
            }
            Group::Cities => crate::files::dir()
                .ok_or_else(no_home)
                .and_then(|base| sync_cities(&base, &settings, auto, now, &|text| crate::notify::desktop("Aequalis", text))),
        };
        match line {
            Ok(line) => report.done.push(line),
            Err(e) => report.failed.push(format!("{}: {e}", group.name())),
        }
    }
    phase("");
    // Something synced: the data's age for people starts again (`Data::last_sync`), even when a
    // source did not answer.
    if !report.done.is_empty() {
        if let Some(path) = path() {
            let mut data = load_from(&path);
            data.run_at = Some(crate::time::unix_now());
            let _ = save_to(&path, &data);
        }
    }
    Ok(report)
}

// ---------------------------------------------------------------- a sync by hand's progress

/// A sync by hand's progress, as the interactive mode shows it (`aequalis --sync` writes it to
/// `sync-progress.json`): how far the big download is, the speed, and at the end the report.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Syncing {
    /// How far the big download is, 0 to 1; `None`: no percentage (nothing big downloading yet).
    pub fraction: Option<f64>,
    /// The current download speed, bytes a second.
    pub speed: u64,
    /// What the sync is doing: `downloading the city archive`, `installing the city archive`.
    pub phase: String,
    /// Ended: what was synced, and what failed.
    pub done: Option<(Vec<String>, Vec<String>)>,
    /// Seconds since it was last written: a sync that died stops writing.
    pub age: u64,
}

/// A download from this size shows its percentage; the smaller ones (the rates, the live values)
/// finish too fast to show and leave it as it is.
const BIG: u64 = 200_000;

/// A download speed for people, as browsers write it (1000-based): `800 B/s`, `400 kB/s`, `1.2 MB/s`.
pub fn speed_text(bytes_per_second: u64) -> String {
    match bytes_per_second {
        b if b < 1_000 => format!("{b} B/s"),
        b if b < 1_000_000 => format!("{} kB/s", b / 1_000),
        b => format!("{:.1} MB/s", b as f64 / 1_000_000.0),
    }
}

/// The progress file of a sync by hand.
pub fn progress_path() -> Option<PathBuf> {
    crate::files::dir().map(|d| d.join("sync-progress.json"))
}

/// What the running sync is doing, for the apps (`ffi::sync_phase`); empty: nothing.
static PHASE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// The sync's next step: `downloading the exchange rates`, `installing the city archive` (empty:
/// done). Written to the progress file too while a sync by hand is tracked.
pub fn phase(text: &str) {
    *PHASE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = text.to_string();
    // A new step has no percentage yet (installing shows none at all).
    let tracked = TRACKED.with(|t| t.borrow_mut().as_mut().map(|s| s.0 = None).is_some());
    if tracked {
        write_progress(text, None, 0, None);
    }
}

/// What the running sync is doing (`phase`); empty when none runs.
pub fn current_phase() -> String {
    PHASE.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

thread_local! {
    /// A sync by hand tracked on this thread (a sync runs on one, curl's meter read there too): the
    /// big download's percentage so far, and when the file was last written.
    static TRACKED: std::cell::RefCell<Option<(Option<f64>, Instant)>> = const { std::cell::RefCell::new(None) };
}

fn write_progress(phase: &str, fraction: Option<f64>, speed: u64, done: Option<&Report>) {
    let Some(path) = progress_path() else { return };
    let mut text = format!("{{\"speed\":{speed},\"phase\":{}", json::string(phase));
    if let Some(f) = fraction {
        text += &format!(",\"fraction\":{f:.4}");
    }
    if let Some(r) = done {
        let list = |l: &[String]| l.iter().map(|s| json::string(s)).collect::<Vec<_>>().join(",");
        text += &format!(",\"done\":[{}],\"failed\":[{}]", list(&r.done), list(&r.failed));
    }
    text += "}";
    let _ = crate::files::write_atomic(&path, text.as_bytes());
}

/// A sync by hand starts: its downloads' progress written to the progress file from now on, at
/// most every 200 ms, the big download's percentage kept while small ones run.
pub fn track() {
    TRACKED.with(|t| *t.borrow_mut() = Some((None, Instant::now())));
    write_progress(&current_phase(), None, 0, None);
    crate::http::observe(Some(Box::new(|p: crate::http::Progress| {
        let due = TRACKED.with(|t| {
            let mut tracked = t.borrow_mut();
            let s = tracked.as_mut()?;
            if let Some(total) = p.total.filter(|&t| t >= BIG) {
                s.0 = Some((p.received as f64 / total as f64).min(1.0));
            }
            (s.1.elapsed().as_millis() >= 200 || s.0 == Some(1.0)).then(|| {
                s.1 = Instant::now();
                s.0
            })
        });
        if let Some(fraction) = due {
            write_progress(&current_phase(), fraction, p.speed, None);
        }
    })));
}

/// The sync by hand ended: its report written, for the interactive mode to show.
pub fn tracked(result: &Result<Report, String>) {
    crate::http::observe(None);
    TRACKED.with(|t| *t.borrow_mut() = None);
    let report = match result {
        Ok(r) => r.clone(),
        Err(e) => Report { done: Vec::new(), failed: vec![e.clone()] },
    };
    write_progress("", None, 0, Some(&report));
}

/// Where a sync is, for a UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncState {
    /// None runs, none ended lately.
    #[default]
    Idle,
    Running,
    /// Ended, everything synced.
    Done,
    /// Ended with something that failed, or died.
    Failed,
}

impl SyncState {
    pub fn name(self) -> &'static str {
        match self {
            SyncState::Idle => "idle",
            SyncState::Running => "running",
            SyncState::Done => "done",
            SyncState::Failed => "failed",
        }
    }
}

/// What every UI says of a sync, written here once (the step and the speed, the button's text, the
/// bar's fraction, how it ended), from the progress a sync by hand writes (`track`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SyncView {
    pub state: SyncState,
    /// While it runs: its step, and the speed while downloading (`downloading the city archive · 400 kB/s`).
    pub line: String,
    /// The Sync button's text: `Sync`, `Syncing 40%`, `Syncing…`, then `Synced` or `Sync failed`.
    pub label: String,
    /// The big download's progress, 0 to 1; `None`: no percentage (checking, installing).
    pub fraction: Option<f64>,
    /// How it ended: what was synced and what failed, on one line.
    pub message: String,
}

/// A sync that stopped writing for this long (seconds) died; one that ended this long ago is history.
const DIED_AFTER: u64 = 30;
const FORGOTTEN_AFTER: u64 = 120;

/// The step of a running sync with its speed while it downloads (`downloading the city archive · 400 kB/s`).
pub fn step_line(phase: &str, speed: u64) -> String {
    let step = if phase.is_empty() { "starting" } else { phase };
    match step.starts_with("downloading") {
        true => format!("{step} · {}", speed_text(speed)),
        false => step.to_string(),
    }
}

/// How the sync whose progress was read (`read_progress`) looks to a UI.
pub fn view_of(progress: Option<&Syncing>) -> SyncView {
    let Some(p) = progress else { return SyncView { label: "Sync".into(), ..SyncView::default() } };
    let ended = |state, label: &str, message: String| SyncView { state, label: label.into(), message, ..SyncView::default() };
    match &p.done {
        // Ended: what it says for a little while.
        Some((done, failed)) if p.age <= FORGOTTEN_AFTER => {
            let all: Vec<String> = done.iter().chain(failed).cloned().collect();
            let message = if all.is_empty() { "synced".to_string() } else { all.join(", ") };
            match failed.is_empty() {
                true => ended(SyncState::Done, "Synced", message),
                false => ended(SyncState::Failed, "Sync failed", message),
            }
        }
        Some(_) => view_of(None),
        // Stopped writing: it died.
        None if p.age > FORGOTTEN_AFTER => view_of(None),
        None if p.age > DIED_AFTER => ended(SyncState::Failed, "Sync failed", "sync stopped".into()),
        None => SyncView {
            state: SyncState::Running,
            line: step_line(&p.phase, p.speed),
            label: match p.fraction {
                Some(f) => format!("Syncing {:.0}%", f * 100.0),
                None => "Syncing…".to_string(),
            },
            fraction: p.fraction,
            message: String::new(),
        },
    }
}

/// `view_of` the sync by hand now running, or that ended lately (`read_progress`).
pub fn sync_view() -> SyncView {
    view_of(read_progress().as_ref())
}

/// The progress of a sync by hand, if one wrote it.
pub fn read_progress() -> Option<Syncing> {
    let path = progress_path()?;
    let age = std::fs::metadata(&path).ok()?.modified().ok()?.elapsed().map_or(0, |d| d.as_secs());
    let doc = json::parse(&std::fs::read_to_string(&path).ok()?).ok()?;
    let list = |key: &str| doc.get(key).map(|v| v.array().iter().filter_map(|s| s.str().map(String::from)).collect::<Vec<_>>());
    Some(Syncing {
        fraction: doc.get("fraction").and_then(Value::number),
        speed: doc.get("speed").and_then(Value::number).unwrap_or(0.0) as u64,
        phase: doc.get("phase").and_then(Value::str).unwrap_or_default().to_string(),
        done: list("done").map(|done| (done, list("failed").unwrap_or_default())),
        age,
    })
}

/// A sync by hand skips a city archive younger than this: a month.
const FRESH_CITIES: i64 = 30 * 86_400;

/// The city archive: under `notify` (in the background) only its version is checked, and a newer
/// one noted (with a desktop notification, once); otherwise downloaded when newer (by hand:
/// always).
/// In the folder `base` (the archive, `state.json`); `notify` shows a desktop notification.
fn sync_cities(base: &Path, settings: &crate::settings::Settings, auto: bool, now: i64, notify: &dyn Fn(&str)) -> Result<String, String> {
    if !settings.cities_enabled() {
        return Ok("cities: off (the cities.enabled setting)".into());
    }
    // By hand, an archive downloaded less than a month ago is kept: it is rebuilt monthly.
    if let Some(m) = crate::cities::meta_in(base).filter(|m| !auto && now - m.downloaded_at < FRESH_CITIES) {
        return Ok(format!("cities: up to date (downloaded {} ago)", crate::network::ago(now - m.downloaded_at)));
    }
    let url = settings.get("cities.url");
    let state_path = base.join("state.json");
    let mut state = crate::notify::load_from(&state_path);
    let current = crate::cities::meta_in(base).map(|m| m.version).unwrap_or_default();
    let published = crate::cities::remote_version(url);
    state.cities_checked_at = now;
    if let Some(v) = &published {
        state.cities_available = v.clone();
    }
    let newer = published.as_deref().is_some_and(|v| v != current);
    if auto && settings.refresh("cities") == crate::settings::Refresh::Notify {
        if newer && state.cities_notified != state.cities_available {
            notify("A new city archive is available: aequalis --sync");
            state.cities_notified = state.cities_available.clone();
        }
        crate::notify::save_to(&state_path, &state);
        return Ok(if newer { "cities: a new archive is available".into() } else { "cities: up to date".into() });
    }
    crate::notify::save_to(&state_path, &state);
    if auto && !newer {
        return Ok("cities: up to date".into());
    }
    let count = crate::cities::download(base, url, now)?;
    Ok(format!("{count} cities"))
}

/// The sync itself, on `path`: every source whose values are older than `interval` is fetched in
/// turn; a new version of the file is saved after every `BATCH` values or `BATCH_SECONDS`, and at
/// the end (then `synced_at`, if every source answered).
pub fn sync_into(path: &Path, sources: Vec<Source>, interval: i64, clock: &dyn Fn() -> i64) -> usize {
    let mut data = load_from(path);
    let started = clock();
    data.attempted_at = Some(started);
    data.ok = false;
    let (mut updated, mut pending, mut all_answered) = (0, 0, true);
    let mut last_save = Instant::now();
    for source in sources {
        let fresh = source.names.iter().all(|n| data.get(n).is_some_and(|s| started - s.at < interval));
        if fresh {
            continue;
        }
        match (source.fetch)() {
            // "Nothing new" (CelesTrak within its 2 hours) with nothing stored yet: not answered,
            // so it is tried again later.
            Some(values) if values.is_empty() && source.names.iter().any(|n| data.get(n).is_none()) => all_answered = false,
            Some(values) => {
                let at = clock();
                for (name, value) in values {
                    data.set(&name, value, at);
                    updated += 1;
                    pending += 1;
                }
            }
            None => all_answered = false,
        }
        if (pending >= BATCH || last_save.elapsed().as_secs() >= BATCH_SECONDS) && save_to(path, &data).is_ok() {
            pending = 0;
            last_save = Instant::now();
        }
    }
    data.ok = all_answered;
    if all_answered {
        data.synced_at = Some(clock());
    }
    let _ = save_to(path, &data);
    updated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_background_sync_started_at_a_time() {
        let dir = std::env::temp_dir().join(format!("aequalis-spawn-{}", std::process::id()));
        let path = dir.join("online.spawn");
        let at = |s: u64| std::time::UNIX_EPOCH + std::time::Duration::from_secs(s);
        let first = spawn_gate(&path, at(1_000)).expect("the first run starts one");
        assert!(spawn_gate(&path, at(1_100)).is_none(), "another run while it is held: none");
        drop(first);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "1000");
        assert!(spawn_gate(&path, at(1_000 + SPAWN_GAP - 1)).is_none(), "one started a moment ago: none");
        assert!(spawn_gate(&path, at(1_000 + SPAWN_GAP)).is_some(), "a minute on: one more");
        assert!(spawn_gate(&path, at(500)).is_some(), "the clock put back: the time written is not trusted");
        std::fs::write(&path, "not a time").unwrap();
        assert!(spawn_gate(&path, at(2_000)).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn holidays_that_failed_wait_an_hour() {
        let day = 86_400;
        assert!(holidays_stale(None, 0, 10_000, day), "never downloaded");
        assert!(!holidays_stale(None, 10_000 - 60, 10_000, day), "failed a minute ago: not yet");
        assert!(holidays_stale(None, 10_000 - RETRY_AFTER, 10_000, day), "an hour later: again");
        assert!(!holidays_stale(Some(10_000 - 60), 0, 10_000, day), "downloaded a minute ago");
        assert!(holidays_stale(Some(10_000 - day), 0, 10_000, day), "a day old");
        // An interval shorter than the hour: retried after the interval.
        assert!(holidays_stale(None, 10_000 - 600, 10_000, 600));
    }

    #[test]
    fn the_age_starts_again_with_a_partial_sync() {
        // A complete sync at 1000; a later run at 5000 where a source did not answer: 5000.
        let data = Data { synced_at: Some(1_000), run_at: Some(5_000), ..Data::default() };
        assert_eq!(data.last_sync(), Some(5_000));
        let data = Data { synced_at: Some(6_000), run_at: Some(5_000), ..Data::default() };
        assert_eq!(data.last_sync(), Some(6_000));
        let back = Data::from_json(&Data { run_at: Some(5_000), ..Data::default() }.to_json()).unwrap();
        assert_eq!(back.run_at, Some(5_000));
    }

    #[test]
    fn speeds_as_browsers_write_them() {
        assert_eq!((speed_text(800), speed_text(400_123), speed_text(1_234_567)), ("800 B/s".to_string(), "400 kB/s".to_string(), "1.2 MB/s".to_string()));
    }

    #[test]
    fn a_sync_by_hand_writes_its_progress() {
        let _m = crate::machine::Mock::italy().install();
        track();
        let start = read_progress().unwrap();
        assert_eq!((start.fraction, start.speed, start.done), (None, 0, None));
        // A small download leaves no percentage; a big one says how far it is.
        let observe = |p| crate::http::OBSERVER.lock().unwrap().as_ref().map(|o| o(p));
        observe(crate::http::Progress { received: 50_000, total: Some(100_000), speed: 400_000 });
        std::thread::sleep(std::time::Duration::from_millis(210));
        observe(crate::http::Progress { received: 8_000_000, total: Some(20_000_000), speed: 1_200_000 });
        let p = read_progress().unwrap();
        assert_eq!((p.fraction, p.speed), (Some(0.4), 1_200_000));
        // A new step: no percentage yet, what it is doing said.
        phase("installing the city archive");
        let p = read_progress().unwrap();
        assert_eq!((p.fraction, p.phase.as_str()), (None, "installing the city archive"));
        phase("");
        tracked(&Ok(Report { done: vec!["9 cities".into()], failed: vec!["rates: offline".into()] }));
        assert_eq!(read_progress().unwrap().done, Some((vec!["9 cities".to_string()], vec!["rates: offline".to_string()])));
        assert!(crate::http::OBSERVER.lock().unwrap().is_none(), "no more observed");
    }
    use std::cell::Cell;
    use std::rc::Rc;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aequalis-online-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("online.json")
    }

    /// What the background sync does, by policy, for out-of-date and fresh data.
    #[test]
    fn the_policies_decide_what_is_due() {
        use crate::settings::Settings;
        let with = |pairs: &[(&str, &str)]| {
            let mut s = Settings::default();
            for (k, v) in pairs {
                s.set(k, Some(v)).unwrap();
            }
            s
        };
        let all_stale = |_: Group| true;
        let none_stale = |_: Group| false;
        // The defaults: cities manual, rates and holidays silent, live manual.
        assert_eq!(due_when(&Settings::default(), &all_stale), [Group::Rates, Group::Holidays]);
        assert!(due_when(&Settings::default(), &none_stale).is_empty());
        // Silent: when out of date; notify: only the archive's check; manual: never.
        let s = with(&[("cities.refresh", "silent"), ("live.refresh", "silent")]);
        assert_eq!(due_when(&s, &all_stale), [Group::Rates, Group::Live, Group::Holidays, Group::Cities], "the rates first, the archive last");
        let s = with(&[("cities.refresh", "notify"), ("rates.refresh", "notify"), ("live.refresh", "notify"), ("holidays.refresh", "notify")]);
        assert_eq!(due_when(&s, &all_stale), [Group::Cities], "rates, live values and holidays out of date are only said");
        assert_eq!(due_when(&s, &|g| g != Group::Cities), Vec::<Group>::new());
        let s = with(&[("rates.refresh", "manual"), ("holidays.refresh", "manual")]);
        assert!(due_when(&s, &all_stale).is_empty());
        // The cities off: never, whatever the policy; the holidays alike.
        let s = with(&[("cities.refresh", "silent"), ("cities.enabled", "false"), ("holidays", "false")]);
        assert_eq!(due_when(&s, &all_stale), [Group::Rates]);
    }

    /// The city archive's sync, from a local copy of the sample archive (`file://`, no network):
    /// under `notify` only its version is checked (one desktop notification per new archive), under
    /// `silent` it is downloaded when newer, by hand always.
    #[test]
    fn the_city_archive_by_policy() {
        use crate::settings::Settings;
        use std::cell::RefCell;
        let base = scratch("cities-sync");
        let served = base.join("served");
        std::fs::create_dir_all(&served).unwrap();
        let json = served.join("cities.json");
        std::fs::write(&json, crate::cities::tests::SAMPLE).unwrap();
        std::fs::write(served.join("cities.version"), "1791877958 sample\n").unwrap();
        let mut settings = Settings::default();
        let path = json.display().to_string().replace('\\', "/");
        settings.set("cities.url", Some(&format!("file://{}{path}", if path.starts_with('/') { "" } else { "/" }))).unwrap();
        settings.set("cities.refresh", Some("notify")).unwrap();
        let shown = RefCell::new(Vec::new());
        let notify = |text: &str| shown.borrow_mut().push(text.to_string());
        let home = base.join("home");
        // Notify: said once, nothing downloaded.
        assert_eq!(sync_cities(&home, &settings, true, 1_000, &notify).unwrap(), "cities: a new archive is available");
        assert_eq!(sync_cities(&home, &settings, true, 2_000, &notify).unwrap(), "cities: a new archive is available");
        assert_eq!(shown.borrow().len(), 1, "one notification per new archive");
        assert!(crate::cities::meta_in(&home).is_none());
        let state = crate::notify::load_from(&home.join("state.json"));
        assert_eq!((state.cities_available.as_str(), state.cities_checked_at), ("1791877958 sample", 2_000));
        // Silent: downloaded when newer, then up to date.
        settings.set("cities.refresh", Some("silent")).unwrap();
        assert_eq!(sync_cities(&home, &settings, true, 3_000, &notify).unwrap(), "9 cities");
        assert_eq!(sync_cities(&home, &settings, true, 4_000, &notify).unwrap(), "cities: up to date");
        // By hand: kept while less than a month old, then downloaded again.
        assert_eq!(sync_cities(&home, &settings, false, 5_000, &notify).unwrap(), "cities: up to date (downloaded 33m ago)");
        assert_eq!(sync_cities(&home, &settings, false, 3_000 + 30 * 86_400, &notify).unwrap(), "9 cities");
        // Off: nothing.
        settings.set("cities.enabled", Some("false")).unwrap();
        assert_eq!(sync_cities(&home, &settings, false, 6_000, &notify).unwrap(), "cities: off (the cities.enabled setting)");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn expiry_and_retry() {
        let day = 86_400;
        assert!(expired(&Data::default(), 1_000, day), "never synced");
        let fresh = Data { synced_at: Some(1_000), attempted_at: Some(1_000), ok: true, run_at: None, values: Vec::new() };
        assert!(!expired(&fresh, 1_000 + day - 1, day));
        assert!(expired(&fresh, 1_000 + day, day));
        // A failed try: not again for an hour.
        let failed = Data { synced_at: Some(0), attempted_at: Some(1_000_000), ok: false, run_at: None, values: Vec::new() };
        assert!(!expired(&failed, 1_000_000 + 60, day));
        assert!(expired(&failed, 1_000_000 + 3_600, day));
    }

    #[test]
    fn the_file_round_trips() {
        let path = scratch("round");
        let mut data = Data { synced_at: Some(5), attempted_at: Some(4), ok: true, run_at: None, values: Vec::new() };
        data.set("Voyager 1", 172.4223, 3);
        data.set("Starlink", 11129.0, 4);
        save_to(&path, &data).unwrap();
        assert_eq!(load_from(&path), data);
        assert!(std::fs::read_dir(path.parent().unwrap()).unwrap().count() == 1, "no temporary file left");
        // Missing or broken: empty.
        assert_eq!(load_from(&scratch("missing")), Data::default());
    }

    /// `n` sources of one value each; the one at `fail` does not answer.
    fn sources(n: usize, fetched: Rc<Cell<usize>>, fail: Option<usize>) -> Vec<Source> {
        (0..n)
            .map(|i| {
                let fetched = Rc::clone(&fetched);
                Source {
                    names: vec![format!("v{i}")],
                    fetch: Box::new(move || {
                        fetched.set(fetched.get() + 1);
                        (Some(i) != fail).then(|| vec![(format!("v{i}"), i as f64)])
                    }),
                }
            })
            .collect()
    }

    #[test]
    fn syncs_in_batches_and_resumes() {
        let path = scratch("batches");
        let fetched = Rc::new(Cell::new(0));
        // 25 values: saved at 10, 20 and the end; complete.
        assert_eq!(sync_into(&path, sources(25, Rc::clone(&fetched), None), 3_600, &|| 1_000), 25);
        let data = load_from(&path);
        assert_eq!((data.values.len(), data.synced_at, data.ok), (25, Some(1_000), true));
        // Still fresh: nothing fetched.
        fetched.set(0);
        assert_eq!(sync_into(&path, sources(25, Rc::clone(&fetched), None), 3_600, &|| 2_000), 0);
        assert_eq!(fetched.get(), 0);
        // Expired, and one source does not answer: the others update, not marked synced.
        fetched.set(0);
        assert_eq!(sync_into(&path, sources(25, Rc::clone(&fetched), Some(3)), 3_600, &|| 10_000), 24);
        let data = load_from(&path);
        assert_eq!((data.synced_at, data.attempted_at, data.ok), (Some(2_000), Some(10_000), false), "the last complete sync: the one at 2000");
        assert_eq!(data.get("v3").unwrap().at, 1_000, "the old value kept");
        // The next sync only fetches what is still old.
        fetched.set(0);
        sync_into(&path, sources(25, Rc::clone(&fetched), None), 3_600, &|| 10_100);
        assert_eq!(fetched.get(), 1);
        assert_eq!(load_from(&path).synced_at, Some(10_100));
    }

    #[test]
    fn nothing_new_counts_only_with_values_stored() {
        let path = scratch("unchanged");
        let unchanged = || vec![Source { names: vec!["Starlink".into()], fetch: Box::new(|| Some(Vec::new())) }];
        // Nothing stored: not complete (tried again later).
        sync_into(&path, unchanged(), 3_600, &|| 1_000);
        assert_eq!((load_from(&path).ok, load_from(&path).synced_at), (false, None));
        // Stored (and expired): the old value stays, and the sync is complete.
        let mut data = load_from(&path);
        data.set("Starlink", 11_129.0, 0);
        save_to(&path, &data).unwrap();
        sync_into(&path, unchanged(), 3_600, &|| 10_000);
        let data = load_from(&path);
        assert_eq!((data.ok, data.synced_at, data.get("Starlink").map(|s| s.value)), (true, Some(10_000), Some(11_129.0)));
    }

    #[test]
    fn an_interrupted_sync_leaves_a_consistent_file() {
        let path = scratch("interrupted");
        // The 15th fetch never returns (the process dies): the file has the first batch of 10.
        let mut all = sources(14, Rc::new(Cell::new(0)), None);
        all.push(Source { names: vec!["v14".into()], fetch: Box::new(|| panic!("killed")) });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sync_into(&path, all, 3_600, &|| 1_000)));
        assert!(result.is_err());
        let text = std::fs::read_to_string(&path).unwrap();
        let data = Data::from_json(&text).expect("valid JSON");
        assert_eq!(data.values.len(), 10, "the first batch");
        assert_eq!((data.synced_at, data.ok), (None, false), "not complete");
    }

    #[test]
    fn what_a_sync_looks_like() {
        let running = |phase: &str, fraction, speed, age| Syncing { fraction, speed, phase: phase.into(), done: None, age };
        let view = view_of(None);
        assert_eq!((view.state, view.label.as_str()), (SyncState::Idle, "Sync"));
        let v = view_of(Some(&running("downloading the city archive", Some(0.4), 400_000, 1)));
        assert_eq!(
            (v.state, v.line.as_str(), v.label.as_str(), v.fraction),
            (SyncState::Running, "downloading the city archive · 400 kB/s", "Syncing 40%", Some(0.4))
        );
        let v = view_of(Some(&running("installing the city archive", None, 0, 1)));
        assert_eq!((v.line.as_str(), v.label.as_str(), v.fraction), ("installing the city archive", "Syncing…", None));
        assert_eq!(view_of(Some(&running("", None, 0, 0))).line, "starting");
        // It stopped writing: it died; much later, nothing to say.
        let v = view_of(Some(&running("downloading the exchange rates", None, 0, 40)));
        assert_eq!((v.state, v.label.as_str(), v.message.as_str()), (SyncState::Failed, "Sync failed", "sync stopped"));
        assert_eq!(view_of(Some(&running("x", None, 0, 500))).state, SyncState::Idle);
        // Ended.
        let ended = |done: &[&str], failed: &[&str], age| Syncing {
            fraction: None,
            speed: 0,
            phase: String::new(),
            done: Some((done.iter().map(|s| s.to_string()).collect(), failed.iter().map(|s| s.to_string()).collect())),
            age,
        };
        let v = view_of(Some(&ended(&["rates: synced"], &[], 2)));
        assert_eq!((v.state, v.label.as_str(), v.message.as_str()), (SyncState::Done, "Synced", "rates: synced"));
        let v = view_of(Some(&ended(&["rates: synced"], &["cities: could not download"], 2)));
        assert_eq!((v.state, v.label.as_str(), v.message.as_str()), (SyncState::Failed, "Sync failed", "rates: synced, cities: could not download"));
        assert_eq!(view_of(Some(&ended(&[], &[], 2))).message, "synced");
        assert_eq!(view_of(Some(&ended(&["x"], &[], 500))).state, SyncState::Idle, "an end long ago is history");
        assert_eq!(step_line("checking the city archive", 5), "checking the city archive");
    }
}
