//! Trivia: useful values with a short description (`src/data/trivia.json`, compiled in), by
//! section. An entry is a value (with an optional unit), the distance between two places
//! (`geo.rs`), or a spacecraft's distance from the Earth: retrieved from JPL Horizons when the
//! device is online (`curl`), else estimated from a reference distance and speed.

use crate::config::Config;
use crate::convert;
use crate::json::{self, Value};
use std::sync::OnceLock;

const TRIVIA: &str = include_str!("data/trivia.json");

/// A spacecraft's distance, estimated: `au` at `epoch` (Unix seconds), growing at `km_per_s`.
#[derive(Debug, Clone, PartialEq)]
pub struct Estimate {
    pub epoch: i64,
    pub au: f64,
    pub km_per_s: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Value(f64),
    /// The distance between two places (names or coordinates).
    Between(String, String),
    /// A spacecraft: its JPL Horizons id (`-31`) and the estimate used offline.
    Spacecraft {
        horizons: String,
        estimate: Estimate,
    },
    /// A tip: an example request, written into the expression (`London to Paris km`).
    Tip(String),
    /// A size table: a person's shoes (Nike's chart) or a clothes chart (`sizes.rs`), a row per
    /// size, each writing its request (`42 eu men shoes`, `44 it dress`).
    Table(Table),
    /// A count of active satellites (`all`, `starlink`, `leo`, `meo`, `geo`, `heo`, `beyond`),
    /// from CelesTrak when online, else `count` as of `as_of`.
    Satellites {
        kind: String,
        as_of: String,
        count: f64,
    },
}

/// A size table of Trivia: a person's shoes or a clothes chart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Table {
    Shoes(crate::sizes::Person),
    Clothes(crate::sizes::Chart),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub section: String,
    pub name: String,
    /// A few words, shown in its row: `Mean distance from the Sun`.
    pub short: String,
    pub description: String,
    pub source: Source,
    pub unit: Option<String>,
    /// Shown (and written) in the first unit, and in brackets in the second: `["km", "au"]` →
    /// `57909050 km (0.3871 au)`.
    /// (The popup shows it in all of them: `57,909,050 km - 0.3871 au - 6.1213e-6 ly`.)
    pub show: Vec<String>,
    /// Decimals shown and written (default: the precision); with `show`, of the first unit.
    pub decimals: Option<usize>,
    /// For a spacecraft: when it left.
    pub launched: Option<String>,
    /// A planet whose distances the popup computes (`planets.rs`).
    pub planet: Option<String>,
    /// More lines for the popup: (label, text).
    pub details: Vec<(String, String)>,
    /// A link shown in the popup.
    pub wikipedia: Option<String>,
    /// The value's label in the popup: `Current distance` (default `Value`).
    pub value_label: Option<String>,
    /// A few characters that recall it, shown beside it: `♃` (Jupiter), `π`.
    pub symbol: Option<String>,
}

/// Every entry, in the file's order; the sections in their order.
pub fn entries() -> &'static [Entry] {
    static ENTRIES: OnceLock<Vec<Entry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        let mut table = parse(TRIVIA).expect("src/data/trivia.json is valid");
        // Kept for the life of the process: no spare room.
        table.shrink_to_fit();
        table
    })
}

pub fn sections() -> Vec<&'static str> {
    let mut names: Vec<&str> = Vec::new();
    for e in entries() {
        if !names.contains(&e.section.as_str()) {
            names.push(&e.section);
        }
    }
    names
}

/// Reads a trivia document; the error names the entry that is wrong.
pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let mut doc = json::parse(text)?;
    let mut out = Vec::new();
    for mut section in doc.take("sections").into_array() {
        let section_name = section.take("name").into_string().ok_or("a section without a name")?;
        for e in section.take("entries").into_array() {
            out.push(entry(&section_name, e)?);
        }
    }
    Ok(out)
}

/// An entry of the section `section`, its strings taken from the document.
fn entry(section: &str, mut e: Value) -> Result<Entry, String> {
    let name = e.take("name").into_string().ok_or(format!("an entry of {section} without a name"))?;
    let source = source(&name, &e)?;
    let text = |e: &mut Value, key: &str| e.take(key).into_string();
    Ok(Entry {
        section: section.to_string(),
        short: text(&mut e, "short").unwrap_or_default(),
        description: text(&mut e, "description").unwrap_or_default(),
        show: e.take("show").into_strings(),
        source,
        unit: text(&mut e, "unit"),
        decimals: e.get("decimals").and_then(Value::number).map(|d| d as usize),
        launched: text(&mut e, "launched"),
        planet: text(&mut e, "planet"),
        wikipedia: text(&mut e, "wikipedia"),
        value_label: text(&mut e, "value_label"),
        symbol: text(&mut e, "symbol"),
        details: e
            .take("details")
            .into_array()
            .into_iter()
            .filter_map(|pair| match <[Value; 2]>::try_from(pair.into_array()) {
                Ok([label, line]) => Some((label.into_string()?, line.into_string()?)),
                Err(_) => None,
            })
            .collect(),
        name,
    })
}

