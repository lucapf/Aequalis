//! The city archive, for distances between any two cities (`Springfield Paris`), not only the
//! capitals built in (`geo.rs`).
//!
//! The repository publishes `cities.json` (built monthly from GeoNames by
//! `scripts/build-cities.py`, CC BY 4.0; plain JSON, so any downloader can fetch it as it is) with
//! its `cities.version`. Tally downloads it when the settings say so (`cities.refresh`,
//! `tally --sync`) and turns it once into two files, searched without loading them:
//!
//! - `cities.tsv`: one city per line: id, name, lat, lon, population, country code, country,
//!   region, updated, aliases (`|`-separated);
//! - `cities.idx`: one line per name and alias, `key<TAB>offset of its city in cities.tsv`, sorted
//!   by key (lowercase, no accents, punctuation as spaces: `tz::key`).
//!
//! A lookup is a binary search on `cities.idx` by byte position (about 20 small reads), then one
//! read per city found: no database, nothing parsed but the lines read, fast enough per keystroke.
//! Both live in a directory named after the archive (`cities-<built_at>`), named by
//! `cities.json` (the meta file), so a new archive replaces the old one atomically.

use crate::geo::Point;
use crate::time::tz;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the archive comes from (also a setting, `cities.url`, for mirrors and tests).
pub const URL: &str = "https://github.com/lucapf/tally/releases/download/cities/cities.json";
/// Where it came from when it was published compressed: a settings file that still holds this
/// address gets `URL` instead (`settings.rs`).
pub const LEGACY_URL: &str = "https://github.com/lucapf/tally/releases/download/cities/cities.json.gz";
/// The data's origin, for About and the notes.
pub const SOURCE: &str = "GeoNames (https://www.geonames.org), CC BY 4.0";
/// At most this many cities offered to choose from.
pub const MAX_CHOICES: usize = 8;

/// A city of the archive.
#[derive(Debug, Clone, PartialEq)]
pub struct City {
    pub id: u64,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub population: u64,
    pub country_code: String,
    pub country: String,
    pub region: String,
    pub updated: String,
    pub aliases: Vec<String>,
}

impl City {
    pub fn point(&self) -> Point {
        Point { lat: self.lat, lon: self.lon }
    }

    /// Its location on Google Maps.
    pub fn maps(&self) -> String {
        format!("https://www.google.com/maps?q={},{}", self.lat, self.lon)
    }

    /// The name that tells it apart: `Springfield, Illinois, United States`.
    pub fn qualified(&self) -> String {
        [self.name.as_str(), self.region.as_str(), self.country.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(", ")
    }

    /// What the person calls it: its name, or the alias they typed or started typing (`Milano`
    /// for `Mila…` only when the name does not start so: `Milan`), in the archive's spelling.
    pub fn called(&self, typed: &str) -> String {
        let target = tz::key(typed.split(',').next().unwrap_or(""));
        if target.is_empty() || tz::key(&self.name).starts_with(&target) {
            return self.name.clone();
        }
        // An alias written with capitals first (`Roma`, not `roma`); one in lower case gets them.
        let found = |exact: bool| -> Vec<&String> {
            self.aliases.iter().filter(|a| if exact { tz::key(a) == target } else { tz::key(a).starts_with(&target) }).collect()
        };
        let aliases = Some(found(true)).filter(|f| !f.is_empty()).unwrap_or_else(|| found(false));
        let capital = |a: &&&String| a.chars().next().is_some_and(char::is_uppercase);
        match (aliases.iter().find(capital), aliases.first()) {
            (Some(a), _) => a.to_string(),
            (None, Some(a)) => a.split(' ').map(|w| {
                let mut c = w.chars();
                c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
            }).collect::<Vec<_>>().join(" "),
            (None, None) => self.name.clone(),
        }
    }

    /// A short label for people: `Milano - Italy`; with the region when another city of its
    /// country has that name (`twin`): `Springfield - Illinois - United States`.
    pub fn label(&self, called: &str, twin: bool) -> String {
        let region = if twin && !self.region.is_empty() { format!(" - {}", self.region) } else { String::new() };
        format!("{called}{region} - {}", self.country)
    }

    fn from_line(line: &str) -> Option<City> {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 10 {
            return None;
        }
        Some(City {
            id: f[0].parse().ok()?,
            name: f[1].to_string(),
            lat: f[2].parse().ok()?,
            lon: f[3].parse().ok()?,
            population: f[4].parse().ok()?,
            country_code: f[5].to_string(),
            country: f[6].to_string(),
            region: f[7].to_string(),
            updated: f[8].to_string(),
            aliases: f[9].split('|').filter(|a| !a.is_empty()).map(str::to_string).collect(),
        })
    }

    /// A city of the archive's JSON: one line, as the build script writes them.
    fn from_json(text: &str) -> Option<City> {
        let c = crate::json::parse(text).ok()?;
        let s = |k: &str| c.get(k).and_then(|v| v.str()).unwrap_or_default().to_string();
        let n = |k: &str| c.get(k).and_then(|v| v.number());
        Some(City {
            id: n("id")? as u64,
            name: s("name"),
            lat: n("lat")?,
            lon: n("lon")?,
            population: n("population").unwrap_or(0.0) as u64,
            country_code: s("country_code"),
            country: s("country"),
            region: s("region"),
            updated: s("updated"),
            aliases: c.get("aliases").map(|a| a.array().iter().filter_map(|x| x.str().map(str::to_string)).collect()).unwrap_or_default(),
        })
    }

    fn to_line(&self) -> String {
        let clean = |s: &str| s.replace(['\t', '\n', '\r', '|'], " ");
        let aliases: Vec<String> = self.aliases.iter().map(|a| clean(a)).collect();
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            self.id,
            clean(&self.name),
            self.lat,
            self.lon,
            self.population,
            clean(&self.country_code),
            clean(&self.country),
            clean(&self.region),
            clean(&self.updated),
            aliases.join("|")
        )
    }
}

