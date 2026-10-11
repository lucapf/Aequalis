//! Unit suggestions for the word under the cursor in the expression bar.

use crate::calc;
use crate::config::{Category, Config};
use crate::convert;

#[derive(Debug, PartialEq)]
pub struct Suggestions {
    /// Position (in chars) where the word being completed starts.
    pub start: usize,
    pub names: Vec<&'static str>,
}

/// Units that fit the word ending at `cursor`: the source unit after the expression, then the target
/// unit, which must be of the same kind as the source. Symbols come before aliases.
pub fn suggest(config: &Config, line: &str, cursor: usize) -> Option<Suggestions> {
    let before: String = line.chars().take(cursor).collect();
    let (_, rest) = calc::parse_prefix(&before).ok()?;
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
    let names = matching(config, category, prefix);
    (!names.is_empty()).then(|| Suggestions { start: cursor - prefix.chars().count(), names })
}

fn matching(config: &Config, only: Option<&Category>, prefix: &str) -> Vec<&'static str> {
    let lower = prefix.to_lowercase();
    let fits = |cat: &Category, name: &str| match cat.case_sensitive {
        true => name.starts_with(prefix),
        false => name.to_lowercase().starts_with(&lower),
    };
    let categories = || config.categories.iter().filter(|c| only.map_or(true, |o| o.name == c.name));
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
        suggest(&Config::default(), line, line.chars().count()).map(|s| s.names).unwrap_or_default()
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
}
