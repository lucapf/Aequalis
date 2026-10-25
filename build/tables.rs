//! The tables of names, written at build time as Rust statics into `OUT_DIR`, which the library
//! `include!`s: the countries and the built-in places, normalised and sorted to be searched by name
//! (`time/tz.rs`); the currencies and their index (`currency.rs`); the places' coordinates
//! (`geo.rs`). Every process used to parse the data files and build them (about 1 ms each); now
//! the binary holds them ready, read-only. The names are normalised with the library's own
//! functions (`src/time/names.rs`), so a name looked up is written as the tables write it.

use crate::config::Config;
use crate::json::{self, Value};
use crate::names::{key, sorted, squeeze, CITIES, EURO_AREA};
use std::collections::BTreeMap;
use std::path::Path;

/// Writes `countries.rs`, `currencies.rs`, `places.rs`, `units.rs` and `sizes.rs` into `out`.
pub fn write(out: &Path) {
    let read = |name: &str| {
        let text = std::fs::read_to_string(format!("src/data/{name}.json")).unwrap_or_else(|e| panic!("src/data/{name}.json: {e}"));
        json::parse(&text).unwrap_or_else(|e| panic!("src/data/{name}.json: {e}"))
    };
    let save = |name: &str, text: String| std::fs::write(out.join(name), text).unwrap_or_else(|e| panic!("{name}: {e}"));
    save("countries.rs", countries_rs(&countries(read("countries"))));
    save("currencies.rs", currencies_rs(&currencies(read("currencies"))));
    save("places.rs", places_rs(read("places")));
    save("units.rs", units_rs());
    save("sizes.rs", sizes_rs(read("sizes")));
}

