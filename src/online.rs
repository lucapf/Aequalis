//! The online data, in three groups, each with its refresh policy and interval in the settings
//! (`settings.rs`): the Trivia's live values (`live`: the Voyagers' distances, the satellite counts)
//! in `~/.config/tally/online.json`, the exchange rates (`rates`, `currency.rs`) in
//! `currencies.json`, the city archive (`cities`, `cities.rs`). Tally reads only these files: no
//! request ever waits for the network. Under the `silent` policy, any run of tally starts a sync
//! in the background (`tally --sync --auto`, a separate process) when a group is out of date,
//! unless one is running (`online.lock`) or the `offline` setting is on; under `notify` it says so
//! (`notify.rs`); under `manual` only `tally --sync` (or Settings) downloads.
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
    /// By Trivia entry name.
    pub values: Vec<(String, Stored)>,
}

impl Data {
    /// When the data was last synced, for people: the last complete sync, else the newest value
    /// (a sync that did not reach every source).
    pub fn last_sync(&self) -> Option<i64> {
        self.synced_at.or_else(|| self.values.iter().map(|(_, s)| s.at).max())
    }

    pub fn get(&self, name: &str) -> Option<Stored> {
        self.values.iter().find(|(n, _)| n == name).map(|(_, s)| *s)
    }

    fn set(&mut self, name: &str, value: f64, at: i64) {
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
    Cities,
}

impl Group {
    pub const ALL: [Group; 3] = [Group::Cities, Group::Rates, Group::Live];

    /// Its name in the settings and in `tally --sync <group>`.
    pub fn name(self) -> &'static str {
        match self {
            Group::Live => "live",
            Group::Rates => "rates",
            Group::Cities => "cities",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Group::Live => "the Trivia's live values",
            Group::Rates => "the exchange rates",
            Group::Cities => "the city archive",
        }
    }

    pub fn parse(name: &str) -> Option<Group> {
        Group::ALL.into_iter().find(|g| g.name().eq_ignore_ascii_case(name))
    }
}

/// Whether a group's data is older than its interval (or missing). For the city archive: whether
/// it is time to look for a new one.
pub fn stale(group: Group, settings: &crate::settings::Settings, now: i64) -> bool {
    let interval = settings.interval(group.name());
    match group {
        Group::Live => expired(&load(), now, interval),
        Group::Rates => expired(&crate::currency::load(), now, interval),
        Group::Cities => {
            let checked = crate::cities::meta().map_or(0, |m| m.downloaded_at).max(crate::notify::load().cities_checked_at);
            now - checked >= interval
        }
    }
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
        .filter(|&g| match settings.refresh(g.name()) {
            Refresh::Silent => stale(g),
            Refresh::Notify => g == Group::Cities && stale(g),
            Refresh::Manual => false,
        })
        .collect()
}

