//! The online data (the Trivia's live values: the Voyagers' distances, the satellite counts), kept
//! in `~/.config/tally/online.json` (`$XDG_CONFIG_HOME`) with the time each was synced. Tally reads
//! only this file: while it is fresh no network is used at all. When it is older than
//! `TALLY_ONLINE_REFRESH_INTERVAL` (default `24h`), any run of tally starts a sync in the
//! background (`tally --sync`, a separate process, so nothing waits for it) unless one is running
//! (`online.lock`) or `TALLY_MODE=offline`.
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

/// The refresh interval when `TALLY_ONLINE_REFRESH_INTERVAL` is not set: 24 hours.
pub const DEFAULT_INTERVAL: i64 = 24 * 3600;
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
        let opt = |v: Option<i64>| v.map_or("null".to_string(), |t| t.to_string());
        let values: Vec<String> = self.values.iter().map(|(n, s)| format!(r#"{}:{{"value":{},"at":{}}}"#, json::string(n), s.value, s.at)).collect();
        format!(
            r#"{{"format":1,"synced_at":{},"attempted_at":{},"ok":{},"values":{{{}}}}}"#,
            opt(self.synced_at),
            opt(self.attempted_at),
            self.ok,
            values.join(",")
        ) + "\n"
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

/// `~/.config/tally` (`$XDG_CONFIG_HOME/tally`).
pub fn dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("tally"))
}

pub fn path() -> Option<PathBuf> {
    dir().map(|d| d.join("online.json"))
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
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let mut file = std::fs::File::create(&tmp)?;
    file.write_all(data.to_json().as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)
}

// ---------------------------------------------------------------- when

/// `TALLY_ONLINE_REFRESH_INTERVAL`: `24h` (the default), `90m`, `2d`, `45s`, or seconds (`3600`).
pub fn parse_interval(value: Option<&str>) -> Result<i64, String> {
    let text = match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => return Ok(DEFAULT_INTERVAL),
        Some(t) => t.to_ascii_lowercase(),
    };
    let digits = text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len());
    let (number, unit) = text.split_at(digits);
    let n: i64 = number.parse().map_err(|_| interval_error(value))?;
    let seconds = match unit.trim() {
        "" | "s" => n,
        "m" | "min" => n * 60,
        "h" => n * 3600,
        "d" => n * 86_400,
        _ => return Err(interval_error(value)),
    };
    if seconds < 60 {
        return Err(interval_error(value));
    }
    Ok(seconds)
}

fn interval_error(value: Option<&str>) -> String {
    format!("TALLY_ONLINE_REFRESH_INTERVAL must be a duration of a minute or more, such as 24h, 90m or 2d (it is `{}`)", value.unwrap_or_default())
}

/// The interval from the environment (invalid: the default; the command line reports it).
pub fn interval() -> i64 {
    parse_interval(std::env::var("TALLY_ONLINE_REFRESH_INTERVAL").ok().as_deref()).unwrap_or(DEFAULT_INTERVAL)
}

/// Whether the data needs a sync at `now`: never synced, synced longer ago than `interval`, or
/// missing one of `names` (the exchange rates, which come all together or not at all: added since
/// the last sync); after a failed try, not
/// before `RETRY_AFTER` (or the interval) has passed.
pub fn expired(data: &Data, now: i64, interval: i64, names: &[String]) -> bool {
    if let (Some(tried), false) = (data.attempted_at, data.ok) {
        if now - tried < RETRY_AFTER.min(interval) {
            return false;
        }
    }
    data.synced_at.is_none_or(|t| now - t >= interval) || names.iter().any(|n| data.get(n).is_none())
}

/// Every source: the Trivia's live values and the exchange rates.
fn sources() -> Vec<Source> {
    let mut sources = crate::trivia::sources();
    sources.push(crate::currency::source());
    sources
}

pub fn now() -> i64 {
    crate::time::now().ns.div_euclid(1_000_000_000) as i64
}

// ---------------------------------------------------------------- the background sync

/// Starts `tally --sync` in the background when the data has expired (and the network may be used,
/// and no sync is running). Returns the process, for a long-running caller to reap.
pub fn ensure_fresh() -> Option<std::process::Child> {
    if !crate::network::allowed() || !expired(&load(), now(), interval(), &crate::currency::source().names) || lock_held() {
        return None;
    }
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    let exe = std::env::current_exe().ok()?;
    Command::new(exe)
        .arg("--sync")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Its own process group: Ctrl-C in the terminal does not stop it.
        .process_group(0)
        .spawn()
        .ok()
}

fn lock_path() -> Option<PathBuf> {
    dir().map(|d| d.join("online.lock"))
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
    pub fetch: Box<dyn Fn() -> Option<Vec<(String, f64)>>>,
}

/// `tally --sync`: fetches what has expired, under the lock, saving as it goes. Returns how many
/// values were updated.
pub fn sync() -> Result<usize, String> {
    if !crate::network::allowed() {
        return Err("TALLY_MODE=offline: no sync".into());
    }
    let _lock = take_lock()?;
    let path = path().ok_or("no home directory")?;
    Ok(sync_into(&path, sources(), interval(), &now))
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
        if pending >= BATCH || last_save.elapsed().as_secs() >= BATCH_SECONDS {
            if save_to(path, &data).is_ok() {
                pending = 0;
                last_save = Instant::now();
            }
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

    #[test]
    fn intervals() {
        assert_eq!(parse_interval(None), Ok(86_400));
        assert_eq!(parse_interval(Some("")), Ok(86_400));
        assert_eq!(parse_interval(Some("24h")), Ok(86_400));
        assert_eq!(parse_interval(Some("90m")), Ok(5_400));
        assert_eq!(parse_interval(Some("2d")), Ok(172_800));
        assert_eq!(parse_interval(Some("3600")), Ok(3_600));
        assert_eq!(parse_interval(Some("45 MIN")), Ok(2_700));
        assert!(parse_interval(Some("30s")).unwrap_err().contains("a minute or more"));
        assert!(parse_interval(Some("soon")).is_err());
    }

    #[test]
    fn expiry_and_retry() {
        let day = 86_400;
        assert!(expired(&Data::default(), 1_000, day, &[]), "never synced");
        let fresh = Data { synced_at: Some(1_000), attempted_at: Some(1_000), ok: true, values: Vec::new() };
        assert!(!expired(&fresh, 1_000 + day - 1, day, &[]));
        assert!(expired(&fresh, 1_000 + day, day, &[]));
        // A source added since the last sync (the exchange rates): synced now.
        assert!(expired(&fresh, 1_000 + 60, day, &["EUR/USD".to_string()]));
        // A failed try: not again for an hour.
        let failed = Data { synced_at: Some(0), attempted_at: Some(1_000_000), ok: false, values: Vec::new() };
        assert!(!expired(&failed, 1_000_000 + 60, day, &[]));
        assert!(expired(&failed, 1_000_000 + 3_600, day, &[]));
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
