//! Currencies: `12 usd euro`, `12 USA Europe`, `12 USD UAE`, `$12 €`, `12 dollars to yen`.
//!
//! A currency is written as its ISO code, its name (singular or plural), a symbol, a common
//! abbreviation (`src/data/currencies.json`), a country (its currency, `src/data/countries.json`)
//! or Europe / EU / Eurozone; case and accents don't matter, and the amount may be attached
//! (`$12`, `12€`, `12usd`). Always one amount: with a single currency the target is the local one
//! (the currency of the device time zone's country, or `TALLY_CURRENCY`). A name several
//! currencies share (`dollar`, `$`, `kr`) or a misspelt one (`dolar`) is asked about.
//!
//! The rates come from ExchangeRate-API (open.er-api.com, daily), kept in
//! `~/.config/tally/currencies.json` with the online data's policies (`online.rs`): synced in the
//! background when older than the refresh interval, never during a request.

use crate::config::Config;
use crate::online::{Data, Source};
use crate::time::tz;
use crate::time::{Ambiguity, AmbiguityKind, Choice};
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// A currency, as `src/data/currencies.json` describes it.
#[derive(Debug)]
pub struct Currency {
    pub code: String,
    pub name: String,
    pub words: Vec<String>,
    pub symbols: Vec<String>,
    pub abbreviations: Vec<String>,
}

const DATA: &str = include_str!("data/currencies.json");
/// Europe and the like: the euro.
const EURO_AREA: [&str; 6] = ["Europe", "EU", "Eurozone", "Euro area", "European Union", "Euroland"];
const URL: &str = "https://open.er-api.com/v6/latest/USD";
/// Where the rates come from, for people (their terms ask for it to be named).
pub const PROVIDER: &str = "ExchangeRate-API (open.er-api.com)";
/// The same, with its site, for About.
pub const SOURCE: &str = "ExchangeRate-API (https://www.exchangerate-api.com)";
/// The rates' own date (Unix seconds), stored with them.
pub const RATES_DATE: &str = "rates date";
/// Rates this many days old or more are reported: the source updates every day.
const STALE_DAYS: i64 = 3;
/// At most this many choices in a question.
const MAX_CHOICES: usize = 8;
const SHAPE: &str = "a currency conversion is one amount and one or two currencies: `12 USD EUR`, `12 USA Europe`";

/// Every currency, the most used first.
pub fn all() -> &'static [Currency] {
    static TABLE: OnceLock<Vec<Currency>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let doc = crate::json::parse(DATA).expect("src/data/currencies.json is valid JSON");
        let strings = |v: Option<&crate::json::Value>| -> Vec<String> {
            v.map(|a| a.array().iter().filter_map(|s| s.str().map(str::to_string)).collect()).unwrap_or_default()
        };
        doc.get("currencies")
            .map(|c| c.array())
            .unwrap_or_default()
            .iter()
            .filter_map(|c| {
                Some(Currency {
                    code: c.get("code")?.str()?.to_string(),
                    name: c.get("name")?.str()?.to_string(),
                    words: strings(c.get("words")),
                    symbols: strings(c.get("symbols")),
                    abbreviations: strings(c.get("abbreviations")),
                })
            })
            .collect()
    })
}

/// The currency with ISO code `code`, in any case.
pub fn get(code: &str) -> Option<&'static Currency> {
    all().iter().find(|c| c.code.eq_ignore_ascii_case(code))
}

// ---------------------------------------------------------------- what a word means

/// The currencies `text` names exactly, the most used first: one (`usd`, `euros`, `Japan`, `€`), or
/// several when they share it (`dollar`, `$`, `kr`); `None`: not a currency.
pub fn exact(text: &str) -> Option<Vec<&'static str>> {
    let raw = text.trim();
    if raw.is_empty() {
        return None;
    }
    let key = tz::key(raw);
    let squeezed = key.replace(' ', "");
    // An ISO code: only that one.
    if squeezed.len() == 3 && squeezed.chars().all(|c| c.is_ascii_alphabetic()) {
        if let Some(c) = get(&squeezed) {
            return Some(vec![c.code.as_str()]);
        }
    }
    if EURO_AREA.iter().any(|r| tz::key(r) == key) {
        return Some(vec!["EUR"]);
    }
    if let Some(country) = tz::country(raw).filter(|c| !c.currency.is_empty()) {
        return Some(vec![country.currency.as_str()]);
    }
    // `$` alone: any dollar.
    let lower = raw.to_lowercase();
    let lower = if lower == "$" { "dollar".to_string() } else { lower };
    let key = if raw == "$" { "dollar".to_string() } else { key };
    let named = |form: &str| {
        let k = tz::key(form);
        !k.is_empty() && (k == key || (key.ends_with('s') && k == key[..key.len() - 1]))
    };
    let found: Vec<&'static str> = all()
        .iter()
        .filter(|c| {
            named(&c.name)
                || c.words.iter().any(|w| named(w))
                || c.abbreviations.iter().any(|a| tz::key(a) == key)
                || c.symbols.iter().any(|s| s.to_lowercase() == lower)
        })
        .map(|c| c.code.as_str())
        .collect();
    (!found.is_empty()).then_some(found)
}

