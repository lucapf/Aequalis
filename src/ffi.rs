//! The C interface for apps that embed aequalis (the Android app; feature `ffi`, built as a shared
//! library). Strings in and out are UTF-8 and NUL-terminated; every string returned is JSON and is
//! given back with `aequalis_free`. `include/aequalis.h` declares them.
//!
//! - `aequalis_init(config)` once, first: the data folder, the time zone, the number format and the
//!   measurement system, which the app knows (an app gets no `HOME`, `TZ` or `LANG` that say
//!   them).
//! - `aequalis_set_fetcher(fetch)`: the app's downloader (the app can't start `curl`).
//! - `aequalis_eval(request)`: the reply of `aequalis --json <request>` (results, errors, choices);
//!   `aequalis_view(request, submitted)`: the same, with what a UI shows (`json::view`).
//! - `aequalis_settings()`, `aequalis_set(key, value)`, `aequalis_sync(auto)`, `aequalis_trivia()`,
//!   `aequalis_measures()`, `aequalis_currencies()`.

use crate::config::{Config, System};
use crate::json::{self, Object};
use std::ffi::{c_char, CStr, CString};
use std::path::Path;

/// A string from the app (`None`: null or not UTF-8).
///
/// # Safety
/// `s` is null or a NUL-terminated string that stays valid during the call.
unsafe fn text<'a>(s: *const c_char) -> Option<&'a str> {
    if s.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(s) }.to_str().ok()
}

/// A request (or the text before the cursor) from the app, read only as far as the longest request
/// can reach: `Err` when it has more bytes than `MAX_INPUT` characters can take (4 each), unread
/// past them; `Ok(None)`: null or not UTF-8.
///
/// # Safety
/// `s` is null or a NUL-terminated string that stays valid during the call.
unsafe fn request_text<'a>(s: *const c_char) -> Result<Option<&'a str>, ()> {
    if s.is_null() {
        return Ok(None);
    }
    let mut n = 0;
    // SAFETY: every byte up to the NUL is the caller's string; none past it is read.
    while unsafe { *s.add(n) } != 0 {
        if n == MAX_BYTES {
            return Err(());
        }
        n += 1;
    }
    Ok(std::str::from_utf8(unsafe { std::slice::from_raw_parts(s.cast::<u8>(), n) }).ok())
}

/// The most bytes a request of `MAX_INPUT` characters can take (4 a character, in UTF-8): more is
/// refused without reading them (also by the JNI bridge).
pub const MAX_BYTES: usize = 4 * crate::MAX_INPUT;

/// The reply to a request longer than `MAX_INPUT` characters, given before anything reads it.
pub fn too_long() -> String {
    json::refused(crate::too_long())
}

/// No suggestions: `suggest`'s reply for a text with nothing to offer.
pub const NO_SUGGESTIONS: &str = r#"{"start":0,"items":[]}"#;

/// A string for the app, given back with `aequalis_free`.
fn give(s: String) -> *mut c_char {
    CString::new(s.replace('\0', "")).map_or(std::ptr::null_mut(), CString::into_raw)
}

fn error(message: &str) -> *mut c_char {
    give(Object::new().num("ok", false).str("error", message).finish())
}

/// Frees a string returned by aequalis.
///
/// # Safety
/// `s` was returned by a `aequalis_*` function and is freed once.
#[no_mangle]
pub unsafe extern "C" fn aequalis_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// What the app says about the device, at start.
#[derive(Debug, Clone, PartialEq)]
pub struct Init {
    /// The folder where aequalis keeps its data (`<dir>/aequalis/…`): the app's Application Support.
    pub dir: String,
    /// The time zone (`Europe/Rome`), and where the zone files are (the system's or a copy).
    pub zone: Option<String>,
    pub zoneinfo: Option<String>,
    pub format: Option<crate::numbers::NumberFormat>,
    pub imperial: Option<bool>,
    /// A frozen "now" (Unix seconds), for tests and the fuzzer (`fuzz/`): every date answer
    /// then stays the same.
    pub now: Option<i64>,
}

