//! What the board shows when the screen is wide: the clocks, the units of each class (the most
//! used first) and the currencies against the base currency. Pure data; `render.rs` draws it.

use aequalis::config::Config;
use aequalis::usage::{Kind, Usage};

/// How many units a class shows, and how many currencies.
pub const UNITS_PER_CLASS: usize = 5;
pub const CURRENCIES: usize = 5;

/// The units a class starts with, the most common first (the symbols of `config.rs`); the rest of
/// the class follows in its own order.
const COMMON: &[(&str, &[&str])] = &[
    ("Date & Time", &["s", "min", "h", "d", "wk"]),
    ("Bits & Bytes", &["B", "kB", "MB", "GB", "TB"]),
    ("Length", &["km", "m", "cm", "mi", "ft", "in"]),
    ("Area", &["m2", "ft2", "ha", "ac", "km2"]),
    ("Volume", &["l", "ml", "gal-us", "cup", "floz-us"]),
    ("Angle", &["deg", "rad", "turn", "grad", "arcmin"]),
    ("Mass", &["kg", "g", "lb", "oz", "t"]),
    ("Temperature", &["C", "F", "K"]),
    ("Speed", &["km/h", "mph", "m/s", "kn", "ft/s"]),
    ("Pressure", &["bar", "psi", "atm", "Pa", "hPa"]),
    ("Fuel economy", &["l/100km", "mpg", "km/l", "mpg-uk"]),
    ("Energy", &["kWh", "J", "kcal", "kJ", "BTU"]),
    ("Power", &["W", "kW", "hp", "MW", "PS"]),
];

/// The currencies the board starts with: the most traded ones (BIS Triennial Survey).
pub const COMMON_CURRENCIES: [&str; CURRENCIES] = ["USD", "EUR", "JPY", "GBP", "CNY"];

/// A button of the board: what it writes, what it shows, and whether it was used.
#[derive(Debug, Clone, PartialEq)]
pub struct Button {
    pub written: String,
    pub label: String,
    pub used: bool,
}

/// A unit class and its buttons.
#[derive(Debug, Clone, PartialEq)]
pub struct Class {
    pub name: &'static str,
    pub letter: char,
    pub buttons: Vec<Button>,
    /// How often its units were used, all of them.
    pub total: u64,
}

/// Every class with its `UNITS_PER_CLASS` most used units, the most used first; those not used yet
/// in the order of `COMMON`, then the class's own.
pub fn classes(config: &Config, usage: &Usage) -> Vec<Class> {
    config
        .categories
        .iter()
        .map(|c| {
            let common = COMMON.iter().find(|(name, _)| *name == c.name).map_or(&[][..], |(_, units)| *units);
            let mut symbols: Vec<&str> = common.iter().copied().filter(|s| c.units.iter().any(|u| u.symbol == *s)).collect();
            let rest: Vec<&str> = c.units.iter().map(|u| u.symbol).filter(|s| !symbols.contains(s)).collect();
            symbols.extend(rest);
            let buttons = usage
                .ranked(Kind::Unit, &symbols)
                .into_iter()
                .take(UNITS_PER_CLASS)
                .map(|symbol| Button {
                    written: symbol.to_string(),
                    label: aequalis::favourites::label(symbol).to_string(),
                    used: usage.hits(Kind::Unit, symbol) > 0,
                })
                .collect();
            let total = c.units.iter().map(|u| usage.hits(Kind::Unit, u.symbol)).sum();
            Class { name: c.name, letter: super::measures::key(c), buttons, total }
        })
        .collect()
}

/// An entry of the board's list of measures: the currencies, or a unit class (its index in
/// `classes`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Item {
    Currencies,
    Class(usize),
}

/// The list the board scrolls: the currencies and the classes, the one used most first; the
/// currencies lead when nothing is used (or as often as another), then the classes in their order.
pub fn items(classes: &[Class], usage: &Usage) -> Vec<Item> {
    let money: u64 = usage.top(Kind::Currency, usize::MAX).iter().map(|(_, n)| n).sum();
    let mut items: Vec<(u64, Item)> = std::iter::once((money, Item::Currencies)).chain(classes.iter().enumerate().map(|(i, c)| (c.total, Item::Class(i)))).collect();
    items.sort_by_key(|(total, _)| std::cmp::Reverse(*total));
    items.into_iter().map(|(_, item)| item).collect()
}

/// A currency of the board and what one of it is in the base currency (when the rate is known).
#[derive(Debug, Clone, PartialEq)]
pub struct Money {
    pub code: String,
    pub value: Option<String>,
    pub used: bool,
}

