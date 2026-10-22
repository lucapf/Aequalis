//! Suggestions for the word under the cursor, for every UI (the TUI's Tab, the Android app's chips):
//! units after a value (the target of the same kind as the source), currencies after an amount,
//! places (the city archive's, else the built-in capitals and major cities) and countries, the first
//! words (`now`, `today`, `distance`) and date formats after a date.

use crate::calc;
use crate::config::{Category, Config};
use crate::convert;

/// What the suggestions are: an app shows units by their symbol, once each (`km - kilometre`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Units,
    Currencies,
    /// Places, countries, the first words (`now`, `distance`).
    Words,
    /// Date formats (`unix`, `iso-week`).
    Formats,
}

#[derive(Debug, PartialEq)]
pub struct Suggestions {
    pub kind: Kind,
    /// Position (in chars) where the word being completed starts.
    pub start: usize,
    /// What is written in its place.
    pub names: Vec<String>,
    /// What is shown for each (cities: `Milano - Italy`, currencies: `USD - US dollar`); the
    /// names themselves when empty.
    pub labels: Vec<String>,
}

/// At most this many suggestions.
pub const MAX: usize = 8;

/// What fits the word ending at `cursor`, once it has `min` characters (the phone: 3, while a few
/// letters still say little; the TUI: 0). After a value, a unit or currency from its first letter:
/// they are short, and a case may matter (`1 gb` offers `GB` and `Gb`). After its unit, the target
/// units of the same kind at once, nothing typed (`10 cm ` offers `mm`, `m`, `in`…): the kind chosen
/// stays.
pub fn suggest(config: &Config, line: &str, cursor: usize, min: usize) -> Option<Suggestions> {
    // A request too long for the engine: nothing offered (`crate::check_length`).
    crate::check_length(line).ok()?;
    let before: String = line.chars().take(cursor).collect();
    let typing = !before.ends_with(char::is_whitespace);
    let prefix = if typing { before.split_whitespace().last().unwrap_or("") } else { "" };
    let value = calc::parse_prefix(&before).ok();
    let min = match value {
        // The target slot (`10 cm `, `12 EUR `): the source says the kind, its units are few.
        Some((_, rest)) if !typing && rest.split_whitespace().filter(|w| !crate::currency::connector(w)).count() == 1 => 0,
        Some(_) => min.min(1),
        None => min,
    };
    // A size request: its words first (`scar` → `scarpe - shoes`); after a shoe size, the systems.
    let sized = crate::sizes::suggestions(&before, typing);
    if let Some((start, systems)) = sized.as_ref().filter(|_| !typing) {
        let (names, labels) = systems.iter().cloned().unzip();
        return Some(Suggestions { kind: Kind::Words, start: *start, names, labels });
    }
    if prefix.chars().count() < min {
        return None;
    }
    let found = match value {
        Some((_, rest)) => after_a_value(config, &before, rest, typing),
        // Not a value: a place (a distance, a time there), a first word, a date's format.
        None => words(config, &before, typing),
    };
    let found = match (sized, found) {
        (Some((start, words)), found) => with_size_words(start, words, found),
        (None, found) => found?,
    };
    (!found.names.is_empty()).then_some(found)
}

/// The size words first, then what else fits, at most `MAX`.
fn with_size_words(start: usize, words: Vec<(String, String)>, found: Option<Suggestions>) -> Suggestions {
    let (mut names, mut labels): (Vec<String>, Vec<String>) = words.into_iter().unzip();
    let kind = found.as_ref().map_or(Kind::Words, |f| f.kind);
    for f in found.into_iter().filter(|f| f.start == start) {
        let shown = if f.labels.is_empty() { f.names.clone() } else { f.labels };
        for (name, label) in f.names.into_iter().zip(shown) {
            if names.len() < MAX && !names.contains(&name) {
                names.push(name);
                labels.push(label);
            }
        }
    }
    Suggestions { kind, start, names, labels }
}