/// `SHOE_CHARTS`, `CLOTHES_CHARTS`, `ITEMS`, `BODY` and `SIZE_WORDS` (`sizes.rs`): the shoe charts,
/// their foot lengths in cm; the clothes charts, every system filled in (the men's continental number
/// in EU, IT, FR and DE); every word of a size request normalised (`key`), sorted to be found by
/// name, the first of a word written twice winning.
fn sizes_rs(mut doc: Value) -> String {
    let mut out = String::from("/// The shoe charts of `src/data/sizes.json`, in its order (`build.rs`).\nstatic SHOE_CHARTS: &[ShoeChart] = &[\n");
    for mut chart in doc.take("charts").into_array() {
        let person = person(&chart.take("person").into_string().expect("a chart's person"));
        let brand = lit(&chart.take("brand").into_string().expect("a chart's brand"));
        let inches = chart.take("foot").into_string().as_deref() == Some("in");
        let columns = chart.take("columns").into_strings();
        let at = |name: &str| columns.iter().position(|c| c == name);
        out += &format!("    ShoeChart {{ person: {person}, brand: {brand}, rows: &[\n");
        for row in chart.take("rows").into_array() {
            let cells = row.into_strings();
            let cell = |name: &str| at(name).map_or("", |i| cells[i].as_str());
            let foot = cell("foot");
            let cm = if inches { inches_value(foot) * 2.54 } else { foot.parse::<f64>().expect("a foot length in mm") / 10.0 };
            let (us, uk, eu, jp) = (lit(cell("us")), lit(cell("uk")), lit(cell("eu")), lit(cell("jp")));
            out += &format!("        ShoeRow {{ us: {us}, uk: {uk}, eu: {eu}, jp: {jp}, foot_cm: {cm:?} }},\n");
        }
        out += "    ] },\n";
    }
    out += "];\n\n";
    out += &clothes_rs(doc.take("clothes"));
    let mut words = doc.take("words");
    let weak = words.take("weak").into_strings();
    let mut table: BTreeMap<String, String> = BTreeMap::new();
    let mut add = |list: Vec<String>, word: String| {
        for w in list {
            table.entry(key(&w)).or_insert_with(|| word.clone());
        }
    };
    out += "/// The items of a size request: (name, not answered yet).\nstatic ITEMS: &[(&str, bool)] = &[\n";
    for (i, mut item) in words.take("items").into_array().into_iter().enumerate() {
        let name = item.take("item").into_string().expect("an item's name");
        let coming = item.get("coming").is_some();
        out += &format!("    ({}, {coming}),\n", lit(&name));
        add(item.take("words").into_strings(), format!("Word::Item({i})"));
    }
    out += "];\n\n/// The body measures: (measure, its item, not answered yet).\nstatic BODY: &[(&str, &str, bool)] = &[\n";
    for (i, mut body) in words.take("body").into_array().into_iter().enumerate() {
        let (measure, item) = (body.take("measure").into_string().expect("a measure"), body.take("item").into_string().expect("its item"));
        let coming = body.get("coming").is_some();
        out += &format!("    ({}, {}, {coming}),\n", lit(&measure), lit(&item));
        add(body.take("words").into_strings(), format!("Word::Body({i})"));
    }
    for mut p in words.take("persons").into_array() {
        let person = person(&p.take("person").into_string().expect("a person"));
        add(p.take("words").into_strings(), format!("Word::Person({person})"));
    }
    for mut a in words.take("ages").into_array() {
        let age = match a.take("age").into_string().as_deref() {
            Some("toddler") => "Age::Toddler",
            Some("youth") => "Age::Youth",
            other => panic!("src/data/sizes.json: no age {other:?}"),
        };
        add(a.take("words").into_strings(), format!("Word::Age({age})"));
    }
    add(words.take("size").into_strings(), "Word::Size".to_string());
    for mut s in words.take("systems").into_array() {
        let system = system(&s.take("system").into_string().expect("a system"));
        add(s.take("words").into_strings(), format!("Word::System({system})"));
    }
    add(words.take("fillers").into_strings(), "Word::Filler".to_string());
    let weak: Vec<String> = weak.iter().map(|w| key(w)).collect();
    out += "];\n\n/// Every word of a size request (`key`), sorted: (word, what it is, counted only beside another).\nstatic SIZE_WORDS: &[(&str, Word, bool)] = &[\n";
    for (k, word) in &table {
        out += &format!("    ({}, {word}, {}),\n", lit(k), weak.contains(k));
    }
    out + "];\n"
}

/// `CLOTHES_CHARTS`: each chart's rows with a size in every system (empty: none) and the body
/// measures in cm (0: none).
fn clothes_rs(charts: Value) -> String {
    let mut out =
        String::from("/// The clothes charts of `src/data/sizes.json`, in priority order (`build.rs`).\nstatic CLOTHES_CHARTS: &[ClothesChart] = &[\n");
    for mut chart in charts.into_array() {
        let id = chart.take("chart").into_string().expect("a chart's name");
        let kind = match id.as_str() {
            "women" => "Chart::Women",
            "suit" => "Chart::Suit",
            "trousers" => "Chart::Trousers",
            "shirt" => "Chart::Shirt",
            other => panic!("src/data/sizes.json: no clothes chart `{other}`"),
        };
        let person = person(&chart.take("person").into_string().expect("a chart's person"));
        let columns = chart.take("columns").into_strings();
        out += &format!("    ClothesChart {{ chart: {kind}, person: {person}, rows: &[\n");
        for row in chart.take("rows").into_array() {
            let cells = row.into_strings();
            let cell = |name: &str| columns.iter().position(|c| c == name).map_or("", |i| cells[i].as_str());
            let measure = |name: &str| if cell(name).is_empty() { 0.0 } else { cell(name).parse::<f64>().expect("a measure in cm") };
            // The men's charts: US = UK, one continental number; the shirts have no IT size.
            let (us, eu) = (cell("us"), if id == "women" { cell("de") } else { cell("eu") });
            let uk = if id == "women" { cell("uk") } else { us };
            let (it, fr, de) = match id.as_str() {
                "women" => (cell("it"), cell("fr"), cell("de")),
                "shirt" => ("", eu, eu),
                _ => (eu, eu, eu),
            };
            let (chest, neck) = match id.as_str() {
                "suit" => (us.parse::<f64>().expect("a chest in inches") * 2.54, 0.0),
                "shirt" => (0.0, eu.parse::<f64>().expect("a collar in cm")),
                _ => (0.0, 0.0),
            };
            let sizes = [us, uk, eu, it, fr, de, cell("jp"), cell("int"), cell("cn")].map(lit);
            let [us, uk, eu, it, fr, de, jp, int, cn] = sizes;
            let (bust, waist, hip) = (measure("bust"), measure("waist"), measure("hip"));
            out += &format!(
                "        ClothesRow {{ us: {us}, uk: {uk}, eu: {eu}, it: {it}, fr: {fr}, de: {de}, jp: {jp}, int: {int}, cn: {cn}, bust: {bust:?}, waist: {waist:?}, hip: {hip:?}, chest: {chest:?}, neck: {neck:?} }},\n"
            );
        }
        out += "    ] },\n";
    }
    out + "];\n\n"
}

