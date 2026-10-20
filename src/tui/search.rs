//! Ctrl+K: search every unit and every currency by symbol, code, alias or name.

use aequalis::config::{Category, Config, Unit};
use aequalis::currency::Currency;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Search {
    pub query: String,
    pub selected: usize,
}

/// A match: a unit with its category, or a currency.
#[derive(Clone, Copy)]
pub enum Found<'a> {
    Unit(&'a Category, &'a Unit),
    Currency(&'static Currency),
}

impl Found<'_> {
    /// What Enter writes: the unit's symbol, the currency's code.
    pub fn written(&self) -> &str {
        match self {
            Found::Unit(_, u) => u.symbol,
            Found::Currency(c) => &c.code,
        }
    }
}

/// The units and currencies matching `query` (case aside), best first: a symbol, code or alias
/// equal to it, then one that starts with it, then a name with a word that starts with it, then
/// any that contains it; units before currencies when they match as well. All of them for an
/// empty query: the units in the categories' order, then the currencies, the most used first.
pub fn matches<'a>(config: &'a Config, query: &str) -> Vec<Found<'a>> {
    let q = query.trim().to_lowercase();
    // (how well it matches, 0 unit or 1 currency, its place in the lists)
    let mut scored: Vec<(usize, usize, usize, Found)> = Vec::new();
    let mut order = 0;
    for c in &config.categories {
        for u in &c.units {
            order += 1;
            let names: Vec<String> = std::iter::once(u.symbol).chain(u.aliases.iter().copied()).map(str::to_lowercase).collect();
            let name = u.name.to_lowercase();
            let score = if q.is_empty() || names.contains(&q) {
                Some(0)
            } else if names.iter().any(|n| n.starts_with(&q)) {
                Some(1)
            } else if name.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(&q)) {
                Some(2)
            } else if name.contains(&q) || names.iter().any(|n| n.contains(&q)) || c.name.to_lowercase().starts_with(&q) {
                Some(3)
            } else {
                None
            };
            if let Some(s) = score {
                scored.push((s, 0, order, Found::Unit(c, u)));
            }
        }
    }
    // `currency::search` ranks them (and finds the countries using them); here only the tier.
    for (order, c) in aequalis::currency::search(&q).into_iter().enumerate() {
        let lower = |t: &String| t.to_lowercase();
        let score = if q.is_empty() || lower(&c.code) == q || c.symbols.iter().any(|s| lower(s) == q) {
            0
        } else if lower(&c.code).starts_with(&q) {
            1
        } else if lower(&c.name).split(|ch: char| !ch.is_alphanumeric()).any(|w| w.starts_with(&q)) {
            2
        } else {
            3
        };
        scored.push((score, 1, order, Found::Currency(c)));
    }
    scored.sort_by_key(|&(s, kind, o, _)| (s, kind, o));
    scored.into_iter().map(|(.., f)| f).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The symbols of the units found.
    fn symbols(q: &str) -> Vec<&'static str> {
        let config = Box::leak(Box::new(Config::default()));
        matches(config, q).iter().filter_map(|f| if let Found::Unit(_, u) = f { Some(u.symbol) } else { None }).collect()
    }

    /// What Enter would write for each match, units and currencies together.
    fn written(q: &str) -> Vec<String> {
        let config = Box::leak(Box::new(Config::default()));
        matches(config, q).iter().map(|f| f.written().to_string()).collect()
    }

    #[test]
    fn best_first() {
        assert_eq!(symbols("km")[0], "km");
        assert_eq!(symbols("mile")[0], "mi", "an alias");
        assert_eq!(symbols("kilo")[0], "km", "starts an alias: kilometer");
        assert!(symbols("kilo").contains(&"kB"), "a name's word: kilobyte");
        assert!(symbols("light").contains(&"ly"));
        assert_eq!(symbols("horse"), ["hp", "PS"], "an alias (horsepower), then a name (metric horsepower)");
        assert!(symbols("joule").starts_with(&["J"]));
        assert!(symbols("zzz").is_empty());
        assert_eq!(symbols("").len(), Config::default().categories.iter().map(|c| c.units.len()).sum::<usize>());
        assert_eq!(written("").len(), symbols("").len() + aequalis::currency::all().len(), "every unit, then every currency");
        // A category's name finds its units.
        assert!(symbols("pressure").contains(&"psi"));
    }

    #[test]
    fn currencies_too() {
        // By name, code, symbol and country; the unit first when both match as well.
        assert_eq!(written("yen")[0], "JPY");
        assert_eq!(written("eur")[0], "EUR");
        assert_eq!(written("€")[0], "EUR");
        assert_eq!(written("italy")[0], "EUR", "a country using it");
        assert_eq!(written("japanese yen")[0], "JPY");
        let pound = written("pound");
        assert_eq!(pound[0], "lb", "the unit's alias equals it; the currencies' names only start with it");
        assert!(pound.contains(&"GBP".to_string()), "{pound:?}");
        // A code that is a unit's symbol too: both, the unit first.
        let all = written("ang");
        assert!(all.contains(&"AOA".to_string()) || all.contains(&"ANG".to_string()), "{all:?}");
        assert!(written("zzz").is_empty());
        // Every match writes something Enter can place.
        assert!(written("k").iter().all(|w| !w.is_empty()));
    }
}