/// The `CURRENCIES` most used currencies, then the most traded ones, each against `base`.
pub fn currencies(usage: &Usage, rates: &aequalis::online::Data, base: Option<&str>) -> Vec<Money> {
    let mut codes: Vec<String> = usage.top(Kind::Currency, CURRENCIES).into_iter().map(|(code, _)| code).collect();
    for common in COMMON_CURRENCIES {
        if codes.len() < CURRENCIES && !codes.iter().any(|c| c == common) {
            codes.push(common.to_string());
        }
    }
    codes
        .into_iter()
        .map(|code| {
            let value = base.and_then(|base| {
                if base == code {
                    return Some("base".to_string());
                }
                aequalis::currency::rate(rates, &code, base).map(|rate| aequalis::convert::format_value(rate, 4))
            });
            let used = usage.hits(Kind::Currency, &code) > 0;
            Money { code, value, used }
        })
        .collect()
}

/// The cities the clocks start with, after the device's own zone.
pub const COMMON_CITIES: [&str; 6] = ["New York", "London", "Tokyo", "Los Angeles", "Sydney", "Dubai"];

/// A clock: a city, its time of day and how it stands against ours.
#[derive(Debug, Clone, PartialEq)]
pub struct Clock {
    pub city: String,
    /// `16:42` or `4:42 PM`, as the time settings write it.
    pub time: String,
    /// `+9h`, `−6h`, `+5:30h`; empty for our own zone.
    pub difference: String,
    /// `+1d` or `−1d` when its day is not ours.
    pub day: String,
}

