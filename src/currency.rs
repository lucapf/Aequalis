//! Currencies: `12 usd euro`, `12 USA Europe`, `12 USD UAE`, `$12 €`, `12 dollars to yen`.
//!
//! A currency is written as its ISO code, its name (singular or plural), a symbol, a common
//! abbreviation (`src/data/currencies.json`), a country (its currency, `src/data/countries.json`)
//! or Europe / EU / Eurozone; case and accents don't matter, and the amount may be attached
//! (`$12`, `12€`, `12usd`). Always one amount: with a single currency the target is the local one
//! (the currency of the device time zone's country, or the `currency.local` setting). A name several
//! currencies share (`dollar`, `$`, `kr`) or a misspelt one (`dolar`) is asked about.
//!
//! The rates come from ExchangeRate-API (open.er-api.com, daily), kept in
//! `~/.config/aequalis/currencies.json` with the online data's policies (`online.rs`): synced in the
//! background when older than the refresh interval, never during a request.

use crate::config::Config;
use crate::online::{Data, Source};
use crate::time::tz;
use crate::time::{Ambiguity, AmbiguityKind, Choice};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

/// A currency, as `src/data/currencies.json` describes it.
#[derive(Debug)]
pub struct Currency {
    pub code: &'static str,
    pub name: &'static str,
    pub words: &'static [&'static str],
    pub symbols: &'static [&'static str],
    pub abbreviations: &'static [&'static str],
}

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
    CURRENCY_TABLE
}

/// The currency with ISO code `code`, in any case.
pub fn get(code: &str) -> Option<&'static Currency> {
    let i = lookup(index().codes, &code.to_ascii_uppercase())?;
    Some(&all()[usize::from(i)])
}

/// The countries using the currency with ISO code `code` (`Italy`… for EUR), in `tz::countries`
/// order.
pub fn users(code: &str) -> &'static [&'static tz::Country] {
    lookup(index().codes, &code.to_ascii_uppercase()).map_or(&[], |i| catalog().users[usize::from(i)].as_slice())
}

/// Whether one of the currency's names or words (as `tz::key` writes them), or its code, starts with
/// `typed` (as `tz::key` writes it): what `suggest` offers.
pub fn starts_with(currency: &Currency, typed: &str) -> bool {
    let code = currency.code.as_bytes().get(..typed.len()).is_some_and(|c| c.eq_ignore_ascii_case(typed.as_bytes()));
    code || lookup(index().codes, currency.code).is_some_and(|i| catalog().keys[usize::from(i)].iter().any(|k| k.starts_with(typed)))
}

// ---------------------------------------------------------------- what a word means

/// The ways currencies are written, normalised and sorted by `build.rs`: a lookup is a binary
/// search, nothing is built when the program starts.
struct Index {
    /// An ISO code (upper case) → its currency (position in `all()`).
    codes: &'static [(&'static str, u16)],
    /// A name or word (`tz::key`: `us dollar`, `dollar`) → the currencies with it, in order.
    names: &'static [(&'static str, &'static [u16])],
    abbreviations: &'static [(&'static str, &'static [u16])],
    /// A symbol, lower case (`€`, `us$`).
    symbols: &'static [(&'static str, &'static [u16])],
    /// Europe, EU… (`tz::key`).
    euro_area: &'static [&'static str],
    /// For misspellings: every name and word of 4 letters or more, squeezed, and its plural.
    forms: &'static [(&'static str, u16)],
}

// `CURRENCY_TABLE` and `INDEX`, written by `build.rs` (build/tables.rs).
include!(concat!(env!("OUT_DIR"), "/currencies.rs"));

fn index() -> &'static Index {
    &INDEX
}

/// The value of `wanted` in a table sorted by name (`build.rs`).
fn lookup<V: Copy>(table: &[(&str, V)], wanted: &str) -> Option<V> {
    table.binary_search_by(|(k, _)| (*k).cmp(wanted)).ok().map(|at| table[at].1)
}

/// What the lists and searches need besides (`users`, `starts_with`, `search`): built when one of
/// them is first asked for, which a request alone never does.
struct Catalog {
    /// By currency (as in `all()`): its name and words as `tz::key` writes them (prefixes typed).
    keys: Vec<Vec<String>>,
    /// By currency: the countries using it (`Italy` for EUR), in `tz::countries` order.
    users: Vec<Vec<&'static tz::Country>>,
    /// By currency: its texts in lower case, for `search`.
    lower: Vec<Lower>,
}

/// A currency's texts in lower case: what `search` compares a query with.
struct Lower {
    code: String,
    name: String,
    words: Vec<String>,
    symbols: Vec<String>,
    /// The names and other names of the countries using it.
    countries: Vec<String>,
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut users = vec![Vec::new(); all().len()];
        for country in tz::countries() {
            if let Some(i) = lookup(index().codes, country.currency) {
                users[usize::from(i)].push(country);
            }
        }
        let keys = all().iter().map(|c| std::iter::once(c.name).chain(c.words.iter().copied()).map(tz::key).collect()).collect();
        let lower = |texts: &[&str]| texts.iter().map(|t| t.to_lowercase()).collect::<Vec<_>>();
        let lower = all()
            .iter()
            .zip(&users)
            .map(|(c, users)| Lower {
                code: c.code.to_lowercase(),
                name: c.name.to_lowercase(),
                words: lower(c.words),
                symbols: lower(c.symbols),
                countries: users.iter().flat_map(|k| std::iter::once(k.name).chain(k.aliases.iter().copied())).map(str::to_lowercase).collect(),
            })
            .collect();
        Catalog { keys, users, lower }
    })
}

