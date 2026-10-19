//! `--json`: one JSON object per request, for programs such as the Omarchy bar widget.
//!
//! ```text
//! {"ok":true,"result":"6.2137 mi","notes":[],"warnings":[],"peak_kb":1024}
//! {"ok":false,"exit":1,"error":"unknown unit `foo` (see `aequalis --list`)","peak_kb":1024}
//! {"ok":false,"exit":2,"error":"…","ambiguous":{"text":"12/04/2026","choices":[{"pattern":"MM/DD/YYYY","date":"2026-12-04","request":"12/04/2026 MM/DD/YYYY unix"},…]},"peak_kb":1024}
//! ```
//! Nothing is asked and nothing goes to stderr: an ambiguous date is an error carrying its choices.
//! `peak_kb` is the process's peak memory (resident set) so far, in KiB: what the request took.

use crate::{CliError, Output};

/// A JSON string literal.
pub fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn strings(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| string(s)).collect::<Vec<_>>().join(","))
}

/// A JSON object, written member by member (escaping the keys and strings):
/// `Object::new().str("name", "Rome").num("count", 3).finish()` → `{"name":"Rome","count":3}`.
pub struct Object(String);

impl Default for Object {
    fn default() -> Object {
        Object::new()
    }
}

impl Object {
    pub fn new() -> Object {
        Object(String::from("{"))
    }

    fn key(mut self, key: &str) -> Object {
        if self.0.len() > 1 {
            self.0.push(',');
        }
        self.0.push_str(&string(key));
        self.0.push(':');
        self
    }

    pub fn str(self, key: &str, value: &str) -> Object {
        let mut o = self.key(key);
        o.0.push_str(&string(value));
        o
    }

    /// A number or a boolean, as Rust writes it (`1791877958`, `0.8945`, `true`).
    pub fn num(self, key: &str, value: impl std::fmt::Display) -> Object {
        let mut o = self.key(key);
        o.0.push_str(&value.to_string());
        o
    }

    /// A number, or `null`.
    pub fn opt(self, key: &str, value: Option<i64>) -> Object {
        match value {
            Some(v) => self.num(key, v),
            None => self.raw(key, "null"),
        }
    }

    pub fn strs(self, key: &str, values: &[String]) -> Object {
        self.raw(key, &strings(values))
    }

    /// An array of values already written as JSON (objects: `Object::finish`).
    pub fn list(self, key: &str, items: &[String]) -> Object {
        self.raw(key, &format!("[{}]", items.join(",")))
    }

    /// A value already written as JSON (an object, an array).
    pub fn raw(self, key: &str, json: &str) -> Object {
        let mut o = self.key(key);
        o.0.push_str(json);
        o
    }

    pub fn finish(mut self) -> String {
        self.0.push('}');
        self.0
    }
}

/// The peak resident memory of this process so far, in KiB (0 if unknown). On Linux `VmHWM` from
/// `/proc/self/status`: `getrusage`'s `ru_maxrss` survives `exec`, so a process started by a large
/// one (the Omarchy shell, ~360 MiB) would report its parent's peak.
pub fn peak_kb() -> u64 {
    if cfg!(target_os = "linux") {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            let kb = status.lines().find_map(|l| l.strip_prefix("VmHWM:")?.trim().strip_suffix("kB")?.trim().parse().ok());
            if let Some(kb) = kb {
                return kb;
            }
        }
    }
    maxrss_kb()
}

/// The resident memory of this process now, in KiB (Linux: `VmRSS`); `None` where it is not
/// known (then the peak is shown instead).
#[cfg_attr(not(feature = "tui"), allow(dead_code))]
#[cfg(not(windows))]
pub fn current_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|l| l.strip_prefix("VmRSS:")?.trim().strip_suffix("kB")?.trim().parse().ok())
}

/// The working set now, in KiB.
#[cfg_attr(not(feature = "tui"), allow(dead_code))]
#[cfg(windows)]
pub fn current_kb() -> Option<u64> {
    crate::windows::memory_kb().map(|(_, now)| now)
}

