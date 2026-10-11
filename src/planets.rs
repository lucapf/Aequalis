//! The planets' positions, from JPL's approximate Keplerian elements (Standish, valid 1800–2050:
//! J2000 elements and their rates per century, the ecliptic and equinox of J2000), good to a
//! fraction of a degree: enough for distances and for the dates (to a day or so) of perihelion,
//! aphelion and the closest and farthest approaches to the Earth.

/// a (au), e, I (°), L (°), ϖ (longitude of perihelion, °), Ω (°): at J2000, then per century.
struct Elements {
    name: &'static str,
    at: [f64; 6],
    rate: [f64; 6],
}

const PLANETS: [Elements; 8] = [
    Elements { name: "Mercury", at: [0.38709927, 0.20563593, 7.00497902, 252.25032350, 77.45779628, 48.33076593], rate: [0.00000037, 0.00001906, -0.00594749, 149472.67411175, 0.16047689, -0.12534081] },
    Elements { name: "Venus", at: [0.72333566, 0.00677672, 3.39467605, 181.97909950, 131.60246718, 76.67984255], rate: [0.00000390, -0.00004107, -0.00078890, 58517.81538729, 0.00268329, -0.27769418] },
    Elements { name: "Earth", at: [1.00000261, 0.01671123, -0.00001531, 100.46457166, 102.93768193, 0.0], rate: [0.00000562, -0.00004392, -0.01294668, 35999.37244981, 0.32327364, 0.0] },
    Elements { name: "Mars", at: [1.52371034, 0.09339410, 1.84969142, -4.55343205, -23.94362959, 49.55953891], rate: [0.00001847, 0.00007882, -0.00813131, 19140.30268499, 0.44441088, -0.29257343] },
    Elements { name: "Jupiter", at: [5.20288700, 0.04838624, 1.30439695, 34.39644051, 14.72847983, 100.47390909], rate: [-0.00011607, -0.00013253, -0.00183714, 3034.74612775, 0.21252668, 0.20469106] },
    Elements { name: "Saturn", at: [9.53667594, 0.05386179, 2.48599187, 49.95424423, 92.59887831, 113.66242448], rate: [-0.00125060, -0.00050991, 0.00193609, 1222.49362201, -0.41897216, -0.28867794] },
    Elements { name: "Uranus", at: [19.18916464, 0.04725744, 0.77263783, 313.23810451, 170.95427630, 74.01692503], rate: [-0.00196176, -0.00004397, -0.00242939, 428.48202785, 0.40805281, 0.04240589] },
    Elements { name: "Neptune", at: [30.06992276, 0.00859048, 1.77004347, -55.12002969, 44.96476227, 131.78422574], rate: [0.00026291, 0.00005105, 0.00035372, 218.45945325, -0.32241464, -0.00508664] },
];

/// Unix seconds of J2000.0 (2000-01-01 12:00 TT, ~UTC).
const J2000: f64 = 946_728_000.0;
const DAY: f64 = 86_400.0;
pub const AU_KM: f64 = 149_597_870.7;

fn find(name: &str) -> Option<&'static Elements> {
    PLANETS.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

/// The elements at `t` (Unix seconds): a, e, I, L, ϖ, Ω.
fn elements(p: &Elements, t: f64) -> [f64; 6] {
    let centuries = (t - J2000) / DAY / 36_525.0;
    std::array::from_fn(|i| p.at[i] + p.rate[i] * centuries)
}

/// The heliocentric position (au, ecliptic of J2000) at `t`.
fn position(p: &Elements, t: f64) -> [f64; 3] {
    let [a, e, i, l, varpi, node] = elements(p, t);
    let m = (l - varpi).rem_euclid(360.0).to_radians();
    // Kepler's equation, E − e·sin E = M, by Newton's method.
    let mut big_e = if e < 0.8 { m } else { std::f64::consts::PI };
    for _ in 0..30 {
        let delta = (big_e - e * big_e.sin() - m) / (1.0 - e * big_e.cos());
        big_e -= delta;
        if delta.abs() < 1e-12 {
            break;
        }
    }
    let (xp, yp) = (a * (big_e.cos() - e), a * (1.0 - e * e).sqrt() * big_e.sin());
    let (w, o, i) = ((varpi - node).to_radians(), node.to_radians(), i.to_radians());
    let (cw, sw, co, so, ci, si) = (w.cos(), w.sin(), o.cos(), o.sin(), i.cos(), i.sin());
    [
        (cw * co - sw * so * ci) * xp + (-sw * co - cw * so * ci) * yp,
        (cw * so + sw * co * ci) * xp + (-sw * so + cw * co * ci) * yp,
        (sw * si) * xp + (cw * si) * yp,
    ]
}

/// The distance between two planets (au) at `t`.
fn between(a: &Elements, b: &Elements, t: f64) -> f64 {
    let (p, q) = (position(a, t), position(b, t));
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
}

/// A distance (au) and when it is reached (Unix seconds), if it is a date.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reached {
    pub au: f64,
    pub at: Option<f64>,
}

