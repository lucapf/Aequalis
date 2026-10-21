//! The shortcuts of the interactive mode's keypad: the units and the currencies the operator uses,
//! each with how many times it was used. Stored in the settings (`tui.favourite_units`,
//! `tui.favourite_currencies`) as `USD:3,EUR:1`; nothing stored means the defaults.
//!
//! Using one that is listed counts a hit for it. Using one that is not listed replaces the listed
//! one with the fewest hits (the last of them when several tie), in its place, with a hit of its
//! own: the often-used stay, and the last slot rotates.

use crate::config::System;

/// How many shortcuts of each kind.
pub const SLOTS: usize = 5;

/// The currencies listed until the operator's own use replaces them.
pub const CURRENCIES: [&str; SLOTS] = ["USD", "EUR", "JPY", "CNY", "AED"];
/// The units listed until the operator's own use replaces them, by measurement system.
/// (the units' own symbols: `l`, `C`, `gal-us`; `label` is how a button shows them).
pub const METRIC: [&str; SLOTS] = ["km", "kg", "l", "C", "m"];
pub const IMPERIAL: [&str; SLOTS] = ["mi", "lb", "gal-us", "F", "ft"];

/// What a button shows for a unit's symbol.
pub fn label(symbol: &str) -> &str {
    match symbol {
        "l" => "L",
        "C" => "°C",
        "F" => "°F",
        "gal-us" => "gal",
        other => other,
    }
}

/// The default units of a measurement system.
pub fn default_units(system: System) -> [&'static str; SLOTS] {
    match system {
        System::Imperial => IMPERIAL,
        _ => METRIC,
    }
}

/// A list of names with their hits, in the order shown.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Favourites {
    entries: Vec<(String, u32)>,
}