/// Where the value of the entry `name` comes from.
fn source(name: &str, e: &Value) -> Result<Source, String> {
    if let Some(v) = e.get("value").and_then(Value::number) {
        return Ok(Source::Value(v));
    }
    if let (Some(kind), Some(est)) = (e.get("satellites").and_then(Value::str), e.get("estimate")) {
        return Ok(Source::Satellites {
            kind: kind.to_string(),
            as_of: est.get("as_of").and_then(Value::str).unwrap_or_default().to_string(),
            count: est.get("count").and_then(Value::number).ok_or(format!("{name}: no estimate count"))?,
        });
    }
    if let Some(example) = e.get("tip").and_then(Value::str) {
        return Ok(Source::Tip(example.to_string()));
    }
    if let Some(table) = e.get("table").and_then(Value::str) {
        use crate::sizes::{Chart, Person};
        let table = match table {
            "men" => Table::Shoes(Person::Men),
            "women" => Table::Shoes(Person::Women),
            "kids" => Table::Shoes(Person::Kids),
            "clothes women" => Table::Clothes(Chart::Women),
            "clothes suit" => Table::Clothes(Chart::Suit),
            "clothes trousers" => Table::Clothes(Chart::Trousers),
            "clothes shirt" => Table::Clothes(Chart::Shirt),
            other => return Err(format!("{name}: no size table `{other}`")),
        };
        return Ok(Source::Table(table));
    }
    if let [a, b] = e.get("between").map(Value::array).unwrap_or_default() {
        return Ok(Source::Between(a.str().unwrap_or_default().to_string(), b.str().unwrap_or_default().to_string()));
    }
    let (Some(live), Some(est)) = (e.get("live"), e.get("estimate")) else {
        return Err(format!("{name}: needs `value`, `between`, `tip`, `satellites` + `estimate` or `live` + `estimate`"));
    };
    let epoch = est.get("epoch").and_then(Value::str).and_then(crate::time::parse_utc).ok_or(format!("{name}: a bad estimate epoch"))?;
    Ok(Source::Spacecraft {
        horizons: live.get("horizons").and_then(Value::str).ok_or(format!("{name}: no Horizons id"))?.to_string(),
        estimate: Estimate {
            epoch,
            au: est.get("au").and_then(Value::number).ok_or(format!("{name}: no estimate au"))?,
            km_per_s: est.get("km_per_s").and_then(Value::number).unwrap_or(0.0),
        },
    })
}

/// A row of a shoe table: its US size (`US 8.5`), the others and the foot (`UK 7.5 · EU 42 · JP 26.5
/// · foot 25.9 cm`), and the request it writes (`42 eu men shoes`).
#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    pub label: String,
    pub text: String,
    pub request: String,
}

/// The rows of a size table: a person's shoes (Nike's chart), a clothes chart with its body measures.
pub fn table_rows(table: Table) -> Vec<TableRow> {
    use crate::sizes::{shoe_charts, Person};
    let person = match table {
        Table::Shoes(p) => p,
        Table::Clothes(chart) => return clothes_rows(chart),
    };
    let who = match person {
        Person::Men => "men",
        Person::Women => "women",
        Person::Kids => "kids",
    };
    let Some(chart) = shoe_charts().iter().find(|c| c.person == person) else { return Vec::new() };
    chart
        .rows
        .iter()
        .map(|r| {
            let foot = crate::convert::format_value((r.foot_cm * 10.0).round() / 10.0, 1);
            TableRow {
                label: format!("US {}", r.us),
                text: format!("UK {} · EU {} · JP {} · foot {foot} cm", r.uk, r.eu, r.jp),
                request: format!("{} eu {who} shoes", r.eu),
            }
        })
        .collect()
}

/// The rows of a clothes chart: its first system (`US 8`), the others and the body measures in cm,
/// and the request a row writes (`44 it dress`).
fn clothes_rows(chart: crate::sizes::Chart) -> Vec<TableRow> {
    use crate::sizes::Chart;
    let cm = |v: f64| crate::convert::format_value((v * 10.0).round() / 10.0, 1);
    chart
        .rows()
        .iter()
        .map(|r| match chart {
            Chart::Women => TableRow {
                label: format!("US {}", r.us),
                text: format!("UK {} · DE {} · FR {} · IT {} · JP {} · {} · CN {} · bust {} · waist {} · hip {} cm", r.uk, r.de, r.fr, r.it, r.jp, r.int, r.cn, cm(r.bust), cm(r.waist), cm(r.hip)),
                request: format!("{} it dress", r.it),
            },
            Chart::Suit => TableRow {
                label: format!("US {}", r.us),
                text: [format!("EU {}", r.eu), format!("JP {}", r.jp), r.int.to_string(), format!("chest {} cm", cm(r.chest))].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "),
                request: format!("{} eu suit", r.eu),
            },
            Chart::Trousers => TableRow {
                label: format!("W{}", r.us),
                text: format!("EU {} · {} · waist {} cm", r.eu, r.int, cm(r.waist)),
                request: format!("W{} trousers", r.us),
            },
            Chart::Shirt => TableRow { label: format!("EU {}", r.eu), text: format!("US {} · {}", r.us, r.int), request: format!("{} eu men shirt", r.eu) },
        })
        .collect()
}

/// How a value was obtained.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Origin {
    Fixed,
    /// From the online data (`online.rs`), synced at this time (Unix seconds).
    Synced(i64),
    /// Estimated (offline).
    Estimated,
}

/// The entry's value in its unit, at `now` (Unix seconds). `synced`: its value in the online data
/// (`online.rs`), if any: a satellite count as it is, a spacecraft's distance (au) carried forward
/// to `now` at its speed; else the entry's estimate.
pub fn value(config: &Config, entry: &Entry, synced: Option<crate::online::Stored>, now: i64) -> Result<(f64, Origin), String> {
    // A planet: its distance now from the Earth (the Earth's: from the Sun); its `value` (the mean
    // distance from the Sun) is a line of the popup.
    if let Some(planet) = &entry.planet {
        let distances = if planet == "Earth" { crate::planets::from_sun(planet, now as f64) } else { crate::planets::from_earth(planet, now as f64) };
        return planet_value(config, entry, distances);
    }
    match &entry.source {
        Source::Value(v) => Ok((*v, Origin::Fixed)),
        Source::Tip(example) => Err(format!("a tip: {example}")),
        Source::Table(_) => Err("a table".to_string()),
        Source::Between(a, b) => {
            let (a, b) = (crate::geo::place(a)?, crate::geo::place(b)?);
            let metres = crate::geo::distance(a.point, b.point);
            let unit = entry.unit.as_deref().unwrap_or("km");
            let (v, _) = convert::convert(config, metres, "m", unit).map_err(|e| e.to_string())?;
            Ok((v, Origin::Fixed))
        }
        Source::Satellites { count, .. } => Ok(match synced {
            Some(s) => (s.value, Origin::Synced(s.at)),
            None => (*count, Origin::Estimated),
        }),
        Source::Spacecraft { estimate, .. } => {
            let (au, origin) = match synced {
                Some(s) => (s.value + estimate.km_per_s * (now - s.at) as f64 / 149_597_870.7, Origin::Synced(s.at)),
                None => (estimate.au + estimate.km_per_s * (now - estimate.epoch) as f64 / 149_597_870.7, Origin::Estimated),
            };
            let unit = entry.unit.as_deref().unwrap_or("au");
            let (v, _) = convert::convert(config, au, "au", unit).map_err(|e| e.to_string())?;
            Ok((v, origin))
        }
    }
}

