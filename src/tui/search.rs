//! `/` in command mode: search every unit by symbol, alias or name, across the categories.

use aequalis::config::{Category, Config, Unit};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Search {
    pub query: String,
    pub selected: usize,
}

/// The units matching `query` (case aside), best first: a symbol or alias equal to it, then one
/// that starts with it, then a name with a word that starts with it, then any that contains it.
/// All of them for an empty query, in the categories' order.
pub fn matches<'a>(config: &'a Config, query: &str) -> Vec<(&'a Category, &'a Unit)> {
    let q = query.trim().to_lowercase();
    let mut scored: Vec<(usize, usize, &Category, &Unit)> = Vec::new();
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
                scored.push((s, order, c, u));
            }
        }
    }
    scored.sort_by_key(|&(s, o, ..)| (s, o));
    scored.into_iter().map(|(_, _, c, u)| (c, u)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbols(q: &str) -> Vec<&'static str> {
        let config = Box::leak(Box::new(Config::default()));
        matches(config, q).iter().map(|(_, u)| u.symbol).collect()
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
        // A category's name finds its units.
        assert!(symbols("pressure").contains(&"psi"));
    }
}
