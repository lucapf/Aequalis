//! The C interface for apps that embed tally (the Android app; feature `ffi`, built as a shared
//! library). Strings in and out are UTF-8 and NUL-terminated; every string returned is JSON and is
//! given back with `tally_free`. `include/tally.h` declares them.
//!
//! - `tally_init(config)` once, first: the data folder, the time zone, the number format and the
//!   measurement system, which the app knows (an app gets no `HOME`, `TZ` or `LANG` that say
//!   them).
//! - `tally_set_fetcher(fetch)`: the app's downloader (the app can't start `curl`).
//! - `tally_eval(request)`: the reply of `tally --json <request>` (results, errors, choices).
//! - `tally_settings()`, `tally_set(key, value)`, `tally_sync(auto)`, `tally_trivia()`,
//!   `tally_measures()`, `tally_currencies()`.

use crate::config::{Config, System};
use crate::json::{self, Object};
use std::ffi::{c_char, CStr, CString};
use std::path::Path;

fn config() -> &'static Config {
    Config::shared()
}

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

/// A string for the app, given back with `tally_free`.
fn give(s: String) -> *mut c_char {
    CString::new(s.replace('\0', "")).map_or(std::ptr::null_mut(), CString::into_raw)
}

fn error(message: &str) -> *mut c_char {
    give(Object::new().num("ok", false).str("error", message).finish())
}

/// Frees a string returned by tally.
///
/// # Safety
/// `s` was returned by a `tally_*` function and is freed once.
#[no_mangle]
pub unsafe extern "C" fn tally_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// What the app says about the device, at start.
#[derive(Debug, Clone, PartialEq)]
pub struct Init {
    /// The folder where tally keeps its data (`<dir>/tally/…`): the app's Application Support.
    pub dir: String,
    /// The time zone (`Europe/Rome`), and where the zone files are (the system's or a copy).
    pub zone: Option<String>,
    pub zoneinfo: Option<String>,
    pub format: Option<crate::numbers::NumberFormat>,
    pub imperial: Option<bool>,
}

/// `{"dir": "…", "zone": "Europe/Rome", "zoneinfo": "…", "decimal": ",", "thousands": ".",
/// "grouping": [3], "imperial": false}` (only `dir` is required).
pub fn parse_init(text: &str) -> Result<Init, String> {
    let doc = json::parse(text).map_err(|e| format!("tally_init: {e}"))?;
    let s = |key: &str| doc.get(key).and_then(|v| v.str()).filter(|v| !v.is_empty()).map(str::to_string);
    let dir = s("dir").ok_or("tally_init: `dir` is required")?;
    let format = match (s("decimal"), doc.get("thousands").and_then(|v| v.str())) {
        (Some(decimal), Some(thousands)) => Some(crate::numbers::NumberFormat {
            decimal,
            thousands: thousands.to_string(),
            grouping: doc.get("grouping").map(|g| g.array().iter().filter_map(json::Value::number).map(|n| n as u8).collect()).unwrap_or_else(|| vec![3]),
        }),
        _ => None,
    };
    Ok(Init { dir, zone: s("zone"), zoneinfo: s("zoneinfo"), format, imperial: doc.get("imperial").and_then(|v| v.boolean()) })
}

/// Starts tally for the app: `config` as in `parse_init`. Call once, before anything else.
/// Returns `{"ok":true}` or `{"ok":false,"error":…}`.
///
/// # Safety
/// `config` is a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn tally_init(config_json: *const c_char) -> *mut c_char {
    let Some(text) = (unsafe { text(config_json) }) else { return error("tally_init: no configuration") };
    let init = match parse_init(text) {
        Ok(i) => i,
        Err(e) => return error(&e),
    };
    start(&init);
    give(Object::new().num("ok", true).finish())
}