/// A planet's value: its distance now from the Earth (the Earth's: from the Sun), in its unit.
fn planet_value(config: &Config, entry: &Entry, distances: Option<crate::planets::Distances>) -> Result<(f64, Origin), String> {
    let planet = entry.planet.as_deref().unwrap_or_default();
    let au = distances.ok_or(format!("{planet}: not a planet"))?.now;
    let (v, _) = convert::convert(config, au, "au", entry.unit.as_deref().unwrap_or("au")).map_err(|e| e.to_string())?;
    Ok((v, Origin::Fixed))
}

/// What an entry's popup tells, worked out once: its value and, for a planet, its distances from
/// the Sun and from the Earth. `popup` writes it, with or without the device's separators.
pub struct Facts {
    pub value: Result<(f64, Origin), String>,
    sun: Option<crate::planets::Distances>,
    earth: Option<crate::planets::Distances>,
}

/// The facts of `entry` at `now` (`synced`: its value in the online data, as for `value`).
pub fn facts(config: &Config, entry: &Entry, synced: Option<crate::online::Stored>, now: i64) -> Facts {
    let Some(planet) = &entry.planet else {
        return Facts { value: value(config, entry, synced, now), sun: None, earth: None };
    };
    let t = now as f64;
    let (sun, earth) = (crate::planets::from_sun(planet, t), crate::planets::from_earth(planet, t));
    let value = planet_value(config, entry, if planet == "Earth" { sun } else { earth });
    Facts { value, sun, earth }
}

/// The few words of an entry's row (its `short`); a spacecraft's with its years of travel, counted
/// from its launch to `now`: `Distance from the Earth after 49 years of travel`.
pub fn short(entry: &Entry, now: i64) -> String {
    // `1977-09-05 12:56 UTC`, as ISO 8601.
    let launched = entry.launched.as_deref().and_then(|l| crate::time::parse_utc(&l.trim_end_matches(" UTC").replacen(' ', "T", 1)));
    match (&entry.source, launched) {
        (Source::Spacecraft { .. }, Some(at)) if now > at => format!("Distance from the Earth after {} years of travel", (now - at) / 31_557_600),
        _ => entry.short.clone(),
    }
}

/// Where a live value comes from: `JPL Horizons`, `CelesTrak`; `None` for a fixed one.
pub fn source_name(entry: &Entry) -> Option<&'static str> {
    match entry.source {
        Source::Spacecraft { .. } => Some("JPL Horizons"),
        Source::Satellites { .. } => Some("CelesTrak"),
        _ => None,
    }
}

/// What an estimate is: `estimated` (a spacecraft, now), `as of 2026-10-11` (a count).
pub fn estimate_name(entry: &Entry) -> String {
    match &entry.source {
        Source::Satellites { as_of, .. } => format!("as of {as_of}"),
        _ => "estimated".to_string(),
    }
}

/// The value in `unit` (the entry's own when `None`).
fn in_unit(config: &Config, entry: &Entry, value: f64, unit: &str) -> f64 {
    match &entry.unit {
        Some(own) if own != unit => convert::convert(config, value, own, unit).map(|(v, _)| v).unwrap_or(value),
        _ => value,
    }
}

/// The value as shown: in the entry's (first `show`) unit, with its decimals else `precision`, and
/// in brackets in the second `show` unit with `precision`. `number` formats each number (the
/// device's separators): `57,909,050 km (0.3871 au)`.
pub fn shown(config: &Config, entry: &Entry, value: f64, precision: usize, number: &dyn Fn(String) -> String) -> String {
    match &parts(config, entry, value, precision, number)[..] {
        [first, second, ..] => format!("{first} ({second})"),
        [first] => first.clone(),
        [] => String::new(),
    }
}

/// The value in each `show` unit (the first with the entry's decimals, the others with
/// `precision`), else in its own unit: [`57,909,050 km`, `0.3871 au`, `6.1213e-6 ly`].
fn parts(config: &Config, entry: &Entry, value: f64, precision: usize, number: &dyn Fn(String) -> String) -> Vec<String> {
    let with = |n: String, unit: Option<&str>| match unit {
        Some(u) => format!("{} {u}", number(n)),
        None => number(n),
    };
    if entry.show.is_empty() {
        return vec![with(convert::format_value(value, entry.decimals.unwrap_or(precision)), entry.unit.as_deref())];
    }
    entry
        .show
        .iter()
        .enumerate()
        .map(|(i, unit)| {
            let v = in_unit(config, entry, value, unit);
            match i {
                0 => with(convert::format_value(v, entry.decimals.unwrap_or(precision)), Some(unit)),
                _ => with(other_unit(v, precision), Some(unit)),
            }
        })
        .collect()
}

/// A number in a second or third unit: below 0.01 in scientific notation with `precision`
/// decimals, so small values keep their digits (`8.2276e-5 ly`, not `0.0001 ly`).
fn other_unit(v: f64, precision: usize) -> String {
    if v == 0.0 || v.abs() >= 0.01 {
        return convert::format_value(v, precision);
    }
    let s = format!("{v:.precision$e}");
    match s.split_once('e') {
        Some((mantissa, exponent)) if mantissa.contains('.') => format!("{}e{exponent}", mantissa.trim_end_matches('0').trim_end_matches('.')),
        _ => s,
    }
}

/// A distance in km (whole) and in the entry's other `show` units, for the popup's planet lines:
/// `235,002,614 km - 1.5709 au - 2.4840e-5 ly`.
fn distance_in_units(config: &Config, entry: &Entry, km: f64, precision: usize, number: &dyn Fn(String) -> String) -> String {
    let mut out = vec![format!("{} km", number(convert::format_value(km, 0)))];
    for unit in entry.show.iter().filter(|u| *u != "km") {
        if let Ok((v, _)) = convert::convert(config, km, "km", unit) {
            out.push(format!("{} {unit}", number(other_unit(v, precision))));
        }
    }
    out.join(" - ")
}

/// The speed of light, km/s.
const LIGHT_KM_S: f64 = 299_792.458;

