//! The city archive, for distances between any two cities (`Springfield Paris`), not only the
//! capitals built in (`geo.rs`).
//!
//! The repository publishes `cities.json.gz` (built monthly from GeoNames by
//! `scripts/build-cities.py`, CC BY 4.0) with its `cities.version`. Tally downloads it when the
//! settings say so (`cities.refresh`, `tally --sync cities`) and turns it once into two files,
//! searched without loading them:
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
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// Where the archive comes from (also a setting, `cities.url`, for mirrors and tests).
pub const URL: &str = "https://github.com/lucapf/tally/releases/download/cities/cities.json.gz";
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

/// The key a name is searched by: `New-York` and `new york` are the same, `Zürich` and `zurich`.
pub fn key(name: &str) -> String {
    tz::key(name)
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
    meta_in(&crate::online::dir()?)
}

/// The archive to search: downloaded, and the feature on.
pub fn store() -> Option<Store> {
    if !crate::settings::current().cities_enabled() {
        return None;
    }
    let base = crate::online::dir()?;
    Store::open(&base, &meta_in(&base)?)
}

/// The archive's files, open.
pub struct Store {
    tsv: File,
    idx: File,
    idx_len: u64,
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
        let start = if pos == 0 {
            0
        } else {
            // Right after a newline, a line starts at `pos`; else after the next newline.
            let mut file = &self.idx;
            file.seek(SeekFrom::Start(pos - 1)).ok()?;
            let mut buf = [0u8; 4096];
            let n = file.read(&mut buf).ok()?;
            pos + buf[..n].iter().position(|&b| b == b'\n')? as u64
        };
        if start >= self.idx_len {
            return None;
        }
        let mut reader = BufReader::new(&self.idx);
        reader.seek(SeekFrom::Start(start)).ok()?;
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        Some((start, line.trim_end_matches('\n').to_string()))
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
        let mut reader = BufReader::new(&self.tsv);
        reader.seek(SeekFrom::Start(offset)).ok()?;
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        City::from_line(line.trim_end_matches('\n'))
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
        let qualifiers: Vec<String> = parts.map(key).collect();
        let target = key(first);
        if target.is_empty() {
            return Vec::new();
        }
        let hits = self.scan(self.lower_bound(&target), 5000, &|k| k == target);
        let found = self.cities(hits.into_iter().map(|(_, o)| o));
        found
            .into_iter()
            .filter(|c| {
                let names = [key(&c.region), key(&c.country), key(&c.country_code)];
                let country = tz::countries().iter().find(|t| t.code == c.country_code);
                qualifiers.iter().all(|q| {
                    names.contains(q) || country.is_some_and(|t| std::iter::once(&t.name).chain(&t.aliases).any(|n| key(n) == *q))
                })
            })
            .collect()
    }

    /// Cities whose name or alias starts with `prefix`, the most populous first (Tab).
    #[cfg_attr(not(feature = "tui"), allow(dead_code))]
    pub fn starting_with(&self, prefix: &str, limit: usize) -> Vec<City> {
        let target = key(prefix);
        if target.chars().count() < 2 {
            return Vec::new();
        }
        let hits = self.scan(self.lower_bound(&target), 2000, &|k| k.starts_with(target.as_str()));
        let mut found = self.cities(hits.into_iter().map(|(_, o)| o));
        found.truncate(limit);
        found
    }

    /// Cities whose name or alias is within 1 letter of `name` (2 from 6 letters): `Sprinfield`.
    pub fn near(&self, name: &str) -> Vec<City> {
        let target = key(name).replace(' ', "");
        let len = target.chars().count();
        if len < 4 {
            return Vec::new();
        }
        let limit = if len <= 5 { 1 } else { 2 };
        // Names sharing the first two letters (a misspelling rarely changes them).
        let first: String = key(name).chars().take(2).collect();
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

// ---------------------------------------------------------------- download and conversion

/// Turns the archive's JSON (one city per line, as the build script writes it) into the two
/// search files in `dir`; returns how many cities. Read line by line: the 20 MB archive is never
/// held in memory at once.
pub fn convert(json: &Path, dir: &Path) -> Result<(u64, i64), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let input = BufReader::new(File::open(json).map_err(|e| e.to_string())?);
    let mut tsv = BufWriter::new(File::create(dir.join("cities.tsv")).map_err(|e| e.to_string())?);
    let mut keys: Vec<(String, u64)> = Vec::new();
    let (mut offset, mut count, mut built_at) = (0u64, 0u64, 0i64);
    for line in input.lines() {
        let line = line.map_err(|e| e.to_string())?;
        let text = line.trim().trim_end_matches(',');
        if let Some(head) = text.strip_suffix("\"cities\": [") {
            // The header: `{"format": 1, …, "count": N, "cities": [`.
            let head = format!("{}\"x\": 0}}", head);
            if let Ok(doc) = crate::json::parse(&head) {
                built_at = doc.get("built_at").and_then(|v| v.number()).unwrap_or(0.0) as i64;
            }
            continue;
        }
        if !text.starts_with('{') {
            continue;
        }
        let Ok(c) = crate::json::parse(text) else { continue };
        let s = |k: &str| c.get(k).and_then(|v| v.str()).unwrap_or_default().to_string();
        let n = |k: &str| c.get(k).and_then(|v| v.number());
        let (Some(id), Some(lat), Some(lon)) = (n("id"), n("lat"), n("lon")) else { continue };
        let city = City {
            id: id as u64,
            name: s("name"),
            lat,
            lon,
            population: n("population").unwrap_or(0.0) as u64,
            country_code: s("country_code"),
            country: s("country"),
            region: s("region"),
            updated: s("updated"),
            aliases: c.get("aliases").map(|a| a.array().iter().filter_map(|x| x.str().map(str::to_string)).collect()).unwrap_or_default(),
        };
        let line = city.to_line();
        let mut names: Vec<String> = std::iter::once(&city.name).chain(&city.aliases).map(|n| key(n)).filter(|k| !k.is_empty()).collect();
        names.sort();
        names.dedup();
        for k in names {
            keys.push((k, offset));
        }
        tsv.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
        offset += line.len() as u64;
        count += 1;
    }
    tsv.flush().map_err(|e| e.to_string())?;
    keys.sort();
    let mut idx = BufWriter::new(File::create(dir.join("cities.idx")).map_err(|e| e.to_string())?);
    for (k, o) in keys {
        writeln!(idx, "{k}\t{o}").map_err(|e| e.to_string())?;
    }
    idx.flush().map_err(|e| e.to_string())?;
    Ok((count, built_at))
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

/// Downloads the archive at `url` (`curl … | gzip -dc`) and makes it the one in use, in `base`.
/// Returns how many cities.
pub fn download(base: &Path, url: &str, now: i64) -> Result<u64, String> {
    let version = remote_version(url).unwrap_or_default();
    let tmp = base.join(format!("cities.{}.download", std::process::id()));
    std::fs::create_dir_all(base).map_err(|e| e.to_string())?;
    let json = tmp.with_extension("json");
    {
        let mut curl = Command::new("curl").args(["-sfL", "--max-time", "600", url]).stdout(Stdio::piped()).spawn().map_err(|e| format!("curl: {e}"))?;
        let out = File::create(&json).map_err(|e| e.to_string())?;
        let gunzip = Command::new("gzip")
            .arg("-dc")
            .stdin(curl.stdout.take().ok_or("curl: no output")?)
            .stdout(out)
            .status()
            .map_err(|e| format!("gzip: {e}"))?;
        let fetched = curl.wait().map_err(|e| e.to_string())?;
        if !fetched.success() || !gunzip.success() {
            let _ = std::fs::remove_file(&json);
            return Err(format!("could not download the city archive ({url})"));
        }
    }
    let result = install(base, &json, &version, now);
    let _ = std::fs::remove_file(&json);
    result
}

/// Converts a downloaded archive and switches to it (the meta file renamed over the old one), then
/// removes the previous one.
pub fn install(base: &Path, json: &Path, version: &str, now: i64) -> Result<u64, String> {
    let previous = meta_in(base);
    let name = format!("cities-{now}-{}", std::process::id());
    let (count, built_at) = convert(json, &base.join(&name))?;
    if count == 0 {
        let _ = std::fs::remove_dir_all(base.join(&name));
        return Err("the city archive is empty".into());
    }
    let version = if version.is_empty() { format!("{built_at} -") } else { version.to_string() };
    let meta = format!(
        "{{\"version\":{},\"built_at\":{built_at},\"count\":{count},\"downloaded_at\":{now},\"dir\":{},\"source\":{}}}\n",
        crate::json::string(&version),
        crate::json::string(&name),
        crate::json::string(SOURCE)
    );
    let tmp = base.join(format!("cities.json.{}.tmp", std::process::id()));
    std::fs::write(&tmp, meta).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, base.join("cities.json")).map_err(|e| e.to_string())?;
    if let Some(old) = previous.filter(|p| p.dir != name && !p.dir.is_empty()) {
        let _ = std::fs::remove_dir_all(base.join(old.dir));
    }
    Ok(count)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::path::PathBuf;

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
        assert_eq!(names(&store.starting_with("spr", 2)), ["Springfield, Missouri, United States", "Springfield, Massachusetts, United States"]);
        assert_eq!(store.starting_with("Par", 8).len(), 2);
        assert!(store.starting_with("p", 8).is_empty(), "at least 2 letters");
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
            ("prefix Spr", Box::new(|| store.starting_with("Spr", 8).len())),
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