// ---------------------------------------------------------------- the files

/// What `cities.json` (the meta file) says about the archive in use.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Meta {
    /// `<built_at> <sha256>`, as `cities.version`.
    pub version: String,
    pub built_at: i64,
    pub count: u64,
    pub downloaded_at: i64,
    /// The directory with `cities.tsv` and `cities.idx`.
    pub dir: String,
}

pub fn meta_in(base: &Path) -> Option<Meta> {
    let doc = crate::json::parse(&std::fs::read_to_string(base.join("cities.json")).ok()?).ok()?;
    let n = |k: &str| doc.get(k).and_then(|v| v.number()).unwrap_or(0.0);
    let s = |k: &str| doc.get(k).and_then(|v| v.str()).unwrap_or_default().to_string();
    Some(Meta { version: s("version"), built_at: n("built_at") as i64, count: n("count") as u64, downloaded_at: n("downloaded_at") as i64, dir: s("dir") })
}

/// The archive in use, if one was downloaded.
pub fn meta() -> Option<Meta> {
    meta_in(&crate::files::dir()?)
}

/// The archive to search: downloaded, and the feature on.
pub fn store() -> Option<Store> {
    if !crate::settings::current().cities_enabled() {
        return None;
    }
    let base = crate::files::dir()?;
    Store::open(&base, &meta_in(&base)?)
}

/// The archive's files, open.
pub struct Store {
    tsv: File,
    idx: File,
    idx_len: u64,
}

/// The order of cities offered for what was `typed` (Tab, "which one?"): those named exactly so
/// (their name, not an alias) in the device's country `local`; the others in that country; those
/// named exactly so elsewhere; the rest. The most populous first in each (`cities` come in that
/// order: the sort is stable). `Alessandria`: Alessandria, Piedmont before Alexandria, Egypt (whose
/// Italian name it is); `Roma` in Italy: Rome, Lazio before Roma, Lesotho.
pub fn rank(cities: &mut [City], typed: &str, local: Option<&str>) {
    let target = tz::key(typed.split(',').next().unwrap_or(""));
    cities.sort_by_key(|c| {
        let exact = tz::key(&c.name) == target;
        match (local == Some(c.country_code.as_str()), exact) {
            (true, true) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (false, false) => 3,
        }
    });
}

impl Store {
    pub fn open(base: &Path, meta: &Meta) -> Option<Store> {
        let dir = base.join(&meta.dir);
        let tsv = File::open(dir.join("cities.tsv")).ok()?;
        let idx = File::open(dir.join("cities.idx")).ok()?;
        let idx_len = idx.metadata().ok()?.len();
        Some(Store { tsv, idx, idx_len })
    }

    /// The first index line starting at or after `pos`: (its start, the line).
    fn line_from(&self, pos: u64) -> Option<(u64, String)> {
        // Right after a newline, a line starts at `pos`; else after the next newline.
        let start = if pos == 0 { 0 } else { next_line(&self.idx, pos - 1)? };
        (start < self.idx_len).then(|| Some((start, line_at(&self.idx, start)?)))?
    }