/// `{"dir": "…", "zone": "Europe/Rome", "zoneinfo": "…", "decimal": ",", "thousands": ".",
/// "grouping": [3], "imperial": false, "now": 1792222391}` (only `dir` is required).
pub fn parse_init(text: &str) -> Result<Init, String> {
    let doc = json::parse(text).map_err(|e| format!("aequalis_init: {e}"))?;
    let s = |key: &str| doc.get(key).and_then(|v| v.str()).filter(|v| !v.is_empty()).map(str::to_string);
    let dir = s("dir").ok_or("aequalis_init: `dir` is required")?;
    let format = match (s("decimal"), doc.get("thousands").and_then(|v| v.str())) {
        (Some(decimal), Some(thousands)) => Some(crate::numbers::NumberFormat {
            decimal,
            thousands: thousands.to_string(),
            grouping: doc.get("grouping").map(|g| g.array().iter().filter_map(json::Value::number).map(|n| n as u8).collect()).unwrap_or_else(|| vec![3]),
        }),
        _ => None,
    };
    let now = doc.get("now").and_then(|v| v.number()).map(|n| n as i64);
    Ok(Init { dir, zone: s("zone"), zoneinfo: s("zoneinfo"), format, imperial: doc.get("imperial").and_then(|v| v.boolean()), now })
}

/// Starts aequalis for the app: `config` as in `parse_init`. Call once, before anything else.
/// Returns `{"ok":true}` or `{"ok":false,"error":…}`.
///
/// # Safety
/// `config` is a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn aequalis_init(config_json: *const c_char) -> *mut c_char {
    let Some(text) = (unsafe { text(config_json) }) else { return error("aequalis_init: no configuration") };
    let init = match parse_init(text) {
        Ok(i) => i,
        Err(e) => return error(&e),
    };
    start(&init);
    give(started())
}

/// `aequalis_init`'s reply: `{"ok":true,"max_input":512}`, the longest request in characters, for an
/// app that cuts a long paste.
pub fn started() -> String {
    Object::new().num("ok", true).num("max_input", crate::MAX_INPUT).finish()
}

/// Gives aequalis what the app knows, once, before anything else: set directly, not through
/// environment variables (`setenv` races with the app's other threads).
pub fn start(init: &Init) {
    crate::files::set_dir(Path::new(&init.dir));
    // The database first: setting the zone reads it.
    if let Some(database) = &init.zoneinfo {
        crate::time::tz::set_zoneinfo(database);
    }
    if let Some(zone) = &init.zone {
        // A zone the database does not know: local time is the C library's.
        let _ = crate::time::set_device_zone(zone);
    }
    if let Some(format) = &init.format {
        crate::numbers::set_device(format.clone());
    }
    if let Some(imperial) = init.imperial {
        crate::locale::set_device_system(if imperial { System::Imperial } else { System::Metric });
    }
    if let Some(now) = init.now {
        crate::time::set_now(now);
    }
}

/// The app's downloader: saves `url` into the file `to` within `seconds`; with `strict`, an HTTP
/// error is a failure. Returns whether it saved it. Called from the thread that syncs.
pub type Fetch = extern "C" fn(url: *const c_char, to: *const c_char, seconds: u32, strict: bool) -> bool;

struct AppFetcher(Fetch);

impl crate::http::Fetcher for AppFetcher {
    fn get(&self, url: &str, seconds: u32, strict: bool) -> Option<Vec<u8>> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("aequalis-fetch-{}-{n}", std::process::id()));
        let body = self.call(url, &path, seconds, strict).then(|| std::fs::read(&path).ok()).flatten();
        let _ = std::fs::remove_file(&path);
        body
    }

    fn save(&self, url: &str, to: &Path, seconds: u32) -> bool {
        self.call(url, to, seconds, true)
    }
}

impl AppFetcher {
    fn call(&self, url: &str, to: &Path, seconds: u32, strict: bool) -> bool {
        let (Ok(url), Ok(to)) = (CString::new(url), CString::new(to.to_string_lossy().as_bytes())) else { return false };
        (self.0)(url.as_ptr(), to.as_ptr(), seconds, strict)
    }
}

/// Registers the app's downloader (once).
#[no_mangle]
pub extern "C" fn aequalis_set_fetcher(fetch: Fetch) {
    crate::http::register(Box::new(AppFetcher(fetch)));
}

