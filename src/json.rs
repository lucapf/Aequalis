//! JSON, without crates: a writer (`string`, `Object`) and a reader (`parse`, `Value`), for the data
//! files compiled in, the files kept (`files.rs`), the downloads, and the replies (`reply.rs`).

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

    /// The character of a `\u` escape (the reader is after `\u`), and of a second one after it
    /// when the first is half of a surrogate pair (`\ud83d\ude00`: JSON writes a character past
    /// U+FFFF so).
    fn escaped_char(&mut self) -> Option<char> {
        let high = self.hex4()?;
        if !(0xD800..0xDC00).contains(&high) {
            return char::from_u32(high);
        }
        if self.s.get(self.pos..self.pos + 2) != Some(b"\\u") {
            return None;
        }
        self.pos += 2;
        let low = self.hex4().filter(|l| (0xDC00..0xE000).contains(l))?;
        char::from_u32(0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00))
    }

    /// Four hex digits, read.
    fn hex4(&mut self) -> Option<u32> {
        let hex = std::str::from_utf8(self.s.get(self.pos..self.pos + 4)?).ok()?;
        let unit = u32::from_str_radix(hex, 16).ok()?;
        self.pos += 4;
        Some(unit)
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
                            let c = self.escaped_char().ok_or_else(|| self.error("bad \\u escape"))?;
                            out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
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

    #[test]
    fn strings_are_escaped() {
        assert_eq!(string("plain"), r#""plain""#);
        assert_eq!(string("a \"b\" \\ c"), r#""a \"b\" \\ c""#);
        assert_eq!(string("line\nnext\ttab\u{1}"), r#""line\nnext\ttab\u0001""#);
        assert_eq!(string("°C m² µs"), "\"°C m² µs\"", "non-ASCII stays as UTF-8");
    }

    #[test]
    fn parses_documents() {
        let v = parse(r#"{"a": [1, -2.5, 3e2], "b": {"c": "x\"y\\n\u00e9"}, "t": true, "f": false, "n": null, "e": []}"#).unwrap();
        assert_eq!(v.get("a").unwrap().array().iter().map(|n| n.number().unwrap()).collect::<Vec<_>>(), [1.0, -2.5, 300.0]);
        assert_eq!(v.get("b").and_then(|b| b.get("c")).and_then(Value::str), Some("x\"y\\né"));
        assert_eq!(v.get("t"), Some(&Value::Bool(true)));
        assert_eq!(v.get("n"), Some(&Value::Null));
        assert_eq!(v.members().len(), 6);
        // A character past U+FFFF, written as a surrogate pair; half a pair is an error.
        assert_eq!(parse(r#""\ud83d\ude00 \u00e9""#).unwrap().str(), Some("😀 é"));
        assert!(parse(r#""\ud83d""#).is_err() && parse(r#""\ude00""#).is_err());
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
}
