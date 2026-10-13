//! The Currencies window (`U`, or Currencies in the top bar): every currency (`currency::all`),
//! searched by code, name or symbol as you type; Enter writes the code into the expression.

use crate::currency::{self, Currency};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct View {
    pub query: String,
    pub selected: usize,
}

/// The currencies matching `query` (case aside), best first: the code or a symbol equal to it,
/// then a code that starts with it, then a name with a word that starts with it, then any that
/// contains it. All of them for an empty query, the most used first.
pub fn matches(query: &str) -> Vec<&'static Currency> {
    let q = query.trim().to_lowercase();
    let mut scored: Vec<(usize, usize, &Currency)> = Vec::new();
    for (order, c) in currency::all().iter().enumerate() {
        let code = c.code.to_lowercase();
        let name = c.name.to_lowercase();
        let score = if q.is_empty() || code == q || c.symbols.iter().any(|s| s.to_lowercase() == q) {
            Some(0)
        } else if code.starts_with(&q) {
            Some(1)
        } else if name.split(|ch: char| !ch.is_alphanumeric()).any(|w| w.starts_with(&q)) {
            Some(2)
        } else if name.contains(&q) || c.words.iter().any(|w| w.to_lowercase().contains(&q)) {
            Some(3)
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
}
