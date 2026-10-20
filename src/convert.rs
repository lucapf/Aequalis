//! Unit lookup, conversion through the base unit, and result formatting. Units are reduced to
//! SI units and their dimensions, so compound units (`km/h`, `m/s^2`, `Mb/s`) convert to and
//! from the named ones (`mph`, `MB/s`).

use crate::config::{Category, Config, DataSize, Dims, Prefix, Unit};
use std::fmt;

#[derive(Debug, PartialEq)]
pub enum ConvertError {
    UnknownUnit(String),
    Incompatible {
        from: String,
        to: String,
    },
    /// A unit that can't be multiplied or divided: `C` and `F` (an offset scale), `l/100km`
    /// (a reciprocal one).
    NotInCompound(String),
    /// A unit and a currency (`3 d to USD`, `42 USD to ly`): never converted into each other.
    UnitAndCurrency {
        from: String,
        to: String,
    },
    NotFinite,
    DivisionByZero,
    /// A function outside its domain: `sqrt(-1)`.
    Domain(String),
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConvertError::UnknownUnit(u) => write!(f, "unknown unit `{u}`"),
            ConvertError::Incompatible { from, to } => {
                write!(f, "cannot convert `{from}` to `{to}`: different kinds of unit")
            }
            ConvertError::NotInCompound(u) => write!(f, "`{u}` can't be part of a compound unit"),
            ConvertError::UnitAndCurrency { from, to } => {
                write!(f, "cannot convert `{from}` to `{to}`: a unit and a currency don't mix")
            }
            ConvertError::NotFinite => write!(f, "result is out of range"),
            ConvertError::DivisionByZero => f.write_str("undefined"),
            ConvertError::Domain(msg) => f.write_str(msg),
        }
    }
}

/// Exact match first; then case-insensitive, skipping case-sensitive (data) categories.
pub fn find_unit<'a>(config: &'a Config, name: &str) -> Option<(&'a Category, &'a Unit)> {
    let names = config.unit_names();
    // In lower case already (most words typed): looked up as it is, without a copy.
    let lower = || match name.chars().any(char::is_uppercase) {
        true => names.lower.get(&name.to_lowercase()),
        false => names.lower.get(name),
    };
    let &(c, u) = names.exact.get(name).or_else(lower)?;
    let category = &config.categories[c];
    Some((category, &category.units[u]))
}

/// A unit reduced to SI: `value` of it is `value * factor + offset` in the SI units of `dims`.
#[derive(Debug)]
pub struct Resolved<'a> {
    /// How it is printed: a named unit's symbol (`kilometers` → `km`), or a compound unit
    /// written with its terms' symbols (`miles/hour` → `mi/h`).
    pub symbol: String,
    /// The named unit, unless it is a compound one.
    pub unit: Option<&'a Unit>,
    /// The named unit's category: two named units of different categories never convert
    /// directly, even with the same dimensions (`ha` and `l/100km`, an area).
    category: Option<&'static str>,
    /// From a category that converts to and from the reciprocal quantity.
    reciprocal: bool,
    dims: Dims,
    factor: f64,
    offset: f64,
}

fn named<'a>(category: &'a Category, unit: &'a Unit) -> Resolved<'a> {
    // An inverse unit (`l/100km`) is a plain unit of the reciprocal quantity (volume per length).
    let (factor, dims) = match unit.inverse {
        true => (1.0 / (unit.factor * category.si), category.dims.map(|d| -d)),
        false => (unit.factor * category.si, category.dims),
    };
    Resolved {
        symbol: unit.symbol.to_string(),
        unit: Some(unit),
        category: Some(category.name),
        reciprocal: category.reciprocal,
        dims,
        factor,
        offset: unit.offset * category.si,
    }
}

