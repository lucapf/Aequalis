//! The Currencies window (`U`, or Currencies in the top bar): every currency (`currency::all`),
//! searched by code, name, symbol or country as you type; Enter writes the code into the expression.

use crate::currency::{self, Currency};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct View {
    pub query: String,
    pub selected: usize,
}

/// The currencies matching `query` (case aside), best first: the code or a symbol equal to it,
/// then a code that starts with it, then a name with a word that starts with it, then a country
/// using it with a word that does (`italy` → EUR, `usa` → USD, as requests take countries), then
/// any that contains it. All of them for an empty query, the most used first.
pub fn matches(query: &str) -> Vec<&'static Currency> {
    let q = query.trim().to_lowercase();
    let starts_a_word = |text: &str| text.split(|ch: char| !ch.is_alphanumeric()).any(|w| w.starts_with(&q));
    // The countries' names and other names, by currency.
    let mut countries: std::collections::HashMap<&str, Vec<String>> = std::collections::HashMap::new();
    for k in crate::time::tz::countries() {
        countries.entry(k.currency.as_str()).or_default().extend(std::iter::once(&k.name).chain(&k.aliases).map(|n| n.to_lowercase()));
    }
    let mut scored: Vec<(usize, usize, &Currency)> = Vec::new();
    for (order, c) in currency::all().iter().enumerate() {
        let code = c.code.to_lowercase();
        let name = c.name.to_lowercase();
        let of_countries = countries.get(c.code.as_str()).map_or(&[][..], |v| v.as_slice());
        let score = if q.is_empty() || code == q || c.symbols.iter().any(|s| s.to_lowercase() == q) {
            Some(0)
        } else if code.starts_with(&q) {
            Some(1)
        } else if starts_a_word(&name) {
            Some(2)
        } else if of_countries.iter().any(|n| starts_a_word(n)) {
            Some(3)
        } else if name.contains(&q) || c.words.iter().any(|w| w.to_lowercase().contains(&q)) || of_countries.iter().any(|n| n.contains(&q)) {
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

    fn codes(q: &str) -> Vec<&'static str> {
        matches(q).iter().map(|c| c.code.as_str()).collect()
    }

    #[test]
    fn best_first() {
        assert_eq!(codes("eur")[0], "EUR");
        assert_eq!(codes("€")[0], "EUR", "a symbol");
        assert_eq!(codes("yen")[0], "JPY");
        assert!(codes("dollar").contains(&"USD") && codes("dollar").contains(&"CAD"));
        assert!(codes("zzzz").is_empty());
        assert_eq!(codes("").len(), currency::all().len());
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