/// How long light takes to cover `km`, for people: `1.3 s`, `8 min 19 s`, `23 h 54 min`, `2 d 4 h`.
fn light_time(km: f64) -> String {
    let s = (km / LIGHT_KM_S).round() as i64;
    match s {
        _ if km / LIGHT_KM_S < 60.0 => format!("{:.1} s", km / LIGHT_KM_S),
        0..3_600 => format!("{} min {} s", s / 60, s % 60),
        3_600..172_800 => format!("{} h {} min", s / 3_600, s % 3_600 / 60),
        _ => format!("{} d {} h", s / 86_400, s % 86_400 / 3_600),
    }
}

/// The Sun's gravitational parameter GM, km³/s².
const GM_SUN: f64 = 1.327_124_400_18e11;

/// A speed for people: `16.88 km/s - 60,768 km/h - 37,759 mph` (km/h and mph whole, through
/// `number`).
fn speed_text(km_s: f64, number: &dyn Fn(String) -> String) -> String {
    let per_hour = km_s * 3600.0;
    format!(
        "{} km/s - {} km/h - {} mph",
        number(convert::format_value(km_s, 2)),
        number(convert::format_value(per_hour, 0)),
        number(convert::format_value(per_hour / 1.609_344, 0))
    )
}

/// The speed along an orbit around the Sun at `r` au from it, the orbit's semi-major axis `a` au
/// (vis-viva: v² = GM (2/r − 1/a)), in km/s.
fn orbital_speed(r: f64, a: f64) -> f64 {
    let au = crate::planets::AU_KM;
    (GM_SUN * (2.0 / (r * au) - 1.0 / (a * au))).sqrt()
}

/// The value as written into an expression: its first unit, plain digits, the entry's decimals
/// (else 10): `57909050 km`, `3.1415926536`.
pub fn written(config: &Config, entry: &Entry, value: f64) -> String {
    let unit = entry.show.first().map(String::as_str).or(entry.unit.as_deref());
    let n = convert::format_value(unit.map_or(value, |u| in_unit(config, entry, value, u)), entry.decimals.unwrap_or(10));
    match unit {
        Some(u) => format!("{n} {u}"),
        None => n,
    }
}

/// The popup of an entry (Enter in the Trivia): (label, text) lines after its description, in
/// order: its Wikipedia link; its value (`Current distance: 25,794,004,533 km - 172.4223 au
/// (updated now)`); a spacecraft's speed; its details (the Moon's and the satellites' speeds are
/// there); a spacecraft's launch; for a planet, its distances from the Sun and from the Earth:
/// now, mean, the next minimum and maximum with their dates, and its orbital speed. Kilometres
/// are whole, through `number` (the device's separators).
pub fn popup(config: &Config, entry: &Entry, facts: &Facts, precision: usize, number: &dyn Fn(String) -> String) -> Vec<(String, String)> {
    let mut lines = Vec::new();
    if let Some(link) = &entry.wikipedia {
        lines.push(("Wikipedia".to_string(), link.clone()));
    }
    match &entry.source {
        Source::Tip(example) => lines.push(("Example".to_string(), example.clone())),
        Source::Table(table) => lines.extend(table_rows(*table).into_iter().map(|r| (r.label, r.text))),
        _ => {
            if let Ok((v, origin)) = facts.value {
                let how = origin_text(entry, origin).map(|t| format!(" ({t})")).unwrap_or_default();
                let shown = parts(config, entry, v, precision, number).join(" - ");
                lines.push((entry.value_label.clone().unwrap_or_else(|| "Value".to_string()), format!("{shown}{how}")));
            }
        }
    }
    if let Source::Spacecraft { estimate, .. } = &entry.source {
        lines.push(("Speed".to_string(), format!("{}, away from the Sun", speed_text(estimate.km_per_s, number))));
        // Its distance is the Earth's (JPL Horizons, centred on the Earth): the light from the Earth
        // is exact (the signal's delay); from the Sun within 1 au of it, about 8 min 19 s either way.
        if let Ok((v, _)) = facts.value {
            let km = in_unit(config, entry, v, "km");
            lines.push(("Light from the Earth now".to_string(), light_time(km)));
            lines.push(("Light from the Sun now".to_string(), format!("about {} (±8 min: the Earth's side of its orbit)", light_time(km))));
        }
    }
    lines.extend(entry.details.iter().cloned());
    if let Some(launched) = &entry.launched {
        lines.push(("Launched".to_string(), launched.clone()));
    }
    if let Some(planet) = &entry.planet {
        let km = |au: f64| distance_in_units(config, entry, au * crate::planets::AU_KM, precision, number);
        let when = |r: crate::planets::Reached| r.at.map(|t| format!(", next on {}", crate::time::day_for_people(t))).unwrap_or_default();
        // The value's line is the distance now (from the Earth; the Earth's from the Sun): not twice.
        if let Some(d) = facts.sun {
            // The file's figure (the orbit's semi-major axis at J2000), as before it moved here.
            let mean = if let Source::Value(au) = entry.source { au } else { d.mean };
            lines.push(("Mean distance from the Sun".to_string(), km(mean)));
            if planet != "Earth" {
                lines.push(("Distance from the Sun now".to_string(), km(d.now)));
            }
            lines.push(("Light from the Sun now".to_string(), light_time(d.now * crate::planets::AU_KM)));
            lines.push(("Closest to the Sun (perihelion)".to_string(), format!("{}{}", km(d.min.au), when(d.min))));
            lines.push(("Farthest from the Sun (aphelion)".to_string(), format!("{}{}", km(d.max.au), when(d.max))));
            // Fastest at perihelion, slowest at aphelion.
            lines.push(("Orbital speed now".to_string(), speed_text(orbital_speed(d.now, d.mean), number)));
            lines.push(("Fastest (perihelion)".to_string(), speed_text(orbital_speed(d.min.au, d.mean), number)));
            lines.push(("Slowest (aphelion)".to_string(), speed_text(orbital_speed(d.max.au, d.mean), number)));
        }
        if let Some(d) = facts.earth {
            lines.push(("Light from the Earth now".to_string(), light_time(d.now * crate::planets::AU_KM)));
            lines.push(("Mean distance from the Earth".to_string(), km(d.mean)));
            lines.push(("Closest to the Earth".to_string(), format!("{}{}", km(d.min.au), when(d.min))));
            lines.push(("Farthest from the Earth".to_string(), format!("{}{}", km(d.max.au), when(d.max))));
        }
    }
    lines
}

