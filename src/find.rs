//! One search for everything a person may look for: units, currencies, settings, trivia, cities
//! and earlier requests (the interactive mode's Ctrl+K). Each match says what it is, what choosing
//! it writes, and its name and a few words about it.

use crate::config::Config;

/// What a match is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Unit,
    Currency,
    Setting,
    Trivia,
    City,
    History,
}

impl Kind {
    /// A short word for a column.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Unit => "unit",
            Kind::Currency => "currency",
            Kind::Setting => "setting",
            Kind::Trivia => "trivia",
            Kind::City => "city",
            Kind::History => "history",
        }
    }
}

/// A match.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub kind: Kind,
    /// What choosing it writes (a unit's symbol, a currency's code, a city as it is typed, a
    /// trivia entry's name, a setting's key, an earlier request).
    pub written: String,
    /// Its name; for a city its label; for an earlier request its result.
    pub name: String,
    /// Where it belongs: a unit's class, a trivia entry's section.
    pub about: String,
}

/// What the search needs besides the query: the earlier requests (request, result; oldest first)
/// and the device's country, which ranks the cities. Cities are searched when the archive is there.
#[derive(Default)]
pub struct Sources<'a> {
    pub history: &'a [(String, String)],
    pub country: Option<&'a str>,
}

/// How well a match fits `q` (in lower case): 0 is a name equal to it, 1 a name starting with it,
/// 2 a word of `words` starting with it, 3 anything containing it.
fn score(q: &str, names: &[String], words: &[String]) -> Option<usize> {
    let lower = |s: &String| s.to_lowercase();
    if names.iter().any(|n| lower(n) == q) {
        Some(0)
    } else if names.iter().any(|n| lower(n).starts_with(q)) {
        Some(1)
    } else if words.iter().any(|t| lower(t).split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(q))) {
        Some(2)
    } else if names.iter().chain(words).any(|t| lower(t).contains(q)) {
        Some(3)
    } else {
        None
    }
}

/// A match with how well it fits and its place, to sort.
struct Scored {
    score: usize,
    order: usize,
    found: Found,
}