/// A named unit, or a compound one: terms joined by `/` and `*` (or `·`), left to right. A term is
/// a unit with an optional number before it (`100km`) and a power after it (`s^2`, `s2`, `s²`):
/// `km/h`, `m/s^2`, `l/100km`, `kg*m/s2`, `1/min`.
pub fn resolve<'a>(config: &'a Config, text: &str) -> Result<Resolved<'a>, ConvertError> {
    if let Some((category, unit)) = find_unit(config, text) {
        return Ok(named(category, unit));
    }
    let mut out = Resolved { symbol: String::new(), unit: None, category: None, reciprocal: false, dims: [0; 5], factor: 1.0, offset: 0.0 };
    let mut has_unit = false;
    let mut divide = false;
    let mut rest = text;
    loop {
        let end = rest.find(['/', '*', '·']).unwrap_or(rest.len());
        let (factor, dims, symbol) = term(config, &rest[..end]).ok_or_else(|| ConvertError::UnknownUnit(rest[..end].to_string()))??;
        has_unit |= dims != [0; 5];
        match divide {
            true => out.factor /= factor,
            false => out.factor *= factor,
        }
        for (total, d) in out.dims.iter_mut().zip(dims) {
            // A power past what a unit's dimensions hold (±127): an error, not a wrapped power
            // (`m^64*m^64` read as m^-128, bug 012).
            *total = match divide {
                true => total.checked_sub(d),
                false => total.checked_add(d),
            }
            .ok_or_else(|| too_large(text))?;
        }
        out.symbol.push_str(&symbol);
        let Some(op) = rest[end..].chars().next() else { break };
        divide = op == '/';
        out.symbol.push(if divide { '/' } else { '*' });
        rest = &rest[end + op.len_utf8()..];
    }
    if !has_unit {
        return Err(ConvertError::UnknownUnit(text.to_string()));
    }
    Ok(out)
}

/// One term of a compound unit: its SI factor, dimensions and symbol. `None`: not a term at all
/// (the whole text is then an unknown unit).
fn term(config: &Config, text: &str) -> Option<Result<(f64, Dims, String), ConvertError>> {
    // The number before the unit: `100` in `l/100km`, `1` in `1/min`.
    let digits = text.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(text.len());
    let (number, name) = text.split_at(digits);
    let coefficient = match number {
        "" => 1.0,
        n => n.parse::<f64>().ok().filter(|v| *v != 0.0)?,
    };
    if name.is_empty() {
        return (!number.is_empty()).then(|| Ok((coefficient, [0; 5], number.to_string())));
    }
    // The unit, whole (`m2` is a unit of area) or with a power after it.
    let (category, unit, power, power_text) = match find_unit(config, name) {
        Some((c, u)) => (c, u, 1, ""),
        None => {
            let (base, power_text) = split_power(name)?;
            let (c, u) = find_unit(config, base)?;
            let power = match power_text.trim_start_matches('^') {
                "²" => 2,
                "³" => 3,
                p => p.parse::<i8>().ok()?,
            };
            (c, u, power, power_text)
        }
    };
    if unit.offset != 0.0 || unit.inverse {
        return Some(Err(ConvertError::NotInCompound(unit.symbol.to_string())));
    }
    let r = named(category, unit);
    let factor = coefficient * r.factor.powi(power.into());
    let mut dims = [0; 5];
    for (out, d) in dims.iter_mut().zip(r.dims) {
        let Some(p) = d.checked_mul(power) else { return Some(Err(too_large(text))) };
        *out = p;
    }
    Some(Ok((factor, dims, format!("{number}{}{power_text}", unit.symbol))))
}

fn too_large(unit: &str) -> ConvertError {
    ConvertError::Domain(format!("the powers in `{unit}` are too large (at most 127)"))
}

/// `s^2` → (`s`, `^2`), `s2` → (`s`, `2`), `m³` → (`m`, `³`), `s^-1` → (`s`, `^-1`).
fn split_power(name: &str) -> Option<(&str, &str)> {
    if let Some(i) = name.find('^') {
        return Some((&name[..i], &name[i..]));
    }
    if let Some(base) = name.strip_suffix(['²', '³']) {
        return Some((base, &name[base.len()..]));
    }
    let base = name.trim_end_matches(|c: char| c.is_ascii_digit());
    (base.len() < name.len() && !base.is_empty()).then(|| (base, &name[base.len()..]))
}