/// The reply to a request, as `aequalis --json <words…>`: `{"ok":true,"result":…}`, an error, or
/// the choices of an ambiguous request; the notices too. Only the request: no command-line
/// options (`-v`, `--precision`), whose messages would make no sense in an app.
///
/// # Safety
/// `request` is a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn aequalis_eval(request: *const c_char) -> *mut c_char {
    match unsafe { request_text(request) } {
        Err(()) => give(too_long()),
        Ok(None) => error("no request"),
        Ok(Some(request)) => give(eval(request)),
    }
}

/// `aequalis_eval`'s reply, as submitted (Enter pressed): nothing hidden.
pub fn eval(request: &str) -> String {
    json::view(request, true)
}

/// The reply to a request as a UI shows it (`json::view`): what to show, the value to copy, the
/// question's choices; `submitted`: Enter was pressed on it, else a reply that is no result shows
/// nothing while a word is being typed.
///
/// # Safety
/// `request` is a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn aequalis_view(request: *const c_char, submitted: bool) -> *mut c_char {
    match unsafe { request_text(request) } {
        Err(()) => give(too_long()),
        Ok(None) => error("no request"),
        Ok(Some(request)) => give(json::view(request, submitted)),
    }
}

/// About, for an app: the version, a tagline, and the facts (`[label, text]`: the online data,
/// the last sync), then the sources of the data with their credits.
/// The memory the core holds, as its allocator counts it (`alloc::counted`): `{"held":bytes,
/// "peak":bytes}`, or `{}` where it isn't counted (off Android).
pub fn memory() -> String {
    match crate::alloc::counted() {
        Some((held, peak)) => Object::new().num("held", held).num("peak", peak).finish(),
        None => Object::new().finish(),
    }
}

pub fn about() -> String {
    let pair = |(label, text): (&str, String)| format!("[{},{}]", json::string(label), json::string(&text));
    // The process's memory is the app's, not aequalis's; the version is the reply's own: left out.
    let facts: Vec<String> = crate::about::facts("").into_iter().filter(|(label, _)| !["Memory", "Version"].contains(label)).map(pair).collect();
    let sources: Vec<String> = [
        ("Cities", crate::cities::SOURCE.to_string()),
        ("Exchange rates", crate::currency::SOURCE.to_string()),
        ("Spacecraft", "JPL Horizons (https://ssd.jpl.nasa.gov/horizons)".to_string()),
        ("Satellites", "CelesTrak (https://celestrak.org)".to_string()),
    ]
    .into_iter()
    .map(pair)
    .collect();
    Object::new()
        .str("name", crate::about::NAME)
        .str("tagline", crate::about::TAGLINE)
        .str("version", env!("CARGO_PKG_VERSION"))
        .list("facts", &facts)
        .list("sources", &sources)
        .finish()
}

/// Every setting: key, value, default, kind, options, what it does (`aequalis --settings --json`),
/// but the interactive mode's (`tui.`), which mean nothing in an app.
#[no_mangle]
pub extern "C" fn aequalis_settings() -> *mut c_char {
    give(settings())
}

pub fn settings() -> String {
    let settings = crate::settings::path().map(|p| crate::settings::load_from(&p)).unwrap_or(Ok(Default::default()));
    match settings {
        Ok(s) => crate::settings::json_where(&s, |k| !k.key.starts_with("tui.")),
        Err(e) => Object::new().num("ok", false).str("error", &e).finish(),
    }
}

/// Changes a setting (`value` null: back to its default) and saves it: `{"ok":true,"key":…,
/// "value":…}` or the error.
///
/// # Safety
/// `key` is a NUL-terminated string; `value` too, or null.
#[no_mangle]
pub unsafe extern "C" fn aequalis_set(key: *const c_char, value: *const c_char) -> *mut c_char {
    let Some(key) = (unsafe { text(key) }) else { return error("no key") };
    give(set(key, unsafe { text(value) }))
}