/// A system's variant: `us` → `System::Us`.
fn system(code: &str) -> &'static str {
    match code {
        "us" => "System::Us",
        "uk" => "System::Uk",
        "eu" => "System::Eu",
        "jp" => "System::Jp",
        "cn" => "System::Cn",
        "it" => "System::It",
        "fr" => "System::Fr",
        "de" => "System::De",
        "int" => "System::Int",
        other => panic!("src/data/sizes.json: no system `{other}`"),
    }
}

fn person(name: &str) -> &'static str {
    match name {
        "men" => "Person::Men",
        "women" => "Person::Women",
        "kids" => "Person::Kids",
        other => panic!("src/data/sizes.json: no person `{other}`"),
    }
}

/// `9 5/16` → 9.3125.
fn inches_value(text: &str) -> f64 {
    text.split_whitespace()
        .map(|part| match part.split_once('/') {
            Some((n, d)) => n.parse::<f64>().expect("a numerator") / d.parse::<f64>().expect("a denominator"),
            None => part.parse::<f64>().expect("inches"),
        })
        .sum()
}

/// `UNITS_EXACT` and `UNITS_LOWER` (`convert.rs`): every unit by symbol and alias, the first unit
/// with a name winning, as (category, unit); in lower case only where case does not matter.
fn units_rs() -> String {
    let (mut exact, mut lower) = (BTreeMap::new(), BTreeMap::new());
    for (c, category) in Config::default().categories.iter().enumerate() {
        for (u, unit) in category.units.iter().enumerate() {
            let at = format!("({}, {})", u8::try_from(c).expect("a few classes"), u8::try_from(u).expect("a few units"));
            for name in std::iter::once(unit.symbol).chain(unit.aliases.iter().copied()) {
                exact.entry(name.to_string()).or_insert_with(|| at.clone());
                if !category.case_sensitive {
                    lower.entry(name.to_lowercase()).or_insert_with(|| at.clone());
                }
            }
        }
    }
    format!("static UNITS_EXACT: &[(&str, (u8, u8))] = {};\nstatic UNITS_LOWER: &[(&str, (u8, u8))] = {};\n", pairs(exact), pairs(lower))
}

/// A string literal.
fn lit(text: &str) -> String {
    format!("{text:?}")
}

/// A slice of string literals.
fn lits<T: AsRef<str>>(list: &[T]) -> String {
    format!("&[{}]", list.iter().map(|t| lit(t.as_ref())).collect::<Vec<_>>().join(", "))
}

/// A slice of (name, number) pairs.
fn pairs<T: std::fmt::Display>(list: impl IntoIterator<Item = (String, T)>) -> String {
    format!("&[{}]", list.into_iter().map(|(k, v)| format!("({}, {v})", lit(&k))).collect::<Vec<_>>().join(", "))
}