/// Converts `value` from one unit to another (named or compound) of the same dimensions; or of
/// the reciprocal ones when a side is a fuel economy unit (`mpg` ↔ `l/100km`).
pub fn convert<'a>(config: &'a Config, value: f64, from: &str, to: &str) -> Result<(f64, Resolved<'a>), ConvertError> {
    let currency = |text: &str| crate::currency::exact(text).is_some();
    let (a, b) = match (resolve(config, from), resolve(config, to)) {
        (Ok(a), Ok(b)) => (a, b),
        // A currency where a unit was expected, with a unit on the other side.
        (Ok(_), Err(_)) if currency(to) => return Err(ConvertError::UnitAndCurrency { from: from.to_string(), to: to.to_string() }),
        (Err(_), Ok(_)) if currency(from) => return Err(ConvertError::UnitAndCurrency { from: from.to_string(), to: to.to_string() }),
        (a, b) => (a?, b?),
    };
    let si = value * a.factor + a.offset;
    let categories_differ = matches!((a.category, b.category), (Some(x), Some(y)) if x != y);
    let result = if a.dims == b.dims && !categories_differ {
        (si - b.offset) / b.factor
    // Compared in i16: a power of −128 has no opposite in i8 (bug 015).
    } else if a.dims.iter().zip(b.dims).all(|(&x, y)| i16::from(x) == -i16::from(y)) && (a.reciprocal || b.reciprocal) {
        if si == 0.0 {
            return Err(ConvertError::DivisionByZero);
        }
        (1.0 / si - b.offset) / b.factor
    } else {
        return Err(ConvertError::Incompatible { from: from.to_string(), to: to.to_string() });
    };
    if !result.is_finite() {
        return Err(ConvertError::NotFinite);
    }
    Ok((result, b))
}

/// Warnings for conversions that are valid but probably not what was meant:
/// mixing bits with bytes, or decimal (1000-based) with binary (1024-based) prefixes.
pub fn warnings(from: &Unit, to: &Unit) -> Vec<String> {
    let (Some(a), Some(b)) = (from.data, to.data) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if a.size != b.size {
        let (bits, bytes) = if a.size == DataSize::Bit { (from, to) } else { (to, from) };
        out.push(format!("converting between bits ({}) and bytes ({}); 1 byte = 8 bits. Use `b` for bits and `B` for bytes", bits.symbol, bytes.symbol));
    }
    if (a.prefix, b.prefix) == (Prefix::Si, Prefix::Iec) || (a.prefix, b.prefix) == (Prefix::Iec, Prefix::Si) {
        let (si, iec) = if a.prefix == Prefix::Si { (from, to) } else { (to, from) };
        out.push(format!("mixing decimal ({}, powers of 1000) and binary ({}, powers of 1024) units", si.symbol, iec.symbol));
    }
    out
}

/// `value` with exactly `decimals` digits after the point, halves rounded away from zero on the
/// shortest decimal that gives the value back (what `{}` prints): the digits people see decide,
/// as on a calculator. `2.5` is `3`, `0.125` is `0.13`, `2.675` is `2.68`; Rust's own formatting
/// reads the binary value (2.67499…) and rounds halves to even (`2`, `0.12`, `2.67`). `-0` is `0`.
pub fn fixed(value: f64, decimals: usize) -> String {
    // Plain digits, never an exponent: `2.675`, `0.0000001`, `1000000000000000000000`.
    let shortest = format!("{}", value.abs());
    let (int, frac) = shortest.split_once('.').unwrap_or((&shortest, ""));
    let mut digits: Vec<u8> = int.bytes().chain(frac.bytes().chain(std::iter::repeat(b'0')).take(decimals)).collect();
    // The first digit dropped decides: from 5 up, the last kept one goes up (and carries).
    if frac.as_bytes().get(decimals).is_some_and(|&d| d >= b'5') {
        match digits.iter().rposition(|&d| d != b'9') {
            Some(i) => {
                digits[i] += 1;
                digits[i + 1..].fill(b'0');
            }
            None => {
                digits.fill(b'0');
                digits.insert(0, b'1');
            }
        }
    }
    let int_len = digits.len() - decimals;
    let mut out: String = digits.into_iter().map(char::from).collect();
    if decimals > 0 {
        out.insert(int_len, '.');
    }
    if value.is_sign_negative() && out.bytes().any(|b| b.is_ascii_digit() && b != b'0') {
        out.insert(0, '-');
    }
    out
}