/// A planet's distances from the Sun or from the Earth: now, mean, the next minimum and maximum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Distances {
    pub now: f64,
    pub mean: f64,
    pub min: Reached,
    pub max: Reached,
}

/// From the Sun, after `now` (Unix seconds): the mean (the semi-major axis), perihelion and
/// aphelion with their next dates. `None`: not a planet.
pub fn from_sun(name: &str, now: f64) -> Option<Distances> {
    let p = find(name)?;
    let [a, e, _, l, varpi, _] = elements(p, now);
    // The mean anomaly grows by the mean motion (L's rate minus ϖ's), in degrees per day.
    let motion = (p.rate[3] - p.rate[4]) / 36_525.0;
    let m = (l - varpi).rem_euclid(360.0);
    let next = |target: f64| now + (target - m).rem_euclid(360.0) / motion * DAY;
    let sun = |t: f64| {
        let r = position(p, t);
        (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt()
    };
    Some(Distances {
        now: sun(now),
        mean: a,
        min: Reached { au: a * (1.0 - e), at: Some(next(0.0)) },
        max: Reached { au: a * (1.0 + e), at: Some(next(180.0)) },
    })
}

/// From the Earth, after `now`: the mean (over a century), the next closest and farthest
/// approaches (found day by day over one synodic period and a half, then to the hour). `None`:
/// not a planet, or the Earth.
pub fn from_earth(name: &str, now: f64) -> Option<Distances> {
    let p = find(name).filter(|p| p.name != "Earth")?;
    let earth = find("Earth")?;
    let d = |t: f64| between(p, earth, t);
    // The synodic period, from the mean motions: 1 / |1/P1 − 1/P2|.
    let (n1, n2) = (p.rate[3] / 36_525.0, earth.rate[3] / 36_525.0);
    let synodic = 360.0 / (n1 - n2).abs();
    let days = (synodic * 1.5).ceil() as usize + 2;
    let samples: Vec<f64> = (0..=days).map(|k| d(now + k as f64 * DAY)).collect();
    // The first local minimum and maximum after now, refined to the hour.
    let refine = |k: usize, lowest: bool| {
        let around = now + k as f64 * DAY;
        let (mut best_t, mut best) = (around, d(around));
        for h in -24..=24 {
            let t = around + h as f64 * 3600.0;
            let v = d(t);
            if (lowest && v < best) || (!lowest && v > best) {
                (best_t, best) = (t, v);
            }
        }
        Reached { au: best, at: Some(best_t) }
    };
    let extreme = |lowest: bool| {
        (1..samples.len() - 1)
            .find(|&k| {
                let (a, b, c) = (samples[k - 1], samples[k], samples[k + 1]);
                if lowest { b <= a && b <= c } else { b >= a && b >= c }
            })
            .map(|k| refine(k, lowest))
    };
    // The mean over a century, every 5 days.
    let n = 36_525 / 5;
    let mean = (0..n).map(|k| d(now + (k * 5) as f64 * DAY)).sum::<f64>() / n as f64;
    Some(Distances { now: d(now), mean, min: extreme(true)?, max: extreme(false)? })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unix seconds of a UTC date.
    fn at(y: i64, m: u32, d: u32) -> f64 {
        (crate::time::days_from_civil(y, m, d) * 86_400) as f64
    }

    #[test]
    fn sun_distances() {
        let earth = from_sun("Earth", at(2026, 10, 11)).unwrap();
        // Perihelion early January (2027-01-03), aphelion early July.
        assert!((earth.min.au - 0.98329).abs() < 0.0005, "{earth:?}");
        assert!((earth.max.au - 1.01671).abs() < 0.0005);
        let perihelion = earth.min.at.unwrap();
        assert!((perihelion - at(2027, 1, 3)).abs() < 4.0 * DAY, "{}", (perihelion - at(2027, 1, 3)) / DAY);
        let aphelion = earth.max.at.unwrap();
        assert!((aphelion - at(2027, 7, 5)).abs() < 4.0 * DAY, "{}", (aphelion - at(2027, 7, 5)) / DAY);
        assert!(from_sun("Pluto", 0.0).is_none());
    }

    #[test]
    fn earth_distances() {
        // Mars's closest approach of 2003 (27 August, 0.3727 au): found from a month before.
        let mars = from_earth("Mars", at(2003, 7, 27)).unwrap();
        assert!((mars.min.au - 0.3727).abs() < 0.002, "{mars:?}");
        assert!((mars.min.at.unwrap() - at(2003, 8, 27)).abs() < 2.0 * DAY);
        // Its mean distance from the Earth is about 1.7 au; Venus's closest is about 0.26–0.29 au.
        assert!((1.5..1.8).contains(&mars.mean), "{}", mars.mean);
        let venus = from_earth("Venus", at(2026, 10, 11)).unwrap();
        assert!((0.25..0.30).contains(&venus.min.au), "{venus:?}");
        assert!(venus.min.at.unwrap() > at(2026, 10, 11));
        assert!(from_earth("Earth", 0.0).is_none());
    }
}