/// KiB for people: `900 KiB`, `3.4 MiB`.
#[cfg_attr(not(feature = "tui"), allow(dead_code))]
pub fn size(kb: u64) -> String {
    match kb {
        0..=1023 => format!("{kb} KiB"),
        _ => format!("{:.1} MiB", kb as f64 / 1024.0),
    }
}

/// The peak working set, in KiB.
#[cfg(windows)]
fn maxrss_kb() -> u64 {
    crate::windows::memory_kb().map_or(0, |(peak, _)| peak)
}

/// `getrusage`'s peak resident memory, in KiB.
#[cfg(not(windows))]
fn maxrss_kb() -> u64 {
    // `struct rusage`: two `struct timeval` (16 bytes each on 64-bit Linux and macOS), then
    // `ru_maxrss` and 13 more longs.
    #[repr(C)]
    struct Rusage {
        times: [i64; 4],
        maxrss: i64,
        rest: [i64; 13],
    }
    const _: () = assert!(std::mem::size_of::<Rusage>() == 144);
    extern "C" {
        fn getrusage(who: i32, usage: *mut Rusage) -> i32;
    }
    const RUSAGE_SELF: i32 = 0;
    let mut usage = Rusage { times: [0; 4], maxrss: 0, rest: [0; 13] };
    // SAFETY: `usage` is a writable `struct rusage`.
    if unsafe { getrusage(RUSAGE_SELF, &mut usage) } != 0 {
        return 0;
    }
    let maxrss = u64::try_from(usage.maxrss).unwrap_or(0);
    // Linux reports KiB, macOS bytes.
    if cfg!(target_os = "macos") {
        maxrss / 1024
    } else {
        maxrss
    }
}

/// One request's outcome as a JSON object, on one line, with the peak memory so far.
pub fn result(result: &Result<Output, CliError>, notices: &[String]) -> String {
    with(result, notices, "")
}

/// `result`'s object with `extra` members (each starting with a comma) before the notices.
fn with(result: &Result<Output, CliError>, notices: &[String], extra: &str) -> String {
    let object = outcome(result);
    let notices = match notices.is_empty() {
        true => String::new(),
        false => format!(",\"notices\":[{}]", notices.iter().map(|n| string(n)).collect::<Vec<_>>().join(",")),
    };
    format!("{}{extra}{notices},\"peak_kb\":{}}}", &object[..object.len() - 1], peak_kb())
}