/// Changes a setting and saves it (`None`: its default): `{"ok":true,"key":…,"value":…}` or the
/// error.
pub fn set(key: &str, value: Option<&str>) -> String {
    match crate::settings::change(key, value) {
        Ok(s) => Object::new().num("ok", true).str("key", key).str("value", s.get(key)).finish(),
        Err(e) => Object::new().num("ok", false).str("error", &e).finish(),
    }
}

/// The data shipped with an app (as valid when it was built), so the features work before any
/// download: `rates`, ExchangeRate-API's answer, stored when newer than the rates there; `cities`,
/// the city archive's JSON file, installed when no archive is (it takes a second or two: call it
/// off the main thread). `{"ok":true,"rates":N,"cities":N}`: how many were stored (0: the ones
/// there were kept), or the error.
pub fn seed(rates: Option<&str>, cities: Option<&Path>) -> String {
    let now = crate::time::unix_now();
    let stored = match rates {
        Some(body) => match crate::currency::store(body, now) {
            Ok(n) => n,
            Err(e) => return Object::new().num("ok", false).str("error", &e).finish(),
        },
        None => 0,
    };
    let installed = match (cities, crate::files::dir()) {
        (Some(json), Some(base)) if crate::cities::meta_in(&base).is_none() => match crate::cities::install(&base, json, "", now) {
            Ok(n) => n,
            Err(e) => return Object::new().num("ok", false).str("error", &e).finish(),
        },
        _ => 0,
    };
    Object::new().num("ok", true).num("rates", stored).num("cities", installed).finish()
}

/// `seed`, for the C interface: `rates` the answer's text, `cities` the archive file's path (null
/// or empty: none).
///
/// # Safety
/// `rates` and `cities` are null or NUL-terminated strings.
#[no_mangle]
pub unsafe extern "C" fn aequalis_seed(rates: *const c_char, cities: *const c_char) -> *mut c_char {
    let rates = unsafe { text(rates) }.filter(|r| !r.is_empty());
    let cities = unsafe { text(cities) }.filter(|c| !c.is_empty());
    give(seed(rates, cities.map(Path::new)))
}

/// Downloads the online data, in this thread (call it off the main thread): everything, or with
/// `auto` what the refresh settings say (at launch, in a background refresh). The reply of
/// `aequalis --sync --json`.
#[no_mangle]
pub extern "C" fn aequalis_sync(auto: bool) -> *mut c_char {
    // The groups that failed: the reply's warnings, after the others had their turn.
    let result = crate::online::sync(auto)
        .map(|report| crate::Output { stdout: report.done.join("\n"), warnings: report.failed, ..crate::Output::default() })
        .map_err(|e| crate::CliError::Convert(crate::convert::ConvertError::Domain(e)));
    give(json::result(&result, &crate::notify::current()))
}

/// The Trivia: `{"sections":[{"name":…,"entries":[{"name","short","description","value",
/// "written","popup":[[label, text]…]}]}]}`; `written` is what goes into the request (a tip: its
/// example).
#[no_mangle]
pub extern "C" fn aequalis_trivia() -> *mut c_char {
    give(trivia())
}

pub fn trivia() -> String {
    use crate::trivia::{self, Source};
    let config = Config::shared();
    let precision = crate::settings::current().precision();
    let (now, online) = (crate::time::unix_now(), crate::online::load());
    let number = |n: String| crate::numbers::device().apply(&n);
    let sections: Vec<String> = trivia::sections()
        .into_iter()
        .map(|section| {
            let entries: Vec<String> = trivia::entries()
                .iter()
                .filter(|e| e.section == section)
                .map(|e| {
                    let live = online.get(&e.name);
                    let (value, written) = match (&e.source, trivia::value(config, e, live, now)) {
                        (Source::Tip(example), _) => (example.clone(), example.clone()),
                        (_, Ok((v, origin))) => {
                            let how = trivia::origin_text(e, origin).map(|t| format!(" ({t})")).unwrap_or_default();
                            (format!("{}{how}", trivia::shown(config, e, v, precision, &number)), format!("{} ", trivia::written(config, e, v)))
                        }
                        (_, Err(err)) => (format!("error: {err}"), String::new()),
                    };
                    // Each line twice: for people (the device's separators) and plain, to write a measure.
                    let plain = trivia::popup(config, e, live, now, precision, &|n| n);
                    let popup: Vec<String> = trivia::popup(config, e, live, now, precision, &number)
                        .iter()
                        .zip(&plain)
                        .map(|((label, text), (_, written))| {
                            let options: Vec<String> =
                                trivia_measures(text, written).iter().map(|(s, w)| format!("[{},{}]", json::string(s), json::string(w))).collect();
                            format!("[{},{},[{}]]", json::string(label), json::string(text), options.join(","))
                        })
                        .collect();
                    Object::new()
                        .str("name", &e.name)
                        .str("short", &trivia::short(e, now))
                        .str("description", &e.description)
                        .str("value", &value)
                        .str("written", &written)
                        .str("symbol", e.symbol.as_deref().unwrap_or(""))
                        .list("popup", &popup)
                        .finish()
                })
                .collect();
            Object::new().str("name", section).list("entries", &entries).finish()
        })
        .collect();
    Object::new().list("sections", &sections).finish()
}