/// Gives tally what the app knows, once, before anything else: set directly, not through
/// environment variables (`setenv` races with the app's other threads).
pub fn start(init: &Init) {
    crate::files::set_dir(Path::new(&init.dir));
    if let Some(zone) = &init.zone {
        crate::currency::set_local_zone(zone);
    }
    if let Some(database) = &init.zoneinfo {
        crate::time::tz::set_zoneinfo(database);
    }
    if let Some(format) = &init.format {
        crate::numbers::set_device(format.clone());
    }
    if let Some(imperial) = init.imperial {
        crate::locale::set_device_system(if imperial { System::Imperial } else { System::Metric });
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
        let path = std::env::temp_dir().join(format!("tally-fetch-{}-{n}", std::process::id()));
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
pub extern "C" fn tally_set_fetcher(fetch: Fetch) {
    crate::http::register(Box::new(AppFetcher(fetch)));
}

/// The reply to a request, as `tally --json <words…>`: `{"ok":true,"result":…}`, an error, or
/// the choices of an ambiguous request; the notices too. Only the request: no command-line
/// options (`-v`, `--precision`), whose messages would make no sense in an app.
///
/// # Safety
/// `request` is a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn tally_eval(request: *const c_char) -> *mut c_char {
    let Some(request) = (unsafe { text(request) }) else { return error("no request") };
    give(eval(request))
}

pub fn eval(request: &str) -> String {
    if let Some(e) = crate::settings::error() {
        return Object::new().num("ok", false).str("error", &format!("{e} (fix it in Settings)")).finish();
    }
    // The request itself, not the command line's options: an app has no `--precision` or `-v`, and
    // a message about them would make no sense there.
    let tokens: Vec<&str> = request.split_whitespace().collect();
    let options = crate::Options { positional: Vec::new(), precision: crate::settings::current().precision(), silent: false, verbose: false };
    let result = crate::convert_request(config(), &options, &tokens);
    json::result(&result, &crate::notify::current())
}

/// Every setting: key, value, default, kind, options, what it does (`tally --settings --json`).
#[no_mangle]
pub extern "C" fn tally_settings() -> *mut c_char {
    give(settings())
}

pub fn settings() -> String {
    let settings = crate::settings::path().map(|p| crate::settings::load_from(&p)).unwrap_or(Ok(Default::default()));
    match settings {
        Ok(s) => crate::settings::json(&s),
        Err(e) => Object::new().num("ok", false).str("error", &e).finish(),
    }
}

/// Changes a setting (`value` null: back to its default) and saves it: `{"ok":true,"key":…,
/// "value":…}` or the error.
///
/// # Safety
/// `key` is a NUL-terminated string; `value` too, or null.
#[no_mangle]
pub unsafe extern "C" fn tally_set(key: *const c_char, value: *const c_char) -> *mut c_char {
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

/// Downloads the online data, in this thread (call it off the main thread): everything, or with
/// `auto` what the refresh settings say (at launch, in a background refresh). The reply of
/// `tally --sync --json`.
#[no_mangle]
pub extern "C" fn tally_sync(auto: bool) -> *mut c_char {
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
pub extern "C" fn tally_trivia() -> *mut c_char {
    give(trivia())
}

pub fn trivia() -> String {
    use crate::trivia::{self, Source};
    let config = config();
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

/// The unit classes for the Measures: `{"classes":[{"name","key","systems":[…,…],"units":
/// [{"symbol","name","system":"metric"|"imperial"|"both"}]}]}`, in the TUI's order.
#[no_mangle]
pub extern "C" fn tally_measures() -> *mut c_char {
    give(measures())
}

pub fn measures() -> String {
    let classes: Vec<String> = config()
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
            Object::new().str("name", c.name).str("key", &c.key.to_string()).strs("systems", &systems).list("units", &units).finish()
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
    let found = crate::suggest::suggest(config(), before, before.chars().count(), min);
    let mut items: Vec<(String, String)> = Vec::new();
    if let Some(s) = &found {
        for (i, name) in s.names.iter().enumerate() {
            let label = s.labels.get(i).cloned().unwrap_or_else(|| name.clone());
            let item = match (s.kind, crate::convert::find_unit(config(), name)) {
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
pub unsafe extern "C" fn tally_suggest(before: *const c_char, min: u32) -> *mut c_char {
    let Some(before) = (unsafe { text(before) }) else { return error("no text") };
    give(suggest(before, min as usize))
}

/// Every currency, the most used first: `{"currencies":[{"code","name","symbols":[…]}]}`.
#[no_mangle]
pub extern "C" fn tally_currencies() -> *mut c_char {
    give(currencies())
}

pub fn currencies() -> String {
    let list: Vec<String> =
        crate::currency::all().iter().map(|c| Object::new().str("code", &c.code).str("name", &c.name).strs("symbols", &c.symbols).finish()).collect();
    Object::new().list("currencies", &list).finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(f: impl FnOnce() -> *mut c_char) -> String {
        let p = f();
        let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_string();
        unsafe { tally_free(p) };
        s
    }

    #[test]
    fn requests_answer_as_the_command_line() {
        let request = CString::new("10 km mi").unwrap();
        let reply = call(|| unsafe { tally_eval(request.as_ptr()) });
        assert!(reply.starts_with(r#"{"ok":true,"result":"6.2137 mi""#), "{reply}");
        // An ambiguous request: its choices, as `--json`.
        let reply = eval("12/04/2026 unix");
        assert!(reply.contains(r#""ambiguous":{"kind":"date""#), "{reply}");
        assert!(eval("10 foo bar").starts_with(r#"{"ok":false,"exit":"#));
        // No command-line options, nor their messages: `--version` is no request, and an unknown
        // unit says so without `see tally --list`.
        assert!(!eval("--version").contains("tally "));
        assert!(eval("10 foo m").contains(r#""error":"unknown unit `foo`""#), "{}", eval("10 foo m"));
        let parsed = json::parse(&eval("12+4")).unwrap();
        assert_eq!(parsed.get("result").and_then(|v| v.str()), Some("16"));
        // A null request: an error, not a crash.
        assert!(call(|| unsafe { tally_eval(std::ptr::null()) }).contains(r#""ok":false"#));
    }

    /// The app's suggestions: from the third letter, units once each by their symbol.
    #[test]
    fn suggestions_for_the_app() {
        assert_eq!(suggest("10 ki", 3), r#"{"start":0,"items":[]}"#, "two letters: nothing yet");
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
        assert!(classes.iter().any(|c| c.get("key").and_then(|v| v.str()) == Some("K")));
        assert!(currencies().contains(r#"{"code":"EUR","name":"euro","symbols":["€"]}"#));
    }

    #[test]
    fn settings_for_the_app() {
        let settings = call(|| tally_settings());
        assert!(settings.contains(r#""key":"offline""#), "{settings}");
        let bad = CString::new("no.such.key").unwrap();
        assert!(call(|| unsafe { tally_set(bad.as_ptr(), std::ptr::null()) }).contains(r#""ok":false"#));
    }
}