    /// The start of the first index line whose key is `target` or after it.
    fn lower_bound(&self, target: &str) -> u64 {
        let (mut lo, mut hi) = (0u64, self.idx_len);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match self.line_from(mid) {
                Some((start, line)) if start < hi => {
                    if line.split('\t').next().unwrap_or("") < target {
                        lo = start + line.len() as u64 + 1;
                    } else {
                        hi = mid;
                    }
                }
                _ => hi = mid,
            }
        }
        lo
    }

    /// The index lines from `start` on, while `keep` says so (at most `limit`).
    fn scan(&self, start: u64, limit: usize, keep: &dyn Fn(&str) -> bool) -> Vec<(String, u64)> {
        let mut reader = BufReader::new(&self.idx);
        if reader.seek(SeekFrom::Start(start)).is_err() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for line in reader.lines().map_while(Result::ok) {
            let Some((k, offset)) = line.split_once('\t') else { break };
            if !keep(k) || out.len() >= limit {
                break;
            }
            if let Ok(offset) = offset.parse() {
                out.push((k.to_string(), offset));
            }
        }
        out
    }

    /// The city at `offset` in `cities.tsv`.
    fn city_at(&self, offset: u64) -> Option<City> {
        City::from_line(&line_at(&self.tsv, offset)?)
    }

    fn cities(&self, offsets: impl IntoIterator<Item = u64>) -> Vec<City> {
        let mut out: Vec<City> = Vec::new();
        for offset in offsets {
            if let Some(c) = self.city_at(offset) {
                if !out.iter().any(|o| o.id == c.id) {
                    out.push(c);
                }
            }
        }
        out.sort_by(|a, b| b.population.cmp(&a.population).then(a.name.cmp(&b.name)));
        out
    }

    /// The cities named `name` (or with it as an alias), the most populous first. A qualified name
    /// (`Springfield, Illinois`, `Springfield, US`, `Springfield, Illinois, United States`) keeps
    /// those in that region or country.
    pub fn find(&self, name: &str) -> Vec<City> {
        let mut parts = name.split(',').map(str::trim).filter(|p| !p.is_empty());
        let Some(first) = parts.next() else { return Vec::new() };
        let qualifiers: Vec<String> = parts.map(tz::key).collect();
        let target = tz::key(first);
        if target.is_empty() {
            return Vec::new();
        }
        let hits = self.scan(self.lower_bound(&target), 5000, &|k| k == target);
        let found = self.cities(hits.into_iter().map(|(_, o)| o));
        // Each qualifier: the city's region, its country, or a name of its country (`USA`).
        found
            .into_iter()
            .filter(|c| {
                let names = [tz::key(&c.region), tz::key(&c.country), tz::key(&c.country_code)];
                qualifiers.iter().all(|q| names.contains(q) || tz::country(q).is_some_and(|t| t.code == c.country_code))
            })
            .collect()
    }

    /// How to offer `city` for what was `typed` (Tab): its short label (`City::label`), and what
    /// is written, as short as still finds it alone: `Milano`, `Alexandria, Egypt`,
    /// `Springfield, Illinois, United States`.
    #[cfg_attr(not(feature = "tui"), allow(dead_code))]
    pub fn offer(&self, city: &City, typed: &str) -> (String, String) {
        let called = city.called(typed);
        let same = self.find(&called);
        let twin = same.iter().any(|c| c.id != city.id && c.country_code == city.country_code);
        let written = match (same.len(), twin) {
            (0 | 1, _) => called.clone(),
            (_, false) => format!("{called}, {}", city.country),
            (_, true) => [called.as_str(), city.region.as_str(), city.country.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(", "),
        };
        (city.label(&called, twin), written)
    }

    /// Cities whose name or alias starts with `prefix` (Tab), in the order of `rank` (`local`: the
    /// device's country).
    #[cfg_attr(not(feature = "tui"), allow(dead_code))]
    pub fn starting_with(&self, prefix: &str, limit: usize, local: Option<&str>) -> Vec<City> {
        let target = tz::key(prefix);
        if target.chars().count() < 2 {
            return Vec::new();
        }
        let hits = self.scan(self.lower_bound(&target), 2000, &|k| k.starts_with(target.as_str()));
        let mut found = self.cities(hits.into_iter().map(|(_, o)| o));
        rank(&mut found, prefix, local);
        found.truncate(limit);
        found
    }

    /// Cities whose name or alias is within 1 letter of `name` (2 from 6 letters): `Sprinfield`.
    pub fn near(&self, name: &str) -> Vec<City> {
        let target = tz::key(name).replace(' ', "");
        let len = target.chars().count();
        if len < 4 {
            return Vec::new();
        }
        let limit = if len <= 5 { 1 } else { 2 };
        // Names sharing the first two letters (a misspelling rarely changes them).
        let first: String = tz::key(name).chars().take(2).collect();
        let hits = self.scan(self.lower_bound(&first), 200_000, &|k| k.starts_with(first.as_str()));
        let mut best: Vec<(usize, u64)> = Vec::new();
        for (k, offset) in hits {
            let squeezed = k.replace(' ', "");
            if squeezed.chars().count().abs_diff(len) > limit {
                continue;
            }
            let d = tz::levenshtein(&target, &squeezed);
            if d <= limit {
                best.push((d, offset));
            }
        }
        // An exact name is not a misspelling.
        let Some(min) = best.iter().map(|b| b.0).min().filter(|&m| m > 0) else { return Vec::new() };
        self.cities(best.into_iter().filter(|b| b.0 == min).map(|b| b.1))
    }
}