/// A spacecraft's distance from the Earth now, in au, from JPL Horizons (`curl`, 8 s at most).
/// `None` offline or on any error.
pub fn fetch_live(horizons: &str) -> Option<f64> {
    if !crate::network::allowed() {
        return None;
    }
    let now = crate::time::now();
    let day = crate::time::format(&now, "iso-date").ok()?;
    let tomorrow = crate::time::format(&crate::time::Instant { ns: now.ns + 86_400 * 1_000_000_000, ..now }, "iso-date").ok()?;
    let url = format!(
        "https://ssd.jpl.nasa.gov/api/horizons.api?format=text&COMMAND='{horizons}'&OBJ_DATA='NO'&MAKE_EPHEM='YES'&EPHEM_TYPE='OBSERVER'&CENTER='500@399'&START_TIME='{day}'&STOP_TIME='{tomorrow}'&STEP_SIZE='1d'&QUANTITIES='20'"
    );
    horizons_distance(&String::from_utf8_lossy(&crate::http::get(&url, 8, false)?))
}

/// Where a value comes from, for people: `synced 11 Oct 26 6:52 PM from JPL Horizons`, `as of
/// 2026-10-11 (estimated, not synced yet)`; `None` for a fixed value.
pub fn origin_text(entry: &Entry, origin: Origin) -> Option<String> {
    let when = crate::time::human_moment;
    match origin {
        Origin::Fixed => None,
        Origin::Synced(at) => Some(format!("synced {} from {}", when(at), source_name(entry).unwrap_or("online"))),
        Origin::Estimated => Some(format!("{}, not synced yet", estimate_name(entry))),
    }
}

/// The live sources, for the sync (`online.rs`): each spacecraft (JPL Horizons), and the satellite
/// counts (one CelesTrak download for all of them).
pub fn sources() -> Vec<crate::online::Source> {
    let mut out: Vec<crate::online::Source> = entries()
        .iter()
        .filter_map(|e| match &e.source {
            Source::Spacecraft { horizons, .. } => {
                let (name, id) = (e.name.clone(), horizons.clone());
                Some(crate::online::Source { names: vec![name.clone()], fetch: Box::new(move || fetch_live(&id).map(|au| vec![(name.clone(), au)])) })
            }
            _ => None,
        })
        .collect();
    let kinds: Vec<(String, String)> = entries()
        .iter()
        .filter_map(|e| match &e.source {
            Source::Satellites { kind, .. } => Some((e.name.clone(), kind.clone())),
            _ => None,
        })
        .collect();
    if !kinds.is_empty() {
        out.push(crate::online::Source {
            names: kinds.iter().map(|(n, _)| n.clone()).collect(),
            fetch: Box::new(move || match fetch_satellites()? {
                // Refused within 2 hours of the last download: the data has not changed, the counts
                // already stored stay (answered, nothing new).
                Satellites::Unchanged => Some(Vec::new()),
                Satellites::Counts(counts) => Some(kinds.iter().filter_map(|(name, kind)| Some((name.clone(), counts.get(kind)?))).collect()),
            }),
        });
    }
    out
}

/// What CelesTrak answered.
pub enum Satellites {
    Counts(SatelliteCounts),
    /// "GP data has not updated since your last successful download": it serves the list to an
    /// address once every 2 hours, and the data changes no more often.
    Unchanged,
}

/// Active satellites by kind, counted from CelesTrak's list of active satellites (CSV, 20 s at
/// most). `None`: no answer (offline, or not a list).
pub fn fetch_satellites() -> Option<Satellites> {
    if !crate::network::allowed() {
        return None;
    }
    let url = "https://celestrak.org/NORAD/elements/gp.php?GROUP=active&FORMAT=csv";
    let body = crate::http::get(url, 20, false)?;
    let body = String::from_utf8_lossy(&body);
    if body.contains("has not updated since your last successful") {
        return Some(Satellites::Unchanged);
    }
    count_satellites(&body).map(Satellites::Counts)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SatelliteCounts {
    pub all: usize,
    pub starlink: usize,
    pub leo: usize,
    pub meo: usize,
    pub geo: usize,
    pub heo: usize,
    pub beyond: usize,
}

impl SatelliteCounts {
    fn get(&self, kind: &str) -> Option<f64> {
        let n = match kind {
            "all" => self.all,
            "starlink" => self.starlink,
            "leo" => self.leo,
            "meo" => self.meo,
            "geo" => self.geo,
            "heo" => self.heo,
            "beyond" => self.beyond,
            _ => return None,
        };
        Some(n as f64)
    }
}

/// Counts a CelesTrak GP table (CSV with `OBJECT_NAME`, `MEAN_MOTION` in revolutions a day,
/// `ECCENTRICITY`). The orbit from the mean motion (Kepler's third law, μ = 398600.4418 km³/s²):
/// LEO, apogee below 2,000 km; GEO, a period of 1,430–1,450 minutes, near-circular (e < 0.01); HEO,
/// e ≥ 0.25; MEO, the rest below 35,786 km; beyond, the rest. `None`: not such a table.
pub fn count_satellites(csv: &str) -> Option<SatelliteCounts> {
    let mut lines = csv.lines();
    let header: Vec<&str> = lines.next()?.split(',').collect();
    let column = |name: &str| header.iter().position(|h| h.trim() == name);
    let (name, motion, ecc) = (column("OBJECT_NAME")?, column("MEAN_MOTION")?, column("ECCENTRICITY")?);
    let mut c = SatelliteCounts::default();
    for line in lines.filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split(',').collect();
        let (Some(n), Some(e)) = (f.get(motion).and_then(|v| v.parse::<f64>().ok()), f.get(ecc).and_then(|v| v.parse::<f64>().ok())) else { continue };
        if n <= 0.0 {
            continue;
        }
        c.all += 1;
        c.starlink += usize::from(f.get(name).is_some_and(|s| s.starts_with("STARLINK")));
        let radians_per_s = n * std::f64::consts::TAU / 86_400.0;
        let a = (398_600.441_8 / (radians_per_s * radians_per_s)).cbrt();
        let apogee = a * (1.0 + e) - 6_378.137;
        let period = 1_440.0 / n;
        if apogee < 2_000.0 {
            c.leo += 1;
        } else if (1_430.0..=1_450.0).contains(&period) && e < 0.01 {
            c.geo += 1;
        } else if e >= 0.25 {
            c.heo += 1;
        } else if apogee < 35_786.0 {
            c.meo += 1;
        } else {
            c.beyond += 1;
        }
    }
    (c.all > 0).then_some(c)
}