/// A misspelt currency or country: the nearest names within 1 letter (2 for longer words), each
/// as (code, how to show it); empty for a word that is exact or too short.
fn near(text: &str) -> (Vec<(&'static str, String)>, bool) {
    let wanted = tz::key(text).replace(' ', "");
    let len = wanted.chars().count();
    if len < 4 || !wanted.chars().all(char::is_alphabetic) || exact(text).is_some() {
        return (Vec::new(), false);
    }
    let limit = if len <= 5 { 1 } else { 2 };
    let squeeze = |s: &str| tz::key(s).replace(' ', "");
    let mut found: Vec<(usize, &'static str, String, bool)> = Vec::new();
    for c in all() {
        let forms = std::iter::once(&c.name).chain(&c.words).filter(|f| f.chars().count() >= 4);
        let d = forms.map(|f| tz::levenshtein(&wanted, &squeeze(f)).min(tz::levenshtein(&wanted, &(squeeze(f) + "s")))).min();
        if let Some(d) = d.filter(|&d| d <= limit) {
            found.push((d, c.code.as_str(), c.name.clone(), false));
        }
    }
    for country in tz::near_countries(text) {
        if !country.currency.is_empty() {
            let names: Vec<&str> = std::iter::once(country.name.as_str()).chain(country.aliases.iter().map(String::as_str)).collect();
            let d = tz::distance(text, &names);
            if d <= limit {
                found.push((d, country.currency.as_str(), format!("{} (country)", country.name), true));
            }
        }
    }
    let best = found.iter().map(|f| f.0).min().unwrap_or(0);
    let only_countries = found.iter().filter(|f| f.0 == best).all(|f| f.3);
    let mut out: Vec<(&'static str, String)> = Vec::new();
    for (_, code, label, _) in found.into_iter().filter(|f| f.0 == best) {
        if !out.iter().any(|(c, _)| *c == code) {
            out.push((code, label));
        }
    }
    (out, only_countries)
}

/// One number, nothing else: `12`, `23.50`, `1e3`.
fn amount(word: &str) -> Option<f64> {
    if !word.starts_with(|c: char| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let (expr, rest) = crate::calc::parse_prefix(word).ok()?;
    if !rest.trim().is_empty() || expr.operations() != 0 {
        return None;
    }
    expr.eval().ok().filter(|v| v.is_finite() && *v >= 0.0)
}

/// An amount written together with its currency: `$12`, `US$12`, `12€`, `12usd`, `12.50EUR`.
fn split_attached(word: &str) -> Vec<String> {
    // The number first: digits and a point.
    let digits = |s: &str| s.char_indices().take_while(|(_, c)| c.is_ascii_digit() || *c == '.').last().map_or(0, |(i, c)| i + c.len_utf8());
    let head = digits(word);
    if head > 0 && head < word.len() && amount(&word[..head]).is_some() && exact(&word[head..]).is_some() {
        return vec![word[..head].to_string(), word[head..].to_string()];
    }
    // The currency first.
    if let Some(start) = word.char_indices().find(|(_, c)| c.is_ascii_digit()).map(|(i, _)| i) {
        if start > 0 && amount(&word[start..]).is_some() && exact(&word[..start]).is_some() {
            return vec![word[..start].to_string(), word[start..].to_string()];
        }
    }
    vec![word.to_string()]
}

// ---------------------------------------------------------------- the request

/// One part of a currency request.
#[derive(Debug, Clone)]
enum Piece {
    Amount(String),
    /// `to`, `in`.
    Connector,
    /// A currency or country, as written, and what it may be (one or more codes).
    Currency { text: String, codes: Vec<&'static str>, unit_too: bool },
    /// A misspelt one: what it may be (code, how to show it).
    Near { text: String, options: Vec<(&'static str, String)>, countries_only: bool },
    Other,
}

fn pieces(config: &Config, input: &str) -> Vec<Piece> {
    let words: Vec<String> = input.split_whitespace().flat_map(split_attached).collect();
    let longest = tz::longest_country().max(5);
    let mut out = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let w = &words[i];
        if amount(w).is_some() {
            out.push(Piece::Amount(w.clone()));
            i += 1;
            continue;
        }
        if w.eq_ignore_ascii_case("to") || w.eq_ignore_ascii_case("in") {
            out.push(Piece::Connector);
            i += 1;
            continue;
        }
        let span = (1..=longest.min(words.len() - i)).rev().find_map(|n| {
            let text = words[i..i + n].join(" ");
            // A span stops at an amount or a connector.
            if words[i..i + n].iter().skip(1).any(|w| amount(w).is_some() || w.eq_ignore_ascii_case("to") || w.eq_ignore_ascii_case("in")) {
                return None;
            }
            exact(&text).map(|codes| (n, text, codes))
        });
        if let Some((n, text, codes)) = span {
            let unit_too = crate::convert::find_unit(config, &text).is_some();
            out.push(Piece::Currency { text, codes, unit_too });
            i += n;
            continue;
        }
        let (options, countries_only) = near(w);
        out.push(if options.is_empty() { Piece::Other } else { Piece::Near { text: w.clone(), options, countries_only } });
        i += 1;
    }
    out
}

/// A converted amount: the result (`11.08 EUR`), and the notes and warnings.
#[derive(Debug, PartialEq)]
pub struct Converted {
    pub result: String,
    pub notes: Vec<String>,
    pub warnings: Vec<String>,
}

/// Why a currency request has no answer.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// Not written right: a usage error.
    Usage(String),
    /// A currency to choose (shared or misspelt name).
    Ambiguous(Ambiguity),
    /// Written right, but there is no rate to use: a runtime error.
    NoRates(String),
}

/// The local currency: `TALLY_CURRENCY`, else the currency of the device time zone's country; with
/// what it comes from, for the note.
pub struct Local {
    pub code: &'static str,
    pub from: String,
}

/// `request` reads these, so tests can set them.
pub struct Context<'a> {
    pub rates: &'a Data,
    pub now: i64,
    pub local: Option<Local>,
}

/// Whether `input` is a currency request (for the stdin rule: such a request is complete).
pub fn claims(config: &Config, input: &str) -> bool {
    shape(config, input).is_some()
}

/// The request's parts, when it is a currency request: the amount, and the currency pieces in order.
fn shape(config: &Config, input: &str) -> Option<Result<(String, Vec<Piece>), Error>> {
    let parts = pieces(config, input);
    let amounts = parts.iter().filter(|p| matches!(p, Piece::Amount(_))).count();
    if amounts != 1 || parts.iter().any(|p| matches!(p, Piece::Other)) {
        return None;
    }
    let money: Vec<&Piece> = parts.iter().filter(|p| matches!(p, Piece::Currency { .. } | Piece::Near { .. })).collect();
    if money.is_empty() {
        return None;
    }
    // Misspelt countries alone (`12 Tokio`): not money, the time requests' suggestions apply.
    if money.iter().all(|p| matches!(p, Piece::Near { countries_only: true, .. })) {
        return None;
    }
    // Words that are units too (`pounds`, `cup`) with nothing else that is money: units.
    if money.iter().all(|p| matches!(p, Piece::Currency { unit_too: true, .. } | Piece::Near { .. })) {
        return None;
    }
    // `<amount> <from> [to|in] [<to>]`, or the currency first: `<from> <amount> [to|in] [<to>]`.
    let kinds: Vec<char> = parts
        .iter()
        .map(|p| match p {
            Piece::Amount(_) => 'n',
            Piece::Connector => 't',
            _ => 'c',
        })
        .collect();
    let kinds: String = kinds.into_iter().collect();
    let ok = ["nc", "nctc", "ncc", "cn", "cntc", "cnc"].contains(&kinds.as_str());
    let amount = parts.iter().find_map(|p| if let Piece::Amount(a) = p { Some(a.clone()) } else { None })?;
    if !ok {
        return Some(Err(Error::Usage(SHAPE.to_string())));
    }
    Some(Ok((amount, money.into_iter().cloned().collect())))
}

/// A currency request, if `input` is one: one amount (a plain number) and one or two currencies or
/// countries, nothing else. `None` leaves the request to the others (a time with countries is a
/// time request, two countries alone a distance).
pub fn request(config: &Config, input: &str, ctx: &Context) -> Option<Result<Converted, Error>> {
    let (amount, money) = match shape(config, input)? {
        Ok(s) => s,
        Err(e) => return Some(Err(e)),
    };
    Some(convert(&amount, &money, ctx))
}

/// The options of a currency piece: (code, how to show it).
fn options(piece: &Piece) -> Vec<(&'static str, String)> {
    match piece {
        Piece::Currency { codes, .. } => codes.iter().map(|&c| (c, get(c).map_or(c.to_string(), |x| x.name.clone()))).collect(),
        Piece::Near { options, .. } => options.clone(),
        _ => Vec::new(),
    }
}

fn convert(amount: &str, money: &[Piece], ctx: &Context) -> Result<Converted, Error> {
    // Shared or misspelt names: ask, with every combination (at most 8) as a request to choose.
    let unclear: Vec<(usize, &Piece)> = money
        .iter()
        .enumerate()
        .filter(|(_, p)| match p {
            Piece::Currency { codes, .. } => codes.len() > 1,
            _ => true,
        })
        .collect();
    if !unclear.is_empty() {
        return Err(Error::Ambiguous(question(amount, money, &unclear)));
    }
    let codes: Vec<&'static str> = money.iter().map(|p| options(p)[0].0).collect();
    let from = codes[0];
    let mut notes = Vec::new();
    let to = match (codes.get(1), &ctx.local) {
        (Some(&to), _) => to,
        (None, Some(local)) if local.code != from => {
            notes.push(format!("to the local currency, {} ({}): {}", name(local.code), local.code, local.from));
            local.code
        }
        (None, _) => {
            let other = if from == "EUR" { "USD" } else { "EUR" };
            return Err(Error::Usage(format!("now the currency to convert to: `{amount} {from} {other}`")));
        }
    };
    let value: f64 = amount.parse().ok().or_else(|| crate::calc::parse_prefix(amount).ok().and_then(|(e, _)| e.eval().ok())).unwrap_or(0.0);
    let Some(day) = ctx.rates.get(RATES_DATE) else {
        return Err(Error::NoRates(match crate::network::allowed() {
            true => "no exchange rates yet: they are being downloaded in the background, try again in a minute".to_string(),
            false => "no exchange rates: they were never downloaded, and TALLY_MODE=offline".to_string(),
        }));
    };
    let per_dollar = |code: &str| if code == "USD" { Some(1.0) } else { ctx.rates.get(&rate_name(code)).map(|s| s.value) };
    let (a, b) = match (per_dollar(from), per_dollar(to)) {
        (Some(a), Some(b)) => (a, b),
        (None, _) => return Err(Error::NoRates(format!("no exchange rate for the {} ({from})", name(from)))),
        (_, None) => return Err(Error::NoRates(format!("no exchange rate for the {} ({to})", name(to)))),
    };
    let result = value / a * b;
    let day = day.value as i64;
    let shown = crate::time::human(&crate::time::Instant { ns: i128::from(day) * 1_000_000_000, zone: crate::time::Zone::Fixed(0) }, true);
    notes.push(format!("1 {from} = {} {to}, rates of {shown} from {PROVIDER}", crate::convert::format_value(b / a, 6)));
    let mut warnings = Vec::new();
    if ctx.now - day >= STALE_DAYS * 86_400 {
        warnings.push(format!("exchange rates of {shown}: not updated since"));
    }
    Ok(Converted { result: format!("{result:.2} {to}"), notes, warnings })
}

fn name(code: &str) -> String {
    get(code).map_or(code.to_string(), |c| c.name.clone())
}

/// The question for shared or misspelt currencies: each combination of their options (at most
/// `MAX_CHOICES`), written as an unambiguous request (`12 USD EUR`).
fn question(amount: &str, money: &[Piece], unclear: &[(usize, &Piece)]) -> Ambiguity {
    let mut combos: Vec<Vec<(&'static str, String)>> = vec![Vec::new()];
    for piece in money {
        let opts = options(piece);
        combos = combos.iter().flat_map(|c| opts.iter().map(move |o| [c.as_slice(), std::slice::from_ref(o)].concat())).take(MAX_CHOICES).collect();
    }
    let choices = combos
        .into_iter()
        .map(|combo| {
            let request = std::iter::once(amount.to_string()).chain(combo.iter().map(|(c, _)| c.to_string())).collect::<Vec<_>>().join(" ");
            let asked: Vec<&(&str, String)> = unclear.iter().map(|(i, _)| &combo[*i]).collect();
            Choice {
                pattern: asked.iter().map(|(_, label)| label.as_str()).collect::<Vec<_>>().join(", "),
                date: asked.iter().map(|(c, _)| *c).collect::<Vec<_>>().join(", "),
                request,
            }
        })
        .collect();
    let texts: Vec<String> = unclear
        .iter()
        .map(|(_, p)| match p {
            Piece::Currency { text, .. } | Piece::Near { text, .. } => format!("`{text}`"),
            _ => String::new(),
        })
        .collect();
    let misspelt = unclear.iter().any(|(_, p)| matches!(p, Piece::Near { .. }));
    let note = match (misspelt, texts.len()) {
        (false, 1) => format!("{} is several currencies", texts[0]),
        (false, _) => format!("{} are several currencies each", texts.join(", ")),
        (true, 1) => format!("{} is not a currency", texts[0]),
        (true, _) => format!("{} are not all known currencies", texts.join(", ")),
    };
    Ambiguity { text: texts.join(", ").replace('`', ""), choices, kind: AmbiguityKind::Currency, note }
}

// ---------------------------------------------------------------- the local currency

/// The local currency: `TALLY_CURRENCY` (`Err` when it is not a currency), else the one of the
/// country of the device's time zone (`Europe/Rome` → Italy → EUR); `None` when unknown.
pub fn local() -> Result<Option<Local>, String> {
    local_from(std::env::var("TALLY_CURRENCY").ok().as_deref(), local_zone().as_deref())
}

pub fn local_from(setting: Option<&str>, zone: Option<&str>) -> Result<Option<Local>, String> {
    if let Some(s) = setting.map(str::trim).filter(|s| !s.is_empty()) {
        return match exact(s).as_deref() {
            Some(&[code]) => Ok(Some(Local { code, from: "TALLY_CURRENCY".to_string() })),
            _ => Err(format!("TALLY_CURRENCY must be one currency, such as EUR or USD (it is `{s}`)")),
        };
    }
    let Some(zone) = zone else { return Ok(None) };
    let country = tz::countries()
        .iter()
        .find(|c| !c.currency.is_empty() && c.cities.iter().any(|city| tz::zones_of(city).is_some_and(|z| z.split('|').any(|z| z == zone))));
    Ok(country.map(|c| Local { code: c.currency.as_str(), from: format!("{}'s time zone, {zone}", c.name) }))
}

/// The device's time zone name: `TZ`, else where /etc/localtime points (`…/zoneinfo/Europe/Rome`).
fn local_zone() -> Option<String> {
    let after = |path: &str| path.split_once("zoneinfo/").map_or(path.to_string(), |(_, z)| z.to_string());
    if let Some(tz) = std::env::var("TZ").ok().map(|t| t.trim_start_matches(':').to_string()).filter(|t| t.contains('/')) {
        return Some(after(&tz));
    }
    let link = std::fs::read_link("/etc/localtime").ok()?;
    Some(after(&link.to_string_lossy()))
}

// ---------------------------------------------------------------- the rates

/// `~/.config/tally/currencies.json`.
pub fn path() -> Option<PathBuf> {
    crate::online::dir().map(|d| d.join("currencies.json"))
}

/// The stored rates; empty when there is no file.
pub fn load() -> Data {
    path().map(|p| crate::online::load_from(&p)).unwrap_or_default()
}

/// Its name in the file: `USD/EUR`, how many of it one US dollar buys.
fn rate_name(code: &str) -> String {
    format!("USD/{code}")
}

/// The sync's source: every rate, and their date, from one download.
pub fn source() -> Source {
    Source { names: vec![RATES_DATE.to_string()], fetch: Box::new(fetch) }
}

/// The rates from ExchangeRate-API (`curl`, 15 s at most). `None`: no answer.
fn fetch() -> Option<Vec<(String, f64)>> {
    if !crate::network::allowed() {
        return None;
    }
    let out = Command::new("curl").args(["-s", "--max-time", "15", URL]).output().ok()?;
    parse(&String::from_utf8_lossy(&out.stdout))
}

/// The rates in ExchangeRate-API's answer (`{"result":"success","time_last_update_unix":…,
/// "rates":{"USD":1,"EUR":0.92,…}}`); `None` unless it succeeded with rates and a date.
pub fn parse(body: &str) -> Option<Vec<(String, f64)>> {
    let doc = crate::json::parse(body).ok()?;
    if doc.get("result")?.str()? != "success" {
        return None;
    }
    let day = doc.get("time_last_update_unix")?.number()?;
    let mut out: Vec<(String, f64)> = doc
        .get("rates")?
        .members()
        .iter()
        .filter(|(code, _)| code.len() == 3 && code.chars().all(|c| c.is_ascii_uppercase()))
        .filter_map(|(code, v)| Some((rate_name(code), v.number().filter(|r| r.is_finite() && *r > 0.0)?)))
        .collect();
    if out.len() < 2 {
        return None;
    }
    out.push((RATES_DATE.to_string(), day));
    Some(out)
}

/// `tally --list`: the currencies.
pub fn list() -> String {
    let mut out = String::from("Currencies (code, name, other ways to write it; countries work too: `12 USA Europe`)\n");
    for c in all() {
        let other: Vec<&str> = c.words.iter().chain(&c.symbols).chain(&c.abbreviations).map(String::as_str).collect();
        out.push_str(format!("  {:<5} {}{}\n", c.code, c.name, if other.is_empty() { String::new() } else { format!(": {}", other.join(", ")) }).as_str());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::online::Stored;

    /// ExchangeRate-API's answer, abridged (USD base).
    const REPLY: &str = r#"{"result":"success","provider":"https://www.exchangerate-api.com","time_last_update_unix":1791763351,
        "base_code":"USD","rates":{"USD":1,"AED":3.6725,"CHF":0.8312,"CNY":7.1243,"EUR":0.8945,"GBP":0.7556,"JPY":158.6,
        "CAD":1.4262,"AUD":1.4339,"SEK":10.01,"NOK":9.566,"DKK":6.685,"ISK":121.9,"INR":95.77,"PKR":281.2,"MXN":18.34}}"#;
    /// 2026-10-12, about when those rates were published.
    const NOW: i64 = 1_791_800_000;

    fn rates() -> Data {
        let mut d = Data::default();
        d.values = parse(REPLY).unwrap().into_iter().map(|(n, v)| (n, Stored { value: v, at: NOW })).collect();
        d
    }

    fn with(input: &str, local: Option<&str>) -> Option<Result<Converted, Error>> {
        let rates = rates();
        let local = local.map(|code| Local { code: get(code).map(|c| c.code.as_str()).unwrap(), from: "Italy's time zone, Europe/Rome".into() });
        request(&Config::default(), input, &Context { rates: &rates, now: NOW, local })
    }

    fn ok(input: &str) -> String {
        match with(input, Some("EUR")) {
            Some(Ok(c)) => c.result,
            other => panic!("{input}: {other:?}"),
        }
    }

    fn ask(input: &str) -> Ambiguity {
        match with(input, Some("EUR")) {
            Some(Err(Error::Ambiguous(a))) => a,
            other => panic!("{input}: {other:?}"),
        }
    }

    #[test]
    fn the_table_is_complete() {
        assert!(all().len() >= 160);
        for c in all() {
            assert!(c.code.len() == 3 && c.code.chars().all(|x| x.is_ascii_uppercase()), "{}", c.code);
            assert!(!c.name.is_empty());
        }
        // Every country's currency is in the table.
        for country in tz::countries() {
            assert!(get(&country.currency).is_some(), "{}: {}", country.name, country.currency);
        }
    }

    #[test]
    fn every_way_to_write_one() {
        let one = |t: &str| exact(t).unwrap_or_else(|| panic!("{t}"));
        for t in ["usd", "USD", "Usd", "US dollar", "us dollars", "United States dollar", "US$", "USA", "United States", "america"] {
            assert_eq!(one(t), ["USD"], "{t}");
        }
        for t in ["eur", "euro", "Euros", "€", "Europe", "EU", "eurozone", "Italy", "germany"] {
            assert_eq!(one(t), ["EUR"], "{t}");
        }
        for t in ["AED", "dhs", "DHM", "dh", "UAE", "United Arab Emirates", "Emirati dirham"] {
            assert_eq!(one(t), ["AED"], "{t}");
        }
        assert_eq!(one("yen"), ["JPY"]);
        assert_eq!(one("rmb"), ["CNY"]);
        assert_eq!(one("China"), ["CNY"]);
        assert_eq!(one("reais"), ["BRL"]);
        assert_eq!(one("zł"), ["PLN"]);
        assert_eq!(one("Złoty"), ["PLN"]);
        assert_eq!(one("Swiss francs"), ["CHF"]);
        // Shared: the most used first.
        assert_eq!(&one("dollar")[..3], ["USD", "CAD", "AUD"]);
        assert_eq!(&one("$")[..3], ["USD", "CAD", "AUD"]);
        assert_eq!(one("¥"), ["JPY", "CNY"]);
        assert_eq!(one("kr"), ["SEK", "NOK", "DKK", "ISK"]);
        assert_eq!(&one("Rs")[..2], ["INR", "PKR"]);
        assert!(exact("km").is_none() && exact("Tokyo").is_none() && exact("").is_none());
    }

    #[test]
    fn conversions() {
        // 12 / 1 × 0.8945
        assert_eq!(ok("12 usd euro"), "10.73 EUR");
        assert_eq!(ok("12 USA Europe"), "10.73 EUR");
        assert_eq!(ok("12 USD UAE"), "44.07 AED");
        assert_eq!(ok("12 China Euro"), "1.51 EUR");
        assert_eq!(ok("12 us dollars to euros"), "10.73 EUR");
        assert_eq!(ok("12 United Arab Emirates in Japan"), "518.23 JPY");
        // Attached, symbols first or after (`$` alone is any dollar: asked).
        assert_eq!(ask("$12 €").choices[0].request, "12 USD EUR");
        assert_eq!(ok("US$12 €"), "10.73 EUR");
        assert_eq!(ok("12€ usd"), "13.42 USD");
        assert_eq!(ok("12usd eur"), "10.73 EUR");
        assert_eq!(ok("12.50EUR usd"), "13.97 USD");
        assert_eq!(ok("12dhs usd"), "3.27 USD");
        assert_eq!(ok("usd 12 eur"), "10.73 EUR");
    }

    #[test]
    fn one_currency_is_converted_to_the_local_one() {
        let c = with("12 usd", Some("EUR")).unwrap().unwrap();
        assert_eq!(c.result, "10.73 EUR");
        assert_eq!(c.notes[0], "to the local currency, euro (EUR): Italy's time zone, Europe/Rome");
        assert_eq!(ok("12 USA"), "10.73 EUR");
        // Already local, or no local currency: which one to?
        assert_eq!(with("12 eur", Some("EUR")).unwrap(), Err(Error::Usage("now the currency to convert to: `12 EUR USD`".into())));
        assert_eq!(with("12 usd", None).unwrap(), Err(Error::Usage("now the currency to convert to: `12 USD EUR`".into())));
    }

    #[test]
    fn shared_names_are_asked() {
        let a = ask("12 dollar euro");
        assert_eq!(a.kind, AmbiguityKind::Currency);
        assert_eq!(a.note, "`dollar` is several currencies");
        assert_eq!((a.choices[0].pattern.as_str(), a.choices[0].date.as_str(), a.choices[0].request.as_str()), ("US dollar", "USD", "12 USD EUR"));
        assert_eq!(a.choices[1].request, "12 CAD EUR");
        assert!(a.choices.len() <= MAX_CHOICES);
        assert!(a.message().contains("did you mean US dollar or Canadian dollar"), "{}", a.message());
        assert_eq!(ask("$12 ¥").choices[0].request, "12 USD JPY");
        assert_eq!(ask("12 kr eur").choices.iter().map(|c| c.date.as_str()).collect::<Vec<_>>(), ["SEK", "NOK", "DKK", "ISK"]);
        // The choices convert (those with a rate in the test data).
        for c in ask("12 dollar euro").choices.iter().take(3) {
            assert!(with(&c.request, Some("EUR")).unwrap().is_ok(), "{}", c.request);
        }
    }

    #[test]
    fn misspelt_names_are_confirmed() {
        let a = ask("12 dolar eur");
        assert_eq!(a.note, "`dolar` is not a currency");
        assert_eq!(a.choices[0].request, "12 USD EUR");
        assert_eq!(ask("12 usd yuro").choices[0].request, "12 USD EUR");
        let a = ask("12 Jappan usd");
        assert_eq!((a.choices[0].pattern.as_str(), a.choices[0].request.as_str()), ("Japan (country)", "12 JPY USD"));
        // A misspelt country alone is left to the time requests (`12 Tokio`).
        assert!(with("12 Tokio", Some("EUR")).is_none());
    }

    #[test]
    fn not_currency_requests() {
        for input in ["10 km mi", "now usa", "10:00AM USA Japan", "12 Sept 26 10:00 Italy Japan", "USA China", "12 pounds kg", "1 cup ml", "12 cup pound", "12 Rome", "12 + 3 usd eur"] {
            assert!(with(input, Some("EUR")).is_none(), "{input}");
        }
        // A word that is a unit too is money when the other one is: `pound` is several currencies,
        // sterling first (12 / 0.7556 × 0.8945).
        assert_eq!(ask("12 pounds euro").choices[0].request, "12 GBP EUR");
        assert_eq!(ok("12 GBP euro"), "14.21 EUR");
        assert!(matches!(with("12 cup usd", Some("EUR")), Some(Err(Error::NoRates(_)))), "the Cuban peso, not in the test rates");
    }

    #[test]
    fn wrong_shapes() {
        for input in ["12 usd eur gbp", "12 usd 5 eur"] {
            assert!(matches!(with(input, Some("EUR")), Some(Err(Error::Usage(_))) | None), "{input}");
        }
        assert_eq!(with("12 usd eur chf", Some("EUR")).unwrap(), Err(Error::Usage(SHAPE.into())));
    }

    #[test]
    fn rates_missing_or_stale() {
        let empty = Data::default();
        let ctx = Context { rates: &empty, now: NOW, local: None };
        assert!(matches!(request(&Config::default(), "1 usd eur", &ctx), Some(Err(Error::NoRates(_)))));
        let rates = rates();
        let ctx = Context { rates: &rates, now: NOW + 4 * 86_400, local: None };
        let c = request(&Config::default(), "1 usd eur", &ctx).unwrap().unwrap();
        assert_eq!(c.warnings, ["exchange rates of 12 Oct 26: not updated since"]);
        assert_eq!(c.notes, [format!("1 USD = 0.8945 EUR, rates of 12 Oct 26 from {PROVIDER}")]);
        // A currency without a rate.
        let ctx = Context { rates: &rates, now: NOW, local: None };
        assert_eq!(request(&Config::default(), "1 usd kpw", &ctx).unwrap(), Err(Error::NoRates("no exchange rate for the North Korean won (KPW)".into())));
    }

    #[test]
    fn parses_the_answer() {
        let rates = parse(REPLY).unwrap();
        assert!(rates.contains(&("USD/EUR".to_string(), 0.8945)));
        assert!(rates.contains(&(RATES_DATE.to_string(), 1_791_763_351.0)));
        assert_eq!(parse(r#"{"result":"error","error-type":"unknown-code"}"#), None);
        assert_eq!(parse("<html>busy</html>"), None);
    }

    #[test]
    fn local_currency() {
        let code = |s: Option<&str>, z: Option<&str>| local_from(s, z).unwrap().map(|l| l.code);
        assert_eq!(code(None, Some("Europe/Rome")), Some("EUR"));
        assert_eq!(code(None, Some("America/New_York")), Some("USD"));
        assert_eq!(code(None, Some("Asia/Tokyo")), Some("JPY"));
        assert_eq!(code(None, Some("Asia/Dubai")), Some("AED"));
        assert_eq!(code(None, Some("Nowhere/Else")), None);
        assert_eq!(code(None, None), None);
        assert_eq!(code(Some("chf"), Some("Europe/Rome")), Some("CHF"));
        assert_eq!(code(Some("Swiss franc"), None), Some("CHF"));
        assert!(local_from(Some("dollar"), None).is_err(), "several: not one currency");
        assert!(local_from(Some("bogus"), None).is_err());
        assert_eq!(local_from(None, Some("Europe/Rome")).unwrap().unwrap().from, "Italy's time zone, Europe/Rome");
    }

    /// No way of writing a currency means something else to tally, except the units listed here
    /// (decided by the other word: `12 pounds kg`, `12 pounds euro`).
    #[test]
    fn collisions() {
        const UNITS_TOO: [&str; 3] = ["cup", "pound", "pounds"];
        let config = Config::default();
        for c in all() {
            let forms = std::iter::once(c.code.clone())
                .chain(std::iter::once(c.name.clone()))
                .chain(c.words.iter().flat_map(|w| [w.clone(), format!("{w}s")]))
                .chain(c.symbols.iter().cloned())
                .chain(c.abbreviations.iter().cloned());
            for form in forms {
                for word in [form.clone(), form.to_lowercase(), form.to_uppercase()] {
                    let unit = crate::convert::find_unit(&config, &word).is_some();
                    assert!(!unit || UNITS_TOO.contains(&word.to_lowercase().as_str()), "{} `{word}` is a unit", c.code);
                    assert!(!crate::time::date_or_place_word(&config, &word), "{} `{word}` is a date or place word", c.code);
                    assert!(amount(&word).is_none(), "{} `{word}` is a number", c.code);
                }
            }
        }
    }
}