fn outcome(result: &Result<Output, CliError>) -> String {
    match result {
        Ok(out) => {
            let reply = Object::new().num("ok", true).str("result", &out.stdout).strs("notes", &out.notes).strs("warnings", &out.warnings);
            // A currency result: how old its rates are (`2d`).
            match &out.rates_age {
                Some(age) => reply.str("rates_age", age).finish(),
                None => reply.finish(),
            }
        }
        Err(err) => {
            let ambiguous = match err {
                CliError::Ambiguous(a) => {
                    let choices: Vec<String> =
                        a.choices.iter().map(|c| Object::new().str("pattern", &c.pattern).str("date", &c.date).str("request", &c.request).finish()).collect();
                    let kind = match a.kind {
                        crate::time::AmbiguityKind::Date => "date",
                        crate::time::AmbiguityKind::City => "city",
                        crate::time::AmbiguityKind::Country => "country",
                        crate::time::AmbiguityKind::Currency => "currency",
                        crate::time::AmbiguityKind::Place => "place",
                    };
                    // Misspelt places: what was misspelt (`jappan` is not a country), for the question.
                    let note = if a.note.is_empty() { String::new() } else { format!(r#","note":{}"#, string(&a.note)) };
                    format!(r#","ambiguous":{{"kind":"{kind}","text":{}{note},"choices":[{}]}}"#, string(&a.text), choices.join(","))
                }
                // Not finished yet: a hint, not an error.
                CliError::Incomplete(_) => r#","incomplete":true"#.to_string(),
                _ => String::new(),
            };
            format!(r#"{{"ok":false,"exit":{},"error":{}{ambiguous}}}"#, err.exit_code(), string(&err.to_string()))
        }
    }
}

// ---------------------------------------------------------------- a reply for a UI

/// A request's reply for a UI (the Android app, the Omarchy widget): `result`'s object for the
/// request as typed, its grouping if it is a calculation, and what to show, decided here so that
/// no UI reads the request or the messages itself:
///
/// ```text
/// "show":"result"|"hint"|"question"|"error"|"none", "message":"…" (hint, question, error),
/// "value":"7.4565" (the number, for Copy in the app), "copy":"7.4565 mi" (the unit kept, for the
/// widget), "rates":"rates 2d old", "details":[notes, warnings],
/// "default":"<request>" (Enter's choice), "choices":[{"label","detail","request"}],
/// "class":"Length" (the Measures class of the request's unit)
/// ```
/// `submitted`: Enter was pressed on it; until then, and while a word is being typed, a reply that
/// is not a result shows nothing (`crate::typing`). Only the request: no command-line options.
pub fn view(request: &str, submitted: bool) -> String {
    let config = crate::config::Config::shared();
    // Too long: refused unread, the settings and the grouping included.
    if let Err(e) = crate::check_length(request) {
        return refused(e);
    }
    let (result, notices, grouping) = match crate::settings::error() {
        // The settings wrong: nothing else reads the request.
        Some(e) => (Err(CliError::Usage(format!("{e} (fix it in Settings)"))), Vec::new(), None),
        None => {
            let tokens: Vec<&str> = request.split_whitespace().collect();
            let options = crate::Options { positional: Vec::new(), precision: crate::settings::current().precision(), silent: false, verbose: false };
            (crate::convert_request(config, &options, &tokens), crate::notify::current(), crate::grouping(request))
        }
    };
    let mut extra = shown(request, submitted, &result);
    // How a calculation is grouped (`(12 + (4 / 2))`), beside its result, as in the TUI.
    if let Some(g) = grouping.filter(|_| result.is_ok()) {
        extra.push_str(&format!(",\"grouping\":{}", string(&g)));
    }
    if let Some(class) = unit_class(config, request) {
        extra.push_str(&format!(",\"class\":{}", string(class)));
    }
    with(&result, &notices, &extra)
}

/// `view`'s reply to a request refused before it is read (too long): an error, shown at once.
pub fn refused(err: CliError) -> String {
    let result = Err(err);
    with(&result, &[], &shown("", true, &result))
}

/// The members of `view` that say what to show, each starting with a comma.
fn shown(request: &str, submitted: bool, result: &Result<Output, CliError>) -> String {
    let o = match result {
        Ok(out) => {
            let details: Vec<String> = out.warnings.iter().chain(&out.notes).cloned().collect();
            let o = Object::new().str("show", "result").str("value", &value_of(&out.stdout)).str("copy", &copy_of(&out.stdout)).strs("details", &details);
            match &out.rates_age {
                Some(age) => o.str("rates", &format!("rates {age} old")),
                None => o,
            }
        }
        Err(_) if crate::typing(request, submitted) => Object::new().str("show", "none"),
        Err(CliError::Incomplete(hint)) => Object::new().str("show", "hint").str("message", hint),
        Err(CliError::Ambiguous(a)) => {
            let choices: Vec<String> = a
                .choices
                .iter()
                .map(|c| {
                    // The button's text, and more about it (a tooltip): a place's population and map.
                    let (label, detail) = match a.kind {
                        crate::time::AmbiguityKind::Place => (c.pattern.clone(), c.date.as_str()),
                        _ => (format!("{}   {}", c.pattern, c.date), ""),
                    };
                    Object::new().str("label", &label).str("detail", detail).str("request", &c.request).finish()
                })
                .collect();
            let o = Object::new().str("show", "question").str("message", &question(a)).list("choices", &choices);
            match a.choices.first() {
                Some(c) => o.str("default", &c.request),
                None => o,
            }
        }
        Err(e) => Object::new().str("show", "error").str("message", &e.to_string()),
    };
    let o = o.finish();
    format!(",{}", &o[1..o.len() - 1])
}

/// The question an ambiguous request asks, over its choices.
fn question(a: &crate::time::Ambiguity) -> String {
    use crate::time::AmbiguityKind;
    match a.kind {
        AmbiguityKind::City if a.note.is_empty() => format!("`{}` is not a city: did you mean…", a.text),
        AmbiguityKind::City => format!("{}: did you mean…", a.note),
        AmbiguityKind::Country => format!("`{}` has several time zones: which one?", a.text),
        AmbiguityKind::Currency | AmbiguityKind::Place => format!("{}: which one?", a.note),
        AmbiguityKind::Date => format!("Which date is {}?", a.text),
    }
}

/// What Copy and Share give of a result: its value, without the unit (`7.4565 mi` → `7.4565`,
/// `8.94 EUR` → `8.94`) or the pattern's name (`2026/12/11 YYYY/MM/DD` → `2026/12/11`); anything
/// else (a date for people, a time in a city) as it is shown.
pub fn value_of(result: &str) -> String {
    let result = result.trim();
    let parts: Vec<&str> = result.split(' ').collect();
    let number = parts.len() == 2 && parts[0].starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '.');
    if number { parts[0] } else { result }.to_string()
}

/// What the Omarchy widget copies of a result: the unit kept (`6.2137 mi`), a format's name left out
/// (`2026/11/22 YYYY/MM/DD` → `2026/11/22`), and of a city's time (`… = 2026-10-11T16:00+02:00
/// Paris`) the ISO time.
pub fn copy_of(result: &str) -> String {
    let clock = |s: &str| {
        s.as_bytes().windows(6).any(|w| w[0] == b'T' && w[1..3].iter().all(u8::is_ascii_digit) && w[3] == b':' && w[4..6].iter().all(u8::is_ascii_digit))
    };
    let side = match result.rfind(" = ") {
        Some(i) if clock(result) => &result[i + 3..],
        _ => result,
    };
    // `2026-10-11T16:00+02:00 Paris`: the ISO time.
    if let Some((first, _)) = side.split_once(' ') {
        let b = first.as_bytes();
        let date = b.len() > 11 && b[..4].iter().all(u8::is_ascii_digit) && b[4] == b'-' && b[7] == b'-' && b[10] == b'T';
        if date && b[5..7].iter().chain(&b[8..10]).all(u8::is_ascii_digit) {
            return first.to_string();
        }
    }
    match side.rsplit_once(' ') {
        Some((value, last)) if is_format(last) => value.to_string(),
        _ => side.to_string(),
    }
}

/// A format's name: `iso`, `iso-…`, `unix`, `unix-…`, `local`, or a pattern (`YYYY/MM/DD`, with
/// `-12h`, `-24h`, `-12h-tz`…).
fn is_format(word: &str) -> bool {
    let named = |base: &str| {
        word == base || word.strip_prefix(base).and_then(|r| r.strip_prefix('-')).is_some_and(|r| !r.is_empty() && r.bytes().all(|b| b.is_ascii_lowercase()))
    };
    if named("iso") || named("unix") || word == "local" {
        return true;
    }
    let (pattern, clock) = word.split_once('-').unwrap_or((word, ""));
    let parts: Vec<&str> = pattern.split('/').collect();
    let ymd = parts.len() == 3 && parts.iter().all(|p| (2..=4).contains(&p.len()) && p.bytes().all(|b| b"YMD".contains(&b)));
    ymd && ["", "12h", "24h", "12h-tz", "24h-tz"].contains(&clock)
}

/// The Measures class of the request's unit, the word after its value (`10 cm mi` → Length, `1 GB`
/// → Data); none without a value or a unit (`12 EUR`, `Rome Paris`).
pub fn unit_class(config: &crate::config::Config, request: &str) -> Option<&'static str> {
    let mut words = request.split_whitespace();
    let value = words.next()?.starts_with(|c: char| c.is_ascii_digit() || ".-+(".contains(c));
    let unit = words.next().filter(|_| value)?;
    if let Some((category, _)) = crate::convert::find_unit(config, unit) {
        return Some(category.name);
    }
    // A case no unit has (`gb`, between `GB` and `Gb`): any case.
    config.categories.iter().find(|c| c.units.iter().any(|u| u.symbol.eq_ignore_ascii_case(unit))).map(|c| c.name)
}

// ---------------------------------------------------------------- reading

/// A parsed JSON value: the data files compiled into the binary (`src/data/*.json`).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    /// The members in their order.
    Object(Vec<(String, Value)>),
}

impl Value {
    /// A member of an object.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn boolean(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn array(&self) -> &[Value] {
        match self {
            Value::Array(items) => items,
            _ => &[],
        }
    }

    /// The strings of an array (other items skipped); none when it is no array.
    pub fn strings(&self) -> Vec<String> {
        self.array().iter().filter_map(|v| v.str().map(str::to_string)).collect()
    }

    pub fn members(&self) -> &[(String, Value)] {
        match self {
            Value::Object(members) => members,
            _ => &[],
        }
    }
}

