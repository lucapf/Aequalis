//! Public holidays, for the calendar (`calendar.rs`): every country python-holidays knows, last year
//! to next year (`scripts/build-holidays.py`), one file, `holidays.json`, replaced every day in the
//! release `holidays` (`.github/workflows/holidays.yml`). The sync checks it once a day
//! (`holidays.refresh`, `holidays.interval`) and keeps it beside the other online data; without it
//! (never synced, the `holidays` setting off) the calendar shows no holidays.
//!
//! ```json
//! {"format":1,"built":"2026-10-26","years":[2025,2026,2027],"countries":{
//!   "IT":{"default":"it","days":["2025-01-01",…],"names":{"en":["New Year's Day",…],"it":["Capodanno",…]}}}}
//! ```
//! A name per day in each language; a day with two holidays is there twice.

use std::path::PathBuf;
use std::sync::Arc;

/// Where the file is (also a setting, `holidays.url`, for mirrors and tests).
pub const URL: &str = "https://github.com/lucapf/aequalis/releases/download/holidays/holidays.json";
/// Seconds to download it (about 1 MB).
const SECONDS: u32 = 60;

/// The countries kept read at most: a calendar's, and those of the last few before it.
const KEPT: usize = 2 * crate::calendar::MAX_COUNTRIES;

/// The archive as read: all its countries counted, those asked for read.
#[derive(Debug, Default, PartialEq)]
pub struct Archive {
    /// The day it was built (`2026-10-26`).
    pub built: String,
    pub years: Vec<i64>,
    /// How many countries it has.
    count: usize,
    /// The countries read, by code (`IT`), sorted.
    countries: Vec<(String, Country)>,
}

#[derive(Debug, Default, PartialEq)]
struct Country {
    /// Its own language (`it`).
    default: String,
    /// Days since 1970-01-01, in order.
    days: Vec<i64>,
    /// A name per day, by language (`en`, `it`).
    names: Vec<(String, Vec<String>)>,
}

/// A holiday: its day (days since 1970-01-01) and name.
#[derive(Debug, Clone, PartialEq)]
pub struct Holiday {
    pub day: i64,
    pub name: String,
}

/// The archive's text read, all its countries; `None` when it is not one.
pub fn parse(text: &str) -> Option<Archive> {
    read(text, |_| true, true)
}

