//! Currencies: `100 USD EUR`, `100 usd to eur`. Always one amount and two currencies, written as
//! ISO codes in any case. The rates are the European Central Bank's euro reference rates
//! (published on working days around 16:00 CET), kept with the other online data
//! (`online.rs`: `EUR/USD` is how many dollars one euro buys); a conversion reads only that file,
//! never the network.

use crate::online::{Data, Source};
use std::process::Command;

/// The currencies with an ECB reference rate, and their names.
pub const CURRENCIES: [(&str, &str); 30] = [
    ("EUR", "euro"),
    ("USD", "US dollar"),
    ("JPY", "Japanese yen"),
    ("CZK", "Czech koruna"),
    ("DKK", "Danish krone"),
    ("GBP", "pound sterling"),
    ("HUF", "Hungarian forint"),
    ("PLN", "Polish zloty"),
    ("RON", "Romanian leu"),
    ("SEK", "Swedish krona"),
    ("CHF", "Swiss franc"),
    ("ISK", "Icelandic krona"),
    ("NOK", "Norwegian krone"),
    ("TRY", "Turkish lira"),
    ("AUD", "Australian dollar"),
    ("BRL", "Brazilian real"),
    ("CAD", "Canadian dollar"),
    ("CNY", "Chinese yuan renminbi"),
    ("HKD", "Hong Kong dollar"),
    ("IDR", "Indonesian rupiah"),
    ("ILS", "Israeli shekel"),
    ("INR", "Indian rupee"),
    ("KRW", "South Korean won"),
    ("MXN", "Mexican peso"),
    ("MYR", "Malaysian ringgit"),
    ("NZD", "New Zealand dollar"),
    ("PHP", "Philippine peso"),
    ("SGD", "Singapore dollar"),
    ("THB", "Thai baht"),
    ("ZAR", "South African rand"),
];

/// The day the stored rates are from (Unix seconds, midnight UTC), stored with them.
pub const RATES_DATE: &str = "ECB rates date";
const URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";
/// Rates older than this are stale: the ECB skips weekends and holidays, never a whole week.
const STALE_DAYS: i64 = 5;
const SHAPE: &str = "a currency conversion is one amount and two currencies: `100 USD EUR` (`tally --list` lists them)";

/// The currency `word` names, in any case: `usd`, `Usd` → `USD`.
pub fn code(word: &str) -> Option<&'static str> {
    CURRENCIES.iter().map(|(c, _)| *c).find(|c| c.eq_ignore_ascii_case(word))
}

/// Its name in the online data: `EUR/USD`.
fn rate_name(code: &str) -> String {
    format!("EUR/{code}")
}

/// The sync's source: every rate, and their day, from one download.
pub fn source() -> Source {
    let mut names: Vec<String> = CURRENCIES.iter().skip(1).map(|(c, _)| rate_name(c)).collect();
    names.push(RATES_DATE.to_string());
    Source { names, fetch: Box::new(fetch) }
}

/// The ECB's daily XML (`curl`, 15 s at most). `None`: no answer (offline, or not the rates).
fn fetch() -> Option<Vec<(String, f64)>> {
    if !crate::network::allowed() {
        return None;
    }
    let out = Command::new("curl").args(["-s", "--max-time", "15", URL]).output().ok()?;
    parse(&String::from_utf8_lossy(&out.stdout))
}

/// The rates in the ECB's XML: `<Cube time='2026-10-12'>` and `<Cube currency='USD' rate='1.1181'/>`.
/// `None` unless it has the day and every currency.
pub fn parse(xml: &str) -> Option<Vec<(String, f64)>> {
    let attr = |tag: &str, name: &str| -> Option<String> {
        let start = tag.find(&format!("{name}="))? + name.len() + 1;
        let quote = tag[start..].chars().next()?;
        let rest = &tag[start + 1..];
        Some(rest[..rest.find(quote)?].to_string())
    };
    let mut day = None;
    let mut out = Vec::new();
    for tag in xml.split('<').filter(|t| t.starts_with("Cube ")) {
        if let Some(time) = attr(tag, "time") {
            day = Some(time);
        } else if let (Some(c), Some(rate)) = (attr(tag, "currency"), attr(tag, "rate")) {
            let rate: f64 = rate.parse().ok().filter(|r: &f64| r.is_finite() && *r > 0.0)?;
            if let Some(c) = code(&c) {
                out.push((rate_name(c), rate));
            }
        }
    }
    let (y, m, d) = parse_day(&day?)?;
    out.push((RATES_DATE.to_string(), crate::time::days_from_civil(y, m, d) as f64 * 86_400.0));
    (out.len() == CURRENCIES.len()).then_some(out)
}