/// Parses a JSON document; the error says where it went wrong.
pub fn parse(text: &str) -> Result<Value, String> {
    let mut p = Reader { s: text.as_bytes(), pos: 0, depth: 0 };
    let value = p.value()?;
    p.ws();
    match p.pos == p.s.len() {
        true => Ok(value),
        false => Err(p.error("text after the value")),
    }
}

/// How many arrays and objects may stand inside one another: the files and downloads read nest a
/// few levels; a deeper document (`[[[[…`) is an error, not a crash (each level is a call deeper).
const MAX_DEPTH: usize = 64;

struct Reader<'a> {
    s: &'a [u8],
    pos: usize,
    /// The arrays and objects open now.
    depth: usize,
}

impl Reader<'_> {
    fn error(&self, what: &str) -> String {
        let line = self.s[..self.pos.min(self.s.len())].iter().filter(|&&b| b == b'\n').count() + 1;
        format!("JSON: {what} at line {line}")
    }

    fn ws(&mut self) {
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn eat(&mut self, b: u8) -> bool {
        self.ws();
        let found = self.s.get(self.pos) == Some(&b);
        self.pos += usize::from(found);
        found
    }

    fn value(&mut self) -> Result<Value, String> {
        if self.depth == MAX_DEPTH {
            return Err(self.error("nested too deeply"));
        }
        self.depth += 1;
        let value = self.one_value();
        self.depth -= 1;
        value
    }

    fn one_value(&mut self) -> Result<Value, String> {
        self.ws();
        match self.s.get(self.pos) {
            Some(b'{') => {
                self.pos += 1;
                let mut members = Vec::new();
                if self.eat(b'}') {
                    return Ok(Value::Object(members));
                }
                loop {
                    self.ws();
                    let key = self.string()?;
                    if !self.eat(b':') {
                        return Err(self.error("`:` expected"));
                    }
                    members.push((key, self.value()?));
                    if self.eat(b'}') {
                        return Ok(Value::Object(members));
                    }
                    if !self.eat(b',') {
                        return Err(self.error("`,` or `}` expected"));
                    }
                }
            }
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                if self.eat(b']') {
                    return Ok(Value::Array(items));
                }
                loop {
                    items.push(self.value()?);
                    if self.eat(b']') {
                        return Ok(Value::Array(items));
                    }
                    if !self.eat(b',') {
                        return Err(self.error("`,` or `]` expected"));
                    }
                }
            }
            Some(b'"') => self.string().map(Value::String),
            Some(b't') if self.s[self.pos..].starts_with(b"true") => {
                self.pos += 4;
                Ok(Value::Bool(true))
            }
            Some(b'f') if self.s[self.pos..].starts_with(b"false") => {
                self.pos += 5;
                Ok(Value::Bool(false))
            }
            Some(b'n') if self.s[self.pos..].starts_with(b"null") => {
                self.pos += 4;
                Ok(Value::Null)
            }
            Some(b'-' | b'0'..=b'9') => {
                let start = self.pos;
                while self.pos < self.s.len() && matches!(self.s[self.pos], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.pos += 1;
                }
                let text = std::str::from_utf8(&self.s[start..self.pos]).unwrap_or("");
                text.parse().map(Value::Number).map_err(|_| self.error("bad number"))
            }
            _ => Err(self.error("a value expected")),
        }
    }

    fn string(&mut self) -> Result<String, String> {
        if self.s.get(self.pos) != Some(&b'"') {
            return Err(self.error("a string expected"));
        }
        self.pos += 1;
        let mut out = Vec::new();
        loop {
            match self.s.get(self.pos) {
                None => return Err(self.error("unterminated string")),
                Some(b'"') => {
                    self.pos += 1;
                    return String::from_utf8(out).map_err(|_| self.error("invalid UTF-8"));
                }
                Some(b'\\') => {
                    let escaped = self.s.get(self.pos + 1).copied();
                    self.pos += 2;
                    match escaped {
                        Some(b'n') => out.push(b'\n'),
                        Some(b't') => out.push(b'\t'),
                        Some(b'r') => out.push(b'\r'),
                        Some(b'b') => out.push(8),
                        Some(b'f') => out.push(12),
                        Some(b'u') => {
                            let hex = std::str::from_utf8(self.s.get(self.pos..self.pos + 4).unwrap_or(b"")).unwrap_or("");
                            let c = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32).ok_or_else(|| self.error("bad \\u escape"))?;
                            self.pos += 4;
                            out.extend_from_slice(c.to_string().as_bytes());
                        }
                        Some(c) => out.push(c),
                        None => return Err(self.error("unterminated string")),
                    }
                }
                // A run without quotes or escapes, copied at once.
                Some(_) => {
                    let run = self.s[self.pos..].iter().position(|&b| b == b'"' || b == b'\\').unwrap_or(self.s.len() - self.pos);
                    out.extend_from_slice(&self.s[self.pos..self.pos + run]);
                    self.pos += run;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{Ambiguity, Choice};

    #[test]
    fn strings_are_escaped() {
        assert_eq!(string("plain"), r#""plain""#);
        assert_eq!(string("a \"b\" \\ c"), r#""a \"b\" \\ c""#);
        assert_eq!(string("line\nnext\ttab\u{1}"), r#""line\nnext\ttab\u0001""#);
        assert_eq!(string("°C m² µs"), "\"°C m² µs\"", "non-ASCII stays as UTF-8");
    }

    #[test]
    fn peak_memory_is_reported() {
        let kb = peak_kb();
        assert!((100..1_000_000).contains(&kb), "{kb} KiB");
        let ok = Ok(Output { stdout: "1".into(), ..Output::default() });
        let json = result(&ok, &[]);
        assert!(json.starts_with(r#"{"ok":true,"result":"1","notes":[],"warnings":[],"peak_kb":"#), "{json}");
        assert!(json.ends_with('}') && json[..json.len() - 1].rsplit(':').next().unwrap().parse::<u64>().is_ok(), "{json}");
    }

    #[test]
    fn results() {
        let ok = Ok(Output { stdout: "6.2137 mi".into(), notes: vec!["a note".into()], ..Output::default() });
        assert_eq!(outcome(&ok), r#"{"ok":true,"result":"6.2137 mi","notes":["a note"],"warnings":[]}"#);
        let usage = Err(CliError::Usage("bad `x`".into()));
        assert_eq!(outcome(&usage), r#"{"ok":false,"exit":2,"error":"bad `x`"}"#);
        let a = Ambiguity {
            text: "12/04/2026".into(),
            choices: vec![Choice { pattern: "MM/DD/YYYY".into(), date: "2026-12-04".into(), request: "12/04/2026 MM/DD/YYYY unix".into() }],
            kind: crate::time::AmbiguityKind::Date,
            note: String::new(),
        };
        let json = outcome(&Err(CliError::Ambiguous(a)));
        assert!(json.starts_with(r#"{"ok":false,"exit":2,"error":"`12/04/2026` is ambiguous"#), "{json}");
        assert!(json.ends_with(
            r#","ambiguous":{"kind":"date","text":"12/04/2026","choices":[{"pattern":"MM/DD/YYYY","date":"2026-12-04","request":"12/04/2026 MM/DD/YYYY unix"}]}}"#
        ));
    }

    /// A member of `view`'s reply, as a string.
    fn member(reply: &str, key: &str) -> Option<String> {
        parse(reply).unwrap().get(key).and_then(|v| v.str().map(str::to_string))
    }

    /// What a UI shows: only a result while a word is being typed; after a space or Enter, the
    /// hint, the question or the error, ready to show.
    #[test]
    fn view_says_what_to_show() {
        let show = |request: &str, submitted: bool| member(&view(request, submitted), "show").unwrap();
        assert_eq!(show("Rie", false), "none", "a word on its way: no question yet");
        assert_eq!(show("Rie", true), "question");
        assert_eq!(show("10 foo", false), "none");
        assert_eq!(show("10 foo ", false), "error");
        assert_eq!(show("12+4", false), "result", "a result shows at once");
        assert_eq!(show("10 km", false), "none");
        assert_eq!(show("10 km ", false), "hint");
        let hint = view("10 km ", false);
        assert_eq!(member(&hint, "message").as_deref(), Some("now the length unit to convert to: in, ft, yd…"));
        let error = view("10 foo m", true);
        assert_eq!(member(&error, "message").as_deref(), Some("unknown unit `foo`"));
        let too_long = view(&vec!["1"; 300].join("+"), false);
        assert_eq!(member(&too_long, "show").as_deref(), Some("error"), "too long: shown even mid-word");
        assert_eq!(member(&too_long, "message").as_deref(), Some("the request is too long: at most 512 characters"));
        assert!(too_long.contains(",\"peak_kb\":"));
    }

    #[test]
    fn view_of_a_result() {
        let r = view("10 km mi", false);
        assert_eq!(member(&r, "value").as_deref(), Some("6.2137"));
        assert!(r.contains(r#""details":[]"#), "{r}");
        assert_eq!(member(&view("12+4/2", false), "grouping").as_deref(), Some("(12 + (4 / 2))"));
        assert_eq!(member(&view("12/04/2026", true), "grouping").as_deref(), Some("((12 / 4) / 2026)"), "a lone number expression is arithmetic");
        // Notes and warnings, under the result.
        assert!(view("1 GB GiB", false).contains(r#""details":["mixing decimal (GB"#));
    }

    /// An ambiguous request: the question, each choice's label and detail, and Enter's choice.
    #[test]
    fn view_of_a_question() {
        let date = parse(&view("12/04/2026 unix", true)).unwrap();
        assert_eq!(date.get("message").and_then(Value::str), Some("Which date is 12/04/2026?"));
        assert_eq!(date.get("default").and_then(Value::str), Some("12/04/2026 MM/DD/YYYY unix"));
        let first = &date.get("choices").unwrap().array()[0];
        assert_eq!(first.get("label").and_then(Value::str), Some("MM/DD/YYYY   2026-12-04"));
        assert_eq!(first.get("detail").and_then(Value::str), Some(""));
        assert_eq!(first.get("request").and_then(Value::str), Some("12/04/2026 MM/DD/YYYY unix"));
        let city = parse(&view("Tokio ", false)).unwrap();
        assert_eq!(city.get("message").and_then(Value::str), Some("`Tokio` is not a city: did you mean…"));
        assert_eq!(city.get("default").and_then(Value::str), Some("Tokyo"));
        let a = |kind, note: &str| Ambiguity { text: "x".into(), choices: Vec::new(), kind, note: note.into() };
        assert_eq!(question(&a(crate::time::AmbiguityKind::Country, "")), "`x` has several time zones: which one?");
        assert_eq!(question(&a(crate::time::AmbiguityKind::Place, "`romme` is not a known place")), "`romme` is not a known place: which one?");
        assert_eq!(question(&a(crate::time::AmbiguityKind::Currency, "`dollar` is several currencies")), "`dollar` is several currencies: which one?");
    }

    /// What Copy and Share give of a result: the value only.
    #[test]
    fn the_value_of_a_result() {
        assert_eq!(value_of("7.4565 mi"), "7.4565");
        assert_eq!(value_of("8.94 EUR"), "8.94");
        assert_eq!(value_of("-40 F"), "-40");
        assert_eq!(value_of("1,107 km"), "1,107");
        assert_eq!(value_of("14"), "14");
        assert_eq!(value_of("16 Oct 26 4:00 AM GMT+9 Tokyo"), "16 Oct 26 4:00 AM GMT+9 Tokyo");
        // A date in a pattern, a timestamp: without the pattern's name, as a measure without its unit.
        assert_eq!(value_of("2026/12/11 YYYY/MM/DD"), "2026/12/11");
        assert_eq!(value_of("1796338800 unix"), "1796338800");
    }

    /// What the widget copies: the unit kept, a format's name left out, a city's ISO time.
    #[test]
    fn what_the_widget_copies() {
        assert_eq!(copy_of("6.2137 mi"), "6.2137 mi");
        assert_eq!(copy_of("2026/11/22 YYYY/MM/DD"), "2026/11/22");
        assert_eq!(copy_of("1796338800 unix"), "1796338800");
        assert_eq!(copy_of("12/04/2026 3:20 PM MM/DD/YYYY-12h"), "12/04/2026 3:20 PM");
        assert_eq!(copy_of("2026-12-04T10:00:00+01:00 iso"), "2026-12-04T10:00:00+01:00");
        assert_eq!(copy_of("now Rome = 2026-10-11T16:00+02:00 Paris"), "2026-10-11T16:00+02:00");
        assert_eq!(copy_of("16 Oct 26 4:00 AM GMT+9 Tokyo"), "16 Oct 26 4:00 AM GMT+9 Tokyo");
        assert!(!is_format("YYYY/MM"));
        assert!(!is_format("km"));
    }

    /// The Measures open at the class of the request's unit.
    #[test]
    fn the_class_of_the_request_unit() {
        let config = crate::config::Config::shared();
        let class = |r: &str| unit_class(config, r);
        let name = |unit: &str| crate::convert::find_unit(config, unit).map(|(c, _)| c.name);
        let (length, data) = (name("cm"), name("GB"));
        assert_eq!(class("10 cm "), length);
        assert_eq!(class("10 cm in"), length);
        assert_eq!(class("1 GB"), data);
        assert_eq!(class("1 gb"), data, "a case no unit has: any case");
        assert_eq!(class("(1+2) CM"), length);
        assert_eq!(class(""), None);
        assert_eq!(class("10 "), None);
        assert_eq!(class("12 EUR in"), None);
        assert_eq!(class("Rome Paris"), None);
        assert_eq!(member(&view("10 cm ", false), "class").as_deref(), length);
    }

    #[test]
    fn typing_a_word() {
        assert!(crate::typing("Rie", false));
        assert!(!crate::typing("Rie", true));
        assert!(!crate::typing("Rieti ", false));
        assert!(!crate::typing("", true));
    }

    #[test]
    fn parses_documents() {
        let v = parse(r#"{"a": [1, -2.5, 3e2], "b": {"c": "x\"y\\n\u00e9"}, "t": true, "f": false, "n": null, "e": []}"#).unwrap();
        assert_eq!(v.get("a").unwrap().array().iter().map(|n| n.number().unwrap()).collect::<Vec<_>>(), [1.0, -2.5, 300.0]);
        assert_eq!(v.get("b").and_then(|b| b.get("c")).and_then(Value::str), Some("x\"y\\né"));
        assert_eq!(v.get("t"), Some(&Value::Bool(true)));
        assert_eq!(v.get("n"), Some(&Value::Null));
        assert_eq!(v.members().len(), 6);
        // Round trip with the writer.
        assert_eq!(parse(&string("tab\there")).unwrap().str(), Some("tab\there"));
    }

    #[test]
    fn parse_errors_say_where() {
        assert_eq!(parse("{\n\"a\": 1,\n}").unwrap_err(), "JSON: a string expected at line 3");
        assert!(parse("[1 2]").unwrap_err().contains("`,` or `]` expected"));
        assert!(parse("1 2").is_err());
        assert!(parse("\"abc").unwrap_err().contains("unterminated"));
        // Nested past the limit: an error, not a crash.
        assert!(parse(&format!("{}1{}", "[".repeat(MAX_DEPTH - 1), "]".repeat(MAX_DEPTH - 1))).is_ok());
        assert!(parse(&"[".repeat(100_000)).unwrap_err().contains("nested too deeply"));
    }

    #[test]
    fn memory_now() {
        assert_eq!(size(900), "900 KiB");
        assert_eq!(size(3482), "3.4 MiB");
        if cfg!(target_os = "linux") {
            // Two counters read a moment apart, while other tests allocate: a little slack.
            let now = current_kb().expect("VmRSS on Linux");
            assert!(now > 0 && now <= peak_kb() + 1024, "{now} <= {} + 1024", peak_kb());
        }
    }
}