/// A slice of (name, positions) pairs, sorted by name.
fn lists(map: &BTreeMap<String, Vec<u16>>) -> String {
    let item = |(k, v): (&String, &Vec<u16>)| format!("({}, &{v:?})", lit(k));
    format!("&[{}]", map.iter().map(item).collect::<Vec<_>>().join(", "))
}

fn position(i: usize) -> u16 {
    u16::try_from(i).expect("a few hundred names")
}

struct Country {
    code: String,
    name: String,
    aliases: Vec<String>,
    cities: Vec<String>,
    currency: String,
}

fn countries(mut doc: Value) -> Vec<Country> {
    let text = |c: &mut Value, k: &str| c.take(k).into_string().unwrap_or_default();
    doc.take("countries")
        .into_array()
        .into_iter()
        .map(|mut c| Country {
            code: text(&mut c, "code"),
            name: text(&mut c, "name"),
            aliases: c.take("aliases").into_strings(),
            cities: c.take("cities").into_strings(),
            currency: text(&mut c, "currency"),
        })
        .filter(|c| !c.name.is_empty())
        .collect()
}

/// `COUNTRY_TABLE` and `NAMES` (`time/tz.rs`).
fn countries_rs(countries: &[Country]) -> String {
    let mut out = String::from("/// The countries of `src/data/countries.json`, in its order (`build.rs`).\nstatic COUNTRY_TABLE: &[Country] = &[\n");
    for c in countries {
        let (code, name, currency) = (lit(&c.code), lit(&c.name), lit(&c.currency));
        out += &format!("    Country {{ code: {code}, name: {name}, aliases: {}, cities: {}, currency: {currency} }},\n", lits(&c.aliases), lits(&c.cities));
    }
    out + "];\n\n" + &names_rs(countries)
}

/// `NAMES`: the places' and the countries' names normalised, sorted to be found by name; the
/// countries by ISO code and by time zone.
fn names_rs(countries: &[Country]) -> String {
    let city_words: Vec<String> = CITIES.iter().map(|(n, _)| key(n)).collect();
    let longest_city = city_words.iter().map(|k| k.split(' ').count()).max().unwrap_or(1);
    let city_keys: Vec<String> = city_words.iter().map(|k| k.replace(' ', "")).collect();
    let country_words: Vec<String> = countries.iter().map(|c| key(&c.name)).collect();
    let (mut country_keys, mut codes, mut zones) = (Vec::new(), BTreeMap::new(), BTreeMap::new());
    let mut longest_country = 1;
    for (i, c) in countries.iter().enumerate() {
        codes.entry(c.code.clone()).or_insert(position(i));
        for name in std::iter::once(&c.name).chain(&c.aliases) {
            let k = key(name);
            longest_country = longest_country.max(k.split(' ').count());
            country_keys.push((k.replace(' ', ""), position(i)));
        }
        // A zone belongs to the first country with a city in it.
        for city in &c.cities {
            if let Some(ci) = city_keys.iter().position(|k| *k == squeeze(city)) {
                for zone in CITIES[ci].1.split('|') {
                    zones.entry(zone.to_string()).or_insert(position(i));
                }
            }
        }
    }
    let city_order = sorted(city_keys.len(), |i| &city_keys[i]);
    let country_order = sorted(country_keys.len(), |i| &country_keys[i].0);
    format!(
        "static NAMES: Names = Names {{\n    city_keys: {},\n    city_order: &{city_order:?},\n    city_words: {},\n    country_words: {},\n    country_keys: {},\n    country_order: &{country_order:?},\n    codes: {},\n    zones: {},\n    longest_city: {longest_city},\n    longest_country: {longest_country},\n}};\n",
        lits(&city_keys),
        lits(&city_words),
        lits(&country_words),
        pairs(country_keys),
        pairs(codes),
        pairs(zones),
    )
}