fn parse_day(day: &str) -> Option<(i64, u32, u32)> {
    let mut parts = day.split('-');
    let y = parts.next()?.parse().ok()?;
    let m = parts.next()?.parse().ok()?;
    let d = parts.next()?.parse().ok()?;
    (parts.next().is_none() && (1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
}

/// A converted amount: the result (`93.45 EUR`, 2 decimals), and the notes and warnings.
#[derive(Debug, PartialEq)]
pub struct Converted {
    pub result: String,
    pub notes: Vec<String>,
    pub warnings: Vec<String>,
}

/// Why a currency request has no answer.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// Not written as one amount and two currencies: a usage error.
    Usage(String),
    /// Written right, but there are no rates to use: a runtime error.
    NoRates(String),
}

/// A currency request, if `input` has a currency code in it: `<amount> <from> [to|in] <to>`. `now`
/// is Unix seconds, for the rates' age.
pub fn request(input: &str, data: &Data, now: i64) -> Option<Result<Converted, Error>> {
    let words: Vec<&str> = input.split_whitespace().collect();
    if !words.iter().any(|w| code(w).is_some()) {
        return None;
    }
    Some(convert(&words, data, now))
}

fn convert(words: &[&str], data: &Data, now: i64) -> Result<Converted, Error> {
    let usage = |msg: String| Err(Error::Usage(msg));
    let codes: Vec<&str> = words.iter().filter_map(|w| code(w)).collect();
    let others: Vec<&str> = words.iter().copied().filter(|w| code(w).is_none()).collect();
    let amounts: Vec<f64> = others.iter().filter_map(|w| amount(w)).collect();
    // The shape: `<amount> <from> [to|in] <to>`.
    let shape_ok = match words {
        [a, f, t] => amount(a).is_some() && code(f).is_some() && code(t).is_some(),
        [a, f, k, t] => amount(a).is_some() && code(f).is_some() && is_connector(k) && code(t).is_some(),
        _ => false,
    };
    if !shape_ok {
        return match (amounts.len(), codes.len()) {
            (0, 2) if others.iter().all(|w| is_connector(w)) => usage(format!("the amount to convert: `100 {}`", codes.join(" "))),
            (1, 1) if words.len() == 2 && amount(words[0]).is_some() => {
                usage(format!("now the currency to convert to: `{} {} EUR`", words[0], codes[0]))
            }
            _ => usage(SHAPE.to_string()),
        };
    }
    let value = amounts[0];
    let (from, to) = (codes[0], codes[1]);
    let per_euro = |c: &str| if c == "EUR" { Some(1.0) } else { data.get(&rate_name(c)).map(|s| s.value) };
    let (Some(a), Some(b), Some(day)) = (per_euro(from), per_euro(to), data.get(RATES_DATE)) else {
        return Err(Error::NoRates(match crate::network::allowed() {
            true => "no exchange rates yet: they are being downloaded in the background, try again in a minute".to_string(),
            false => "no exchange rates: they were never downloaded, and TALLY_MODE=offline".to_string(),
        }));
    };
    let day = day.value as i64;
    let result = value / a * b;
    if !result.is_finite() {
        return usage(SHAPE.to_string());
    }
    let shown = crate::time::human(&crate::time::Instant { ns: i128::from(day) * 1_000_000_000, zone: crate::time::Zone::Fixed(0) }, true);
    let mut warnings = Vec::new();
    if now - day >= STALE_DAYS * 86_400 {
        warnings.push(format!("exchange rates of {shown}: not updated since"));
    }
    let rate = b / a;
    let notes = vec![format!("ECB reference rates of {shown}: 1 {from} = {} {to}", crate::convert::format_value(rate, 6))];
    Ok(Converted { result: format!("{result:.2} {to}"), notes, warnings })
}

/// `to` and `in` between the currencies: `100 USD to EUR`.
fn is_connector(word: &str) -> bool {
    word.eq_ignore_ascii_case("to") || word.eq_ignore_ascii_case("in")
}

/// One number, nothing else: `100`, `0.5`, `1e3`, `2.50`.
fn amount(word: &str) -> Option<f64> {
    let (expr, rest) = crate::calc::parse_prefix(word).ok()?;
    if !rest.trim().is_empty() || expr.operations() != 0 {
        return None;
    }
    expr.eval().ok().filter(|v| v.is_finite() && *v >= 0.0)
}

/// `tally --list`: the currencies.
pub fn list() -> String {
    let mut out = String::from("Currencies (ECB reference rates, synced daily; any case: usd, USD)\n");
    for (c, name) in CURRENCIES {
        out.push_str(&format!("  {c:<8} {name}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::online::Stored;

    const XML: &str = "<?xml version=\"1.0\"?><gesmes:Envelope><Cube><Cube time='2026-10-12'>\
        <Cube currency='USD' rate='1.1181'/><Cube currency='JPY' rate='177.33'/><Cube currency='CZK' rate='24.391'/>\
        <Cube currency='DKK' rate='7.4748'/><Cube currency='GBP' rate='0.84478'/><Cube currency='HUF' rate='365.13'/>\
        <Cube currency='PLN' rate='4.3850'/><Cube currency='RON' rate='5.3370'/><Cube currency='SEK' rate='11.1965'/>\
        <Cube currency='CHF' rate='0.9312'/><Cube currency='ISK' rate='136.40'/><Cube currency='NOK' rate='10.6955'/>\
        <Cube currency='TRY' rate='55.1635'/><Cube currency='AUD' rate='1.6032'/><Cube currency='BRL' rate='5.5819'/>\
        <Cube currency='CAD' rate='1.5946'/><Cube currency='CNY' rate='7.4832'/><Cube currency='HKD' rate='8.7743'/>\
        <Cube currency='IDR' rate='20036.18'/><Cube currency='ILS' rate='3.4320'/><Cube currency='INR' rate='107.0860'/>\
        <Cube currency='KRW' rate='1490.53'/><Cube currency='MXN' rate='20.5081'/><Cube currency='MYR' rate='4.5663'/>\
        <Cube currency='NZD' rate='1.9968'/><Cube currency='PHP' rate='70.169'/><Cube currency='SGD' rate='1.4314'/>\
        <Cube currency='THB' rate='37.585'/><Cube currency='ZAR' rate='18.4945'/></Cube></Cube></gesmes:Envelope>";
    /// 2026-10-12 00:00 UTC.
    const DAY: i64 = 1_791_763_200;

    fn data() -> Data {
        let mut d = Data::default();
        d.values = parse(XML).unwrap().into_iter().map(|(n, v)| (n, Stored { value: v, at: DAY + 60_000 })).collect();
        d
    }

    fn ok(input: &str) -> String {
        request(input, &data(), DAY + 70_000).unwrap().unwrap().result
    }

    fn usage(input: &str) -> String {
        match request(input, &data(), DAY).unwrap() {
            Err(Error::Usage(m)) => m,
            other => panic!("{input}: {other:?}"),
        }
    }

    #[test]
    fn parses_the_ecb_file() {
        let rates = parse(XML).unwrap();
        assert_eq!(rates.len(), 30);
        assert!(rates.contains(&("EUR/USD".to_string(), 1.1181)));
        assert!(rates.contains(&(RATES_DATE.to_string(), DAY as f64)));
        // Not the rates: an error page, a missing currency.
        assert_eq!(parse("<html>Service unavailable</html>"), None);
        assert_eq!(parse(&XML.replace("<Cube currency='ZAR' rate='18.4945'/>", "")), None);
        assert_eq!(parse(&XML.replace("rate='1.1181'", "rate='n/a'")), None);
    }

    #[test]
    fn converts_in_any_case() {
        assert_eq!(ok("100 USD EUR"), "89.44 EUR");
        assert_eq!(ok("100 usd eur"), "89.44 EUR");
        assert_eq!(ok("100 Usd to Eur"), "89.44 EUR");
        assert_eq!(ok("100 usd in eur"), "89.44 EUR");
        assert_eq!(ok("100 EUR USD"), "111.81 USD");
        // Between two currencies other than the euro: through it.
        assert_eq!(ok("100 gbp usd"), "132.35 USD");
        assert_eq!(ok("1 usd jpy"), "158.60 JPY");
        assert_eq!(ok("0.5 chf chf"), "0.50 CHF");
        assert_eq!(ok("1e3 ron eur"), "187.37 EUR");
    }

    #[test]
    fn one_amount_and_two_currencies() {
        assert_eq!(usage("usd eur"), "the amount to convert: `100 USD EUR`");
        assert_eq!(usage("usd to eur"), "the amount to convert: `100 USD EUR`");
        assert_eq!(usage("100 usd"), "now the currency to convert to: `100 USD EUR`");
        for bad in ["usd", "100 usd eur gbp", "100 200 usd eur", "usd 100 eur", "100 usd eur 5", "100+5 usd eur", "-5 usd eur", "100 usd at eur", "100 usd km"] {
            assert_eq!(usage(bad), SHAPE, "{bad}");
        }
        // No currency: not a currency request.
        assert!(request("100 km mi", &data(), DAY).is_none());
        assert!(request("now Rome", &data(), DAY).is_none());
    }

    #[test]
    fn rates_missing_or_stale() {
        assert!(matches!(request("1 usd eur", &Data::default(), DAY), Some(Err(Error::NoRates(_)))));
        let fresh = request("1 usd eur", &data(), DAY + 3 * 86_400).unwrap().unwrap();
        assert!(fresh.warnings.is_empty());
        assert_eq!(fresh.notes, ["ECB reference rates of 12 Oct 26: 1 USD = 0.894374 EUR"]);
        let stale = request("1 usd eur", &data(), DAY + 6 * 86_400).unwrap().unwrap();
        assert_eq!(stale.warnings, ["exchange rates of 12 Oct 26: not updated since"]);
    }

    /// No currency code, in any case, is anything else tally reads: a unit, a format, a month, a
    /// keyword, a number word, a time zone, a city or a country.
    #[test]
    fn codes_collide_with_nothing() {
        let config = crate::config::Config::default();
        for (c, _) in CURRENCIES {
            for word in [c.to_string(), c.to_lowercase(), format!("{}{}", &c[..1], c[1..].to_lowercase())] {
                assert!(crate::convert::find_unit(&config, &word).is_none(), "{word} is a unit");
                assert!(!crate::time::other_word(&config, &word), "{word} is a date or place word");
                assert!(crate::calc::parse_prefix(&word).is_err() || !crate::calc::parse_prefix(&word).unwrap().1.trim().is_empty(), "{word} is a number");
            }
        }
    }
}