/// The archive's text read: its countries counted, those `wanted` (their codes, `IT`) read, and,
/// with `check`, the others read too to see they can be, then let go. `None` when it is not an
/// archive, or a country read is not one. The rest stays text: a calendar shows a few countries of
/// some 250, and reading them all took time and some 6 MB.
fn read(text: &str, wanted: impl Fn(&str) -> bool, check: bool) -> Option<Archive> {
    let mut archive = Archive::default();
    let mut format = None;
    for (key, raw) in crate::json::members_raw(text).ok()? {
        match key.as_str() {
            "format" => format = crate::json::parse(raw).ok()?.number(),
            "built" => archive.built = crate::json::parse(raw).ok()?.into_string().unwrap_or_default(),
            "years" => archive.years = crate::json::parse(raw).ok()?.array().iter().filter_map(|y| y.number()).map(|y| y as i64).collect(),
            "countries" => {
                let countries = crate::json::members_raw(raw).ok()?;
                archive.count = countries.len();
                for (code, raw) in countries {
                    let code = code.to_ascii_uppercase();
                    let keep = wanted(&code);
                    if keep || check {
                        let country = country_of(raw)?;
                        if keep {
                            archive.countries.push((code, country));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if format != Some(1.0) {
        return None;
    }
    archive.countries.sort_by(|a, b| a.0.cmp(&b.0));
    Some(archive)
}

/// A country's holidays, as the archive writes them.
fn country_of(raw: &str) -> Option<Country> {
    let c = crate::json::parse(raw).ok()?;
    let days: Vec<i64> = c.get("days")?.array().iter().map(|d| day_of(d.str()?)).collect::<Option<_>>()?;
    let names: Vec<(String, Vec<String>)> = c.get("names")?.members().iter().map(|(lang, list)| (lang.clone(), list.strings())).collect();
    if names.is_empty() || names.iter().any(|(_, list)| list.len() != days.len()) {
        return None;
    }
    let default = c.get("default").and_then(|d| d.str()).unwrap_or("en").to_string();
    Some(Country { default, days, names })
}

/// `2026-12-25` as days since 1970-01-01.
fn day_of(date: &str) -> Option<i64> {
    let mut parts = date.splitn(3, '-').map(str::parse::<i64>);
    let (y, m, d) = (parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?);
    let (m, d) = (u32::try_from(m).ok().filter(|m| (1..=12).contains(m))?, u32::try_from(d).ok().filter(|d| (1..=31).contains(d))?);
    Some(crate::time::days_from_civil(y, m, d))
}

impl Archive {
    /// The holidays of a country (its code, `IT`) from day `from` to day `to` (both included), named
    /// in `language` when the archive has it, else in English, else in the country's own; `None`
    /// when the archive has no such country.
    pub fn of(&self, code: &str, from: i64, to: i64, language: Option<&str>) -> Option<Vec<Holiday>> {
        let i = self.countries.binary_search_by(|(c, _)| c.as_str().cmp(code)).ok()?;
        let country = &self.countries[i].1;
        let names = |lang: &str| country.names.iter().find(|(l, _)| l == lang).map(|(_, n)| n);
        let names = language.and_then(names).or_else(|| names("en")).or_else(|| names(&country.default)).unwrap_or(&country.names[0].1);
        let start = country.days.partition_point(|&d| d < from);
        Some(country.days[start..].iter().zip(&names[start..]).take_while(|(&d, _)| d <= to).map(|(&day, name)| Holiday { day, name: name.clone() }).collect())
    }

    /// How many countries it has (read or not).
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// `~/.config/aequalis/holidays.json`.
pub fn path() -> Option<PathBuf> {
    crate::files::dir().map(|d| d.join("holidays.json"))
}

/// The archive as last read: its file's stamp, the countries asked for (`IT`), oldest first.
type Read = (crate::files::Stamp, Vec<String>, Arc<Archive>);

/// The archive kept, with the countries `codes` (`IT`) read; `None` when never downloaded. Read
/// again only when the file changes or a country is new; the last few asked for stay read, so going
/// back to them (`cal it`, `cal it us`, `cal it`) reads nothing.
pub fn load(codes: &[&str]) -> Option<Arc<Archive>> {
    static KEPT_READ: std::sync::Mutex<Option<Read>> = std::sync::Mutex::new(None);
    load_kept(&KEPT_READ, &path()?, codes)
}

/// `load`, with what was read last in `kept`.
fn load_kept(kept: &std::sync::Mutex<Option<Read>>, path: &std::path::Path, codes: &[&str]) -> Option<Arc<Archive>> {
    let stamp = crate::files::stamp(path)?;
    let mut kept = kept.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut asked = match &*kept {
        Some((s, asked, archive)) if *s == stamp => {
            if codes.iter().all(|c| asked.iter().any(|a| a == c)) {
                return Some(Arc::clone(archive));
            }
            asked.clone()
        }
        _ => Vec::new(),
    };
    // These last: the oldest asked for go first.
    asked.retain(|a| !codes.contains(&a.as_str()));
    asked.extend(codes.iter().map(|c| c.to_string()));
    asked.drain(..asked.len().saturating_sub(KEPT.max(codes.len())));
    let text = std::fs::read_to_string(path).ok()?;
    let archive = Arc::new(read(&text, |c| asked.iter().any(|a| a == c), false)?);
    *kept = Some((stamp, asked, Arc::clone(&archive)));
    Some(archive)
}

/// When the archive was last downloaded (Unix seconds); `None`: never.
pub fn downloaded_at() -> Option<i64> {
    let modified = std::fs::metadata(path()?).ok()?.modified().ok()?;
    modified.duration_since(std::time::UNIX_EPOCH).ok().and_then(|d| i64::try_from(d.as_secs()).ok())
}

/// Downloads the archive (`holidays.url`) and keeps it: what it holds (`247 countries' holidays,
/// 2025-2027`), or why not. The one kept stays when the new one cannot be had.
pub fn sync(settings: &crate::settings::Settings) -> Result<String, String> {
    if !settings.holidays_enabled() {
        return Ok("holidays: off (the holidays setting)".into());
    }
    let path = path().ok_or("no home directory")?;
    let body = crate::http::get(settings.get("holidays.url"), SECONDS, true).ok_or("could not download the public holidays")?;
    let text = String::from_utf8_lossy(&body);
    let archive = read(&text, |_| false, true).ok_or("the public holidays downloaded are not readable")?;
    crate::files::write_atomic(&path, text.as_bytes()).map_err(|e| format!("could not keep the public holidays: {e}"))?;
    Ok(format!("{} countries' holidays, {}", archive.len(), span(&archive.years)))
}

/// `2025-2027`.
fn span(years: &[i64]) -> String {
    match (years.iter().min(), years.iter().max()) {
        (Some(a), Some(b)) if a != b => format!("{a}-{b}"),
        (Some(a), _) => a.to_string(),
        _ => String::new(),
    }
}

/// What is kept, for people: `247 countries, 2025-2027`, `none yet`, or `off`.
pub fn summary(enabled: bool) -> String {
    if !enabled {
        return "off".to_string();
    }
    match load(&[]) {
        Some(a) if !a.is_empty() => format!("{} countries, {}", a.len(), span(&a.years)),
        _ => "none yet".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"format":1,"built":"2026-10-26","years":[2025,2026,2027],"countries":{
        "IT":{"default":"it","days":["2026-01-01","2026-11-01","2026-11-01","2026-12-25"],
              "names":{"en":["New Year's Day","National Unity Day","All Saints' Day","Christmas Day"],
                       "it":["Capodanno","Giorno dell'unità nazionale","Ognissanti","Natale"]}},
        "AE":{"default":"ar","days":["2026-12-02"],"names":{"ar":["اليوم الوطني"],"en":["National Day"]}}}}"#;

    fn day(y: i64, m: u32, d: u32) -> i64 {
        crate::time::days_from_civil(y, m, d)
    }

    #[test]
    fn read_and_looked_up() {
        let a = parse(SAMPLE).unwrap();
        assert_eq!((a.len(), a.years.as_slice(), a.built.as_str()), (2, &[2025, 2026, 2027][..], "2026-10-26"));
        let nov = a.of("IT", day(2026, 11, 1), day(2026, 11, 30), Some("it")).unwrap();
        assert_eq!(nov, [Holiday { day: day(2026, 11, 1), name: "Giorno dell'unità nazionale".into() }, Holiday { day: day(2026, 11, 1), name: "Ognissanti".into() }]);
        // Both ends included; another month has its own.
        let dec = a.of("IT", day(2026, 12, 1), day(2026, 12, 25), Some("it")).unwrap();
        assert_eq!(dec.len(), 1);
        assert!(a.of("IT", day(2026, 2, 1), day(2026, 2, 28), None).unwrap().is_empty());
        assert_eq!(a.of("FR", 0, i64::MAX, None), None, "not in the archive");
    }

    #[test]
    fn named_in_the_language_asked_else_english_else_the_countrys() {
        let a = parse(SAMPLE).unwrap();
        let name = |code: &str, lang: Option<&str>| a.of(code, day(2026, 12, 2), day(2026, 12, 25), lang).unwrap()[0].name.clone();
        assert_eq!(name("AE", Some("ar")), "اليوم الوطني");
        assert_eq!(name("AE", Some("it")), "National Day", "no Italian: English");
        assert_eq!(name("AE", None), "National Day");
        assert_eq!(name("IT", Some("it")), "Natale");
        let only_arabic = parse(r#"{"format":1,"years":[2026],"countries":{"AE":{"default":"ar","days":["2026-12-02"],"names":{"ar":["اليوم الوطني"]}}}}"#).unwrap();
        assert_eq!(only_arabic.of("AE", 0, i64::MAX, Some("it")).unwrap()[0].name, "اليوم الوطني", "no English: the country's own");
    }

    #[test]
    fn only_the_countries_asked_for_read() {
        let a = read(SAMPLE, |c| c == "AE", false).unwrap();
        assert_eq!((a.len(), a.years.as_slice(), a.built.as_str()), (2, &[2025, 2026, 2027][..], "2026-10-26"), "all counted");
        assert!(a.of("AE", 0, i64::MAX, None).is_some());
        assert_eq!(a.of("IT", 0, i64::MAX, None), None, "not asked for: not read");
        // A country that is not one: unseen unless asked for, or checked.
        let broken = SAMPLE.replace(r#""2026-12-25"]"#, r#""2026-12-25", "x"]"#);
        assert!(read(&broken, |c| c == "AE", false).is_some());
        assert_eq!(read(&broken, |c| c == "IT", false), None);
        assert_eq!(read(&broken, |_| false, true), None);
        assert_eq!(read(SAMPLE, |_| false, true).map(|a| (a.len(), a.countries.len())), Some((2, 0)), "checked, let go");
    }

    #[test]
    fn kept_read_until_a_new_country_or_the_file_changes() {
        let dir = std::env::temp_dir().join(format!("aequalis-holidays-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("holidays.json");
        std::fs::write(&path, SAMPLE).unwrap();
        let kept = std::sync::Mutex::new(None);
        let it = load_kept(&kept, &path, &["IT"]).unwrap();
        assert!(it.of("IT", 0, i64::MAX, None).is_some() && it.of("AE", 0, i64::MAX, None).is_none());
        assert!(Arc::ptr_eq(&it, &load_kept(&kept, &path, &["IT"]).unwrap()), "asked again: nothing read");
        assert!(Arc::ptr_eq(&it, &load_kept(&kept, &path, &[]).unwrap()), "none asked: nothing read");
        // A new country: read, with the one before.
        let both = load_kept(&kept, &path, &["AE"]).unwrap();
        assert!(!Arc::ptr_eq(&it, &both));
        assert!(both.of("IT", 0, i64::MAX, None).is_some() && both.of("AE", 0, i64::MAX, None).is_some());
        assert!(Arc::ptr_eq(&both, &load_kept(&kept, &path, &["IT", "AE"]).unwrap()));
        // At most KEPT countries stay read, the oldest go.
        let many: Vec<String> = (0..KEPT).map(|i| format!("Z{i}")).collect();
        let many: Vec<&str> = many.iter().map(String::as_str).collect();
        let later = load_kept(&kept, &path, &many).unwrap();
        assert!(later.of("IT", 0, i64::MAX, None).is_none(), "IT asked for longest ago: let go");
        // The file changed (a sync): read again.
        std::fs::write(&path, SAMPLE.replace("2026-10-26", "2026-10-27") + "\n").unwrap();
        assert_eq!(load_kept(&kept, &path, &["IT"]).unwrap().built, "2026-10-27");
        std::fs::remove_file(&path).unwrap();
        assert_eq!(load_kept(&kept, &path, &["IT"]), None, "no file");
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn not_an_archive() {
        for text in ["", "{}", r#"{"format":2,"years":[],"countries":{}}"#, r#"{"format":1,"years":[2026],"countries":{"IT":{"days":["2026-01-01"],"names":{"en":[]}}}}"#] {
            assert_eq!(parse(text), None, "{text}");
        }
        assert_eq!(parse(r#"{"format":1,"years":[2026],"countries":{"IT":{"days":["2026-13-01"],"names":{"en":["x"]}}}}"#), None, "a bad date");
    }

    #[test]
    fn spans() {
        assert_eq!(span(&[2025, 2026, 2027]), "2025-2027");
        assert_eq!(span(&[2026]), "2026");
        assert_eq!(span(&[]), "");
    }
}