/// The currencies `text` names exactly, the most used first: one (`usd`, `euros`, `Japan`, `€`), or
/// several when they share it (`dollar`, `$`, `kr`); `None`: not a currency.
pub fn exact(text: &str) -> Option<Vec<&'static str>> {
    let raw = text.trim();
    if raw.is_empty() {
        return None;
    }
    let index = index();
    let key = tz::key(raw);
    let squeezed = key.replace(' ', "");
    // An ISO code: only that one; written as one word (`U.S.D.`), not split by spaces (`1 us d`
    // is microseconds to days).
    if squeezed.len() == 3 && !raw.contains(char::is_whitespace) && squeezed.chars().all(|c| c.is_ascii_alphabetic()) {
        if let Some(i) = lookup(index.codes, &squeezed.to_ascii_uppercase()) {
            return Some(vec![all()[usize::from(i)].code]);
        }
    }
    if index.euro_area.contains(&key.as_str()) {
        return Some(vec!["EUR"]);
    }
    if let Some(country) = tz::country(raw).filter(|c| !c.currency.is_empty()) {
        return Some(vec![country.currency]);
    }
    // `$` alone: any dollar.
    let (key, lower) = match raw {
        "$" => ("dollar".to_string(), "dollar".to_string()),
        _ => (key, raw.to_lowercase()),
    };
    // A name or word, or its plural (`dollars`); an abbreviation; a symbol.
    let singular = key.strip_suffix('s').unwrap_or("");
    let mut found: Vec<u16> = [lookup(index.names, &key), lookup(index.names, singular), lookup(index.abbreviations, &key), lookup(index.symbols, &lower)]
        .into_iter()
        .flatten()
        .flatten()
        .copied()
        .collect();
    found.sort_unstable();
    found.dedup();
    (!found.is_empty()).then(|| found.into_iter().map(|i| all()[usize::from(i)].code).collect())
}

