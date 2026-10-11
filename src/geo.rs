//! Distances on the Earth: between two places (a capital or major city, or coordinates), along
//! the surface of the WGS84 ellipsoid (Vincenty's formula; a sphere's great circle when it does
//! not converge, for nearly antipodal points).
//!
//! ```text
//! distance Rome to Paris            1105.8215 km
//! distance New York to London mi    3461.0122 mi
//! distance 41.9,12.5 to 48.86N 2.35E
//! ```
//!
//! The places' coordinates are in `src/data/places.json`, compiled in.

use crate::config::Config;
use crate::convert;
use crate::json;
use crate::numbers::{self, NumberFormat};
use std::sync::OnceLock;

/// A point on the Earth: latitude and longitude in degrees (north and east positive).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub lat: f64,
    pub lon: f64,
}

/// A place: as shown (`Rome`, `41.9028, 12.4964`) and where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub name: String,
    pub point: Point,
}

const PLACES: &str = include_str!("data/places.json");

/// The places' coordinates, by name as shown.
fn places() -> &'static [(String, Point)] {
    static PLACES_TABLE: OnceLock<Vec<(String, Point)>> = OnceLock::new();
    PLACES_TABLE.get_or_init(|| {
        let doc = json::parse(PLACES).expect("src/data/places.json is valid JSON");
        doc.get("places")
            .map(|p| p.members())
            .unwrap_or_default()
            .iter()
            .filter_map(|(name, v)| match v.array() {
                [lat, lon] => Some((name.clone(), Point { lat: lat.number()?, lon: lon.number()? })),
                _ => None,
            })
            .collect()
    })
}

/// The coordinates of a known place (`Rome`, `new york`, `Bogota`).
pub fn known(name: &str) -> Option<Place> {
    let shown = crate::time::tz::place_name(name)?;
    let point = places().iter().find(|(n, _)| n == shown)?.1;
    Some(Place { name: shown.to_string(), point })
}

/// Coordinates: `41.9028,12.4964`, `41.9028, 12.4964`, `41.9 12.5`, `41.9N 12.5E`, `33.9S,151.2E`.
pub fn coordinates(text: &str) -> Option<Point> {
    let parts: Vec<&str> = text.split([',', ' ']).filter(|p| !p.is_empty()).collect();
    let [lat, lon] = parts[..] else { return None };
    let read = |s: &str, positive: char, negative: char| -> Option<f64> {
        let upper = s.to_ascii_uppercase();
        let (number, sign) = match upper.strip_suffix(positive) {
            Some(n) => (n.to_string(), 1.0),
            None => match upper.strip_suffix(negative) {
                Some(n) => (n.to_string(), -1.0),
                None => (upper.clone(), 1.0),
            },
        };
        let v: f64 = number.trim_end_matches('°').parse().ok()?;
        Some(sign * v)
    };
    let point = Point { lat: read(lat, 'N', 'S')?, lon: read(lon, 'E', 'W')? };
    ((-90.0..=90.0).contains(&point.lat) && (-180.0..=180.0).contains(&point.lon)).then_some(point)
}

/// A place by name or coordinates; the error says what is accepted, with the names it may be.
pub fn place(text: &str) -> Result<Place, String> {
    let text = text.trim();
    if let Some(point) = coordinates(text) {
        return Ok(Place { name: format!("{}, {}", point.lat, point.lon), point });
    }
    if let Some(p) = known(text) {
        return Ok(p);
    }
    // A country: its most relevant city (its capital, or the first of its zones: `USA` is New York).
    if let Some(c) = crate::time::tz::country(text) {
        if let Some(p) = c.cities.first().and_then(|city| known(city)) {
            return Ok(p);
        }
    }
    let near = crate::time::tz::near_names(text);
    Err(match near[..] {
        [] => format!("`{text}` is not a known place: write a capital, a major city or coordinates (41.9,12.5)"),
        _ => format!("`{text}` is not a known place: did you mean {}?", near.join(" or ")),
    })
}

/// WGS84: the semi-major axis (m) and the flattening.
const A: f64 = 6_378_137.0;
const F: f64 = 1.0 / 298.257_223_563;
/// The mean radius (m), for the sphere.
const R: f64 = 6_371_008.8;

/// The distance in metres along the surface: on the ellipsoid (Vincenty), or the great circle.
pub fn distance(a: Point, b: Point) -> f64 {
    vincenty(a, b).unwrap_or_else(|| haversine(a, b))
}