/// The matches of `query` (case aside), best first: by how well they fit, then units, currencies,
/// settings, trivia, cities, earlier requests. An empty query matches everything but the cities,
/// the units in the classes' order, the earlier requests newest first.
pub fn search(config: &Config, query: &str, sources: &Sources) -> Vec<Found> {
    let q = query.trim().to_lowercase();
    let mut scored: Vec<Scored> = Vec::new();
    let mut add = |score: usize, found: Found| {
        let order = scored.len();
        scored.push(Scored { score, order, found });
    };

    for c in &config.categories {
        let class = c.name.to_lowercase();
        for u in &c.units {
            let names: Vec<String> = std::iter::once(u.symbol).chain(u.aliases.iter().copied()).map(str::to_string).collect();
            let s = if q.is_empty() { Some(0) } else { score(&q, &names, &[u.name.to_string()]).or_else(|| class.starts_with(&q).then_some(3)) };
            if let Some(s) = s {
                add(s, Found { kind: Kind::Unit, written: u.symbol.to_string(), name: u.name.to_string(), about: c.name.to_string() });
            }
        }
    }

    // `currency::search` finds them (and by the countries using them); here only the tier.
    for c in crate::currency::search(&q) {
        let mut names = vec![c.code.clone()];
        names.extend(c.symbols.iter().cloned());
        let s = if q.is_empty() { 0 } else { score(&q, &names, std::slice::from_ref(&c.name)).unwrap_or(3) };
        add(s, Found { kind: Kind::Currency, written: c.code.clone(), name: c.name.clone(), about: String::new() });
    }

    for k in crate::settings::KEYS {
        let words = vec![k.key.replace(['.', '_'], " "), k.about.to_string()];
        let s = if q.is_empty() { Some(0) } else { score(&q, &[k.key.to_string()], &words) };
        if let Some(s) = s {
            add(s, Found { kind: Kind::Setting, written: k.key.to_string(), name: k.about.to_string(), about: String::new() });
        }
    }

    for e in crate::trivia::entries() {
        let names: Vec<String> = std::iter::once(e.name.clone()).chain(e.symbol.clone()).collect();
        let words = vec![e.short.clone(), e.section.clone()];
        let s = if q.is_empty() { Some(0) } else { score(&q, &names, &words) };
        if let Some(s) = s {
            add(s, Found { kind: Kind::Trivia, written: e.name.clone(), name: e.short.clone(), about: e.section.clone() });
        }
    }

    // The cities starting so, from the archive (two letters at least): as names starting the query.
    if q.chars().count() >= 2 {
        if let Some(store) = crate::cities::store() {
            for city in store.starting_with(&q, 6, sources.country) {
                let (label, written) = store.offer(&city, &q);
                add(1, Found { kind: Kind::City, written, name: label, about: String::new() });
            }
        }
    }

    for (input, result) in sources.history.iter().rev() {
        let s = if q.is_empty() { Some(0) } else { score(&q, std::slice::from_ref(input), &[]).map(|s| s.max(1)) };
        if let Some(s) = s {
            add(s, Found { kind: Kind::History, written: input.clone(), name: result.clone(), about: String::new() });
        }
    }

    scored.sort_by_key(|s| (s.score, s.found.kind as usize, s.order));
    scored.into_iter().map(|s| s.found).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(q: &str) -> Vec<Found> {
        search(&Config::default(), q, &Sources::default())
    }

    fn first(q: &str) -> (Kind, String) {
        let f = found(q);
        (f[0].kind, f[0].written.clone())
    }

    #[test]
    fn units_currencies_settings_and_trivia() {
        assert_eq!(first("km"), (Kind::Unit, "km".into()));
        assert_eq!(first("mile"), (Kind::Unit, "mi".into()), "an alias");
        assert_eq!(first("yen"), (Kind::Currency, "JPY".into()));
        assert_eq!(first("€"), (Kind::Currency, "EUR".into()));
        assert_eq!(first("precision"), (Kind::Setting, "precision".into()));
        assert_eq!(first("tui.mouse"), (Kind::Setting, "tui.mouse".into()));
        assert!(found("pi").iter().any(|f| f.kind == Kind::Trivia), "a trivia entry by its symbol");
        assert!(found("zzzzzz").is_empty());
    }

    #[test]
    fn a_unit_before_a_currency_that_matches_as_well() {
        let pound = found("pound");
        assert_eq!((pound[0].kind, pound[0].written.as_str()), (Kind::Unit, "lb"), "the alias equals it; the currencies' names start with it");
        assert!(pound.iter().any(|f| f.written == "GBP"));
    }

    #[test]
    fn earlier_requests_newest_first() {
        let history = vec![("10 km mi".to_string(), "6.2137 mi".to_string()), ("5 km ft".to_string(), "16404 ft".to_string())];
        let sources = Sources { history: &history, country: None };
        let past =
            |q: &str| -> Vec<String> { search(&Config::default(), q, &sources).into_iter().filter(|f| f.kind == Kind::History).map(|f| f.written).collect() };
        assert_eq!(past("5 km"), ["5 km ft"]);
        assert_eq!(past(""), ["5 km ft", "10 km mi"], "newest first");
        assert!(search(&Config::default(), "", &sources).iter().all(|f| f.kind != Kind::City), "no cities for an empty query");
    }

    #[test]
    fn everything_for_an_empty_query() {
        let config = Config::default();
        let units: usize = config.categories.iter().map(|c| c.units.len()).sum();
        let all = found("");
        assert_eq!(all.iter().filter(|f| f.kind == Kind::Unit).count(), units);
        assert_eq!(all.iter().filter(|f| f.kind == Kind::Currency).count(), crate::currency::all().len());
        assert_eq!(all.iter().filter(|f| f.kind == Kind::Setting).count(), crate::settings::KEYS.len());
        assert_eq!(all[0].kind, Kind::Unit, "the units first");
    }
}
