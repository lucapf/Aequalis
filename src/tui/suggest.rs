//! Suggestions for the word under the cursor in the expression bar: units, or cities (the city
//! archive, `cities.rs`) for a distance.

use crate::calc;
use crate::config::{Category, Config};
use crate::convert;

#[derive(Debug, PartialEq)]
pub struct Suggestions {
    /// Position (in chars) where the word being completed starts.
    pub start: usize,
    /// What Tab writes.
    pub names: Vec<String>,
    /// What is shown for each (cities: `Milano - Italy`); the names themselves when empty.
    pub labels: Vec<String>,
}

/// Units that fit the word ending at `cursor`: the source unit after the expression, then the target
/// unit, which must be of the same kind as the source. Symbols come before aliases.
pub fn suggest(config: &Config, line: &str, cursor: usize) -> Option<Suggestions> {
    let before: String = line.chars().take(cursor).collect();
    let Ok((_, rest)) = calc::parse_prefix(&before) else {
        // Not an expression: a place, for a distance.
        return cities(config, &before, crate::cities::store().as_ref(), crate::geo::local_country().as_deref());
    };
    let words: Vec<&str> = rest.split_whitespace().collect();
    let (done, prefix) = match words.split_last() {
        Some((last, done)) if !before.ends_with(char::is_whitespace) => (done, *last),
        _ => (&words[..], ""),
    };
    let category = match done {
        [] => None,
        [from] => Some(convert::find_unit(config, from)?.0),
        _ => return None,
    };
    // An empty source unit would list every unit; wait for the first letter.
    if prefix.is_empty() && category.is_none() {
        return None;
    }
    let names: Vec<String> = matching(config, category, prefix).into_iter().map(str::to_string).collect();
    (!names.is_empty()).then(|| Suggestions { start: cursor - prefix.chars().count(), names, labels: Vec::new() })
}

/// Cities whose name starts with the last words typed (up to 3: `New Yo`; never across `to`), in
/// the order of `cities::rank` (`local`: the device's country); a name several cities share is
/// written qualified (`Springfield, Illinois, United States`). Not after a word that is no place
/// (a unit, a keyword).
pub fn cities(config: &Config, before: &str, store: Option<&crate::cities::Store>, local: Option<&str>) -> Option<Suggestions> {
    let store = store?;
    if before.ends_with(char::is_whitespace) {
        return None;
    }
    let words: Vec<&str> = before.split_whitespace().collect();
    for n in (1..=words.len().min(3)).rev() {
        let typed = words[words.len() - n..].join(" ");
        if n == 1 && crate::time::reserved(config, &typed) {
            return None;
        }
        // `Rieti to R`: the place after `to`, not one named `to R…`.
        if n > 1 && words[words.len() - n..].iter().any(|w| ["to", "->"].contains(&w.to_ascii_lowercase().as_str())) {
            continue;
        }
        let found = store.starting_with(&typed, 8, local);
        if found.is_empty() {
            continue;
        }
        let (mut names, mut labels) = (Vec::new(), Vec::new());
        for (label, name) in found.iter().map(|c| store.offer(c, &typed)) {
            if !names.contains(&name) {
                names.push(name);
                labels.push(label);
            }
        }
        // Replaced from where the typed words start (the last `n` words).
        return Some(Suggestions { start: before_start(before, n), names, labels });
    }
    None
}

/// The position (in chars) where the last `n` words of `text` start.
fn before_start(text: &str, n: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut seen = 0;
    let mut i = chars.len();
    while i > 0 {
        // The start of a word: a non-space after a space (or the line's start).
        if !chars[i - 1].is_whitespace() && (i == 1 || chars[i - 2].is_whitespace()) {
            seen += 1;
            if seen == n {
                return i - 1;
            }
        }
        i -= 1;
    }
    0
}