struct Currency {
    code: String,
    name: String,
    words: Vec<String>,
    symbols: Vec<String>,
    abbreviations: Vec<String>,
}

fn currencies(mut doc: Value) -> Vec<Currency> {
    let text = |c: &mut Value, k: &str| c.take(k).into_string().unwrap_or_default();
    doc.take("currencies")
        .into_array()
        .into_iter()
        .map(|mut c| Currency {
            code: text(&mut c, "code"),
            name: text(&mut c, "name"),
            words: c.take("words").into_strings(),
            symbols: c.take("symbols").into_strings(),
            abbreviations: c.take("abbreviations").into_strings(),
        })
        .filter(|c| !c.code.is_empty() && !c.name.is_empty())
        .collect()
}

/// `CURRENCY_TABLE` and `INDEX` (`currency.rs`).
fn currencies_rs(currencies: &[Currency]) -> String {
    let mut out =
        String::from("/// The currencies of `src/data/currencies.json`, the most used first (`build.rs`).\nstatic CURRENCY_TABLE: &[Currency] = &[\n");
    for c in currencies {
        let (code, name) = (lit(&c.code), lit(&c.name));
        let (words, symbols, abbreviations) = (lits(&c.words), lits(&c.symbols), lits(&c.abbreviations));
        out += &format!("    Currency {{ code: {code}, name: {name}, words: {words}, symbols: {symbols}, abbreviations: {abbreviations} }},\n");
    }
    out + "];\n\n" + &index_rs(currencies)
}

/// `INDEX`: the ways currencies are written, normalised and sorted to be found by name.
fn index_rs(currencies: &[Currency]) -> String {
    let (mut codes, mut names, mut abbreviations, mut symbols) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let mut forms: Vec<(String, u16)> = Vec::new();
    // Each currency once per name, in the order of the currencies.
    let add = |map: &mut BTreeMap<String, Vec<u16>>, k: String, i: u16| {
        if !k.is_empty() {
            let list = map.entry(k).or_default();
            if list.last() != Some(&i) {
                list.push(i);
            }
        }
    };
    for (i, c) in currencies.iter().enumerate() {
        let i = position(i);
        codes.entry(c.code.clone()).or_insert(i);
        for form in std::iter::once(&c.name).chain(&c.words) {
            let k = key(form);
            // For misspellings: names and words of 4 letters or more, squeezed, and their plurals.
            if form.chars().count() >= 4 {
                let squeezed = k.replace(' ', "");
                forms.push((format!("{squeezed}s"), i));
                forms.push((squeezed, i));
            }
            add(&mut names, k, i);
        }
        for a in &c.abbreviations {
            add(&mut abbreviations, key(a), i);
        }
        for symbol in &c.symbols {
            add(&mut symbols, symbol.to_lowercase(), i);
        }
    }
    let euro_area: Vec<String> = EURO_AREA.iter().map(|r| key(r)).collect();
    format!(
        "static INDEX: Index = Index {{\n    codes: {},\n    names: {},\n    abbreviations: {},\n    symbols: {},\n    euro_area: {},\n    forms: {},\n}};\n",
        pairs(codes),
        lists(&names),
        lists(&abbreviations),
        lists(&symbols),
        lits(&euro_area),
        pairs(forms),
    )
}

/// `PLACES` (`geo.rs`): the places' coordinates, by name as shown.
fn places_rs(mut doc: Value) -> String {
    let mut out = String::from("/// The coordinates of `src/data/places.json` (`build.rs`).\nstatic PLACES: &[(&str, Point)] = &[\n");
    for (name, v) in doc.take("places").into_members() {
        if let [lat, lon] = v.array() {
            let (lat, lon) = (lat.number().expect("a latitude"), lon.number().expect("a longitude"));
            out += &format!("    ({}, Point {{ lat: {lat:?}, lon: {lon:?} }}),\n", lit(&name));
        }
    }
    out + "];\n"
}