/// The measures of a Trivia popup line, to write one into the request: the line's parts (split at
/// ` - `), each part's leading number and unit, as shown (`25,800,920,358 km`) and as written
/// (`25800920358 km`, from the same line in plain digits). A part that starts with no number
/// (`about 1.02 km/s…`) is none.
pub fn trivia_measures(shown: &str, plain: &str) -> Vec<(String, String)> {
    let head = |part: &str| -> Option<String> {
        // What follows the measure: `(estimated…)`, `, next on 12 Mar 27`, `, away from the Sun`.
        let end = [" (", ", "].iter().filter_map(|s| part.find(s)).min().unwrap_or(part.len());
        let measure = part[..end].trim();
        // A number and its unit: `8 min 19 s` (a light time) is no value to calculate with.
        let shape = measure.split_whitespace().count() == 2;
        (shape && measure.starts_with(|c: char| c.is_ascii_digit() || c == '-')).then(|| measure.to_string())
    };
    shown.split(" - ").zip(plain.split(" - ")).filter_map(|(s, p)| Some((head(s)?, head(p)?))).collect()
}

/// The unit classes for the Measures: `{"classes":[{"name","systems":[…,…],"units":
/// [{"symbol","name","system":"metric"|"imperial"|"both"}]}]}`, in the TUI's order.
#[no_mangle]
pub extern "C" fn aequalis_measures() -> *mut c_char {
    give(measures())
}

pub fn measures() -> String {
    let classes: Vec<String> = Config::shared()
        .categories
        .iter()
        .map(|c| {
            let units: Vec<String> = c
                .units
                .iter()
                .map(|u| {
                    let system = match u.system {
                        System::Metric => "metric",
                        System::Imperial => "imperial",
                        System::Both => "both",
                    };
                    Object::new().str("symbol", u.symbol).str("name", u.name).str("system", system).finish()
                })
                .collect();
            let systems: Vec<String> = c.systems.iter().map(|s| s.to_string()).collect();
            Object::new().str("name", c.name).strs("systems", &systems).list("units", &units).finish()
        })
        .collect();
    Object::new().list("classes", &classes).finish()
}

/// Suggestions for the word being typed at the end of `before` (the text up to the cursor), from
/// its `min`-th character: `{"start":N,"items":[{"write":"km","label":"km - kilometre"}]}`, `start`
/// in characters (Unicode scalar values), `items` empty when there is nothing to offer. Units come
/// once each, by their symbol (`kilometer`, `kilometres`… are `km`).
pub fn suggest(before: &str, min: usize) -> String {
    use crate::suggest::Kind;
    if crate::check_length(before).is_err() {
        return NO_SUGGESTIONS.to_string();
    }
    let found = crate::suggest::suggest(Config::shared(), before, before.chars().count(), min);
    let mut items: Vec<(String, String)> = Vec::new();
    if let Some(s) = &found {
        for (i, name) in s.names.iter().enumerate() {
            let label = s.labels.get(i).cloned().unwrap_or_else(|| name.clone());
            let item = match (s.kind, crate::convert::find_unit(Config::shared(), name)) {
                // `megabyte (10⁶ bytes)`: the name, not the note.
                (Kind::Units, Some((_, unit))) => (unit.symbol.to_string(), format!("{} - {}", unit.symbol, unit.name.split(" (").next().unwrap_or(unit.name))),
                _ => (name.clone(), label),
            };
            if !items.iter().any(|(write, _)| *write == item.0) {
                items.push(item);
            }
        }
    }
    let list: Vec<String> = items.iter().map(|(write, label)| Object::new().str("write", write).str("label", label).finish()).collect();
    Object::new().num("start", found.as_ref().map_or(0, |s| s.start)).list("items", &list).finish()
}