/// After a value: its unit, or after the unit the target unit of the same kind; for an amount, a
/// currency, then the target currency (`to`, `in` or `->` between allowed).
fn after_a_value(config: &Config, before: &str, rest: &str, typing: bool) -> Option<Suggestions> {
    let words: Vec<&str> = rest.split_whitespace().filter(|w| !crate::currency::connector(w)).collect();
    let (done, prefix) = match words.split_last() {
        Some((last, done)) if typing => (done, *last),
        _ => (&words[..], ""),
    };
    let start = before.chars().count() - prefix.chars().count();
    // The source: any unit, or a currency.
    let category = match done {
        [] => None,
        [from] => match convert::find_unit(config, from) {
            Some((category, _)) => Some(category),
            None if currency_codes(from) => {
                let (names, labels) = currencies(prefix, Some(from));
                return Some(Suggestions { kind: Kind::Currencies, start, names, labels });
            }
            None => return None,
        },
        _ => return None,
    };
    // An empty source unit would list every unit; wait for the first letter.
    if prefix.is_empty() && category.is_none() {
        return None;
    }
    // The target: any unit of the source's kind but the source itself (`10 cm `: not `cm`).
    let source = done.first().copied();
    let mut names: Vec<String> = matching(config, category, prefix).into_iter().filter(|n| Some(*n) != source).map(str::to_string).collect();
    let mut labels = Vec::new();
    let mut kind = Kind::Units;
    // An amount (no unit yet): currencies too, after the units, from two letters (`x` is times).
    // Nothing at all, from four letters: a misspelt currency (`dolar`: the dollars).
    if category.is_none() && names.is_empty() && prefix.chars().count() >= 4 {
        let (near, _) = crate::currency::near(prefix);
        if !near.is_empty() {
            let (names, labels) = near.into_iter().map(|(code, name)| (code.to_string(), format!("{code} - {name}"))).unzip();
            return Some(Suggestions { kind: Kind::Currencies, start, names, labels });
        }
    }
    if category.is_none() && prefix.chars().count() >= 2 {
        let (codes, shown) = currencies(prefix, None);
        if !codes.is_empty() {
            kind = if names.is_empty() { Kind::Currencies } else { Kind::Units };
            labels = names.clone();
            for (code, label) in codes.into_iter().zip(shown) {
                if !names.contains(&code) {
                    names.push(code);
                    labels.push(label);
                }
            }
        }
    }
    Some(Suggestions { kind, start, names, labels })
}

/// Whether `word` is one currency (`usd`, `euro`, `€`): a target currency follows.
fn currency_codes(word: &str) -> bool {
    crate::currency::exact(word).is_some_and(|codes| codes.len() == 1)
}

/// Currencies whose code, name or another of their words starts with `prefix` (any case, accents
/// aside), the most used first, but not `besides` (the source): written as their code, shown as
/// `USD - US dollar`.
fn currencies(prefix: &str, besides: Option<&str>) -> (Vec<String>, Vec<String>) {
    let typed = crate::time::tz::key(prefix);
    let source = besides.and_then(crate::currency::exact).and_then(|codes| codes.first().copied());
    let mut names = Vec::new();
    let mut labels = Vec::new();
    for c in crate::currency::all() {
        let fits = typed.is_empty() || crate::currency::starts_with(c, &typed);
        if fits && Some(c.code) != source {
            names.push(c.code.to_string());
            labels.push(format!("{} - {}", c.code, c.name));
        }
        if names.len() == MAX {
            break;
        }
    }
    (names, labels)
}

/// Not after a value: a date's format (`now uni`), else places and countries, and the first words.
fn words(config: &Config, before: &str, typing: bool) -> Option<Suggestions> {
    let prefix = if typing { before.split_whitespace().last().unwrap_or("") } else { "" };
    let start = before.chars().count() - prefix.chars().count();
    let earlier = before[..before.len() - prefix.len()].trim_end();
    // A format after a date: `now uni` → unix (else, `Rome to Tok`, a place).
    if !earlier.is_empty() && crate::time::is_date_request(earlier) {
        let formats: Vec<String> = crate::config::DATE_WORDS[3..]
            .iter()
            .map(|f| f.to_string())
            .chain(crate::time::pattern_words())
            .filter(|f| !prefix.is_empty() && starts_with_any_case(f, prefix))
            .take(MAX)
            .collect();
        if !formats.is_empty() {
            return Some(Suggestions { kind: Kind::Formats, start, names: formats, labels: Vec::new() });
        }
    }
    // The archive's cities, for a distance (written as short as still finds them).
    if let Some(found) = cities(config, before, crate::cities::store().as_deref(), crate::geo::local_country().as_deref()) {
        return Some(found);
    }
    // Without the archive: the built-in capitals and major cities, the countries, the first words.
    if prefix.chars().count() < 2 {
        return None;
    }
    let typed = crate::time::tz::key(prefix);
    let first = earlier.is_empty();
    let keywords = ["now", "today", "distance"].into_iter().filter(|_| first).map(|k| (k, k, true));
    let places = crate::time::tz::place_words().map(|(name, key)| (name, key, false));
    let countries = crate::time::tz::country_words().map(|(name, key)| (name, key, false));
    let mut names: Vec<String> = Vec::new();
    for (name, key, keyword) in keywords.chain(places).chain(countries) {
        // A place whose name is something else to aequalis (`Bar`, a unit) is not offered (asked only
        // of the names that fit: it looks the name up as a unit, a date word and a currency).
        let fits = key.starts_with(&typed) && !names.iter().any(|n| n == name);
        if fits && (keyword || !crate::time::reserved(config, name)) {
            names.push(name.to_string());
        }
        if names.len() == MAX {
            break;
        }
    }
    // Nothing starts so: a misspelling, from four letters (`Room`: Rome, `Tokio`: Tokyo); not in
    // a calculation being typed (`12+4/2*s`: no place has an operator in its name).
    if names.is_empty() && prefix.chars().count() >= 4 && !prefix.contains(['+', '*', '/', '^', '=', '(', ')', '[', ']', '%']) {
        return misspelt_places(prefix, start);
    }
    Some(Suggestions { kind: Kind::Words, start, names, labels: Vec::new() })
}