/// On a sphere of the Earth's mean radius.
pub fn haversine(a: Point, b: Point) -> f64 {
    let (p1, p2) = (a.lat.to_radians(), b.lat.to_radians());
    let (dp, dl) = (p2 - p1, (b.lon - a.lon).to_radians());
    let h = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * R * h.sqrt().min(1.0).asin()
}

/// Vincenty's inverse formula on the WGS84 ellipsoid: `None` when it does not converge.
pub fn vincenty(a: Point, b: Point) -> Option<f64> {
    let b_axis = A * (1.0 - F);
    let l = (b.lon - a.lon).to_radians();
    let u1 = ((1.0 - F) * a.lat.to_radians().tan()).atan();
    let u2 = ((1.0 - F) * b.lat.to_radians().tan()).atan();
    let (sin_u1, cos_u1, sin_u2, cos_u2) = (u1.sin(), u1.cos(), u2.sin(), u2.cos());
    let mut lambda = l;
    for _ in 0..200 {
        let (sin_l, cos_l) = (lambda.sin(), lambda.cos());
        let sin_sigma = ((cos_u2 * sin_l).powi(2) + (cos_u1 * sin_u2 - sin_u1 * cos_u2 * cos_l).powi(2)).sqrt();
        if sin_sigma == 0.0 {
            return Some(0.0);
        }
        let cos_sigma = sin_u1 * sin_u2 + cos_u1 * cos_u2 * cos_l;
        let sigma = sin_sigma.atan2(cos_sigma);
        let sin_alpha = cos_u1 * cos_u2 * sin_l / sin_sigma;
        let cos2_alpha = 1.0 - sin_alpha * sin_alpha;
        let cos_2sm = if cos2_alpha == 0.0 { 0.0 } else { cos_sigma - 2.0 * sin_u1 * sin_u2 / cos2_alpha };
        let c = F / 16.0 * cos2_alpha * (4.0 + F * (4.0 - 3.0 * cos2_alpha));
        let previous = lambda;
        lambda = l + (1.0 - c) * F * sin_alpha * (sigma + c * sin_sigma * (cos_2sm + c * cos_sigma * (-1.0 + 2.0 * cos_2sm * cos_2sm)));
        if (lambda - previous).abs() < 1e-12 {
            let u2_ = cos2_alpha * (A * A - b_axis * b_axis) / (b_axis * b_axis);
            let big_a = 1.0 + u2_ / 16384.0 * (4096.0 + u2_ * (-768.0 + u2_ * (320.0 - 175.0 * u2_)));
            let big_b = u2_ / 1024.0 * (256.0 + u2_ * (-128.0 + u2_ * (74.0 - 47.0 * u2_)));
            let delta = big_b
                * sin_sigma
                * (cos_2sm + big_b / 4.0 * (cos_sigma * (-1.0 + 2.0 * cos_2sm * cos_2sm) - big_b / 6.0 * cos_2sm * (-3.0 + 4.0 * sin_sigma * sin_sigma) * (-3.0 + 4.0 * cos_2sm * cos_2sm)));
            return Some(b_axis * big_a * (sigma - delta));
        }
    }
    None
}

/// Whether `input` is a distance request: it starts with the word `distance`.
pub fn is_request(input: &str) -> bool {
    input.split_whitespace().next().is_some_and(|w| w.eq_ignore_ascii_case("distance"))
}

/// A distance request: `distance <place> to <place> [length unit]` (default `km`), or two places
/// and nothing else, `<place> to <place> [length unit]` (`Rome to Doha`; with a time it is a time
/// request: `10:00 Rome to Doha`). The result and, for `-v`, a note on the places used. `None` if
/// `input` is not one.
pub fn request(config: &Config, input: &str, precision: usize) -> Option<Result<(String, Vec<String>), String>> {
    request_in(config, input, precision, numbers::device())
}

/// `request`, its numbers written in `format`: whole units (`1,107 km`, `1.107 km`), since the
/// places' coordinates are not precise to the metre; below 1, with `precision` decimals.
pub fn request_in(config: &Config, input: &str, precision: usize, format: &NumberFormat) -> Option<Result<(String, Vec<String>), String>> {
    if is_request(input) {
        let rest = input.trim_start()["distance".len()..].trim();
        return Some(between(config, rest, precision, format));
    }
    // Without the word: only when both sides are places.
    between(config, input, precision, format).ok().map(Ok)
}