/// `suggest`, for the C interface: `before` is the text up to the cursor.
///
/// # Safety
/// `before` is a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn aequalis_suggest(before: *const c_char, min: u32) -> *mut c_char {
    match unsafe { request_text(before) } {
        Err(()) => give(NO_SUGGESTIONS.to_string()),
        Ok(None) => error("no text"),
        Ok(Some(before)) => give(suggest(before, min as usize)),
    }
}

/// Every currency, the most used first: `{"currencies":[{"code","name","symbols":[…],"countries":[…],
/// "aliases":[…]}]}`; `countries`: the names of those that use it (`Italy`), `aliases`: their other
/// names (`USA`), so a search finds a currency by its country, as requests do (`15 Italy UAE`).
#[no_mangle]
pub extern "C" fn aequalis_currencies() -> *mut c_char {
    give(currencies())
}

pub fn currencies() -> String {
    let list: Vec<String> = crate::currency::all()
        .iter()
        .map(|c| {
            let users = crate::currency::users(&c.code);
            let names: Vec<String> = users.iter().map(|k| k.name.clone()).collect();
            let aliases: Vec<String> = users.iter().flat_map(|k| k.aliases.iter().cloned()).collect();
            Object::new().str("code", &c.code).str("name", &c.name).strs("symbols", &c.symbols).strs("countries", &names).strs("aliases", &aliases).finish()
        })
        .collect();
    Object::new().list("currencies", &list).finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(f: impl FnOnce() -> *mut c_char) -> String {
        let p = f();
        let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_string();
        unsafe { aequalis_free(p) };
        s
    }

    #[test]
    fn requests_answer_as_the_command_line() {
        let request = CString::new("10 km mi").unwrap();
        let reply = call(|| unsafe { aequalis_eval(request.as_ptr()) });
        assert!(reply.starts_with(r#"{"ok":true,"result":"6.2137 mi""#), "{reply}");
        // An ambiguous request: its choices, as `--json`.
        let reply = eval("12/04/2026 unix");
        assert!(reply.contains(r#""ambiguous":{"kind":"date""#), "{reply}");
        assert!(eval("10 foo bar").starts_with(r#"{"ok":false,"exit":"#));
        // The grouping of a calculation, beside its result.
        assert!(eval("12+4/2").contains(r#","grouping":"(12 + (4 / 2))""#), "{}", eval("12+4/2"));
        // `aequalis_view`: what to show, a word being typed hiding what is no result.
        let typed = CString::new("10 km").unwrap();
        assert!(call(|| unsafe { aequalis_view(typed.as_ptr(), false) }).contains(r#""show":"none""#));
        assert!(call(|| unsafe { aequalis_view(typed.as_ptr(), true) }).contains(r#""show":"hint""#));
        assert!(started().contains(r#""max_input":512"#));
        assert!(!eval("10 km mi").contains("grouping"));
        let about = json::parse(&about()).unwrap();
        assert_eq!(about.get("name").and_then(|v| v.str()), Some("Aequalis"));
        assert!(about.get("facts").unwrap().array().iter().all(|f| f.array()[0].str() != Some("Memory")));
        // No command-line options, nor their messages: `--version` is no request, and an unknown
        // unit says so without `see aequalis --list`.
        assert!(!eval("--version").contains("aequalis "));
        assert!(eval("10 foo m").contains(r#""error":"unknown unit `foo`""#), "{}", eval("10 foo m"));
        let parsed = json::parse(&eval("12+4")).unwrap();
        assert_eq!(parsed.get("result").and_then(|v| v.str()), Some("16"));
        // A null request: an error, not a crash.
        assert!(call(|| unsafe { aequalis_eval(std::ptr::null()) }).contains(r#""ok":false"#));
    }

    /// A request past 512 characters: refused before anything reads it, with no grouping; no
    /// suggestions; through the C interface, more than 2,048 bytes are not even read.
    #[test]
    fn requests_too_long() {
        let reply = |r: &str| json::parse(&eval(r)).unwrap();
        let long = vec!["1"; 300].join("+");
        let refused = reply(&long);
        assert_eq!(refused.get("error").and_then(|v| v.str()), Some("the request is too long: at most 512 characters"));
        assert!(refused.get("grouping").is_none(), "{}", eval(&long));
        assert_eq!(reply(&format!("1{}+1", " ".repeat(2000))).get("ok").and_then(|v| v.boolean()), Some(false), "spaces count");
        assert_eq!(reply(&format!("1+1{}", " ".repeat(509))).get("result").and_then(|v| v.str()), Some("2"), "512 characters");
        assert_eq!(suggest(&format!("{}km", " ".repeat(511)), 0), NO_SUGGESTIONS);
        assert_ne!(suggest(&format!("10 {}kil", " ".repeat(500)), 3), NO_SUGGESTIONS, "506 characters: suggestions");
        let c = |s: &str| CString::new(s).unwrap();
        let huge = c(&"Rome ".repeat(100_000));
        assert_eq!(call(|| unsafe { aequalis_eval(huge.as_ptr()) }).split(",\"peak_kb\"").next(), too_long().split(",\"peak_kb\"").next());
        assert_eq!(call(|| unsafe { aequalis_suggest(huge.as_ptr(), 3) }), NO_SUGGESTIONS);
        // 512 emoji are 2,048 bytes: read, and answered (as a request of their own, not too long).
        let emoji = c(&"🙂".repeat(512));
        assert!(!call(|| unsafe { aequalis_eval(emoji.as_ptr()) }).contains("too long"));
        let emoji = c(&"🙂".repeat(513));
        assert!(call(|| unsafe { aequalis_eval(emoji.as_ptr()) }).contains("too long"));
    }

    /// The app's suggestions: from the third letter (a value's unit from its first), units once
    /// each by their symbol.
    #[test]
    fn suggestions_for_the_app() {
        assert_eq!(suggest("Ro", 3), r#"{"start":0,"items":[]}"#, "two letters of a place: nothing yet");
        assert!(suggest("10 gb", 3).starts_with(r#"{"start":3,"items":[{"write":"GB","label":"GB - gigabyte"}"#), "{}", suggest("10 gb", 3));
        let units = json::parse(&suggest("10 kil", 3)).unwrap();
        assert_eq!(units.get("start").and_then(|v| v.number()), Some(3.0));
        let items = units.get("items").unwrap().array();
        let write: Vec<&str> = items.iter().filter_map(|i| i.get("write")?.str()).collect();
        assert_eq!(&write[..2], ["km", "kg"]);
        assert_eq!(items[0].get("label").and_then(|v| v.str()), Some("km - kilometre"));
        assert_eq!(write.iter().filter(|w| **w == "km").count(), 1, "kilometer, kilometres…: once");
        assert!(suggest("12 dol", 3).contains(r#"{"write":"USD","label":"USD - US dollar"}"#));
        assert!(suggest("Rome to Tok", 3).contains(r#""write":"Tokyo""#));
    }

    /// A popup line's measures: each part's number and unit, shown and plain, without what follows.
    #[test]
    fn trivia_measures_of_a_line() {
        assert_eq!(
            trivia_measures(
                "25,800,920,358 km - 172.4685 au - 2.7272e-3 ly (estimated, not synced yet)",
                "25800920358 km - 172.4685 au - 2.7272e-3 ly (estimated, not synced yet)"
            ),
            [
                ("25,800,920,358 km".into(), "25800920358 km".into()),
                ("172.4685 au".into(), "172.4685 au".into()),
                ("2.7272e-3 ly".into(), "2.7272e-3 ly".into())
            ]
        );
        assert_eq!(
            trivia_measures("16.88 km/s - 60,768 km/h - 37,759 mph, away from the Sun", "16.88 km/s - 60768 km/h - 37759 mph, away from the Sun")[2],
            ("37,759 mph".to_string(), "37759 mph".to_string())
        );
        assert!(trivia_measures("about 1.02 km/s (3,680 km/h) around the Earth", "about 1.02 km/s (3,680 km/h) around the Earth").is_empty());
        let doc = json::parse(&trivia()).unwrap();
        let space = doc.get("sections").unwrap().array().iter().find(|s| s.get("name").and_then(|v| v.str()) == Some("Space")).unwrap();
        let voyager = &space.get("entries").unwrap().array()[0];
        assert_eq!(voyager.get("symbol").and_then(|v| v.str()), Some("☄"));
    }

    #[test]
    fn the_device_from_the_app() {
        let init = parse_init(r#"{"dir":"/var/app","zone":"Europe/Rome","decimal":",","thousands":".","grouping":[3],"imperial":false}"#).unwrap();
        assert_eq!(init.dir, "/var/app");
        assert_eq!(init.zone.as_deref(), Some("Europe/Rome"));
        assert_eq!(init.format.as_ref().map(|f| (f.decimal.as_str(), f.thousands.as_str(), f.grouping.clone())), Some((",", ".", vec![3])));
        assert_eq!(init.imperial, Some(false));
        let bare = parse_init(r#"{"dir":"/x"}"#).unwrap();
        assert_eq!((bare.zone, bare.format, bare.imperial), (None, None, None));
        assert!(parse_init("{}").unwrap_err().contains("`dir` is required"));
        assert!(parse_init("not json").is_err());
    }

    #[test]
    fn currencies_with_their_countries() {
        let doc = json::parse(&currencies()).unwrap();
        let list = doc.get("currencies").unwrap().array();
        let of = |code: &str| list.iter().find(|c| c.get("code").and_then(|v| v.str()) == Some(code)).unwrap();
        let strings = |c: &json::Value, key: &str| c.get(key).unwrap().strings();
        assert!(strings(of("EUR"), "countries").contains(&"Italy".to_string()));
        assert!(strings(of("AED"), "countries").contains(&"United Arab Emirates".to_string()));
        assert!(strings(of("USD"), "aliases").iter().any(|a| a.eq_ignore_ascii_case("usa")), "{:?}", strings(of("USD"), "aliases"));
    }

    #[test]
    fn memory_where_counted() {
        // Off Android the system's allocator counts nothing: an empty object.
        assert_eq!(memory(), "{}");
    }

    #[test]
    fn lists_for_the_app() {
        for (name, text) in [("trivia", trivia()), ("measures", measures()), ("currencies", currencies())] {
            assert!(json::parse(&text).is_ok(), "{name}: {text}");
        }
        let doc = json::parse(&trivia()).unwrap();
        let sections = doc.get("sections").unwrap().array();
        assert_eq!(sections[0].get("name").and_then(|v| v.str()), Some("Earth"));
        let first = &sections[0].get("entries").unwrap().array()[0];
        assert_eq!(first.get("written").and_then(|v| v.str()), Some("London to Paris km"), "a tip: its example");
        let math = sections.iter().find(|s| s.get("name").and_then(|v| v.str()) == Some("Math")).unwrap();
        let pi = &math.get("entries").unwrap().array()[0];
        assert_eq!(pi.get("written").and_then(|v| v.str()), Some("3.1415926536 "));
        assert!(!pi.get("popup").unwrap().array().is_empty());
        let measures = json::parse(&measures()).unwrap();
        let classes = measures.get("classes").unwrap().array();
        assert_eq!(classes[0].get("name").and_then(|v| v.str()), Some(crate::config::TIME));
        assert!(classes.iter().any(|c| c.get("name").and_then(|v| v.str()) == Some("Temperature")));
        assert!(currencies().contains(r#"{"code":"EUR","name":"euro","symbols":["€"],"countries":["#));
    }

    #[test]
    fn settings_for_the_app() {
        let settings = call(|| aequalis_settings());
        assert!(settings.contains(r#""key":"offline""#), "{settings}");
        let bad = CString::new("no.such.key").unwrap();
        assert!(call(|| unsafe { aequalis_set(bad.as_ptr(), std::ptr::null()) }).contains(r#""ok":false"#));
    }
}