/// The places and countries a misspelt word may be, within two letters, as Enter finds them: the
/// archive's cities (if any), the built-in places, the countries; only the nearest (`Tokio`: Tokyo,
/// one letter off, not Togo, two).
fn misspelt_places(typed: &str, start: usize) -> Option<Suggestions> {
    let tz = crate::time::tz::distance;
    // (letters off, what is written, how it is shown)
    let mut found: Vec<(usize, String, String)> = Vec::new();
    if let Some(store) = crate::cities::store() {
        for city in store.near(typed) {
            let (label, name) = store.offer(&city, &city.name);
            let names: Vec<&str> = std::iter::once(city.name.as_str()).chain(city.aliases.iter().map(String::as_str)).collect();
            found.push((tz(typed, &names), name, label));
        }
    }
    for name in crate::time::tz::near_names(typed) {
        found.push((tz(typed, &[name]), name.to_string(), name.to_string()));
    }
    for country in crate::time::tz::near_countries(typed) {
        let names: Vec<&str> = std::iter::once(country.name).chain(country.aliases.iter().copied()).collect();
        found.push((tz(typed, &names), country.name.to_string(), country.name.to_string()));
    }
    let best = found.iter().map(|f| f.0).min()?;
    let (mut names, mut labels): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for (_, name, label) in found.into_iter().filter(|f| f.0 == best) {
        if !names.contains(&name) && names.len() < MAX {
            names.push(name);
            labels.push(label);
        }
    }
    Some(Suggestions { kind: Kind::Words, start, names, labels })
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
        return Some(Suggestions { kind: Kind::Words, start: before_start(before, n), names, labels });
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
    // A category whose case matters, written in a case one of its units has (`k`, `Gb`): that case
    // only (`kB` and `kb`, not `KiB`). Written in a case none has (`gb`, `GiB` aside), or a category
    // where case doesn't matter: any case (`GB`, `Gb`, to choose the right one).
    let exact_case =
        |cat: &Category| cat.case_sensitive && cat.units.iter().any(|u| std::iter::once(&u.symbol).chain(u.aliases.iter()).any(|n| n.starts_with(prefix)));
    // Each category with how its names are compared, decided once.
    let categories: Vec<(&Category, bool)> = config.categories.iter().filter(|c| only.is_none_or(|o| o.name == c.name)).map(|c| (c, exact_case(c))).collect();
    let fits = |exact: bool, name: &str| match exact {
        true => name.starts_with(prefix),
        false => starts_with_any_case(name, prefix),
    };
    let mut names: Vec<&'static str> =
        categories.iter().flat_map(|&(c, exact)| c.units.iter().filter(move |u| fits(exact, u.symbol)).map(|u| u.symbol)).collect();
    // With no prefix (the target slot), the symbols are enough.
    if !prefix.is_empty() {
        for &(c, exact) in &categories {
            for alias in c.units.iter().flat_map(|u| u.aliases.iter()) {
                if fits(exact, alias) && !names.contains(alias) {
                    names.push(alias);
                }
            }
        }
    }
    names
}