/// Fixed precision (`fixed`), then trailing zeros (and a dangling `.`) trimmed. Scientific
/// notation when the fixed form can't show the value: a non-zero value that would round to 0
/// (`1 eV J` → `1.6022e-19`), or one of 10¹⁵ or more whose digits are not all exact (`1 J eV` →
/// `6.2415e18`; but `1 yr ns` → `31557600000000000`) or of 10²¹ or more. `precision` is then the
/// mantissa's decimals.
pub fn format_value(value: f64, precision: usize) -> String {
    let tiny = value != 0.0 && value.abs() < 0.5 * 10f64.powi(-(precision as i32));
    // The shortest digits that give back the value: beyond 15 of them, the last are noise.
    let inexact = value.abs() >= 1e15 && format!("{}", value.abs()).trim_end_matches('0').len() > 15;
    if tiny || inexact || value.abs() >= 1e21 {
        let s = format!("{value:.precision$e}");
        let (mantissa, exponent) = s.split_once('e').expect("scientific notation has an exponent");
        let mantissa = match mantissa.contains('.') {
            true => mantissa.trim_end_matches('0').trim_end_matches('.'),
            false => mantissa,
        };
        return format!("{mantissa}e{exponent}");
    }
    // At most 15 significant digits: past them a double's decimals are noise (`1 ly km` →
    // `9460730472580.8`, not `…580.8008`).
    let int_digits = if value.abs() >= 1.0 { value.abs().log10().floor() as usize + 1 } else { 0 };
    let precision = precision.min(15usize.saturating_sub(int_digits));
    let mut s = fixed(value, precision);
    if s.contains('.') {
        s.truncate(s.trim_end_matches('0').trim_end_matches('.').len());
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(value: f64, from: &str, to: &str) -> String {
        let config = Config::default();
        let (v, u) = convert(&config, value, from, to).unwrap();
        format!("{} {}", format_value(v, config.default_precision), u.symbol)
    }

    #[test]
    fn spec_examples() {
        assert_eq!(conv(10.0, "km", "mi"), "6.2137 mi");
        assert_eq!(conv(5.0, "ft", "m"), "1.524 m");
        assert_eq!(conv(1500.0, "m", "km"), "1.5 km");
        assert_eq!(conv(100.0, "C", "F"), "212 F");
        assert_eq!(conv(-40.0, "C", "F"), "-40 F");
        assert_eq!(conv(0.0, "C", "K"), "273.15 K");
        assert_eq!(conv(1.0, "gal-uk", "l"), "4.5461 l");
        assert_eq!(conv(1.0, "gal", "l"), "3.7854 l");
        assert_eq!(conv(100.0, "Mb", "MB"), "12.5 MB");
        assert_eq!(conv(1.0, "GiB", "MB"), "1073.7418 MB");
        assert_eq!(conv(2.0, "kilometers", "miles"), "1.2427 mi");
    }

    #[test]
    fn round_trips() {
        let config = Config::default();
        for cat in &config.categories {
            for a in &cat.units {
                for b in &cat.units {
                    let (there, _) = convert(&config, 123.456, a.symbol, b.symbol).unwrap();
                    let (back, _) = convert(&config, there, b.symbol, a.symbol).unwrap();
                    assert!((back - 123.456).abs() / 123.456 < 1e-9, "{} -> {} -> {}", a.symbol, b.symbol, a.symbol);
                }
            }
        }
    }

    #[test]
    fn area_units() {
        assert_eq!(conv(1.0, "ha", "m2"), "10000 m2");
        assert_eq!(conv(1.0, "ac", "ft2"), "43560 ft2");
        assert_eq!(conv(1.0, "mi2", "ac"), "640 ac");
        assert_eq!(conv(1.0, "km2", "mi2"), "0.3861 mi2");
        assert_eq!(conv(1.0, "m2", "ft2"), "10.7639 ft2");
        assert_eq!(conv(1.0, "square-feet", "sq-in"), "144 in2");
        assert_eq!(conv(1.0, "sqft", "sqin"), "144 in2");
        assert_eq!(conv(1.0, "SQM", "sqcm"), "10000 cm2");
        assert_eq!(conv(1.0, "m²", "cm2"), "10000 cm2");
        assert_eq!(conv(1.0, "M2", "mm2"), "1000000 mm2");
    }

    #[test]
    fn speed_pressure_fuel_energy_power() {
        assert_eq!(conv(100.0, "km/h", "mph"), "62.1371 mph");
        assert_eq!(conv(10.0, "m/s", "km/h"), "36 km/h");
        assert_eq!(conv(1.0, "knot", "km/h"), "1.852 km/h");
        assert_eq!(conv(1.0, "kph", "m/s"), "0.2778 m/s");
        assert_eq!(conv(1.0, "bar", "psi"), "14.5038 psi");
        assert_eq!(conv(1.0, "atm", "hPa"), "1013.25 hPa");
        assert_eq!(conv(1.0, "atm", "Torr"), "760 Torr");
        assert_eq!(conv(1.0, "atm", "mmHg"), "759.9999 mmHg");
        assert_eq!(conv(29.92, "inHg", "mbar"), "1013.2076 mbar");
        assert_eq!(conv(5.0, "l/100km", "mpg"), "47.0429 mpg");
        assert_eq!(conv(40.0, "mpg", "l/100km"), "5.8804 l/100km");
        assert_eq!(conv(10.0, "km/l", "l/100km"), "10 l/100km");
        assert_eq!(conv(1.0, "mpg-uk", "mpg"), "0.8327 mpg");
        assert_eq!(conv(1.0, "kWh", "MJ"), "3.6 MJ");
        assert_eq!(conv(100.0, "kcal", "kJ"), "418.4 kJ");
        assert_eq!(conv(1.0, "therm", "kWh"), "29.3001 kWh");
        assert_eq!(conv(1.0, "BTU", "ft-lbf"), "778.1693 ft-lbf");
        assert_eq!(conv(100.0, "hp", "kW"), "74.57 kW");
        assert_eq!(conv(100.0, "cv", "hp"), "98.632 hp");
        assert_eq!(conv(1.0, "kW", "BTU/h"), "3412.1416 BTU/h");
    }

    #[test]
    fn angles_and_astronomy() {
        assert_eq!(conv(180.0, "deg", "rad"), "3.1416 rad");
        assert_eq!(conv(1.0, "turn", "°"), "360 deg");
        assert_eq!(conv(100.0, "gon", "degrees"), "90 deg");
        assert_eq!(conv(1.0, "deg", "arcsec"), "3600 arcsec");
        assert_eq!(conv(1.0, "ly", "au"), "63241.0771 au");
        assert_eq!(conv(1.0, "pc", "ly"), "3.2616 ly");
        assert_eq!(conv(1.0, "au", "km"), "149597870.7 km");
        assert_eq!(conv(1.0, "ly", "km"), "9460730472580.8 km");
        // Angular speed.
        assert_eq!(conv(1.0, "turn/s", "rad/s"), "6.2832 rad/s");
    }

    #[test]
    fn compound_units() {
        // Written with their terms' symbols; equal to the named units.
        assert_eq!(conv(100.0, "kilometers/hour", "mi/h"), "62.1371 mph");
        assert_eq!(conv(1.0, "mi/min", "km/h"), "96.5606 km/h");
        assert_eq!(conv(9.81, "m/s^2", "ft/s2"), "32.185 ft/s2");
        assert_eq!(conv(1.0, "m/s²", "km/h/s"), "3.6 km/h/s");
        assert_eq!(conv(100.0, "Mb/s", "MB/s"), "12.5 MB/s");
        assert_eq!(conv(1.0, "GiB/h", "KiB/s"), "291.2711 KiB/s");
        assert_eq!(conv(20.0, "km/l", "mi/gal"), "47.0429 mpg");
        assert_eq!(conv(1.0, "l/100km", "l/km"), "0.01 l/km");
        assert_eq!(conv(1.0, "kg*m/s2", "kg*m/s^2"), "1 kg*m/s^2");
        assert_eq!(conv(1.0, "kg/m3", "g/cm3"), "0.001 g/cm3");
        // Energy and power from their parts.
        assert_eq!(conv(1.0, "kW*h", "MJ"), "3.6 MJ");
        assert_eq!(conv(1.0, "kg*m2/s2", "J"), "1 J");
        assert_eq!(conv(3.6, "MJ/h", "kW"), "1 kW");
    }

    #[test]
    fn compound_errors() {
        let config = Config::default();
        let err = |from: &str, to: &str| convert(&config, 1.0, from, to).unwrap_err();
        assert_eq!(err("foo/h", "mph"), ConvertError::UnknownUnit("foo".into()));
        assert_eq!(err("km/h", "kg"), ConvertError::Incompatible { from: "km/h".into(), to: "kg".into() });
        assert_eq!(err("C/s", "K/s"), ConvertError::NotInCompound("C".into()));
        // Same dimensions, different categories: an area is not a fuel consumption.
        assert_eq!(err("ha", "l/100km"), ConvertError::Incompatible { from: "ha".into(), to: "l/100km".into() });
        // Reciprocals only for fuel economy.
        assert_eq!(err("m", "1/m"), ConvertError::Incompatible { from: "m".into(), to: "1/m".into() });
        assert_eq!(convert(&config, 0.0, "mpg", "l/100km").unwrap_err(), ConvertError::DivisionByZero);
        // A number alone is not a unit.
        assert_eq!(err("100", "m"), ConvertError::UnknownUnit("100".into()));
    }

    #[test]
    fn temperature_fixed_points() {
        assert_eq!(conv(0.0, "C", "F"), "32 F");
        assert_eq!(conv(32.0, "F", "K"), "273.15 K");
        assert_eq!(conv(-40.0, "F", "C"), "-40 C");
    }

    #[test]
    fn data_units() {
        assert_eq!(conv(8.0, "b", "B"), "1 B");
        assert_eq!(conv(1.0, "KiB", "B"), "1024 B");
        assert_eq!(conv(1.0, "kB", "B"), "1000 B");
        assert_eq!(conv(1.0, "KB", "Kb"), "8 kb");
        let config = Config::default();
        assert!(find_unit(&config, "mb").is_none());
        assert!(find_unit(&config, "mB").is_none());
    }

    #[test]
    fn aliases_and_case() {
        let config = Config::default();
        assert_eq!(find_unit(&config, "Miles").unwrap().1.symbol, "mi");
        assert_eq!(find_unit(&config, "FEET").unwrap().1.symbol, "ft");
        assert_eq!(find_unit(&config, "M").unwrap().1.symbol, "m");
        assert_eq!(find_unit(&config, "°C").unwrap().1.symbol, "C");
    }

    #[test]
    fn units_and_currencies_dont_mix() {
        let config = Config::default();
        let mixed = |from: &str, to: &str| ConvertError::UnitAndCurrency { from: from.into(), to: to.into() };
        assert_eq!(convert(&config, 3.0, "d", "USD").unwrap_err(), mixed("d", "USD"));
        assert_eq!(convert(&config, 42.0, "USD", "ly").unwrap_err(), mixed("USD", "ly"));
        assert_eq!(convert(&config, 1.0, "euro", "km/h").unwrap_err(), mixed("euro", "km/h"));
        assert_eq!(mixed("d", "USD").to_string(), "cannot convert `d` to `USD`: a unit and a currency don't mix");
        // Neither a unit: still unknown.
        assert_eq!(convert(&config, 1.0, "foo", "USD").unwrap_err(), ConvertError::UnknownUnit("foo".into()));
    }

    #[test]
    fn errors() {
        let config = Config::default();
        assert_eq!(convert(&config, 1.0, "km", "kg").unwrap_err(), ConvertError::Incompatible { from: "km".into(), to: "kg".into() });
        assert_eq!(convert(&config, 1.0, "foo", "m").unwrap_err(), ConvertError::UnknownUnit("foo".into()));
        assert_eq!(convert(&config, 1e308, "km", "mm").unwrap_err(), ConvertError::NotFinite);
    }

    #[test]
    fn coherence_warnings() {
        let config = Config::default();
        let warn = |a: &str, b: &str| {
            let (_, ua) = find_unit(&config, a).unwrap();
            let (_, ub) = find_unit(&config, b).unwrap();
            warnings(ua, ub)
        };
        assert!(warn("kB", "GB").is_empty());
        assert!(warn("Kb", "Gb").is_empty());
        assert!(warn("KiB", "GiB").is_empty());
        assert!(warn("B", "GiB").is_empty());
        assert!(warn("km", "mi").is_empty());
        assert_eq!(warn("kB", "Gb").len(), 1);
        assert!(warn("Mb", "MB")[0].contains("bits (Mb) and bytes (MB)"));
        assert!(warn("GB", "GiB")[0].contains("decimal (GB"));
        assert!(warn("MiB", "kB")[0].contains("decimal (kB"));
        assert_eq!(warn("Gib", "MB").len(), 2);
    }

    /// Halves away from zero, on the digits people see.
    #[test]
    fn rounding() {
        for (value, decimals, want) in [
            (2.5, 0, "3"),
            (0.5, 0, "1"),
            (-2.5, 0, "-3"),
            (1.25, 1, "1.3"),
            (0.125, 2, "0.13"),
            (2.675, 2, "2.68"),
            (1.005, 2, "1.01"),
            (9.995, 2, "10.00"),
            (99.5, 0, "100"),
            (0.4, 0, "0"),
            (-0.4, 0, "0"),
            (-0.0, 2, "0.00"),
            (12.3, 3, "12.300"),
            (1e21, 0, "1000000000000000000000"),
            (6.213711922373339, 4, "6.2137"),
            (0.1 + 0.2, 4, "0.3000"),
        ] {
            assert_eq!(fixed(value, decimals), want, "{value} to {decimals}");
        }
        assert_eq!(format_value(2.5, 0), "3");
        assert_eq!(format_value(0.5, 0), "1", "a non-zero value never shows as 0");
        assert_eq!(format_value(2.675, 2), "2.68");
    }

    #[test]
    fn formatting() {
        assert_eq!(format_value(2.0, 4), "2");
        assert_eq!(format_value(2.5, 4), "2.5");
        assert_eq!(format_value(2.204_622, 2), "2.2");
        assert_eq!(format_value(2.6, 0), "3");
        assert_eq!(format_value(-0.0, 4), "0");
        assert_eq!(format_value(-0.00004, 4), "-4e-5", "not 0: scientific");
        assert_eq!(format_value(100.0, 0), "100");
    }

    #[test]
    fn scientific_when_needed() {
        // Too small for the precision: not 0, but the value.
        assert_eq!(format_value(1.602_176_634e-19, 4), "1.6022e-19");
        assert_eq!(format_value(1.602_176_634e-19, 10), "1.602176634e-19");
        assert_eq!(format_value(-0.000_012_5, 4), "-1.25e-5");
        assert_eq!(format_value(0.000_05, 4), "0.0001", "rounds to the last place: fixed");
        assert_eq!(format_value(0.3, 0), "3e-1");
        // Huge.
        assert_eq!(format_value(2.5e21, 4), "2.5e21");
        assert_eq!(format_value(1e20, 4), "100000000000000000000", "exact: fixed");
        assert_eq!(format_value(31_557_600_000_000_000.0, 4), "31557600000000000");
        assert_eq!(format_value(999_999_999_999_999.0, 4), "999999999999999");
        // Exactly 0 stays 0.
        assert_eq!(format_value(0.0, 4), "0");
        assert_eq!(conv(1.0, "eV", "J"), "1.6022e-19 J");
        assert_eq!(conv(1.0, "J", "eV"), "6.2415e18 eV");
    }
}
