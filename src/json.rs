//! `--json`: one JSON object per request, for programs such as the Omarchy bar widget.
//!
//! ```text
//! {"ok":true,"result":"6.2137 mi","notes":[],"warnings":[],"peak_kb":1024}
//! {"ok":false,"exit":1,"error":"unknown unit `foo` (see `tally --list`)","peak_kb":1024}
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
pub fn current_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|l| l.strip_prefix("VmRSS:")?.trim().strip_suffix("kB")?.trim().parse().ok())
}

/// KiB for people: `900 KiB`, `3.4 MiB`.
#[cfg_attr(not(feature = "tui"), allow(dead_code))]
pub fn size(kb: u64) -> String {
    match kb {
        0..=1023 => format!("{kb} KiB"),
        _ => format!("{:.1} MiB", kb as f64 / 1024.0),
    }
}

/// `getrusage`'s peak resident memory, in KiB.
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
    if cfg!(target_os = "macos") { maxrss / 1024 } else { maxrss }
}

/// One request's outcome as a JSON object, on one line, with the peak memory so far.
pub fn result(result: &Result<Output, CliError>, notices: &[String]) -> String {
    let object = outcome(result);
    let notices = match notices.is_empty() {
        true => String::new(),
        false => format!(",\"notices\":[{}]", notices.iter().map(|n| string(n)).collect::<Vec<_>>().join(",")),
    };
    format!("{}{notices},\"peak_kb\":{}}}", &object[..object.len() - 1], peak_kb())
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
                    let choices: Vec<String> = a
                        .choices
                        .iter()
                        .map(|c| Object::new().str("pattern", &c.pattern).str("date", &c.date).str("request", &c.request).finish())
                        .collect();
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

    pub fn members(&self) -> &[(String, Value)] {
        match self {
            Value::Object(members) => members,
            _ => &[],
        }
    }
}

/// Parses a JSON document; the error says where it went wrong.
pub fn parse(text: &str) -> Result<Value, String> {
    let mut p = Reader { s: text.as_bytes(), pos: 0 };
    let value = p.value()?;
    p.ws();
    match p.pos == p.s.len() {
        true => Ok(value),
        false => Err(p.error("text after the value")),
    }
}

struct Reader<'a> {
    s: &'a [u8],
    pos: usize,
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