/// The line of `file` starting at `pos`, without its newline (read by position: no seek, no
/// buffer kept).
fn line_at(file: &File, pos: u64) -> Option<String> {
    let mut line = Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        let n = file.read_at(&mut chunk, pos + line.len() as u64).ok()?;
        if n == 0 {
            break;
        }
        match chunk[..n].iter().position(|&b| b == b'\n') {
            Some(end) => {
                line.extend_from_slice(&chunk[..end]);
                break;
            }
            None => line.extend_from_slice(&chunk[..n]),
        }
    }
    String::from_utf8(line).ok()
}

/// Where the line after the one holding byte `pos` of `file` starts.
fn next_line(file: &File, mut pos: u64) -> Option<u64> {
    let mut chunk = [0u8; 512];
    loop {
        let n = file.read_at(&mut chunk, pos).ok()?;
        if n == 0 {
            return None;
        }
        if let Some(i) = chunk[..n].iter().position(|&b| b == b'\n') {
            return Some(pos + i as u64 + 1);
        }
        pos += n as u64;
    }
}

// ---------------------------------------------------------------- download and conversion

/// How many names are sorted in memory at a time while building the index (about 2 MB): the
/// 575,000 names go to disk in sorted runs, merged at the end, so the conversion stays well below
/// 10 MB instead of holding every name (about 45 MB).
const RUN: usize = 32_768;

/// Turns the archive's JSON (one city per line, as the build script writes it) into the two
/// search files in `dir`; returns how many cities, and the archive's date. Read line by line: the
/// 20 MB archive is never held in memory, nor its index (an external merge sort, `RUN`). Refused
/// when it is compressed, or when it has fewer cities than its header says (a download cut short).
pub fn convert(json: &Path, dir: &Path) -> Result<(u64, i64), String> {
    convert_in_runs(json, dir, RUN)
}

/// `convert`, sorting `run` names at a time.
fn convert_in_runs(json: &Path, dir: &Path, run: usize) -> Result<(u64, i64), String> {
    let err = |e: std::io::Error| e.to_string();
    std::fs::create_dir_all(dir).map_err(err)?;
    let mut input = BufReader::new(File::open(json).map_err(err)?);
    // Gzip (`1f 8b`): the archive as it was once published, or a mirror that still has it.
    if input.fill_buf().map_err(err)?.starts_with(&[0x1f, 0x8b]) {
        return Err("the city archive is compressed (gzip): Tally reads the plain `cities.json` (the cities.url setting)".into());
    }
    let mut tsv = BufWriter::new(File::create(dir.join("cities.tsv")).map_err(err)?);
    let mut keys: Vec<(String, u64)> = Vec::with_capacity(run);
    let mut runs: Vec<PathBuf> = Vec::new();
    let (mut offset, mut count, mut built_at, mut expected) = (0u64, 0u64, 0i64, None);
    for line in input.lines() {
        let line = line.map_err(err)?;
        let text = line.trim().trim_end_matches(',');
        if let Some(head) = text.strip_suffix("\"cities\": [") {
            // The header: `{"format": 1, …, "count": N, "cities": [`.
            let head = format!("{}\"x\": 0}}", head);
            if let Ok(doc) = crate::json::parse(&head) {
                built_at = doc.get("built_at").and_then(|v| v.number()).unwrap_or(0.0) as i64;
                expected = doc.get("count").and_then(|v| v.number()).map(|n| n as u64);
            }
            continue;
        }
        if !text.starts_with('{') {
            continue;
        }
        let Some(city) = City::from_json(text) else { continue };
        let line = city.to_line();
        let mut names: Vec<String> = std::iter::once(&city.name).chain(&city.aliases).map(|n| tz::key(n)).filter(|k| !k.is_empty()).collect();
        names.sort_unstable();
        names.dedup();
        keys.extend(names.into_iter().map(|k| (k, offset)));
        if keys.len() >= run {
            runs.push(write_run(dir, runs.len(), &mut keys)?);
        }
        tsv.write_all(line.as_bytes()).map_err(err)?;
        offset += line.len() as u64;
        count += 1;
    }
    tsv.flush().map_err(err)?;
    // Without gzip's checksum, the header's count tells a whole archive from one cut short.
    if let Some(expected) = expected.filter(|&n| n != count) {
        return Err(format!("the city archive is incomplete: {count} of {expected} cities (download it again)"));
    }
    if !keys.is_empty() {
        runs.push(write_run(dir, runs.len(), &mut keys)?);
    }
    merge(&runs, &dir.join("cities.idx"))?;
    for run in runs {
        let _ = std::fs::remove_file(run);
    }
    Ok((count, built_at))
}