/// Whether `text` starts with `prefix`, case aside (any letters, not only ASCII ones).
fn starts_with_any_case(text: &str, prefix: &str) -> bool {
    let mut text = text.chars().flat_map(char::to_lowercase);
    prefix.chars().flat_map(char::to_lowercase).all(|p| text.next() == Some(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(line: &str) -> Vec<&'static str> {
        // Leaked: a test's few names, compared with literals.
        let found = suggest(&Config::default(), line, line.chars().count(), 0).map(|s| s.names).unwrap_or_default();
        found.into_iter().map(|n| &*Box::leak(n.into_boxed_str())).collect()
    }

    #[test]
    fn size_words_and_systems() {
        let all = |line: &str| suggest(&Config::default(), line, line.chars().count(), 0).map(|s| (s.names, s.labels));
        let (names, labels) = all("scar").unwrap();
        assert_eq!((names[0].as_str(), labels[0].as_str()), ("scarpa", "scarpa - shoes"));
        assert!(labels.contains(&"scarpe - shoes".to_string()), "{labels:?}");
        assert_eq!(all("42 eu don").unwrap().1[0], "donna - women");
        // After a shoe size with no system: the systems; not after an amount alone.
        assert_eq!(all("shoes 42 ").unwrap().0, ["eu", "uk", "us", "jp", "cn"]);
        assert_ne!(all("42 ").map(|s| s.0).unwrap_or_default(), ["eu", "uk", "us", "jp", "cn"]);
        assert_eq!(all("shoes 42 eu "), None);
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
        assert_eq!(to, ["mm", "cm", "m", "au", "ly", "pc", "in", "ft", "yd", "mi"], "not the source");
        assert_eq!(
            names("10 km m"),
            ["mm", "m", "mi", "millimeter", "millimeters", "millimetre", "millimetres", "meter", "meters", "metre", "metres", "mile", "miles"]
        );
        assert_eq!(names("100 sqft m"), ["mm2", "m2", "mi2", "mm²", "mmq", "m²", "mq", "mi²"]);
        assert_eq!(names("1 kg p"), ["pound", "pounds"]);
    }

    #[test]
    fn data_units_are_case_sensitive() {
        assert_eq!(names("1 GB M"), ["MB", "MiB", "Mb", "Mib"], "bytes first");
        // A case no unit has (a phone's keyboard writes lower case): every case, to choose.
        assert_eq!(names("1 GB m"), ["MB", "MiB", "Mb", "Mib"]);
        assert_eq!(names("1 gb"), ["GB", "Gb", "GBP"], "and the pound sterling");
        // A case one has: that one only.
        assert_eq!(names("1 Gb"), ["Gb", "GBP"]);
        // The source slot offers every kind of unit.
        assert!(names("1 M").contains(&"MB") && names("1 M").contains(&"m"));
    }

    /// More than units, for the phone (from the third letter, but a value's unit from its first):
    /// currencies after an amount and after a currency; without the archive, the built-in places and
    /// countries, the first words; formats after a date.
    #[test]
    fn more_than_units() {
        let config = Config::default();
        let at = |line: &str| suggest(&config, line, line.chars().count(), 3).map(|s| s.names).unwrap_or_default();
        assert!(at("Ro").is_empty(), "two letters of a place");
        // After a value, a unit from its first letter, in each case that means one (`gb`: neither).
        let gb = at("1 gb");
        assert!(gb.contains(&"GB".to_string()) && gb.contains(&"Gb".to_string()), "{gb:?}");
        assert!(at("12 k").contains(&"km".to_string()), "{:?}", at("12 k"));
        // After the unit, nothing typed yet: the units of its kind (the kind chosen stays).
        assert_eq!(at("10 cm "), ["mm", "m", "km", "au", "ly", "pc", "in", "ft", "yd", "mi"]);
        assert_eq!(&at("12 EUR ")[..3], ["USD", "JPY", "GBP"], "the most used first");
        assert!(at("10 ").is_empty() && at("10 cm mm ").is_empty(), "no unit yet, or both written");
        assert_eq!(&at("12 dol")[..3], ["USD", "CAD", "AUD"]);
        assert_eq!(at("12 usd eur"), ["EUR"]);
        assert_eq!(at("12 usd to eur"), ["EUR"]);
        assert_eq!(at("12 usd -> eur"), ["EUR"]);
        assert_eq!(at("10 cm to "), at("10 cm "));
        assert_eq!(at("10 cm -> "), at("10 cm "));
        assert_eq!(at("Par"), ["Paris", "Paramaribo", "Paraguay"]);
        assert_eq!(at("Rome to Tok"), ["Tokyo"]);
        assert_eq!(at("now uni"), ["unix", "unix-ms", "unix-us", "unix-ns"]);
        assert_eq!(at("dis"), ["distance"]);
        assert_eq!(at("Bar"), ["Barcelona", "Barbados"], "not Bar: a unit");
        // Misspelt, from four letters: what Enter would ask about.
        assert_eq!(at("Room"), ["Rome"]);
        assert_eq!(at("Tokio"), ["Tokyo"]);
        assert_eq!(at("Jappan"), ["Japan"]);
        assert_eq!(&at("12 dolar")[..2], ["USD", "CAD"]);
        assert!(at("Xyzzy").is_empty(), "too far from any place");
        assert!(at("12+4/2*s").is_empty() && at("(1+2").is_empty(), "a calculation: no place");
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
        assert_eq!(suggest(&config, "100 + 20 sq", 11, 0).unwrap().start, 9);
        assert_eq!(suggest(&config, "10 km ", 6, 0).unwrap().start, 6);
        // Cursor in the middle: only the text before it counts.
        assert_eq!(suggest(&config, "10 k mi", 4, 0).unwrap().start, 3);
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
