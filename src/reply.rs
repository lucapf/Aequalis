//! The replies for programs. `--json`: one JSON object per request, for programs such as the Omarchy
//! bar widget; `view`: a request's reply for a UI (the Android app, the widget, the Windows tray).
//!
//! ```text
//! {"ok":true,"result":"6.2137 mi","notes":[],"warnings":[],"peak_kb":1024}
//! {"ok":false,"exit":1,"error":"unknown unit `foo` (see `aequalis --list`)","peak_kb":1024}
//! {"ok":false,"exit":2,"error":"…","ambiguous":{"text":"12/04/2026","choices":[{"pattern":"MM/DD/YYYY","date":"2026-12-04","request":"12/04/2026 MM/DD/YYYY unix"},…]},"peak_kb":1024}
//! ```
//! Nothing is asked and nothing goes to stderr: an ambiguous date is an error carrying its choices.
//! `peak_kb` is the process's peak memory so far (resident set, in KiB: `memory::peak_kb`): what the
//! request took.

use crate::json::{string, Object};
use crate::memory::peak_kb;
use crate::{CliError, Output};

/// One request's outcome as a JSON object, on one line, with the peak memory so far.
pub fn result(result: &Result<Output, CliError>, notices: &[String]) -> String {
    with(result, notices, "", false)
}

/// `result`'s object with `extra` members (each starting with a comma) before the notices.
fn with(result: &Result<Output, CliError>, notices: &[String], extra: &str, display: bool) -> String {
    let object = outcome_as(result, display);
    let notices = match notices.is_empty() {
        true => String::new(),
        false => format!(",\"notices\":[{}]", notices.iter().map(|n| string(n)).collect::<Vec<_>>().join(",")),
    };
    format!("{}{extra}{notices},\"peak_kb\":{}}}", &object[..object.len() - 1], peak_kb())
}