/// Sorts `keys`, writes them to the run file `n` of `dir` (`key<TAB>offset` lines), and empties it.
fn write_run(dir: &Path, n: usize, keys: &mut Vec<(String, u64)>) -> Result<PathBuf, String> {
    keys.sort_unstable();
    let path = dir.join(format!("run-{n}"));
    let mut out = BufWriter::new(File::create(&path).map_err(|e| e.to_string())?);
    for (k, o) in keys.drain(..) {
        writeln!(out, "{k}\t{o}").map_err(|e| e.to_string())?;
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(path)
}

/// Merges sorted run files into one sorted index: the smallest line of all the runs, each time
/// (a heap with one line per run).
fn merge(runs: &[PathBuf], to: &Path) -> Result<(), String> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let parse = |line: String| -> Option<(String, u64)> {
        let (k, o) = line.split_once('\t')?;
        Some((k.to_string(), o.parse().ok()?))
    };
    let mut readers = Vec::new();
    for run in runs {
        readers.push(BufReader::new(File::open(run).map_err(|e| e.to_string())?).lines());
    }
    let mut heap = BinaryHeap::new();
    for (i, r) in readers.iter_mut().enumerate() {
        if let Some((k, o)) = r.next().and_then(Result::ok).and_then(parse) {
            heap.push(Reverse((k, o, i)));
        }
    }
    let mut out = BufWriter::new(File::create(to).map_err(|e| e.to_string())?);
    while let Some(Reverse((k, o, i))) = heap.pop() {
        writeln!(out, "{k}\t{o}").map_err(|e| e.to_string())?;
        if let Some((k, o)) = readers[i].next().and_then(Result::ok).and_then(parse) {
            heap.push(Reverse((k, o, i)));
        }
    }
    out.flush().map_err(|e| e.to_string())
}

/// `cities.version` next to the archive at `url`: `<built_at> <sha256>`.
pub fn version_url(url: &str) -> String {
    match url.rsplit_once('/') {
        Some((base, _)) => format!("{base}/cities.version"),
        None => "cities.version".into(),
    }
}

/// The published archive's version (a few bytes); `None`: no answer.
pub fn remote_version(url: &str) -> Option<String> {
    let out = Command::new("curl").args(["-sfL", "--max-time", "20", &version_url(url)]).output().ok()?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && v.split(' ').count() == 2).then_some(v)
}

/// Downloads the archive at `url` (plain JSON, saved as it comes: `curl -o`) and makes it the one
/// in use, in `base`. Returns how many cities.
pub fn download(base: &Path, url: &str, now: i64) -> Result<u64, String> {
    let version = remote_version(url).unwrap_or_default();
    let tmp = base.join(format!("cities.{}.download", std::process::id()));
    std::fs::create_dir_all(base).map_err(|e| e.to_string())?;
    let json = tmp.with_extension("json");
    let fetched = Command::new("curl").args(["-sfL", "--max-time", "600", "-o"]).arg(&json).arg(url).status().map_err(|e| format!("curl: {e}"))?;
    let result = match fetched.success() {
        true => install(base, &json, &version, now),
        false => Err(format!("could not download the city archive ({url})")),
    };
    let _ = std::fs::remove_file(&json);
    result
}