fn matching(config: &Config, only: Option<&Category>, prefix: &str) -> Vec<&'static str> {
    let lower = prefix.to_lowercase();
    let fits = |cat: &Category, name: &str| match cat.case_sensitive {
        true => name.starts_with(prefix),
        false => name.to_lowercase().starts_with(&lower),
    };
    let categories = || config.categories.iter().filter(|c| only.is_none_or(|o| o.name == c.name));
    let mut names: Vec<&'static str> = categories()
        .flat_map(|c| c.units.iter().filter(move |u| fits(c, u.symbol)).map(|u| u.symbol))
        .collect();
    // With no prefix (the target slot), the symbols are enough.
    if !prefix.is_empty() {
        for c in categories() {
            for alias in c.units.iter().flat_map(|u| u.aliases.iter()) {
                if fits(c, alias) && !names.contains(alias) {
                    names.push(alias);
                }
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(line: &str) -> Vec<&'static str> {
        // Leaked: a test's few names, compared with literals.
        let found = suggest(&Config::default(), line, line.chars().count()).map(|s| s.names).unwrap_or_default();
        found.into_iter().map(|n| &*Box::leak(n.into_boxed_str())).collect()
    }

    #[test]
    fn source_unit() {
        // In the order of the categories: Time, Data, Length, Area, Volume, Mass, Temperature…
        assert_eq!(names("10 k")[..5], ["kB", "kb", "km", "km2", "kg"]);
        assert!(names("100 + 20 sq").starts_with(&["sq-mm", "sqmm"]));
        assert_eq!(names("100 + 20 sqf"), ["sqft"]);
        // Attached to the value.
        assert_eq!(names("100sqf"), ["sqft"]);
        assert!(names("[1+2]/3 mil").contains(&"mile"));
    }

    #[test]
    fn target_unit_is_the_same_kind() {
        let to = names("10 km ");
        assert_eq!(to, ["mm", "cm", "m", "km", "au", "ly", "pc", "in", "ft", "yd", "mi"]);
        assert_eq!(names("10 km m"), ["mm", "m", "mi", "millimeter", "millimeters", "millimetre", "millimetres", "meter", "meters", "metre", "metres", "mile", "miles"]);
        assert_eq!(names("100 sqft m"), ["mm2", "m2", "mi2", "mm²", "mmq", "m²", "mq", "mi²"]);
        assert_eq!(names("1 kg p"), ["pound", "pounds"]);
    }

    #[test]
    fn data_units_are_case_sensitive() {
        assert_eq!(names("1 GB M"), ["MB", "MiB", "Mb", "Mib"], "bytes first");
        assert!(names("1 GB m").is_empty());
        // The source slot offers every kind of unit.
        assert!(names("1 M").contains(&"MB") && names("1 M").contains(&"m"));
    }

    #[test]
    fn nothing_to_suggest() {
        for line in ["", "10", "10 ", "1 +", "abc", "10 km mi ", "10 foo ", "10 zz", "2x"] {
            assert!(names(line).is_empty(), "{line}");
        }
    }

    #[test]
    fn start_of_the_word() {
        let config = Config::default();
        assert_eq!(suggest(&config, "100 + 20 sq", 11).unwrap().start, 9);
        assert_eq!(suggest(&config, "10 km ", 6).unwrap().start, 6);
        // Cursor in the middle: only the text before it counts.
        assert_eq!(suggest(&config, "10 k mi", 4).unwrap().start, 3);
    }

    /// City names (the sample archive): written as short as still finds them, shown as `Name -
    /// Country` (the region when the country has several); not after a unit or in a calculation.
    #[test]
    fn cities() {
        let config = Config::default();
        let (_, store) = crate::cities::tests::sample("suggest");
        let names = |before: &str| super::cities(&config, before, Some(&store), None).map(|s| (s.start, s.names));
        let (start, found) = names("Rome to Chi").unwrap();
        assert_eq!((start, found), (8, vec!["Chicago".to_string()]));
        let (_, found) = names("spr").unwrap();
        assert_eq!(found[0], "Springfield, Missouri, United States");
        let labels = |before: &str| super::cities(&config, before, Some(&store), None).unwrap().labels;
        assert_eq!(labels("spr")[0], "Springfield - Missouri - United States");
        // One per country: no region; written with the country (another country has one too).
        assert_eq!(labels("Pari"), ["Paris - France", "Paris - United States"]);
        assert_eq!(names("Pari").unwrap().1, ["Paris, France", "Paris, United States"]);
        // The person's wording: an alias typed is shown and written (`Parigi`, Paris's Italian name).
        assert_eq!(labels("Parig"), ["Parigi - France"]);
        assert_eq!(names("Parig").unwrap().1, ["Parigi"]);
        assert!(names("Chicago ").is_none(), "after a space: nothing yet");
        assert!(names("10 ba").is_none_or(|(_, n)| n.iter().all(|n| n != "bar")));
        assert!(names("x").is_none(), "one letter: too few");
    }
}