/// `outcome`, with `display`: the result's number written as the device writes numbers (`1,235,567`),
/// as the TUI shows it; not what is already formatted for people (dates, distances, sizes). The
/// value to copy stays as it is (`shown`).
fn outcome_as(result: &Result<Output, CliError>, display: bool) -> String {
    match result {
        Ok(out) => {
            let raw = out.result.as_ref().unwrap_or(&out.stdout);
            let written = match display && !out.formatted {
                true => crate::numbers::device().apply_to_result(raw),
                false => raw.clone(),
            };
            let reply = Object::new().num("ok", true).str("result", &written);
            // The other results (`42 eu us`: the shoe sizes), each with its kind.
            let reply = match out.extra.is_empty() {
                true => reply,
                false => reply.list("extra", &extra(&out.extra)),
            };
            let reply = reply.strs("notes", &out.notes).strs("warnings", &out.warnings);
            // A currency result: how old its rates are (`2d`); `--online`: how long ago the last sync was.
            let reply = match &out.rates_age {
                Some(age) => reply.str("rates_age", age),
                None => reply,
            };
            match &out.synced {
                Some(ago) => reply.str("synced", ago).finish(),
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
                // Not finished yet: a hint, not an error; with the results it already has.
                CliError::Incomplete(_, more) if more.is_empty() => r#","incomplete":true"#.to_string(),
                CliError::Incomplete(_, more) => format!(r#","incomplete":true,"extra":[{}]"#, extra(more).join(",")),
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
    with(&result, &notices, &extra, true)
}

/// `view`'s reply to a request refused before it is read (too long): an error, shown at once.
pub fn refused(err: CliError) -> String {
    let result = Err(err);
    with(&result, &[], &shown("", true, &result), true)
}

/// The members of `view` that say what to show, each starting with a comma.
fn shown(request: &str, submitted: bool, result: &Result<Output, CliError>) -> String {
    let o = match result {
        Ok(out) => {
            let details: Vec<String> = out.warnings.iter().chain(&out.notes).cloned().collect();
            let result = out.result.as_ref().unwrap_or(&out.stdout);
            // Every person's shoe size: all of them, a line each, for Copy and the expression alike.
            let (value, copy, whole) = match &out.copy {
                Some(all) => (all.clone(), all.clone(), all.clone()),
                None => (value_of(result), copy_of(result), expression(request, out)),
            };
            let o = Object::new().str("show", "result").str("value", &value).str("copy", &copy).str("expression", &whole).strs("details", &details);
            match &out.rates_age {
                Some(age) => o.str("rates", &format!("rates {age} old")),
                None => o,
            }
        }
        Err(_) if crate::typing(request, submitted) => Object::new().str("show", "none"),
        Err(CliError::Incomplete(hint, _)) => Object::new().str("show", "hint").str("message", hint),
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

/// The extra results as JSON objects: `{"type":"shoes","results":["US 8.5 men","US 10 women"]}`; a
/// line of another reading (beside a currency, after a size) with what it converts:
/// `{"type":"clothes","results":["US 12 women"],"from":"42 eu"}`.
fn extra(all: &[crate::Extra]) -> Vec<String> {
    all.iter()
        .map(|e| {
            let o = Object::new().str("type", &e.kind).strs("results", &e.results);
            if e.from.is_empty() { o } else { o.str("from", &e.from) }.finish()
        })
        .collect()
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
    // A table (several lines) as it is: its first line's spaces line its columns up.
    if result.trim().contains('\n') {
        return result.trim_end().to_string();
    }
    let result = result.trim();
    let parts: Vec<&str> = result.split(' ').collect();
    // A size: `US 8.5 (Nike), 9 (Clarks)` → the first chart's, `8.5`.
    if parts.len() > 1 && ["US", "UK", "EU", "JP", "CN"].contains(&parts[0]) {
        return parts[1].trim_end_matches(',').to_string();
    }
    let number = parts.len() == 2 && parts[0].starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '.');
    if number { parts[0] } else { result }.to_string()
}

/// The whole expression, for Copy: `12 AED = 3.27 USD`, `12+4/2 = 14`; what was converted as the
/// core shows it, else the request as written (`now Tokyo = …`). A result that already says both
/// sides (a time in a city, `… = …`) as it is.
pub fn expression(request: &str, out: &Output) -> String {
    let result = out.result.as_ref().unwrap_or(&out.stdout).trim();
    if result.contains(" = ") {
        return result.to_string();
    }
    let left = out.from.as_deref().unwrap_or(request).split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{left} = {result}")
}

/// What the Omarchy widget copies of a result: the unit kept (`6.2137 mi`), a format's name left out
/// (`2026/11/22 YYYY/MM/DD` → `2026/11/22`), and of a city's time (`… = 2026-10-11T16:00+02:00
/// Paris`) the ISO time.
pub fn copy_of(result: &str) -> String {
    if result.trim().contains('\n') {
        return result.trim_end().to_string();
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{parse, Value};
    use crate::time::{Ambiguity, Choice};

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
        assert_eq!(outcome_as(&ok, false), r#"{"ok":true,"result":"6.2137 mi","notes":["a note"],"warnings":[]}"#);
        let usage = Err(CliError::Usage("bad `x`".into()));
        assert_eq!(outcome_as(&usage, false), r#"{"ok":false,"exit":2,"error":"bad `x`"}"#);
        let a = Ambiguity {
            text: "12/04/2026".into(),
            choices: vec![Choice { pattern: "MM/DD/YYYY".into(), date: "2026-12-04".into(), request: "12/04/2026 MM/DD/YYYY unix".into() }],
            kind: crate::time::AmbiguityKind::Date,
            note: String::new(),
        };
        let json = outcome_as(&Err(CliError::Ambiguous(a)), false);
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
    fn the_whole_expression_to_copy() {
        let whole = |request: &str| member(&view(request, true), "expression").unwrap();
        assert_eq!(whole("10 kilometres mi"), "10 km = 6.2137 mi");
        assert_eq!(whole("12+4/2"), "12+4/2 = 14");
        assert_eq!(whole("100 + 20 sqft m2"), "100 + 20 ft2 = 11.1484 m2");
        assert_eq!(member(&view("10 km mi", true), "value").as_deref(), Some("6.2137"));
    }

    #[test]
    fn view_says_what_to_show() {
        // Metric: a unit alone (`10 km`) then asks for its target, whatever the laptop.
        crate::machine::Mock::italy().keep();
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

    /// The result's number as the device writes numbers; what Copy gives stays plain.
    #[test]
    fn the_result_in_the_devices_format() {
        crate::machine::Mock::us().keep();
        let r = view("1234567+1", false);
        assert_eq!(member(&r, "result").as_deref(), Some("1,234,568"));
        assert_eq!(member(&r, "value").as_deref(), Some("1234568"));
        assert_eq!(member(&r, "expression").as_deref(), Some("1234567+1 = 1234568"));
        assert_eq!(member(&view("100000 m km", false), "result").as_deref(), Some("100 km"));
        assert_eq!(member(&view("10000000 m km", false), "result").as_deref(), Some("10,000 km"));
        // A date is already for people: `2026` is a year, not a number to group.
        let stamp = member(&view("2026-12-04 unix", false), "result").unwrap();
        assert!(stamp.ends_with(" unix") && !stamp.contains(','), "{stamp}");
        crate::machine::Mock::italy().keep();
        assert_eq!(member(&view("1234567.5+1", false), "result").as_deref(), Some("1.234.568,5"));
        assert_eq!(member(&view("1234567.5+1", false), "value").as_deref(), Some("1234568.5"));
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
        // The device's date format is YYYY/MM/DD: `12/04/2026` fits none of its own, so it asks, whatever the laptop.
        crate::machine::Mock::japan().keep();
        let date = parse(&view("12/04/2026 unix", true)).unwrap();
        assert_eq!(date.get("message").and_then(Value::str), Some("Which date is 12/04/2026?"));
        assert_eq!(date.get("default").and_then(Value::str), Some("12/04/2026 DD/MM/YYYY unix"));
        let choices = date.get("choices").unwrap().array();
        let first = &choices[0];
        assert_eq!(first.get("label").and_then(Value::str), Some("DD/MM/YYYY   2026-04-12"));
        assert_eq!(first.get("detail").and_then(Value::str), Some(""));
        assert_eq!(first.get("request").and_then(Value::str), Some("12/04/2026 DD/MM/YYYY unix"));
        assert_eq!(choices[1].get("request").and_then(Value::str), Some("12/04/2026 MM/DD/YYYY unix"));
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
        // A size: the first chart's; a table as it is, its first line's spaces kept.
        assert_eq!(value_of("US 8.5 (Nike), 9 (Clarks)"), "8.5");
        assert_eq!(value_of("US 15.5 men (Clarks)"), "15.5");
        assert_eq!(value_of("JP 26.5 cm"), "26.5");
        let table = "         US    UK\nmen     8.5   7.5";
        assert_eq!((value_of(table).as_str(), copy_of(table).as_str()), (table, table));
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
        assert!(is_format("iso-week") && is_format("unix-ms") && is_format("local"));
        assert!(!is_format("iso-") && !is_format("unix-MS"));
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
}