/// Converts a downloaded archive and switches to it (the meta file renamed over the old one), then
/// removes the previous one.
pub fn install(base: &Path, json: &Path, version: &str, now: i64) -> Result<u64, String> {
    let previous = meta_in(base);
    let name = format!("cities-{now}-{}", std::process::id());
    let dir = base.join(&name);
    // A conversion that fails leaves nothing behind: the archive in use stays as it is.
    let (count, built_at) = match convert(json, &dir) {
        Ok((0, _)) => Err("the city archive is empty".to_string()),
        other => other,
    }
    .inspect_err(|_| {
        let _ = std::fs::remove_dir_all(&dir);
    })?;
    let version = if version.is_empty() { format!("{built_at} -") } else { version.to_string() };
    let meta = crate::json::Object::new()
        .str("version", &version)
        .num("built_at", built_at)
        .num("count", count)
        .num("downloaded_at", now)
        .str("dir", &name)
        .str("source", SOURCE)
        .finish();
    crate::files::write_atomic(&base.join("cities.json"), format!("{meta}\n").as_bytes()).map_err(|e| e.to_string())?;
    if let Some(old) = previous.filter(|p| p.dir != name && !p.dir.is_empty()) {
        let _ = std::fs::remove_dir_all(base.join(old.dir));
    }
    Ok(count)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A small archive in the build script's format.
    pub const SAMPLE: &str = r#"{"format": 1, "source": "GeoNames (https://www.geonames.org)", "license": "CC BY 4.0", "built_at": 1791877958, "count": 9, "cities": [
{"id":2988507,"name":"Paris","aliases":["Lutetia","Paname","Parigi","Париж"],"lat":48.85341,"lon":2.3488,"population":2138551,"country":"France","country_code":"FR","region":"Île-de-France","updated":"2026-01-01"},
{"id":3169070,"name":"Rome","aliases":["Roma","Rom","Рим"],"lat":41.89193,"lon":12.51133,"population":2318895,"country":"Italy","country_code":"IT","region":"Lazio","updated":"2026-01-01"},
{"id":4250542,"name":"Springfield","aliases":[],"lat":39.80172,"lon":-89.64371,"population":114394,"country":"United States","country_code":"US","region":"Illinois","updated":"2026-01-01"},
{"id":4409896,"name":"Springfield","aliases":[],"lat":37.21533,"lon":-93.29824,"population":169176,"country":"United States","country_code":"US","region":"Missouri","updated":"2026-01-01"},
{"id":4951788,"name":"Springfield","aliases":[],"lat":42.10148,"lon":-72.58981,"population":155929,"country":"United States","country_code":"US","region":"Massachusetts","updated":"2026-01-01"},
{"id":4717560,"name":"Paris","aliases":[],"lat":33.66094,"lon":-95.55551,"population":24782,"country":"United States","country_code":"US","region":"Texas","updated":"2026-01-01"},
{"id":4219762,"name":"Rome","aliases":[],"lat":34.25704,"lon":-85.16467,"population":37713,"country":"United States","country_code":"US","region":"Georgia","updated":"2026-01-01"},
{"id":4887398,"name":"Chicago","aliases":["Chi-Town"],"lat":41.85003,"lon":-87.65005,"population":2746388,"country":"United States","country_code":"US","region":"Illinois","updated":"2026-01-01"},
{"id":3193044,"name":"Bar","aliases":["Antivari"],"lat":42.0931,"lon":19.1003,"population":13719,"country":"Montenegro","country_code":"ME","region":"Opština Bar","updated":"2026-01-01"}
]}
"#;

    /// The sample installed in a fresh directory.
    pub fn sample(name: &str) -> (PathBuf, Store) {
        let base = std::env::temp_dir().join(format!("tally-cities-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let json = base.join("sample.json");
        std::fs::write(&json, SAMPLE).unwrap();
        assert_eq!(install(&base, &json, "1791877958 abc", 1_791_900_000).unwrap(), 9);
        let store = Store::open(&base, &meta_in(&base).unwrap()).unwrap();
        (base, store)
    }

    fn names(cities: &[City]) -> Vec<String> {
        cities.iter().map(City::qualified).collect()
    }

    /// What a city is called: the person's wording (the name, or the alias typed), and its label.
    #[test]
    fn called_as_typed() {
        let milan = City {
            id: 1,
            name: "Milan".into(),
            lat: 0.0,
            lon: 0.0,
            population: 1,
            country_code: "IT".into(),
            country: "Italy".into(),
            region: "Lombardy".into(),
            updated: String::new(),
            aliases: vec!["Mailand".into(), "milano".into(), "Milano".into()],
        };
        assert_eq!(milan.called("Milan"), "Milan");
        assert_eq!(milan.called("Mila"), "Milan", "the name starts so: the name");
        assert_eq!(milan.called("Milano"), "Milano", "Italian typed: Italian shown, capitals first");
        assert_eq!(milan.called("milano, Italy"), "Milano");
        assert_eq!(milan.called("Mailan"), "Mailand");
        assert_eq!(milan.called(""), "Milan");
        assert_eq!(milan.label("Milano", false), "Milano - Italy");
        assert_eq!(milan.label("Milano", true), "Milano - Lombardy - Italy");
        let rome = City { aliases: vec!["roma".into()], name: "Rome".into(), ..milan.clone() };
        assert_eq!(rome.called("Roma"), "Roma", "an alias only in lower case gets its capitals");
    }

    /// The order of the cities offered: the exact name (not an alias) in the device's country, the
    /// device's country, the exact name elsewhere, the others; by population within each.
    #[test]
    fn ranked_exact_name_then_the_device_s_country() {
        let city = |name: &str, region: &str, country: &str, code: &str, population: u64, aliases: &[&str]| City {
            id: population,
            name: name.into(),
            lat: 0.0,
            lon: 0.0,
            population,
            country_code: code.into(),
            country: country.into(),
            region: region.into(),
            updated: String::new(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
        };
        // By population, as the archive gives them.
        let all = vec![
            city("Alexandria", "Alexandria", "Egypt", "EG", 3_811_516, &["Alessandria"]),
            city("Alexandria", "Virginia", "United States", "US", 159_428, &["Alessandria"]),
            city("Alessandria", "Piedmont", "Italy", "IT", 89_411, &[]),
            city("Alessano", "Apulia", "Italy", "IT", 6_400, &[]),
        ];
        let order = |typed: &str, local: Option<&str>| {
            let mut cities = all.clone();
            rank(&mut cities, typed, local);
            cities.iter().map(|c| format!("{} {}", c.name, c.country_code)).collect::<Vec<_>>()
        };
        // The exact name in the device's country, its country, the exact name elsewhere, the rest.
        assert_eq!(order("Alessandria", Some("IT")), ["Alessandria IT", "Alessano IT", "Alexandria EG", "Alexandria US"]);
        assert_eq!(order("alessandria", Some("US")), ["Alexandria US", "Alessandria IT", "Alexandria EG", "Alessano IT"]);
        assert_eq!(order("Alessandria", Some("FR")), ["Alessandria IT", "Alexandria EG", "Alexandria US", "Alessano IT"], "elsewhere: the exact name first");
        // `Roma` in Italy: Rome, Lazio (an alias, but in Italy) before the cities named Roma elsewhere.
        let roma = vec![
            city("Rome", "Lazio", "Italy", "IT", 2_318_895, &["Roma"]),
            city("Roma", "Maseru", "Lesotho", "LS", 14_259, &[]),
        ];
        let mut cities = roma.clone();
        rank(&mut cities, "Roma", Some("IT"));
        assert_eq!(cities[0].country_code, "IT");
        rank(&mut cities, "Roma", None);
        assert_eq!(cities[0].country_code, "LS", "no device country: the exact name");
        assert_eq!(order("Alessandria, Piedmont", None)[0], "Alessandria IT", "a qualified name: its first part");
        // Typing (no exact name yet): the device's country first.
        assert_eq!(order("Alessan", Some("IT")), ["Alessandria IT", "Alessano IT", "Alexandria EG", "Alexandria US"]);
        assert_eq!(order("Alessan", None), ["Alexandria EG", "Alexandria US", "Alessandria IT", "Alessano IT"], "by population");
    }

    #[test]
    fn exact_names_and_aliases_any_case() {
        let (_, store) = sample("exact");
        assert_eq!(names(&store.find("rome")), ["Rome, Lazio, Italy", "Rome, Georgia, United States"]);
        assert_eq!(names(&store.find("ROMA")), ["Rome, Lazio, Italy"]);
        assert_eq!(names(&store.find("Рим")), ["Rome, Lazio, Italy"]);
        assert_eq!(names(&store.find("chi-town")), ["Chicago, Illinois, United States"]);
        assert_eq!(store.find("Springfield").len(), 3);
        assert_eq!(store.find("Springfield")[0].region, "Missouri", "the most populous first");
        assert!(store.find("Atlantis").is_empty());
    }

    #[test]
    fn qualified_names() {
        let (_, store) = sample("qualified");
        assert_eq!(names(&store.find("Springfield, Illinois")), ["Springfield, Illinois, United States"]);
        assert_eq!(store.find("Springfield, US").len(), 3);
        assert_eq!(store.find("Springfield, USA").len(), 3, "a country alias");
        assert_eq!(names(&store.find("springfield, massachusetts, united states")), ["Springfield, Massachusetts, United States"]);
        assert_eq!(names(&store.find("Paris, France")), ["Paris, Île-de-France, France"]);
        assert!(store.find("Paris, Italy").is_empty());
    }

    #[test]
    fn prefixes_and_misspellings() {
        let (_, store) = sample("prefix");
        assert_eq!(names(&store.starting_with("spr", 2, None)), ["Springfield, Missouri, United States", "Springfield, Massachusetts, United States"]);
        assert_eq!(store.starting_with("Par", 8, None).len(), 2);
        assert!(store.starting_with("p", 8, None).is_empty(), "at least 2 letters");
        assert_eq!(store.near("Sprinfield").len(), 3);
        assert_eq!(names(&store.near("Chicaco")), ["Chicago, Illinois, United States"]);
        assert!(store.near("Rome").is_empty(), "an exact name is not a misspelling");
    }

    #[test]
    fn the_search_finds_every_key() {
        // Every key of the index is found by the binary search, whatever its position.
        let (base, store) = sample("every");
        let meta = meta_in(&base).unwrap();
        let idx = std::fs::read_to_string(base.join(&meta.dir).join("cities.idx")).unwrap();
        for line in idx.lines() {
            let k = line.split('\t').next().unwrap();
            let found = store.scan(store.lower_bound(k), 100, &|x| x == k);
            assert!(!found.is_empty(), "{k}");
        }
        assert_eq!(store.lower_bound("\u{10ffff}"), store.idx_len, "after every key (Cyrillic ones too)");
        assert!(store.lower_bound("zzzz") < store.idx_len, "Cyrillic keys sort after Latin ones");
        assert_eq!(store.lower_bound(""), 0);
    }

    /// The index is the same however many runs it was sorted in (the merge of runs).
    #[test]
    fn sorted_in_runs() {
        let base = std::env::temp_dir().join(format!("tally-cities-runs-{}", std::process::id()));
        let json = base.join("sample.json");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(&json, SAMPLE).unwrap();
        let index = |run: usize| {
            let dir = base.join(format!("run-{run}"));
            assert_eq!(convert_in_runs(&json, &dir, run).unwrap().0, 9);
            let files: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
            assert_eq!(files.len(), 2, "only cities.tsv and cities.idx are left: {files:?}");
            std::fs::read_to_string(dir.join("cities.idx")).unwrap()
        };
        let one = index(10_000);
        assert_eq!(index(3), one);
        assert_eq!(index(1), one);
        let names: Vec<&str> = one.lines().map(|l| l.split('\t').next().unwrap()).collect();
        assert!(names.windows(2).all(|w| w[0] <= w[1]), "sorted by name");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_new_archive_replaces_the_old_one() {
        let (base, _) = sample("replace");
        let old = meta_in(&base).unwrap();
        let json = base.join("sample.json");
        install(&base, &json, "1791999999 def", 1_792_000_000).unwrap();
        let new = meta_in(&base).unwrap();
        assert_eq!((new.version.as_str(), new.count, new.built_at), ("1791999999 def", 9, 1_791_877_958));
        assert!(!base.join(&old.dir).exists() && base.join(&new.dir).join("cities.idx").exists());
        assert_eq!(version_url(URL), "https://github.com/lucapf/tally/releases/download/cities/cities.version");
    }

    /// A download cut short (fewer cities than its header says) or a compressed archive is
    /// refused, and leaves the archive in use as it was, with nothing of its own behind.
    #[test]
    fn a_cut_or_compressed_archive_is_refused() {
        let (base, _) = sample("refused");
        let before = meta_in(&base).unwrap();
        let dirs = || {
            let mut found: Vec<String> = std::fs::read_dir(&base)
                .unwrap()
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("cities-"))
                .collect();
            found.sort();
            found
        };
        let kept = dirs();
        // The last two cities lost: 7 of the 9 the header announces.
        let lines: Vec<&str> = SAMPLE.lines().collect();
        let cut = base.join("cut.json");
        std::fs::write(&cut, lines[..lines.len() - 3].join("\n")).unwrap();
        assert_eq!(install(&base, &cut, "cut", 1_792_000_000).unwrap_err(), "the city archive is incomplete: 7 of 9 cities (download it again)");
        // Gzip's magic bytes: the archive as it was once published.
        let gz = base.join("gz.json");
        std::fs::write(&gz, [0x1f, 0x8b, 0x08, 0x00]).unwrap();
        assert!(install(&base, &gz, "gz", 1_792_000_001).unwrap_err().contains("compressed (gzip)"));
        assert_eq!(dirs(), kept, "nothing left behind");
        assert_eq!(meta_in(&base).unwrap(), before, "the archive in use is kept");
        assert_eq!(Store::open(&base, &before).unwrap().find("Rome").len(), 2);
    }

    /// The real archive (`target/cities-real.json`, from `scripts/build-cities.py target`): the
    /// conversion and how fast the searches are. `cargo test --release -- --ignored real_archive`.
    #[test]
    #[ignore]
    fn real_archive() {
        let json = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/cities-real.json");
        let base = std::env::temp_dir().join(format!("tally-cities-real-{}", std::process::id()));
        let started = std::time::Instant::now();
        let count = install(&base, &json, "real", 1).unwrap();
        println!("converted {count} cities in {:?}", started.elapsed());
        let store = Store::open(&base, &meta_in(&base).unwrap()).unwrap();
        for (what, run) in [
            ("exact Rome", Box::new(|| store.find("Rome").len()) as Box<dyn Fn() -> usize>),
            ("exact Рим", Box::new(|| store.find("Рим").len())),
            ("exact New York", Box::new(|| store.find("New York").len())),
            ("exact Springfield", Box::new(|| store.find("Springfield").len())),
            ("qualified Springfield, Illinois", Box::new(|| store.find("Springfield, Illinois").len())),
            ("prefix Spr", Box::new(|| store.starting_with("Spr", 8, None).len())),
            ("near Sprngfield", Box::new(|| store.near("Sprngfield").len())),
            ("near Chicaco", Box::new(|| store.near("Chicaco").len())),
        ] {
            let t = std::time::Instant::now();
            let n = run();
            println!("{what:<34} {n:>4} found in {:?}", t.elapsed());
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn maps_link() {
        let (_, store) = sample("maps");
        assert_eq!(store.find("Paris, France")[0].maps(), "https://www.google.com/maps?q=48.85341,2.3488");
    }
}