/// The first distance (`delta`, au) in a Horizons observer table.
pub fn horizons_distance(text: &str) -> Option<f64> {
    let table = text.split("$$SOE").nth(1)?.split("$$EOE").next()?;
    let line = table.lines().find(|l| !l.trim().is_empty())?;
    // `2026-Oct-11 00:00     172.422270530110  37.6251248`
    line.split_whitespace().nth(2)?.parse().ok()
}

/// `--trivia`: every entry with its value and description; the live values from the online data
/// (`online.rs`; no network here).
pub fn list(config: &Config, precision: usize) -> String {
    let now = crate::time::unix_now();
    let online = crate::online::load();
    let mut out = String::new();
    let mut section = "";
    for e in entries() {
        if e.section != section {
            section = &e.section;
            out.push_str(&format!("{}{section}\n", if out.is_empty() { "" } else { "\n" }));
        }
        let live = online.get(&e.name);
        if let Source::Tip(example) = &e.source {
            out.push_str(&format!("  {}: {example}\n    {}\n", e.name, e.description));
            continue;
        }
        let number = |n: String| crate::numbers::device().apply(&n);
        let facts = facts(config, e, live, now);
        let value = match (&e.source, &facts.value) {
            (Source::Table(table), _) => format!("{} sizes", table_rows(*table).len()),
            (_, Ok((v, origin))) => {
                let how = origin_text(e, *origin).map(|t| format!(" ({t})")).unwrap_or_default();
                format!("{}{how}", shown(config, e, *v, precision, &number))
            }
            (_, Err(err)) => format!("error: {err}"),
        };
        out.push_str(&format!("  {} = {value} · {}\n    {}\n", e.name, short(e, now), e.description));
        // The popup's other lines: details, launch, a planet's distances.
        for (label, text) in popup(config, e, &facts, precision, &number).into_iter().filter(|(l, _)| l != "Value") {
            out.push_str(&format!("    {}: {text}\n", label.trim()));
        }
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How long light takes, for people.
    /// A spacecraft's row: its years of travel, to now.
    #[test]
    fn years_of_travel() {
        let v1 = entries().iter().find(|e| e.name == "Voyager 1").unwrap();
        // Launched 1977-09-05: 49 years on 2026-10-16, 50 a year later.
        assert_eq!(short(v1, 1_792_108_800), "Distance from the Earth after 49 years of travel");
        assert_eq!(short(v1, 1_792_108_800 + 31_557_600), "Distance from the Earth after 50 years of travel");
        let pi = entries().iter().find(|e| e.name == "π (pi)").unwrap();
        assert_eq!(short(pi, 1_792_108_800), pi.short);
    }

    #[test]
    fn light_times() {
        assert_eq!(light_time(384_400.0), "1.3 s", "the Moon");
        assert_eq!(light_time(149_597_870.7), "8 min 19 s", "1 au");
        assert_eq!(light_time(172.4685 * 149_597_870.7), "23 h 54 min", "Voyager 1");
        assert_eq!(light_time(30.07 * 149_597_870.7 * 20.0), "3 d 11 h");
    }

    #[test]
    fn the_file_is_valid() {
        let all = entries();
        assert!(all.len() > 30);
        assert_eq!(sections(), ["Earth", "Space", "Satellites", "Math", "Physics", "Shoes", "Clothes"]);
        let config = Config::default();
        for e in all {
            assert!(!e.description.is_empty() && !e.short.is_empty(), "{}", e.name);
            assert!(e.short.chars().count() <= 36, "{}: a short description", e.name);
            let tip = matches!(e.source, Source::Tip(_) | Source::Table(_));
            assert!(tip || value(&config, e, None, 1_791_676_800).is_ok(), "{}", e.name);
            // Every unit is one aequalis knows.
            if let Some(unit) = &e.unit {
                assert!(convert::resolve(&config, unit).is_ok(), "{}: {unit}", e.name);
            }
        }
    }

    #[test]
    fn clothes_tables() {
        use crate::sizes::Chart;
        let women = table_rows(Table::Clothes(Chart::Women));
        let row = women.iter().find(|r| r.label == "US 8").unwrap();
        assert_eq!(
            (row.text.as_str(), row.request.as_str()),
            ("UK 12 · DE 38 · FR 40 · IT 44 · JP 11 · M · CN 165/92A · bust 93 · waist 75 · hip 98.5 cm", "44 it dress")
        );
        assert_eq!(table_rows(Table::Clothes(Chart::Suit))[7].text, "EU 58 · JP 3L · chest 121.9 cm", "no letter past XXXL");
        // Every row's request answers.
        let _m = crate::machine::Mock::italy().install();
        for chart in [Chart::Women, Chart::Suit, Chart::Trousers, Chart::Shirt] {
            for r in table_rows(Table::Clothes(chart)) {
                assert!(matches!(crate::sizes::request(&r.request), Some(Ok(_))), "{}", r.request);
            }
        }
    }

    #[test]
    fn shoe_tables() {
        use crate::sizes::Person;
        let men = table_rows(Table::Shoes(Person::Men));
        let row = men.iter().find(|r| r.label == "US 8.5").unwrap();
        assert_eq!((row.text.as_str(), row.request.as_str()), ("UK 7.5 · EU 42 · JP 26.5 · foot 25.9 cm", "42 eu men shoes"));
        assert_eq!(table_rows(Table::Shoes(Person::Kids))[0].request, "19.5 eu kids shoes");
        let entry = entries().iter().find(|e| e.name == "Women's shoes").unwrap();
        assert_eq!(entry.source, Source::Table(Table::Shoes(Person::Women)));
        let lines = popup(&Config::default(), entry, &facts(&Config::default(), entry, None, 0), 4, &|n| n);
        assert_eq!(lines.len(), 15, "a line per size");
    }

    #[test]
    fn values() {
        let config = Config::default();
        let get = |name: &str| entries().iter().find(|e| e.name == name).unwrap();
        let shown_now = |name: &str| {
            let e = get(name);
            shown(&config, e, value(&config, e, None, 1_791_676_800).unwrap().0, 4, &|n| n)
        };
        assert_eq!(shown_now("π (pi)"), "3.1415926536");
        assert_eq!(shown_now("e (Euler's number)"), "2.7182818285");
        assert_eq!(shown_now("Electronvolt"), "1.602176634e-19 J");
        assert_eq!(shown_now("Equator"), "40075.017 km");
        // A tip: its example is a request that works.
        let Source::Tip(example) = &get("Distance between places").source else { panic!("a tip") };
        assert_eq!(example, "London to Paris km");
        assert!(crate::geo::request(&config, example, 4).is_some_and(|r| r.is_ok()));
        // Space: km, and in brackets the astronomical unit (the Earth's distance from the Sun).
        // A planet: its distance now from the Earth (the Earth's from the Sun), in km and au.
        let t = 1_791_676_800;
        for (name, now) in [("Mercury", crate::planets::from_earth("Mercury", t as f64)), ("Earth", crate::planets::from_sun("Earth", t as f64))] {
            let (v, origin) = value(&config, get(name), None, t).unwrap();
            assert!((v - now.unwrap().now).abs() < 1e-9 && origin == Origin::Fixed, "{name}: {v}");
        }
        assert!(shown_now("Mercury").ends_with(" au)"), "{}", shown_now("Mercury"));
        assert_eq!(shown_now("Moon"), "384399 km (2.5695e-3 au)");
        // A spacecraft at the estimate's epoch (2026-10-11), and a year later.
        assert_eq!(shown_now("Voyager 1"), "25794004453 km (172.4223 au)");
        // Written into an expression: the first unit, plain.
        let mercury = get("Mercury");
        assert_eq!(written(&config, mercury, 0.38709927), "57909227 km");
        assert_eq!(written(&config, get("π (pi)"), std::f64::consts::PI), "3.1415926536");
        // The device's separators.
        let grouped = |n: String| n.replace("57909227", "57,909,227");
        assert_eq!(shown(&config, mercury, 0.38709927, 4, &grouped), "57,909,227 km (0.3871 au)");
        let v1 = get("Voyager 1");
        let (later, origin) = value(&config, v1, None, 1_791_676_800 + 31_557_600).unwrap();
        assert_eq!(origin, Origin::Estimated);
        assert!((later - 172.4223 - 3.561).abs() < 0.01, "{later}");
        // Synced: carried forward from the sync to now at its speed.
        let synced = crate::online::Stored { value: 172.5, at: 1_000 };
        assert_eq!(value(&config, v1, Some(synced), 1_000).unwrap(), (172.5, Origin::Synced(1_000)));
        let (a_day_later, _) = value(&config, v1, Some(synced), 1_000 + 86_400).unwrap();
        assert!((a_day_later - 172.5 - 16.88 * 86_400.0 / 149_597_870.7).abs() < 1e-9, "{a_day_later}");
    }

    /// An entry's popup at a fixed time, its numbers as they are.
    fn popup_of(name: &str) -> Vec<(String, String)> {
        let config = Config::default();
        let entry = entries().iter().find(|e| e.name == name).unwrap();
        popup(&config, entry, &facts(&config, entry, None, 1_791_676_800), 4, &|n| n)
    }

    /// A popup's line by its label.
    fn line(lines: &[(String, String)], label: &str) -> String {
        lines.iter().find(|(l, _)| l == label).map(|(_, t)| t.clone()).unwrap()
    }

    #[test]
    fn popup_of_a_math_entry() {
        // The mathematician and the definition.
        let pi = popup_of("π (pi)");
        assert_eq!(pi[0], ("Value".to_string(), "3.1415926536".to_string()));
        assert!(pi.iter().any(|(l, t)| l == "Mathematician" && t.contains("Archimedes")));
        assert!(pi.iter().any(|(l, t)| l == "Definition" && t.starts_with("π = C / d")));
    }

    #[test]
    fn popup_of_a_spacecraft() {
        // Voyager 1: the link, the current distance in km and au, the launch.
        let v1 = popup_of("Voyager 1");
        assert_eq!(v1[0], ("Wikipedia".to_string(), "https://en.wikipedia.org/wiki/Voyager_1".to_string()));
        assert_eq!(v1[1], ("Current distance".to_string(), "25794004453 km - 172.4223 au - 2.7264e-3 ly (estimated, not synced yet)".to_string()));
        assert_eq!(v1[2], ("Speed".to_string(), "16.88 km/s - 60768 km/h - 37759 mph, away from the Sun".to_string()));
        // How long light takes: from the Earth exact (its distance is the Earth's), from the Sun about.
        assert_eq!(v1[3], ("Light from the Earth now".to_string(), "23 h 54 min".to_string()));
        assert_eq!(v1[4], ("Light from the Sun now".to_string(), "about 23 h 54 min (±8 min: the Earth's side of its orbit)".to_string()));
        assert_eq!(v1[5], ("Launched".to_string(), "1977-09-05 12:56 UTC".to_string()));
        let config = Config::default();
        let entry = entries().iter().find(|e| e.name == "Voyager 1").unwrap();
        let synced = Some(crate::online::Stored { value: 172.5, at: 1_791_676_800 });
        let live = popup(&config, entry, &facts(&config, entry, synced, 1_791_676_800), 4, &|n| n);
        assert!(live[1].1.contains(" - 172.5 au - 2.7277e-3 ly (synced ") && live[1].1.ends_with(" from JPL Horizons)"), "{:?}", live[1]);
    }

    #[test]
    fn popup_of_a_planet() {
        // From the Sun and from the Earth, the next dates.
        let mars = popup_of("Mars");
        let text = |label: &str| line(&mars, label);
        assert_eq!(text("Mean distance from the Sun"), "227943822 km - 1.5237 au - 2.4094e-5 ly");
        assert!(text("Distance from the Earth now").ends_with(" ly"), "every distance in light-years too: {mars:?}");
        assert_eq!(mars.iter().filter(|(l, _)| l == "Distance from the Earth now").count(), 1, "the value's line, not twice");
        assert_eq!(mars[1].0, "Distance from the Earth now", "the value first, after the link");
        assert_eq!(text("Year"), "686.98 d - 1.8808 yr");
        // Light from the Sun (Mars: 12.7 to 13.9 min) and from the Earth now.
        assert!(text("Light from the Sun now").contains(" min "), "{mars:?}");
        assert!(text("Light from the Earth now").contains(" min ") || text("Light from the Earth now").ends_with(" s"), "{mars:?}");
        assert!(text("Closest to the Earth").contains(" au - ") && text("Closest to the Earth").contains(" ly, next on "), "{mars:?}");
        assert!(text("Closest to the Sun (perihelion)").contains(" ly, next on "), "{mars:?}");
        // Mars's next closest approach: the opposition of February 2027, about 101 million km.
        assert!(text("Closest to the Earth").starts_with("1014") && text("Closest to the Earth").contains(" Feb 2027"), "{mars:?}");
        assert!(text("Fastest (perihelion)").starts_with("26.5"), "Mars: {mars:?}");
        // The light-year itself, exact.
        assert_eq!(popup_of("Light-year")[1].1, "9460730472581 km - 63241.0771 au - 1 ly");
    }

    #[test]
    fn popup_of_the_earth() {
        let earth = popup_of("Earth");
        assert!(!earth.iter().any(|(l, _)| l == "Light from the Earth now"), "not for the Earth itself");
        assert_eq!(earth.iter().filter(|(l, _)| l == "Distance from the Sun now").count(), 1);
        assert!(!earth.iter().any(|(l, _)| l == "Closest to the Earth"));
        // Orbital speeds: the Earth's 29.78 km/s on average, 30.29 at perihelion, 29.29 at aphelion.
        assert!(line(&earth, "Fastest (perihelion)").starts_with("30.29 km/s - 109"), "{earth:?}");
        assert!(line(&earth, "Slowest (aphelion)").starts_with("29.29 km/s - 105"), "{earth:?}");
        let now: f64 = line(&earth, "Orbital speed now").split(' ').next().unwrap().parse().unwrap();
        assert!((29.29..=30.29).contains(&now), "{earth:?}");
    }

    #[test]
    fn popups_of_every_entry() {
        // The length of a day (a rotation, sunrise to sunrise) and of a year, for every planet.
        for e in entries().iter().filter(|e| e.planet.is_some()) {
            let lines = popup_of(&e.name);
            assert!(lines.iter().any(|(l, _)| l == "Day (one rotation)") && lines.iter().any(|(l, _)| l == "Year"), "{}", e.name);
        }
        // Every Space object and satellite orbit has a speed.
        for e in entries().iter().filter(|e| (e.section == "Space" && e.name != "Light-year") || e.section == "Satellites") {
            assert!(popup_of(&e.name).iter().any(|(l, _)| l.contains("peed") || l.starts_with("Fastest")), "{}", e.name);
        }
        // Every Space entry: a link and a labelled value.
        for e in entries().iter().filter(|e| e.section == "Space") {
            let lines = popup_of(&e.name);
            assert_eq!(lines[0].0, "Wikipedia", "{}", e.name);
            assert_ne!(lines[1].0, "Value", "{}: a label for its value", e.name);
        }
        // Every entry has a popup.
        for e in entries() {
            assert!(!popup_of(&e.name).is_empty(), "{}", e.name);
        }
    }

    #[test]
    fn counts_satellites() {
        let csv = "OBJECT_NAME,OBJECT_ID,EPOCH,MEAN_MOTION,ECCENTRICITY\n\
                   STARLINK-1008,2019-074B,2026-10-11,15.66599351,.00029969\n\
                   GPS BIIR-2,1997-035A,2026-10-11,2.00563000,.0100000\n\
                   INTELSAT 901,2001-024A,2026-10-11,1.00270000,.0002000\n\
                   MOLNIYA 1-93,2004-005A,2026-10-11,2.00600000,.7200000\n\
                   ISS (ZARYA),1998-067A,2026-10-11,15.50000000,.0005000\n";
        let c = count_satellites(csv).unwrap();
        assert_eq!((c.all, c.starlink, c.leo, c.meo, c.geo, c.heo, c.beyond), (5, 1, 2, 1, 1, 1, 0));
        // By the kinds the Trivia names them.
        let kinds: Vec<Option<f64>> = ["all", "starlink", "leo", "meo", "geo", "heo", "beyond", "comets"].iter().map(|k| c.get(k)).collect();
        assert_eq!(kinds, [Some(5.0), Some(1.0), Some(2.0), Some(1.0), Some(1.0), Some(1.0), Some(0.0), None]);
        assert_eq!(count_satellites("not a table"), None);
        // The refusal of a second download within 2 hours is not a table: the online data keeps the last counts.
        assert_eq!(count_satellites("GP data has not updated since your last successful\ndownload of GROUP=active"), None);
        // Offline: the estimate, as of its date.
        let config = Config::default();
        let starlink = entries().iter().find(|e| e.name == "Starlink").unwrap();
        assert_eq!(value(&config, starlink, None, 0).unwrap(), (11129.0, Origin::Estimated));
        assert_eq!(value(&config, starlink, Some(crate::online::Stored { value: 11200.0, at: 5 }), 0).unwrap(), (11200.0, Origin::Synced(5)));
        assert_eq!(estimate_name(starlink), "as of 2026-10-11");
        assert_eq!(source_name(starlink), Some("CelesTrak"));
    }

    #[test]
    fn reads_horizons() {
        let text = "header\n$$SOE\n 2026-Oct-11 00:00     172.422270530110  37.6251248\n 2026-Oct-12 00:00     172.44  37.4\n$$EOE\nfooter";
        assert_eq!(horizons_distance(text), Some(172.422270530110));
        assert_eq!(horizons_distance("no table"), None);
    }

    #[test]
    fn bad_documents() {
        assert!(parse(r#"{"sections":[{"name":"X","entries":[{"name":"Y"}]}]}"#).unwrap_err().contains("Y: needs"));
        assert!(parse("{").is_err());
    }
}