/// Starts `tally --sync --auto` in the background when a group has work (and the network may be
/// used, and no sync is running). Returns the process, for a long-running caller to reap.
pub fn ensure_fresh() -> Option<std::process::Child> {
    let settings = crate::settings::current();
    if settings.offline() || lock_held() || due(&settings, crate::time::unix_now()).is_empty() {
        return None;
    }
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    let exe = std::env::current_exe().ok()?;
    Command::new(exe)
        .args(["--sync", "--auto"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Its own process group: Ctrl-C in the terminal does not stop it.
        .process_group(0)
        .spawn()
        .ok()
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

/// `tally --sync [group…]` (`auto`: the background sync, `--auto`): under the lock, each group
/// asked for (all when none). By hand everything is downloaded again; the background sync does
/// what the policies say (`due`). Returns a line per group.
pub fn sync(groups: &[Group], auto: bool) -> Result<Vec<String>, String> {
    let settings = crate::settings::current();
    if settings.offline() {
        return Err("the offline setting is on: no sync".into());
    }
    let _lock = take_lock()?;
    let now = crate::time::unix_now();
    let groups: Vec<Group> = match (auto, groups.is_empty()) {
        (true, _) => due(&settings, now),
        (false, true) => Group::ALL.to_vec(),
        (false, false) => groups.to_vec(),
    };
    let mut out = Vec::new();
    for group in groups {
        let interval = if auto { settings.interval(group.name()) } else { 0 };
        let line = match group {
            Group::Live => {
                let n = sync_into(&path().ok_or("no home directory")?, crate::trivia::sources(), interval, &crate::time::unix_now);
                format!("{n} live value{}", if n == 1 { "" } else { "s" })
            }
            Group::Rates => {
                let path = crate::currency::path().ok_or("no home directory")?;
                let n = sync_into(&path, vec![crate::currency::source()], interval, &crate::time::unix_now);
                format!("{n} exchange rate{}", if n == 1 { "" } else { "s" })
            }
            Group::Cities => {
                let base = crate::files::dir().ok_or("no home directory")?;
                sync_cities(&base, &settings, auto, now, &|text| crate::notify::desktop("Tally", text))?
            }
        };
        out.push(line);
    }
    Ok(out)
}

/// The city archive: under `notify` (in the background) only its version is checked, and a newer
/// one noted (with a desktop notification, once); otherwise downloaded when newer (by hand:
/// always).
/// In the folder `base` (the archive, `state.json`); `notify` shows a desktop notification.
fn sync_cities(base: &Path, settings: &crate::settings::Settings, auto: bool, now: i64, notify: &dyn Fn(&str)) -> Result<String, String> {
    if !settings.cities_enabled() {
        return Ok("cities: off (the cities.enabled setting)".into());
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
            notify("A new city archive is available: tally --sync cities");
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
    use std::cell::Cell;
    use std::rc::Rc;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tally-online-{}-{name}", std::process::id()));
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
        // The defaults: cities manual, rates silent, live manual.
        assert_eq!(due_when(&Settings::default(), &all_stale), [Group::Rates]);
        assert!(due_when(&Settings::default(), &none_stale).is_empty());
        // Silent: when out of date; notify: only the archive's check; manual: never.
        let s = with(&[("cities.refresh", "silent"), ("live.refresh", "silent")]);
        assert_eq!(due_when(&s, &all_stale), [Group::Cities, Group::Rates, Group::Live]);
        let s = with(&[("cities.refresh", "notify"), ("rates.refresh", "notify"), ("live.refresh", "notify")]);
        assert_eq!(due_when(&s, &all_stale), [Group::Cities], "rates and live values out of date are only said");
        assert_eq!(due_when(&s, &|g| g != Group::Cities), Vec::<Group>::new());
        let s = with(&[("rates.refresh", "manual")]);
        assert!(due_when(&s, &all_stale).is_empty());
        // The cities off: never, whatever the policy.
        let s = with(&[("cities.refresh", "silent"), ("cities.enabled", "false")]);
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
        let gz = std::fs::File::create(served.join("cities.json.gz")).unwrap();
        assert!(std::process::Command::new("gzip").arg("-c").arg(&json).stdout(gz).status().unwrap().success());
        std::fs::write(served.join("cities.version"), "1791877958 sample\n").unwrap();
        let mut settings = Settings::default();
        settings.set("cities.url", Some(&format!("file://{}", served.join("cities.json.gz").display()))).unwrap();
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
        // By hand: always.
        assert_eq!(sync_cities(&home, &settings, false, 5_000, &notify).unwrap(), "9 cities");
        // Off: nothing.
        settings.set("cities.enabled", Some("false")).unwrap();
        assert_eq!(sync_cities(&home, &settings, false, 6_000, &notify).unwrap(), "cities: off (the cities.enabled setting)");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn expiry_and_retry() {
        let day = 86_400;
        assert!(expired(&Data::default(), 1_000, day), "never synced");
        let fresh = Data { synced_at: Some(1_000), attempted_at: Some(1_000), ok: true, values: Vec::new() };
        assert!(!expired(&fresh, 1_000 + day - 1, day));
        assert!(expired(&fresh, 1_000 + day, day));
        // A failed try: not again for an hour.
        let failed = Data { synced_at: Some(0), attempted_at: Some(1_000_000), ok: false, values: Vec::new() };
        assert!(!expired(&failed, 1_000_000 + 60, day));
        assert!(expired(&failed, 1_000_000 + 3_600, day));
    }

    #[test]
    fn the_file_round_trips() {
        let path = scratch("round");
        let mut data = Data { synced_at: Some(5), attempted_at: Some(4), ok: true, values: Vec::new() };
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
}