impl Favourites {
    /// The list stored as `text` (`USD:3,EUR`: a missing count is 0), or `defaults` with no hits
    /// when nothing is stored. At most `SLOTS` entries; a name listed twice counts once.
    pub fn of(text: &str, defaults: &[&str]) -> Favourites {
        let mut list = Favourites::default();
        for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (name, hits) = match part.rsplit_once(':') {
                Some((name, hits)) if hits.trim().parse::<u32>().is_ok() => (name.trim(), hits.trim().parse().unwrap_or(0)),
                _ => (part, 0),
            };
            if !name.is_empty() && list.entries.len() < SLOTS && !list.names().any(|n| n == name) {
                list.entries.push((name.to_string(), hits));
            }
        }
        if list.entries.is_empty() {
            list.entries = defaults.iter().take(SLOTS).map(|n| (n.to_string(), 0)).collect();
        }
        list
    }

    /// `text` as the settings store it, or why it is not a list: `name:hits`, separated by commas.
    pub fn check(text: &str) -> Result<String, String> {
        for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let named = |name: &str| !name.trim().is_empty() && !name.contains(char::is_whitespace);
            let ok = match part.rsplit_once(':') {
                Some((name, hits)) => named(name) && hits.trim().parse::<u32>().is_ok(),
                None => named(part),
            };
            if !ok {
                return Err(format!("`{part}` is not a name with its hits, such as USD:3"));
            }
        }
        Ok(Favourites::of(text, &[]).to_text())
    }

    /// The list as the settings store it (`USD:3,EUR:0`; nothing for an empty list).
    pub fn to_text(&self) -> String {
        self.entries.iter().map(|(name, hits)| format!("{name}:{hits}")).collect::<Vec<_>>().join(",")
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(name, _)| name.as_str())
    }

    pub fn hits(&self, name: &str) -> Option<u32> {
        self.entries.iter().find(|(n, _)| n == name).map(|&(_, hits)| hits)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `name` was used: a hit for it if listed, else it replaces the listed one with the fewest
    /// hits (the last of them), in its place, with one hit.
    pub fn hit(&mut self, name: &str) {
        if let Some((_, hits)) = self.entries.iter_mut().find(|(n, _)| n == name) {
            *hits = hits.saturating_add(1);
        } else if self.entries.len() < SLOTS {
            self.entries.push((name.to_string(), 1));
        } else if let Some(fewest) = self.entries.iter().map(|&(_, hits)| hits).min() {
            if let Some(slot) = self.entries.iter_mut().rev().find(|(_, hits)| *hits == fewest) {
                *slot = (name.to_string(), 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(text: &str) -> Favourites {
        Favourites::of(text, &CURRENCIES)
    }

    #[test]
    fn nothing_stored_is_the_defaults() {
        assert_eq!(list("").to_text(), "USD:0,EUR:0,JPY:0,CNY:0,AED:0");
        assert!(!list("").is_empty() && Favourites::of("", &[]).is_empty());
        assert_eq!(Favourites::of("", &default_units(System::Metric)).to_text(), "km:0,kg:0,l:0,C:0,m:0");
        assert_eq!(Favourites::of(" ", &default_units(System::Imperial)).to_text(), "mi:0,lb:0,gal-us:0,F:0,ft:0");
    }

    #[test]
    fn a_listed_one_counts_a_hit_and_stays() {
        let mut f = list("");
        f.hit("EUR");
        f.hit("EUR");
        f.hit("AED");
        assert_eq!(f.to_text(), "USD:0,EUR:2,JPY:0,CNY:0,AED:1");
    }

    #[test]
    fn a_new_one_replaces_the_last_with_the_fewest_hits() {
        // The agreed example: GBP takes AED's place; then AED returns in CNY's place (the last of
        // those with no hits).
        let mut f = list("");
        f.hit("GBP");
        assert_eq!(f.to_text(), "USD:0,EUR:0,JPY:0,CNY:0,GBP:1");
        f.hit("AED");
        assert_eq!(f.to_text(), "USD:0,EUR:0,JPY:0,AED:1,GBP:1");
        // The ones used often stay: a new one rotates through the last of the fewest.
        let mut f = list("USD:5,EUR:4,JPY:3,CNY:2,AED:2");
        f.hit("GBP");
        assert_eq!(f.to_text(), "USD:5,EUR:4,JPY:3,CNY:2,GBP:1");
        f.hit("EGP");
        assert_eq!(f.to_text(), "USD:5,EUR:4,JPY:3,CNY:2,EGP:1", "the newcomer is the fewest: it is replaced");
    }

    #[test]
    fn parsing_is_lenient_and_checking_is_not() {
        assert_eq!(list("usd:3, EUR ,USD:9").to_text(), "usd:3,EUR:0,USD:9", "no count: 0; case matters");
        assert_eq!(list("EUR:1,EUR:7").to_text(), "EUR:1", "a name once");
        assert_eq!(list("A,B,C,D,E,F,G").len(), SLOTS);
        assert_eq!(Favourites::check(" USD:3 , EUR ").unwrap(), "USD:3,EUR:0");
        assert_eq!(Favourites::check("").unwrap(), "", "empty: the defaults");
        assert!(Favourites::check("USD:-1").is_err());
        assert!(Favourites::check("US D:1").is_err());
        assert!(Favourites::check(":3").is_err());
    }

    #[test]
    fn buttons_show_friendly_names() {
        assert_eq!(["l", "C", "F", "gal-us", "km"].map(label), ["L", "°C", "°F", "gal", "km"]);
    }

    #[test]
    fn the_default_units_exist() {
        let config = crate::config::Config::default();
        for symbol in METRIC.iter().chain(&IMPERIAL) {
            let found = crate::convert::find_unit(&config, symbol);
            assert_eq!(found.map(|(_, u)| u.symbol), Some(*symbol), "{symbol}");
        }
        for code in CURRENCIES {
            assert_eq!(crate::currency::exact(code).as_deref(), Some(&[code][..]));
        }
    }
}