/// Two places in `words`: split at `to` (`London to Paris`), else where both sides are places
/// (`London Paris`, `New York Los Angeles`). `None`: not two places; `Some(Err)`: written with `to`
/// but a side is not a place (the error says which).
fn two_places(words: &[&str]) -> Option<Result<(Place, Place), String>> {
    if let Some(to) = words.iter().position(|w| w.eq_ignore_ascii_case("to")) {
        let (from, target) = (&words[..to], &words[to + 1..]);
        if from.is_empty() || target.is_empty() {
            return None;
        }
        return Some(place(&from.join(" ")).and_then(|a| Ok((a, place(&target.join(" "))?))));
    }
    // Without `to`, coordinates need a comma or N S E W: `1 2 3 4` is not two places.
    let side = |w: &[&str]| {
        let text = w.join(" ");
        let bare_numbers = !text.contains(',') && !text.chars().any(char::is_alphabetic);
        if bare_numbers { None } else { place(&text).ok() }
    };
    (1..words.len()).find_map(|i| Some(Ok((side(&words[..i])?, side(&words[i..])?))))
}

fn between(config: &Config, rest: &str, precision: usize, format: &NumberFormat) -> Result<(String, Vec<String>), String> {
    const USAGE: &str = "write `distance <place> [to] <place> [unit]`, a place being a capital, a major city, a country or coordinates (41.9,12.5)";
    let words: Vec<&str> = rest.split_whitespace().collect();
    // The places alone (unit km), or before a length unit, `in` before it optional (`London to
    // Paris in mi`, `London Paris mi`).
    let mut tries: Vec<(&[&str], &str)> = vec![(&words[..], "km")];
    if let [before @ .., last] = &words[..] {
        if convert::find_unit(config, last).is_some_and(|(c, _)| c.name == "Length") {
            let before = match before {
                [rest @ .., word] if word.eq_ignore_ascii_case("in") => rest,
                _ => before,
            };
            tries.push((before, last));
        }
    }
    let mut error = None;
    let mut found = None;
    for (words, unit) in tries {
        match two_places(words) {
            Some(Ok(places)) => {
                found = Some((places, unit));
                break;
            }
            Some(Err(e)) => error = error.or(Some(e)),
            None => {}
        }
    }
    let Some(((a, b), unit)) = found else { return Err(error.unwrap_or_else(|| USAGE.to_string())) };
    let metres = distance(a.point, b.point);
    let (value, symbol) = convert::convert(config, metres, "m", unit).map_err(|e| e.to_string())?;
    let shown = |p: &Place| match coordinates(&p.name) {
        Some(_) => p.name.clone(),
        None => format!("{} ({:.4}, {:.4})", p.name, p.point.lat, p.point.lon),
    };
    let note = format!("{} to {}, along the Earth's surface (WGS84)", shown(&a), shown(&b));
    let number = convert::format_value(value, if value.abs() >= 1.0 { 0 } else { precision });
    Ok((format!("{} {}", format.apply(&number), symbol.symbol), vec![note]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn km(a: &str, b: &str) -> f64 {
        distance(place(a).unwrap().point, place(b).unwrap().point) / 1000.0
    }

    #[test]
    fn every_place_has_coordinates() {
        for name in crate::time::tz::place_names() {
            assert!(known(name).is_some(), "{name}");
        }
    }

    #[test]
    fn known_distances() {
        // Within 0.5% of published great-circle figures.
        let near = |got: f64, want: f64| assert!((got - want).abs() / want < 0.005, "{got} km, want about {want}");
        near(km("Rome", "Paris"), 1106.0);
        near(km("London", "New York"), 5570.0);
        near(km("Sydney", "London"), 16_994.0);
        near(km("Tokyo", "Los Angeles"), 8815.0);
        near(km("Buenos Aires", "Moscow"), 13_470.0);
        assert_eq!(km("Rome", "Rome"), 0.0);
    }

    #[test]
    fn ellipsoid_and_sphere() {
        // Vincenty's test line (Flinders Peak to Buninyong): 54972.271 m on the ellipsoid.
        let flinders = Point { lat: -(37.0 + 57.0 / 60.0 + 3.72030 / 3600.0), lon: 144.0 + 25.0 / 60.0 + 29.52440 / 3600.0 };
        let buninyong = Point { lat: -(37.0 + 39.0 / 60.0 + 10.15610 / 3600.0), lon: 143.0 + 55.0 / 60.0 + 35.38390 / 3600.0 };
        assert!((vincenty(flinders, buninyong).unwrap() - 54_972.271).abs() < 0.01);
        // Antipodes: Vincenty does not converge; the sphere answers (half a circumference).
        let (p, q) = (Point { lat: 0.0, lon: 0.0 }, Point { lat: 0.5, lon: 179.7 });
        assert!(vincenty(p, q).is_none() || distance(p, q) > 19_900_000.0);
        assert!((haversine(Point { lat: 0.0, lon: 0.0 }, Point { lat: 0.0, lon: 180.0 }) - std::f64::consts::PI * R).abs() < 1.0);
    }

    #[test]
    fn coordinates_forms() {
        let p = |s| coordinates(s).map(|p| (p.lat, p.lon));
        assert_eq!(p("41.9,12.5"), Some((41.9, 12.5)));
        assert_eq!(p("41.9, 12.5"), Some((41.9, 12.5)));
        assert_eq!(p("33.87S 151.21E"), Some((-33.87, 151.21)));
        assert_eq!(p("40.71N,74.01W"), Some((40.71, -74.01)));
        assert_eq!(p("41.9°N 12.5°E"), Some((41.9, 12.5)));
        assert_eq!(p("91,0"), None);
        assert_eq!(p("Rome"), None);
    }

    #[test]
    fn requests() {
        let config = Config::default();
        let plain = NumberFormat::default();
        let r = |s: &str| request_in(&config, s, 1, &plain).unwrap().map(|(out, _)| out);
        assert_eq!(r("distance Rome to Paris"), Ok("1107 km".into()));
        assert_eq!(r("distance rome to PARIS mi"), Ok("688 mi".into()));
        assert_eq!(r("distance New York to Los Angeles"), Ok("3944 km".into()));
        assert_eq!(r("distance 41.9028,12.4964 to Paris"), Ok("1107 km".into()));
        let note = request(&config, "distance 41.9028,12.4964 to Paris", 1).unwrap().unwrap().1;
        assert_eq!(note, ["41.9028, 12.4964 to Paris (48.8566, 2.3522), along the Earth's surface (WGS84)"]);
        assert!(r("distance Rmoe to Paris").unwrap_err().contains("did you mean Rome?"));
        assert!(r("distance Atlantis to Paris").unwrap_err().contains("not a known place"));
        assert!(r("distance Rome").unwrap_err().contains("distance <place> [to] <place>"));
        // `to` is optional: where both sides are places.
        assert_eq!(r("Rome Paris"), r("Rome to Paris"));
        assert_eq!(r("london paris mi"), r("London to Paris mi"));
        assert_eq!(r("New York Los Angeles"), r("New York to Los Angeles"));
        assert_eq!(r("distance Rome Paris in km"), r("Rome to Paris"));
        assert!(request(&config, "Mexico City", 1).is_none(), "a city named after its country");
        assert!(request(&config, "Paris km", 1).is_none());
        assert!(request(&config, "10:00 London Paris", 1).is_none());
        assert!(request(&config, "1 2 3 4", 1).is_none(), "bare numbers are not coordinates without `to`");
        assert!(request(&config, "41.9,12.5 48.86,2.35", 1).is_some());
        assert!(request(&config, "10 km mi", 1).is_none());
        // Two places and nothing else: the distance; with a time, not a distance.
        assert_eq!(r("Rome to Doha"), Ok("4019 km".into()));
        assert_eq!(r("rome to doha mi"), Ok("2497 mi".into()));
        assert_eq!(r("Rome to Doha in mi"), Ok("2497 mi".into()), "`in` before the unit is optional");
        assert_eq!(r("distance London to Paris in km"), r("London to Paris km"));
        assert_eq!(r("Italy to Qatar"), Ok("4019 km".into()), "countries: their capitals");
        assert_eq!(r("USA to UK"), r("New York to London"), "several zones: the most relevant city");
        assert!(request(&config, "10:00 Rome to Doha", 1).is_none());
        assert!(request(&config, "now Rome to Doha", 1).is_none());
        assert!(request(&config, "Paris", 1).is_none());
        assert!(request(&config, "10 km to mi", 1).is_none());
        // Whole units, with the device's thousands separator; below 1, decimals.
        let italian = NumberFormat { decimal: ",".into(), thousands: ".".into(), grouping: vec![3, 3] };
        let english = NumberFormat { decimal: ".".into(), thousands: ",".into(), grouping: vec![3, 3] };
        let shown = |s: &str, f: &NumberFormat| request_in(&config, s, 4, f).unwrap().unwrap().0;
        assert_eq!(shown("Rome to Doha", &italian), "4.019 km");
        assert_eq!(shown("Rome to Doha", &english), "4,019 km");
        assert_eq!(shown("Rome to Paris", &english), "1,107 km");
        assert_eq!(shown("Rome to Tokyo", &plain), "9875 km");
        assert_eq!(shown("41.9,12.5 to 41.9,12.501", &italian), "0,083 km");
    }
}