/// What a time request printed, taken apart: the time of day (with AM/PM), minutes since midnight,
/// and the offset from UTC in minutes (`GMT+5:30`, `UTC-4`).
pub fn parse_time(printed: &str) -> Option<(String, i32, i32)> {
    let words: Vec<&str> = printed.split_whitespace().collect();
    let at = words.iter().position(|w| w.contains(':') && w.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    let (hours, minutes) = words[at].split_once(':')?;
    let (mut h, m): (i32, i32) = (hours.parse().ok()?, minutes.get(..2)?.parse().ok()?);
    let mut shown = words[at].to_string();
    if let Some(ampm) = words.get(at + 1).filter(|w| ["AM", "PM"].contains(&w.to_ascii_uppercase().as_str())) {
        shown = format!("{shown} {ampm}");
        h = h % 12 + if ampm.eq_ignore_ascii_case("PM") { 12 } else { 0 };
    }
    let zone = words.iter().find(|w| w.starts_with("GMT") || w.starts_with("UTC"))?;
    let offset = zone[3..].trim_start_matches(':');
    let sign = if offset.starts_with('-') || offset.starts_with('−') { -1 } else { 1 };
    let offset = offset.trim_start_matches(['+', '-', '−']);
    let (oh, om) = offset.split_once(':').unwrap_or((offset, "0"));
    // `UTC` and `GMT` alone: no offset.
    let utc = if offset.is_empty() { 0 } else { sign * (oh.parse::<i32>().ok()? * 60 + om.parse::<i32>().ok()?) };
    Some((shown, h * 60 + m, utc))
}

/// `+9h`, `−6h`, `+5:30h`.
pub fn difference(minutes: i32) -> String {
    let (sign, minutes) = if minutes < 0 { ('−', -minutes) } else { ('+', minutes) };
    match minutes % 60 {
        0 => format!("{sign}{}h", minutes / 60),
        rest => format!("{sign}{}:{rest:02}h", minutes / 60),
    }
}

/// The clock of `city` against `here` (our time of day and offset, from `parse_time`).
pub fn clock(city: &str, printed: &str, here: (i32, i32)) -> Option<Clock> {
    let (time, of_day, utc) = parse_time(printed)?;
    let delta = utc - here.1;
    let moved = here.0 + delta;
    let day = if moved >= 1440 {
        "+1d"
    } else if moved < 0 {
        "−1d"
    } else {
        ""
    };
    (of_day >= 0).then(|| Clock { city: city.to_string(), time, difference: if delta == 0 { String::new() } else { difference(delta) }, day: day.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_common_units_exist() {
        let config = Config::default();
        for (class, units) in COMMON {
            let category = config.categories.iter().find(|c| c.name == *class).unwrap_or_else(|| panic!("no class {class}"));
            for unit in *units {
                assert!(category.units.iter().any(|u| u.symbol == *unit), "{class}: {unit}");
            }
        }
        assert_eq!(config.categories.len(), COMMON.len(), "every class has its common units");
    }

    #[test]
    fn the_most_used_first() {
        let config = Config::default();
        let mut usage = Usage::default();
        let length = |usage: &Usage| classes(&config, usage).into_iter().find(|c| c.name == "Length").unwrap();
        let shown = |c: &Class| c.buttons.iter().map(|b| b.written.as_str()).collect::<Vec<_>>().join(" ");
        // Nothing used: the common ones, in order.
        assert_eq!(shown(&length(&usage)), "km m cm mi ft");
        assert!(length(&usage).buttons.iter().all(|b| !b.used));
        // Used: first, the most used first, and lit; one not common comes in.
        for unit in ["pc", "pc", "pc", "ft", "ft", "au"] {
            usage.hit(Kind::Unit, unit);
        }
        let class = length(&usage);
        assert_eq!(shown(&class), "pc ft au km m");
        assert_eq!(class.buttons.iter().map(|b| b.used).collect::<Vec<_>>(), [true, true, true, false, false]);
        // A friendly label.
        let mass = classes(&config, &usage).into_iter().find(|c| c.name == "Temperature").unwrap();
        assert_eq!(mass.buttons[0].label, "°C");
    }

    #[test]
    fn the_list_starts_with_what_is_used_most() {
        let config = Config::default();
        let mut usage = Usage::default();
        let order = |usage: &Usage| {
            let classes = classes(&config, usage);
            items(&classes, usage).into_iter().take(3).map(|i| match i {
                Item::Currencies => "Currencies".to_string(),
                Item::Class(c) => classes[c].name.to_string(),
            }).collect::<Vec<_>>()
        };
        // Nothing used: the currencies, then the classes in their order.
        assert_eq!(order(&usage), ["Currencies", "Date & Time", "Bits & Bytes"]);
        // The most used first: a class, then the currencies once they are used as much.
        for _ in 0..3 {
            usage.hit(Kind::Unit, "kg");
        }
        usage.hit(Kind::Currency, "EUR");
        assert_eq!(order(&usage), ["Mass", "Currencies", "Date & Time"]);
    }

    #[test]
    fn currencies_against_the_base() {
        use aequalis::online::{Data, Stored};
        let rate = |name: &str, value: f64| (name.to_string(), Stored { value, at: 1 });
        let rates = Data { values: vec![rate("USD/EUR", 0.9), rate("USD/JPY", 150.0), rate("USD/GBP", 0.8)], ..Data::default() };
        let mut usage = Usage::default();
        let shown = |m: &[Money]| m.iter().map(|m| format!("{} {}", m.code, m.value.clone().unwrap_or_default())).collect::<Vec<_>>();
        // Nothing used: the most traded, against the base.
        let money = currencies(&usage, &rates, Some("EUR"));
        assert_eq!(shown(&money), ["USD 0.9", "EUR base", "JPY 0.006", "GBP 1.125", "CNY "]);
        // Used ones first (lit), the traded ones fill up.
        usage.hit(Kind::Currency, "EGP");
        usage.hit(Kind::Currency, "EGP");
        usage.hit(Kind::Currency, "GBP");
        let money = currencies(&usage, &rates, None);
        assert_eq!(money.iter().map(|m| m.code.as_str()).collect::<Vec<_>>(), ["EGP", "GBP", "USD", "EUR", "JPY"]);
        assert_eq!(money.iter().map(|m| m.used).collect::<Vec<_>>(), [true, true, false, false, false]);
        assert!(money.iter().all(|m| m.value.is_none()), "no base: no values");
    }

    #[test]
    fn times_taken_apart() {
        assert_eq!(parse_time("21 Oct 26 1:53 AM GMT+9 Tokyo"), Some(("1:53 AM".into(), 113, 540)));
        assert_eq!(parse_time("20 Oct 26 12:53 PM GMT-4 New York"), Some(("12:53 PM".into(), 773, -240)));
        assert_eq!(parse_time("2026-10-20 16:42 UTC+5:30 Delhi"), Some(("16:42".into(), 1002, 330)));
        assert_eq!(parse_time("20 Oct 26 4:42 PM UTC"), Some(("4:42 PM".into(), 1002, 0)), "UTC alone");
        assert_eq!(parse_time("nothing here"), None);
        assert_eq!(difference(540), "+9h");
        assert_eq!(difference(-360), "−6h");
        assert_eq!(difference(330), "+5:30h");
    }

    #[test]
    fn clocks_against_ours() {
        // Rome 18:53 (UTC+2); Tokyo 1:53 AM the next day (UTC+9); New York 12:53 (UTC-4).
        let here = parse_time("20 Oct 26 6:53 PM GMT+2 Rome").map(|(_, day, utc)| (day, utc)).unwrap();
        let tokyo = clock("Tokyo", "21 Oct 26 1:53 AM GMT+9 Tokyo", here).unwrap();
        assert_eq!((tokyo.time.as_str(), tokyo.difference.as_str(), tokyo.day.as_str()), ("1:53 AM", "+7h", "+1d"));
        let york = clock("New York", "20 Oct 26 12:53 PM GMT-4 New York", here).unwrap();
        assert_eq!((york.time.as_str(), york.difference.as_str(), york.day.as_str()), ("12:53 PM", "−6h", ""));
        let rome = clock("Rome", "20 Oct 26 6:53 PM GMT+2 Rome", here).unwrap();
        assert_eq!((rome.difference.as_str(), rome.day.as_str()), ("", ""));
        // Early morning here, a city behind us: the day before.
        let early = (30, 120);
        assert_eq!(clock("New York", "19 Oct 26 8:30 PM GMT-4 New York", early).unwrap().day, "−1d");
    }
}