/// A misspelt currency or country: the nearest names within 1 letter (2 for longer words), each
/// as (code, how to show it), and whether they are all countries; empty for a word that is exact
/// or too short.
pub(crate) fn near(text: &str) -> (Vec<(&'static str, String)>, bool) {
    #[derive(Clone, Copy)]
    enum Near {
        Currency(usize),
        Country(&'static tz::Country),
    }
    let wanted = tz::squeeze(text);
    let len = wanted.chars().count();
    if len < 4 || !wanted.chars().all(char::is_alphabetic) || exact(text).is_some() {
        return (Vec::new(), false);
    }
    let limit = if len <= 5 { 1 } else { 2 };
    let currencies = index().forms.iter().map(|&(k, i)| (k, Near::Currency(usize::from(i))));
    let countries = tz::country_keys().filter(|(_, c)| !c.currency.is_empty()).map(|(k, c)| (k, Near::Country(c)));
    let Some((_, found)) = tz::nearest(&wanted, currencies.chain(countries), limit) else { return (Vec::new(), false) };
    let only_countries = found.iter().all(|n| matches!(n, Near::Country(_)));
    let mut out: Vec<(&'static str, String)> = Vec::new();
    for n in found {
        let (code, label) = match n {
            Near::Currency(i) => (all()[i].code, all()[i].name.to_string()),
            Near::Country(c) => (c.currency, format!("{} (country)", c.name)),
        };
        if !out.iter().any(|(c, _)| *c == code) {
            out.push((code, label));
        }
    }
    (out, only_countries)
}

/// The country (or Europe) `text` names, when it names a currency that way: `USA` → United States.
fn place(text: &str) -> Option<String> {
    let squeezed = tz::key(text).replace(' ', "");
    if squeezed.len() == 3 && !text.trim().contains(char::is_whitespace) && get(&squeezed).is_some() {
        return None;
    }
    if index().euro_area.contains(&tz::key(text).as_str()) {
        return Some(text.trim().to_string());
    }
    tz::country(text).filter(|c| !c.currency.is_empty()).map(|c| c.name.to_string())
}

/// A currency for people to check: `US dollar (USD, US$)`, `euro (EUR, €)`, with the country it
/// was written as first: `United States: US dollar (USD, US$)`.
fn shown(code: &str, country: Option<&str>) -> String {
    let currency = match get(code) {
        Some(c) => match c.symbols.first() {
            Some(symbol) => format!("{} ({code}, {symbol})", c.name),
            None => format!("{} ({code})", c.name),
        },
        None => code.to_string(),
    };
    match country {
        Some(country) => format!("{country}: {currency}"),
        None => currency,
    }
}

/// When a country (or Europe) stands for a currency, the currencies used, to check the result:
/// `United States: US dollar (USD, US$) → United Arab Emirates: UAE dirham (AED, د.إ)`. `None`
/// when a side is unclear, or no country is named.
fn sides(money: &[Piece], local: Option<&'static str>) -> Option<String> {
    let mut out = Vec::new();
    let mut by_country = false;
    for piece in money {
        match piece {
            Piece::Currency { codes, country, .. } if codes.len() == 1 => {
                by_country |= country.is_some();
                out.push(shown(codes[0], country.as_deref()));
            }
            _ => return None,
        }
    }
    if out.len() == 1 {
        out.push(shown(local?, None));
    }
    by_country.then(|| out.join(" → "))
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

/// An amount written as a calculation, at the start and followed by words without digits (money,
/// maybe): `12+3 Italy UAE` → (`12+3`, `Italy UAE`), `(10 + 5) * 2 usd eur`. As a plain amount,
/// its value is a number not below 0. None for a plain number (`amount` takes it), for a
/// calculation alone, or one followed by more numbers (`2026/12/04 + 5wd`, `12 + 3 eur 5`).
fn calculated(input: &str) -> Option<(String, &str)> {
    let (expr, rest) = crate::calc::parse_prefix(input).ok()?;
    if expr.operations() == 0 || rest.trim().is_empty() || rest.contains(|c: char| c.is_ascii_digit()) {
        return None;
    }
    expr.eval().ok().filter(|v| v.is_finite() && *v >= 0.0)?;
    Some((input[..input.len() - rest.len()].trim().to_string(), rest))
}

/// An amount written together with its currency: `$12`, `US$12`, `12€`, `12usd`, `12.50EUR`.
fn split_attached(word: &str) -> Vec<String> {
    // The number first: digits and a point, then an exponent (`1e3USD`; `1EUR` is one euro).
    let digits = |s: &str| s.char_indices().take_while(|(_, c)| c.is_ascii_digit() || *c == '.').last().map_or(0, |(i, c)| i + c.len_utf8());
    let head = digits(word);
    let head = match word[head..].strip_prefix(['e', 'E']) {
        Some(after) => {
            let sign = usize::from(after.starts_with(['+', '-']));
            let exponent = after[sign..].bytes().take_while(u8::is_ascii_digit).count();
            if exponent > 0 && head > 0 {
                head + 1 + sign + exponent
            } else {
                head
            }
        }
        None => head,
    };
    if head > 0 && head < word.len() && amount(&word[..head]).is_some() && exact(&word[head..]).is_some() {
        return vec![word[..head].to_string(), word[head..].to_string()];
    }
    // A symbol and the next currency, no space between (`12€gbp`).
    if head > 0 && head < word.len() && amount(&word[..head]).is_some() && !word[head..].starts_with(char::is_alphanumeric) {
        let rest = &word[head..];
        if let Some(cut) = rest.char_indices().skip(1).map(|(i, _)| i).find(|&i| exact(&rest[..i]).is_some() && exact(&rest[i..]).is_some()) {
            return vec![word[..head].to_string(), rest[..cut].to_string(), rest[cut..].to_string()];
        }
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
    /// `to`, `in`, `->`.
    Connector,
    /// A currency or country, as written, and what it may be (one or more codes); `country`: the
    /// country (or Europe) it was written as, if so.
    Currency {
        text: String,
        codes: Vec<&'static str>,
        unit_too: bool,
        country: Option<String>,
    },
    /// A misspelt one: what it may be (code, how to show it).
    Near {
        text: String,
        options: Vec<(&'static str, String)>,
        countries_only: bool,
    },
    Other,
}

/// A word between what is converted and what it is converted to: `to`, `in`, `->`.
pub(crate) fn connector(word: &str) -> bool {
    ["to", "in", "->"].iter().any(|c| word.eq_ignore_ascii_case(c))
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
        if connector(w) {
            out.push(Piece::Connector);
            i += 1;
            continue;
        }
        let span = (1..=longest.min(words.len() - i)).rev().find_map(|n| {
            let text = words[i..i + n].join(" ");
            // A span stops at an amount or a connector.
            if words[i..i + n].iter().skip(1).any(|w| amount(w).is_some() || connector(w)) {
                return None;
            }
            exact(&text).map(|codes| (n, text, codes))
        });
        if let Some((n, text, codes)) = span {
            let unit_too = crate::convert::find_unit(config, &text).is_some();
            let country = place(&text);
            out.push(Piece::Currency { text, codes, unit_too, country });
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
    /// The amount and the currency it is in, as shown (`12 AED`): the expression's left side.
    pub from: String,
    /// How old the rates are, for people (`2d`: `age`).
    pub age: String,
    /// When countries stand for currencies, the currencies used (`describe`): shown always.
    pub check: Option<String>,
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
    /// Not finished yet: the currency to convert to comes next (`12 usd` with no local currency).
    Incomplete(String),
}

/// The local currency: the `currency.local` setting, else the currency of the device time zone's country; with
/// what it comes from, for the note.
pub struct Local {
    pub code: &'static str,
    pub from: String,
}

/// `request` reads these, so tests can set them.
pub struct Context {
    pub rates: Arc<Data>,
    pub now: i64,
    pub local: Option<Local>,
}

/// The request's parts, when it is a currency request: the amount, and the currency pieces in order.
fn shape(config: &Config, input: &str) -> Option<Result<(String, Vec<Piece>), Error>> {
    let parts = match calculated(input) {
        // A calculation for the amount (`12+3 Italy UAE`): the money as for its value (`15 Italy
        // UAE`), the calculation kept as written (`convert` evaluates it; questions repeat it).
        Some((written, rest)) => std::iter::once(Piece::Amount(written)).chain(pieces(config, rest)).collect::<Vec<_>>(),
        None => {
            // One amount, and an amount has digits while no currency or country name has any (the
            // test `names_have_no_digits` keeps it so): without exactly one word with digits it is
            // no currency request, and nothing is looked up (`now Tokyo`, `Rome Paris`).
            if input.split_whitespace().filter(|w| w.contains(|c: char| c.is_ascii_digit())).count() != 1 {
                return None;
            }
            pieces(config, input)
        }
    };
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
/// `context` is asked for only for a currency request (it reads the rates).
pub fn request(config: &Config, input: &str, context: impl FnOnce() -> Context) -> Option<Result<Converted, Error>> {
    let (amount, money) = match shape(config, input)? {
        Ok(s) => s,
        Err(e) => return Some(Err(e)),
    };
    Some(convert(&amount, &money, &context()))
}

/// The options of a currency piece: (code, how to show it).
fn options(piece: &Piece) -> Vec<(&'static str, String)> {
    match piece {
        Piece::Currency { codes, .. } => codes.iter().map(|&c| (c, get(c).map_or(c.to_string(), |x| x.name.to_string()))).collect(),
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
            return Err(Error::Incomplete(format!("now the currency to convert to: `{amount} {from} {other}`")));
        }
    };
    let value: f64 = amount.parse().ok().or_else(|| crate::calc::parse_prefix(amount).ok().and_then(|(e, _)| e.eval().ok())).unwrap_or(0.0);
    let Some(day) = ctx.rates.get(RATES_DATE) else {
        return Err(Error::NoRates(match crate::network::allowed() {
            true => "no exchange rates yet: they are being downloaded in the background, try again in a minute".to_string(),
            false => "no exchange rates: they were never downloaded, and the offline setting is on".to_string(),
        }));
    };
    let (a, b) = match (per_dollar(&ctx.rates, from), per_dollar(&ctx.rates, to)) {
        (Some(a), Some(b)) => (a, b),
        (None, _) => return Err(Error::NoRates(format!("no exchange rate for the {} ({from})", name(from)))),
        (_, None) => return Err(Error::NoRates(format!("no exchange rate for the {} ({to})", name(to)))),
    };
    let result = value / a * b;
    let day = day.value as i64;
    let shown = crate::time::human_day(day);
    notes.push(format!("1 {from} = {} {to}, rates of {shown} from {PROVIDER}", crate::convert::format_value(b / a, 6)));
    let mut warnings = Vec::new();
    if ctx.now - day >= STALE_DAYS * 86_400 {
        warnings.push(format!("exchange rates of {shown}: not updated since"));
    }
    // Countries written for currencies: which ones were used, to check.
    let check = sides(money, Some(to));
    let result = format!("{} {to}", crate::convert::fixed(result, 2));
    Ok(Converted { result, from: format!("{amount} {from}"), age: age(ctx.now - day), check, notes, warnings })
}

/// An age for people, in its largest whole unit: `less than 1h`, `5h`, `2d`, `3w`, `4m`, `1y`.
pub fn age(seconds: i64) -> String {
    let s = seconds.max(0);
    match s {
        0..3_600 => "less than 1h".to_string(),
        3_600..86_400 => format!("{}h", s / 3_600),
        86_400..604_800 => format!("{}d", s / 86_400),
        604_800..2_592_000 => format!("{}w", s / 604_800),
        2_592_000..31_557_600 => format!("{}m", s / 2_592_000),
        _ => format!("{}y", s / 31_557_600),
    }
}

/// Stores rates from an answer of ExchangeRate-API (`parse`), the ones shipped with an app: only
/// when they are newer than the rates already there (a sync's are kept). How many rates were
/// stored (0: the ones there are as new or newer).
pub fn store(body: &str, now: i64) -> Result<usize, String> {
    store_in(&path().ok_or("no data folder")?, body, now)
}

/// `store`, into the rates file at `path`.
fn store_in(path: &std::path::Path, body: &str, now: i64) -> Result<usize, String> {
    let rates = parse(body).ok_or("the exchange rates are not ExchangeRate-API's answer")?;
    let day = |data: &Data| data.get(RATES_DATE).map(|s| s.value);
    let mut data = crate::online::load_from(path);
    let shipped = rates.iter().find(|(n, _)| n == RATES_DATE).map(|(_, v)| *v);
    if day(&data).zip(shipped).is_some_and(|(there, shipped)| there >= shipped) {
        return Ok(0);
    }
    for (name, value) in &rates {
        data.set(name, *value, now);
    }
    // Synced at their own date, not now: their age (and when to sync again) follows from it.
    data.synced_at = shipped.map(|d| d as i64);
    data.attempted_at = Some(now);
    data.ok = true;
    crate::online::save_to(path, &data).map_err(|e| e.to_string())?;
    Ok(rates.len())
}

fn name(code: &str) -> String {
    get(code).map_or(code.to_string(), |c| c.name.to_string())
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
            // Only what was unclear is replaced: `12 dollar UAE` → `12 USD UAE`.
            let written = combo.iter().zip(money).map(|((code, _), piece)| match piece {
                Piece::Currency { text, codes, .. } if codes.len() == 1 => text.clone(),
                _ => code.to_string(),
            });
            let request = std::iter::once(amount.to_string()).chain(written).collect::<Vec<_>>().join(" ");
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

/// The local currency: the `currency.local` setting (`Err` when it is not one currency), else the one of the
/// country of the device's time zone (`Europe/Rome` → Italy → EUR); `None` when unknown.
pub fn local() -> Result<Option<Local>, String> {
    local_from(crate::settings::current().currency(), crate::time::device_zone())
}

pub fn local_from(setting: Option<&str>, zone: Option<&str>) -> Result<Option<Local>, String> {
    if let Some(s) = setting.map(str::trim).filter(|s| !s.is_empty()) {
        return match exact(s).as_deref() {
            Some(&[code]) => Ok(Some(Local { code, from: "the currency.local setting".to_string() })),
            _ => Err(format!("`currency.local` must be one currency, such as EUR or USD (it is `{s}`)")),
        };
    }
    let Some(zone) = zone else { return Ok(None) };
    let country = tz::zone_country(zone).filter(|c| !c.currency.is_empty());
    Ok(country.map(|c| Local { code: c.currency, from: format!("{}'s time zone, {zone}", c.name) }))
}

// ---------------------------------------------------------------- the rates

/// `~/.config/aequalis/currencies.json`.
pub fn path() -> Option<PathBuf> {
    crate::files::dir().map(|d| d.join("currencies.json"))
}

/// The stored rates; empty when there is no file. Read again only when the file changes (a
/// background sync): a long-running interactive mode sees new rates, a request reads them once.
pub fn load() -> Arc<Data> {
    static RATES: crate::files::Cached<Data> = crate::files::Cached::new();
    path().and_then(|path| RATES.get(&path, || Some(crate::online::load_from(&path)))).unwrap_or_default()
}

/// How many of `code` one US dollar buys, from the stored rates.
fn per_dollar(rates: &Data, code: &str) -> Option<f64> {
    if code == "USD" {
        Some(1.0)
    } else {
        rates.get(&rate_name(code)).map(|s| s.value)
    }
}

/// How many `to` one `from` buys, from the stored `rates`; `None` when either has no rate.
pub fn rate(rates: &Data, from: &str, to: &str) -> Option<f64> {
    Some(per_dollar(rates, to)? / per_dollar(rates, from)?)
}

/// Its name in the file: `USD/EUR`, how many of it one US dollar buys.
fn rate_name(code: &str) -> String {
    format!("USD/{code}")
}

/// The sync's source: every rate, and their date, from one download.
pub fn source() -> Source {
    Source { names: vec![RATES_DATE.to_string()], fetch: Box::new(fetch) }
}

/// The rates from ExchangeRate-API (15 s at most). `None`: no answer.
fn fetch() -> Option<Vec<(String, f64)>> {
    if !crate::network::allowed() {
        return None;
    }
    parse(&String::from_utf8_lossy(&crate::http::get(URL, 15, false)?))
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

/// `aequalis --list`: the currencies.
pub fn list() -> String {
    let mut out = String::from("Currencies (code, name, other ways to write it; countries work too: `12 USA Europe`)\n");
    for c in all() {
        let other: Vec<&str> = c.words.iter().chain(c.symbols).chain(c.abbreviations).copied().collect();
        out.push_str(format!("  {:<5} {}{}\n", c.code, c.name, if other.is_empty() { String::new() } else { format!(": {}", other.join(", ")) }).as_str());
    }
    out
}

/// The currencies matching `query` (case aside), best first: the code or a symbol equal to it,
/// then a code that starts with it, then a name with a word that starts with it, then a country
/// using it with a word that does (`italy` → EUR, `usa` → USD, as requests take countries), then
/// any that contains it. All of them for an empty query, the most used first.
pub fn search(query: &str) -> Vec<&'static Currency> {
    let q = query.trim().to_lowercase();
    let starts_a_word = |text: &str| text.split(|ch: char| !ch.is_alphanumeric()).any(|w| w.starts_with(&q));
    let mut scored: Vec<(usize, usize, &Currency)> = Vec::new();
    for (order, (c, l)) in all().iter().zip(&catalog().lower).enumerate() {
        let score = if q.is_empty() || l.code == q || l.symbols.contains(&q) {
            Some(0)
        } else if l.code.starts_with(&q) {
            Some(1)
        } else if starts_a_word(&l.name) {
            Some(2)
        } else if l.countries.iter().any(|n| starts_a_word(n)) {
            Some(3)
        } else if l.name.contains(&q) || l.words.iter().any(|w| w.contains(&q)) || l.countries.iter().any(|n| n.contains(&q)) {
            Some(4)
        } else {
            None
        };
        if let Some(s) = score {
            scored.push((s, order, c));
        }
    }
    scored.sort_by_key(|&(s, o, _)| (s, o));
    scored.into_iter().map(|(_, _, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_symbol_amount_and_the_next_currency_together() {
        assert_eq!(split_attached("12€gbp"), ["12", "€", "gbp"]);
        assert_eq!(split_attached("12€"), ["12", "€"]);
        assert_eq!(split_attached("12usd"), ["12", "usd"]);
        assert_eq!(split_attached("12€xyz"), ["12€xyz"]);
    }

    #[test]
    fn built_tables_match_their_data() {
        // `build.rs` wrote them: the currencies of `currencies.json`, every way of writing them found.
        let doc = crate::json::parse(include_str!("data/currencies.json")).unwrap();
        let data = doc.get("currencies").unwrap().array();
        assert_eq!(all().len(), data.len());
        for (c, d) in all().iter().zip(data) {
            let list = |k: &str| d.get(k).map(|v| v.strings()).unwrap_or_default();
            assert_eq!((c.code, c.name), (d.get("code").and_then(|v| v.str()).unwrap(), d.get("name").and_then(|v| v.str()).unwrap()));
            let owned = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
            assert_eq!((owned(c.words), owned(c.symbols), owned(c.abbreviations)), (list("words"), list("symbols"), list("abbreviations")));
            assert_eq!(get(&c.code.to_lowercase()).map(|g| g.code), Some(c.code));
            for form in std::iter::once(c.name).chain(c.words.iter().copied()).chain(c.abbreviations.iter().copied()).chain(c.symbols.iter().copied()) {
                assert!(exact(form).is_some_and(|codes| codes.contains(&c.code)), "{}: {form}", c.code);
            }
        }
        let index = index();
        for table in [index.names, index.abbreviations, index.symbols] {
            assert!(table.windows(2).all(|w| w[0].0 < w[1].0), "sorted, once each");
        }
    }

    #[test]
    fn countries_using_a_currency() {
        let names = |code: &str| users(code).iter().map(|c| c.name).collect::<Vec<_>>();
        assert!(names("EUR").contains(&"Italy") && names("eur").contains(&"France"), "any case: {:?}", names("eur"));
        assert_eq!(names("JPY"), ["Japan"]);
        assert!(users("XXX").is_empty());
    }
    use crate::online::Stored;

    /// ExchangeRate-API's answer, abridged (USD base).
    const REPLY: &str = r#"{"result":"success","provider":"https://www.exchangerate-api.com","time_last_update_unix":1791763351,
        "base_code":"USD","rates":{"USD":1,"AED":3.6725,"CHF":0.8312,"CNY":7.1243,"EUR":0.8945,"GBP":0.7556,"JPY":158.6,
        "CAD":1.4262,"AUD":1.4339,"SEK":10.01,"NOK":9.566,"DKK":6.685,"ISK":121.9,"INR":95.77,"PKR":281.2,"MXN":18.34}}"#;
    /// 2026-10-12, about when those rates were published.
    const NOW: i64 = 1_791_800_000;

    fn rates() -> Data {
        Data { values: parse(REPLY).unwrap().into_iter().map(|(n, v)| (n, Stored { value: v, at: NOW })).collect(), ..Data::default() }
    }

    fn with(input: &str, local: Option<&str>) -> Option<Result<Converted, Error>> {
        let rates = rates();
        let local = local.map(|code| Local { code: get(code).map(|c| c.code).unwrap(), from: "Italy's time zone, Europe/Rome".into() });
        request(&Config::default(), input, || Context { rates: Arc::new(rates), now: NOW, local })
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

    /// No currency or country name has a digit: `shape` tells an amount by its digits.
    #[test]
    fn names_have_no_digits() {
        let digit = |w: &&str| w.contains(|c: char| c.is_ascii_digit());
        for c in all() {
            let words = std::iter::once(&c.name).chain(c.words).chain(c.symbols).chain(c.abbreviations);
            assert!(!words.clone().any(digit), "{}: {:?}", c.code, words.collect::<Vec<_>>());
        }
        for c in tz::countries() {
            assert!(!std::iter::once(&c.name).chain(c.aliases).any(digit), "{}", c.name);
        }
        assert!(!index().euro_area.iter().any(|r| r.contains(|c: char| c.is_ascii_digit())));
    }

    /// Rates shipped with an app: stored when newer than the ones there, else kept.
    #[test]
    fn shipped_rates() {
        let dir = std::env::temp_dir().join(format!("aequalis-shipped-rates-{}", std::process::id()));
        let path = dir.join("currencies.json");
        let answer = |day: i64| format!(r#"{{"result":"success","time_last_update_unix":{day},"rates":{{"USD":1,"EUR":0.9,"JPY":150}}}}"#);
        assert_eq!(store_in(&path, &answer(1_000), 5_000), Ok(4), "three rates and their date");
        let data = crate::online::load_from(&path);
        assert_eq!((data.get("USD/EUR").map(|s| s.value), data.synced_at), (Some(0.9), Some(1_000)));
        assert_eq!(store_in(&path, &answer(900), 6_000), Ok(0), "older: the ones there are kept");
        assert_eq!(store_in(&path, &answer(2_000), 7_000), Ok(4), "newer: stored");
        assert!(store_in(&path, "not rates", 8_000).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An age for people, in its largest whole unit.
    #[test]
    fn ages() {
        for (seconds, shown) in [
            (0, "less than 1h"),
            (3_599, "less than 1h"),
            (3_600, "1h"),
            (86_399, "23h"),
            (86_400, "1d"),
            (6 * 86_400, "6d"),
            (7 * 86_400, "1w"),
            (29 * 86_400, "4w"),
            (30 * 86_400, "1m"),
            (364 * 86_400, "12m"),
            (366 * 86_400, "1y"),
            (-5, "less than 1h"),
        ] {
            assert_eq!(age(seconds), shown, "{seconds}");
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
            assert!(get(country.currency).is_some(), "{}: {}", country.name, country.currency);
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
        assert_eq!(ok("12 usd -> eur"), "10.73 EUR");
        // Attached, symbols first or after (`$` alone is any dollar: asked).
        assert_eq!(ask("$12 €").choices[0].request, "12 USD €");
        assert_eq!(ok("US$12 €"), "10.73 EUR");
        assert_eq!(ok("12€ usd"), "13.42 USD");
        assert_eq!(ok("12usd eur"), "10.73 EUR");
        // The amount in scientific notation, attached too; `1EUR` is one euro, not `1E` and a word.
        assert_eq!(ok("1e3usd eur"), ok("1000 usd eur"));
        assert_eq!(ok("1E+3USD eur"), ok("1000 usd eur"));
        assert_eq!(ok("1EUR usd"), ok("1 eur usd"));
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
        assert_eq!(with("12 eur", Some("EUR")).unwrap(), Err(Error::Incomplete("now the currency to convert to: `12 EUR USD`".into())));
        assert_eq!(with("12 usd", None).unwrap(), Err(Error::Incomplete("now the currency to convert to: `12 USD EUR`".into())));
    }

    /// Countries written for currencies: the currencies used, with codes and symbols, to check.
    #[test]
    fn countries_show_their_currencies() {
        let check = |input: &str| with(input, Some("EUR")).unwrap().unwrap().check;
        assert_eq!(check("12 USA UAE").as_deref(), Some("United States: US dollar (USD, US$) → United Arab Emirates: UAE dirham (AED, د.إ)"));
        assert_eq!(check("12 usd UAE").as_deref(), Some("US dollar (USD, US$) → United Arab Emirates: UAE dirham (AED, د.إ)"));
        assert_eq!(check("12 Europe China").as_deref(), Some("Europe: euro (EUR, €) → China: Chinese yuan (CNY, ¥)"));
        // One country: to the local currency.
        assert_eq!(check("12 USA").as_deref(), Some("United States: US dollar (USD, US$) → euro (EUR, €)"));
        // No country: nothing to check.
        assert_eq!(check("12 usd eur"), None);
        // An asked choice keeps the country: `12 USD uae`.
        assert_eq!(ask("12 dollar uae").choices[0].request, "12 USD uae");
        assert_eq!(
            sides(&pieces(&Config::default(), "12 USD uae")[1..], None).as_deref(),
            Some("US dollar (USD, US$) → United Arab Emirates: UAE dirham (AED, د.إ)")
        );
    }

    #[test]
    fn shared_names_are_asked() {
        let a = ask("12 dollar euro");
        assert_eq!(a.kind, AmbiguityKind::Currency);
        assert_eq!(a.note, "`dollar` is several currencies");
        assert_eq!((a.choices[0].pattern.as_str(), a.choices[0].date.as_str(), a.choices[0].request.as_str()), ("US dollar", "USD", "12 USD euro"));
        assert_eq!(a.choices[1].request, "12 CAD euro");
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
        assert_eq!(a.choices[0].request, "12 USD eur");
        assert_eq!(ask("12 usd yuro").choices[0].request, "12 usd EUR");
        let a = ask("12 Jappan usd");
        assert_eq!((a.choices[0].pattern.as_str(), a.choices[0].request.as_str()), ("Japan (country)", "12 JPY usd"));
        // A misspelt country alone is left to the time requests (`12 Tokio`).
        assert!(with("12 Tokio", Some("EUR")).is_none());
    }

    #[test]
    fn not_currency_requests() {
        for input in [
            "10 km mi",
            "now usa",
            "10:00AM USA Japan",
            "12 Sept 26 10:00 Italy Japan",
            "USA China",
            "12 pounds kg",
            "1 cup ml",
            "12 cup pound",
            "12 Rome",
            // Calculations with something else than money.
            "12 + 3 km mi",
            "2026/12/04 + 5wd",
            "12 + 3 eur 5",
            "12 + 3",
        ] {
            assert!(with(input, Some("EUR")).is_none(), "{input}");
        }
        // A word that is a unit too is money when the other one is: `pound` is several currencies,
        // sterling first (12 / 0.7556 × 0.8945).
        assert_eq!(ask("12 pounds euro").choices[0].request, "12 GBP euro");
        assert_eq!(ok("12 GBP euro"), "14.21 EUR");
        assert!(matches!(with("12 cup usd", Some("EUR")), Some(Err(Error::NoRates(_)))), "the Cuban peso, not in the test rates");
    }

    /// A calculation for the amount, as its value: `12+3 Italy UAE` is `15 Italy UAE`.
    #[test]
    fn calculated_amounts() {
        // 15 / 0.8945 × 3.6725, with the test rates.
        assert_eq!(ok("12+3 Italy UAE"), "61.58 AED");
        assert_eq!(ok("12+3 Italy UAE"), ok("15 Italy UAE"));
        assert_eq!(ok("12 + 3 eur to aed"), ok("15 eur aed"));
        assert_eq!(ok("(10+5)*2 usd eur"), ok("30 usd eur"));
        assert_eq!(ok("100 + 20% eur usd"), ok("120 eur usd"));
        // To the local currency, and the hint for the one to convert to: the calculation as written.
        assert_eq!(ok("12+3 usd"), ok("15 usd"));
        match with("12+3 eur", Some("EUR")) {
            Some(Err(Error::Incomplete(hint))) => assert_eq!(hint, "now the currency to convert to: `12+3 EUR USD`"),
            other => panic!("{other:?}"),
        }
        // A question's requests keep it too.
        assert_eq!(ask("12+3 pounds euro").choices[0].request, "12+3 GBP euro");
        // Below 0, as a plain amount: no money request.
        assert!(with("3-5 eur usd", Some("EUR")).is_none());
    }

    #[test]
    fn wrong_shapes() {
        for input in ["12 usd eur gbp", "12 usd 5 eur"] {
            assert!(matches!(with(input, Some("EUR")), Some(Err(Error::Usage(_))) | None), "{input}");
        }
        assert_eq!(with("12 usd eur chf", Some("EUR")).unwrap(), Err(Error::Usage(SHAPE.into())));
    }

    #[test]
    fn rate_between_two_currencies() {
        let r = rates();
        assert_eq!(rate(&r, "USD", "EUR"), Some(0.8945));
        assert_eq!(rate(&r, "EUR", "USD"), Some(1.0 / 0.8945));
        assert_eq!(rate(&r, "EUR", "EUR"), Some(1.0));
        assert_eq!(rate(&r, "EUR", "JPY"), Some(158.6 / 0.8945));
        assert_eq!(rate(&r, "EUR", "KPW"), None, "no rate for one of them");
        assert_eq!(rate(&Data::default(), "USD", "EUR"), None, "no rates at all");
    }

    #[test]
    fn rates_missing_or_stale() {
        let ctx = |rates: Data, now: i64| move || Context { rates: Arc::new(rates), now, local: None };
        assert!(matches!(request(&Config::default(), "1 usd eur", ctx(Data::default(), NOW)), Some(Err(Error::NoRates(_)))));
        let c = request(&Config::default(), "1 usd eur", ctx(rates(), NOW + 4 * 86_400)).unwrap().unwrap();
        assert_eq!(c.warnings, ["exchange rates of 12 Oct 26: not updated since"]);
        assert_eq!(c.notes, [format!("1 USD = 0.8945 EUR, rates of 12 Oct 26 from {PROVIDER}")]);
        // A currency without a rate.
        assert_eq!(
            request(&Config::default(), "1 usd kpw", ctx(rates(), NOW)).unwrap(),
            Err(Error::NoRates("no exchange rate for the North Korean won (KPW)".into()))
        );
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

    /// No way of writing a currency means something else to aequalis, except the units listed here
    /// (decided by the other word: `12 pounds kg`, `12 pounds euro`).
    #[test]
    fn collisions() {
        const UNITS_TOO: [&str; 3] = ["cup", "pound", "pounds"];
        let config = Config::default();
        for c in all() {
            let forms = std::iter::once(c.code.to_string())
                .chain(std::iter::once(c.name.to_string()))
                .chain(c.words.iter().flat_map(|w| [w.to_string(), format!("{w}s")]))
                .chain(c.symbols.iter().map(|s| s.to_string()))
                .chain(c.abbreviations.iter().map(|a| a.to_string()));
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

    fn codes(q: &str) -> Vec<&'static str> {
        search(q).iter().map(|c| c.code).collect()
    }

    #[test]
    fn best_first() {
        assert_eq!(codes("eur")[0], "EUR");
        assert_eq!(codes("€")[0], "EUR", "a symbol");
        assert_eq!(codes("yen")[0], "JPY");
        assert!(codes("dollar").contains(&"USD") && codes("dollar").contains(&"CAD"));
        assert!(codes("zzzz").is_empty());
        assert_eq!(codes("").len(), all().len());
    }

    #[test]
    fn by_country() {
        assert_eq!(codes("italy"), ["EUR"]);
        assert_eq!(codes("usa")[0], "USD", "another name of the United States");
        assert!(codes("united").contains(&"AED") && codes("united").contains(&"GBP"));
        // The francs by their names first, the euro after them (France).
        let franc = codes("franc");
        let euro = franc.iter().position(|&c| c == "EUR").unwrap();
        assert!(franc[..euro].contains(&"CHF"), "{franc:?}");
    }
}
