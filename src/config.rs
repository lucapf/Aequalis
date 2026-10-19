//! The single configuration object: unit tables, aliases and output defaults.

use std::collections::HashMap;
use std::sync::OnceLock;

/// A unit is converted through its category's base unit:
/// `base = value * factor + offset`, or `base = factor / value` for an `inverse` unit (`l/100km`,
/// whose base is `km/l`).
#[derive(Debug)]
pub struct Unit {
    pub symbol: &'static str,
    /// The full name, shown in the interactive mode: `kilometre`, `megabyte (10⁶ bytes)`.
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub factor: f64,
    pub offset: f64,
    /// The unit measures the reciprocal of its category's quantity (fuel consumption, `l/100km`).
    pub inverse: bool,
    /// Only set for data units; used to warn about incoherent conversions.
    pub data: Option<DataKind>,
    /// Which tab of the interactive unit keys shows it.
    pub system: System,
}

/// The two groups a category's units are split into. For most categories these are the metric
/// and the imperial (or US customary) systems; for data sizes, decimal (SI, 1000ⁿ) and binary
/// (IEC, 1024ⁿ) prefixes. `Both`: units that belong to either (`b`, `B`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum System {
    Metric,
    Imperial,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataSize {
    Bit,
    Byte,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Prefix {
    /// No prefix (`b`, `B`): coherent with both systems.
    None,
    /// Decimal, powers of 1000 (k, M, G, T).
    Si,
    /// Binary, powers of 1024 (Ki, Mi, Gi, Ti).
    Iec,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataKind {
    pub size: DataSize,
    pub prefix: Prefix,
}

/// The exponents of the base dimensions, whose SI units are `m kg s K b` (data: the bit):
/// `km/h` is m¹·s⁻¹.
pub type Dims = [i8; 5];

pub struct Category {
    pub name: &'static str,
    pub base: &'static str,
    /// The dimensions of the base unit, and the base unit in SI units (`l` = 0.001 m³), so that
    /// compound units (`mi/h`, `m/s^2`) can be converted to and from the category's units.
    pub dims: Dims,
    pub si: f64,
    /// Converts to and from the reciprocal quantity: `mpg` ↔ `l/100km`.
    pub reciprocal: bool,
    /// Data units distinguish `b` (bit) from `B` (byte), so they are matched exactly.
    pub case_sensitive: bool,
    /// The names of the two systems, shown as tabs: `[metric, imperial]`.
    pub systems: [&'static str; 2],
    pub units: Vec<Unit>,
}

/// The category of durations, whose second tab lists the date formats.
pub const TIME: &str = "Date & Time";

/// The category of digital information: bits and bytes, decimal and binary.
pub const BITS: &str = "Bits & Bytes";

pub struct Config {
    pub categories: Vec<Category>,
    /// The decimal places when no precision is given (the `precision` setting's default).
    pub default_precision: usize,
    pub max_precision: usize,
    /// Entries kept in the interactive mode's history (the oldest are dropped).
    pub max_history: usize,
    /// Every unit by name (`Config::unit_names`), built on first use.
    unit_names: OnceLock<UnitNames>,
}

/// The units by symbol and alias, as (category, unit) indexes: written exactly, and in lower case
/// for the categories where case does not matter. The first unit with a name wins.
pub struct UnitNames {
    pub exact: HashMap<&'static str, (usize, usize)>,
    pub lower: HashMap<String, (usize, usize)>,
}

impl Config {
    /// The process's configuration, built once with its tables (the unit names): the data is
    /// constant, so every caller shares it instead of building its own.
    pub fn shared() -> &'static Config {
        static SHARED: OnceLock<Config> = OnceLock::new();
        SHARED.get_or_init(Config::default)
    }

    pub fn unit_names(&self) -> &UnitNames {
        self.unit_names.get_or_init(|| {
            let mut names = UnitNames { exact: HashMap::new(), lower: HashMap::new() };
            for (c, category) in self.categories.iter().enumerate() {
                for (u, unit) in category.units.iter().enumerate() {
                    for name in std::iter::once(unit.symbol).chain(unit.aliases.iter().copied()) {
                        names.exact.entry(name).or_insert((c, u));
                        if !category.case_sensitive {
                            names.lower.entry(name.to_lowercase()).or_insert((c, u));
                        }
                    }
                }
            }
            names
        })
    }
}

const fn metric(symbol: &'static str, name: &'static str, aliases: &'static [&'static str], factor: f64) -> Unit {
    Unit { symbol, name, aliases, factor, offset: 0.0, inverse: false, data: None, system: System::Metric }
}

const fn imperial(symbol: &'static str, name: &'static str, aliases: &'static [&'static str], factor: f64) -> Unit {
    Unit { symbol, name, aliases, factor, offset: 0.0, inverse: false, data: None, system: System::Imperial }
}

/// A unit of the reciprocal quantity: `base = factor / value`.
const fn inverse(symbol: &'static str, name: &'static str, aliases: &'static [&'static str], factor: f64) -> Unit {
    Unit { symbol, name, aliases, factor, offset: 0.0, inverse: true, data: None, system: System::Metric }
}

/// Data units: decimal prefixes go with the first tab, binary ones with the second.
const fn data(symbol: &'static str, name: &'static str, aliases: &'static [&'static str], factor: f64, size: DataSize, prefix: Prefix) -> Unit {
    let system = match prefix {
        Prefix::None => System::Both,
        Si => System::Metric,
        Iec => System::Imperial,
    };
    Unit { symbol, name, aliases, factor, offset: 0.0, inverse: false, data: Some(DataKind { size, prefix }), system }
}

const METRIC_IMPERIAL: [&str; 2] = ["Metric", "Imperial"];

/// Time units, all on the first tab ("Durations"; the second, "Formats", lists the date formats):
/// `ns`…`h` are exact; `d wk mo yr` are fixed lengths in conversions (Julian year, 365.25 d;
/// month = 1/12 year) and calendar steps in date math (see `time.rs`).
const fn time(symbol: &'static str, name: &'static str, aliases: &'static [&'static str], seconds: f64) -> Unit {
    Unit { symbol, name, aliases, factor: seconds, offset: 0.0, inverse: false, data: None, system: System::Metric }
}

/// The `num` target (any case): the request is a number, never a date: `2026-12-02 num` → 2012.
pub const NUMBER_WORDS: &[&str] = &["num", "n", "number"];

pub fn is_number_word(word: &str) -> bool {
    NUMBER_WORDS.iter().any(|w| w.eq_ignore_ascii_case(word))
}

/// Words of the date requests (`time.rs`): the values `now` and `today`, the `date` input word,
/// then the formats (`time::is_format`: from the fourth word on).
pub const DATE_WORDS: &[&str] = &[
    "now",
    "today",
    "date",
    "iso",
    "iso-utc",
    "iso-basic",
    "iso-date",
    "iso-ordinal",
    "iso-week",
    "iso-time",
    "unix",
    "unix-ms",
    "unix-us",
    "unix-ns",
    "local",
];

const KI: f64 = 1024.0;
use DataSize::{Bit, Byte};
use Prefix::{Iec, Si};

impl Default for Config {
    fn default() -> Self {
        Config {
            default_precision: 4,
            max_precision: 15,
            max_history: 500,
            unit_names: OnceLock::new(),
            categories: vec![
                Category {
                    name: TIME,
                    base: "s",
                    dims: [0, 0, 1, 0, 0],
                    si: 1.0,
                    reciprocal: false,
                    case_sensitive: false,
                    systems: ["Durations", "Formats"],
                    units: vec![
                        time("ns", "nanosecond", &["nanosecond", "nanoseconds"], 1e-9),
                        time("us", "microsecond", &["µs", "microsecond", "microseconds"], 1e-6),
                        time("ms", "millisecond", &["millisecond", "milliseconds"], 1e-3),
                        time("s", "second", &["sec", "secs", "second", "seconds"], 1.0),
                        time("min", "minute", &["mins", "minute", "minutes"], 60.0),
                        time("h", "hour", &["hr", "hrs", "hour", "hours"], 3600.0),
                        time("d", "day (86400 s; a calendar day in date math)", &["day", "days"], 86_400.0),
                        time("wk", "week (7 days)", &["week", "weeks"], 604_800.0),
                        time("mo", "month (1/12 year; a calendar month in date math)", &["month", "months"], 2_629_800.0),
                        time("yr", "year (Julian, 365.25 d; a calendar year in date math)", &["y", "year", "years"], 31_557_600.0),
                    ],
                },
                Category {
                    name: BITS,
                    base: "b",
                    dims: [0, 0, 0, 0, 1],
                    si: 1.0,
                    reciprocal: false,
                    case_sensitive: true,
                    systems: ["Decimal", "Binary"],
                    units: vec![
                        data("B", "byte (8 bits)", &["byte", "bytes"], 8.0, Byte, Prefix::None),
                        data("kB", "kilobyte (10³ bytes)", &["KB"], 8e3, Byte, Si),
                        data("MB", "megabyte (10⁶ bytes)", &[], 8e6, Byte, Si),
                        data("GB", "gigabyte (10⁹ bytes)", &[], 8e9, Byte, Si),
                        data("TB", "terabyte (10¹² bytes)", &[], 8e12, Byte, Si),
                        data("KiB", "kibibyte (2¹⁰ bytes)", &[], 8.0 * KI, Byte, Iec),
                        data("MiB", "mebibyte (2²⁰ bytes)", &[], 8.0 * KI * KI, Byte, Iec),
                        data("GiB", "gibibyte (2³⁰ bytes)", &[], 8.0 * KI * KI * KI, Byte, Iec),
                        data("TiB", "tebibyte (2⁴⁰ bytes)", &[], 8.0 * KI * KI * KI * KI, Byte, Iec),
                        data("b", "bit", &["bit", "bits"], 1.0, Bit, Prefix::None),
                        data("kb", "kilobit (10³ bits)", &["Kb"], 1e3, Bit, Si),
                        data("Mb", "megabit (10⁶ bits)", &[], 1e6, Bit, Si),
                        data("Gb", "gigabit (10⁹ bits)", &[], 1e9, Bit, Si),
                        data("Tb", "terabit (10¹² bits)", &[], 1e12, Bit, Si),
                        data("Kib", "kibibit (2¹⁰ bits)", &[], KI, Bit, Iec),
                        data("Mib", "mebibit (2²⁰ bits)", &[], KI * KI, Bit, Iec),
                        data("Gib", "gibibit (2³⁰ bits)", &[], KI * KI * KI, Bit, Iec),
                        data("Tib", "tebibit (2⁴⁰ bits)", &[], KI * KI * KI * KI, Bit, Iec),
                    ],
                },
                Category {
                    name: "Length",
                    base: "m",
                    dims: [1, 0, 0, 0, 0],
                    si: 1.0,
                    reciprocal: false,
                    case_sensitive: false,
                    systems: METRIC_IMPERIAL,
                    units: vec![
                        metric("mm", "millimetre", &["millimeter", "millimeters", "millimetre", "millimetres"], 0.001),
                        metric("cm", "centimetre", &["centimeter", "centimeters", "centimetre", "centimetres"], 0.01),
                        metric("m", "metre", &["meter", "meters", "metre", "metres"], 1.0),
                        metric("km", "kilometre", &["kilometer", "kilometers", "kilometre", "kilometres"], 1000.0),
                        metric("au", "astronomical unit (149597870.7 km)", &["astronomical-unit", "astronomical-units"], 149_597_870_700.0),
                        metric(
                            "ly",
                            "light-year (Julian year of light travel)",
                            &["light-year", "light-years", "lightyear", "lightyears"],
                            9_460_730_472_580_800.0,
                        ),
                        metric("pc", "parsec (648000/π au)", &["parsec", "parsecs"], 30_856_775_814_913_673.0),
                        imperial("in", "inch", &["inch", "inches"], 0.0254),
                        imperial("ft", "foot", &["foot", "feet"], 0.3048),
                        imperial("yd", "yard", &["yard", "yards"], 0.9144),
                        imperial("mi", "mile", &["mile", "miles"], 1609.344),
                    ],
                },
                Category {
                    name: "Area",
                    base: "m2",
                    dims: [2, 0, 0, 0, 0],
                    si: 1.0,
                    reciprocal: false,
                    case_sensitive: false,
                    systems: METRIC_IMPERIAL,
                    units: vec![
                        metric(
                            "mm2",
                            "square millimetre",
                            &["mm²", "sq-mm", "sqmm", "mmq", "square-millimeter", "square-millimeters", "square-millimetre", "square-millimetres"],
                            0.000_001,
                        ),
                        metric(
                            "cm2",
                            "square centimetre",
                            &["cm²", "sq-cm", "sqcm", "cmq", "square-centimeter", "square-centimeters", "square-centimetre", "square-centimetres"],
                            0.0001,
                        ),
                        metric("m2", "square metre", &["m²", "sq-m", "sqm", "mq", "square-meter", "square-meters", "square-metre", "square-metres"], 1.0),
                        metric("ha", "hectare", &["hectare", "hectares"], 10_000.0),
                        metric(
                            "km2",
                            "square kilometre",
                            &["km²", "sq-km", "sqkm", "kmq", "square-kilometer", "square-kilometers", "square-kilometre", "square-kilometres"],
                            1_000_000.0,
                        ),
                        imperial("in2", "square inch", &["in²", "sq-in", "sqin", "square-inch", "square-inches"], 0.000_645_16),
                        imperial("ft2", "square foot", &["ft²", "sq-ft", "sqft", "square-foot", "square-feet"], 0.092_903_04),
                        imperial("yd2", "square yard", &["yd²", "sq-yd", "sqyd", "square-yard", "square-yards"], 0.836_127_36),
                        imperial("ac", "acre", &["acre", "acres"], 4_046.856_422_4),
                        imperial("mi2", "square mile", &["mi²", "sq-mi", "sqmi", "square-mile", "square-miles"], 2_589_988.110_336),
                    ],
                },
                Category {
                    name: "Volume",
                    base: "l",
                    dims: [3, 0, 0, 0, 0],
                    si: 0.001,
                    reciprocal: false,
                    case_sensitive: false,
                    systems: METRIC_IMPERIAL,
                    units: vec![
                        metric("ml", "millilitre", &["milliliter", "milliliters", "millilitre", "millilitres"], 0.001),
                        metric("cl", "centilitre", &["centiliter", "centiliters", "centilitre", "centilitres"], 0.01),
                        metric("l", "litre", &["L", "liter", "liters", "litre", "litres"], 1.0),
                        metric("m3", "cubic metre", &["m³", "cubic-meter", "cubic-meters"], 1000.0),
                        imperial("floz-us", "US fluid ounce", &["floz", "fl-oz"], 0.029_573_529_562_5),
                        imperial("pt-us", "US pint", &["pt", "pint", "pints"], 0.473_176_473),
                        imperial("qt-us", "US quart", &["qt", "quart", "quarts"], 0.946_352_946),
                        imperial("gal-us", "US gallon", &["gal", "gallon", "gallons"], 3.785_411_784),
                        imperial("cup", "US cup", &["cup-us", "cups"], 0.236_588_236_5),
                        imperial("floz-uk", "imperial (UK) fluid ounce", &[], 0.028_413_062_5),
                        imperial("pt-uk", "imperial (UK) pint", &[], 0.568_261_25),
                        imperial("qt-uk", "imperial (UK) quart", &[], 1.136_522_5),
                        imperial("gal-uk", "imperial (UK) gallon", &[], 4.546_09),
                    ],
                },
                Category {
                    name: "Angle",
                    base: "rad",
                    // Dimensionless: a ratio of lengths.
                    dims: [0, 0, 0, 0, 0],
                    si: 1.0,
                    reciprocal: false,
                    case_sensitive: false,
                    systems: METRIC_IMPERIAL,
                    units: vec![
                        metric("rad", "radian", &["radian", "radians"], 1.0),
                        metric("mrad", "milliradian", &["milliradian", "milliradians"], 0.001),
                        metric("grad", "gradian (1/400 turn)", &["gon", "gradian", "gradians"], std::f64::consts::PI / 200.0),
                        imperial("deg", "degree (1/360 turn)", &["°", "degree", "degrees"], std::f64::consts::PI / 180.0),
                        imperial("arcmin", "minute of arc (1/60°)", &["′", "arcminute", "arcminutes"], std::f64::consts::PI / 10_800.0),
                        imperial("arcsec", "second of arc (1/3600°)", &["″", "arcsecond", "arcseconds"], std::f64::consts::PI / 648_000.0),
                        imperial("turn", "turn (full circle)", &["turns", "revolution", "revolutions", "rev"], std::f64::consts::TAU),
                    ],
                },
                Category {
                    name: "Mass",
                    base: "kg",
                    dims: [0, 1, 0, 0, 0],
                    si: 1.0,
                    reciprocal: false,
                    case_sensitive: false,
                    systems: METRIC_IMPERIAL,
                    units: vec![
                        metric("mg", "milligram", &["milligram", "milligrams"], 0.000_001),
                        metric("g", "gram", &["gram", "grams"], 0.001),
                        metric("kg", "kilogram", &["kilogram", "kilograms"], 1.0),
                        metric("t", "tonne (metric ton)", &["tonne", "tonnes", "metric-ton"], 1000.0),
                        imperial("oz", "ounce (avoirdupois)", &["ounce", "ounces"], 0.028_349_523_125),
                        imperial("lb", "pound (avoirdupois)", &["lbs", "pound", "pounds"], 0.453_592_37),
                        imperial("st", "stone (14 lb)", &["stone", "stones"], 6.350_293_18),
                    ],
                },
                Category {
                    name: "Temperature",
                    base: "K",
                    dims: [0, 0, 0, 1, 0],
                    si: 1.0,
                    reciprocal: false,
                    case_sensitive: false,
                    systems: METRIC_IMPERIAL,
                    units: vec![
                        metric("K", "kelvin", &["kelvin"], 1.0),
                        Unit {
                            symbol: "C",
                            name: "degree Celsius",
                            aliases: &["°C", "celsius"],
                            factor: 1.0,
                            offset: 273.15,
                            inverse: false,
                            data: None,
                            system: System::Metric,
                        },
                        Unit {
                            symbol: "F",
                            name: "degree Fahrenheit",
                            aliases: &["°F", "fahrenheit"],
                            factor: 5.0 / 9.0,
                            offset: 459.67 * 5.0 / 9.0,
                            inverse: false,
                            data: None,
                            system: System::Imperial,
                        },
                    ],
                },
                Category {
                    name: "Speed",
                    base: "m/s",
                    dims: [1, 0, -1, 0, 0],
                    si: 1.0,
                    reciprocal: false,
                    case_sensitive: false,
                    systems: METRIC_IMPERIAL,
                    units: vec![
                        metric("m/s", "metre per second", &["mps", "metres-per-second", "meters-per-second"], 1.0),
                        metric("km/h", "kilometre per hour", &["kph", "kmh", "km/hr", "kilometres-per-hour", "kilometers-per-hour"], 1000.0 / 3600.0),
                        imperial("mph", "mile per hour", &["mi/h", "miles-per-hour"], 1609.344 / 3600.0),
                        imperial("ft/s", "foot per second", &["fps", "feet-per-second"], 0.3048),
                        imperial("kn", "knot (nautical mile per hour)", &["kt", "knot", "knots"], 1852.0 / 3600.0),
                    ],
                },
                Category {
                    name: "Pressure",
                    base: "Pa",
                    dims: [-1, 1, -2, 0, 0],
                    si: 1.0,
                    reciprocal: false,
                    case_sensitive: false,
                    systems: METRIC_IMPERIAL,
                    units: vec![
                        metric("Pa", "pascal", &["pascal", "pascals"], 1.0),
                        metric("hPa", "hectopascal", &["hectopascal", "hectopascals"], 100.0),
                        metric("kPa", "kilopascal", &["kilopascal", "kilopascals"], 1000.0),
                        metric("MPa", "megapascal", &["megapascal", "megapascals"], 1e6),
                        metric("mbar", "millibar", &["millibar", "millibars"], 100.0),
                        metric("bar", "bar (100 kPa)", &["bars"], 100_000.0),
                        metric("atm", "standard atmosphere", &["atmosphere", "atmospheres"], 101_325.0),
                        metric("mmHg", "millimetre of mercury", &["millimetres-of-mercury", "millimeters-of-mercury"], 133.322_387_415),
                        metric("Torr", "torr (1/760 atm)", &["torr"], 101_325.0 / 760.0),
                        imperial("psi", "pound-force per square inch", &["lbf/in2"], 6_894.757_293_168_361),
                        imperial("inHg", "inch of mercury", &["inches-of-mercury"], 3_386.389),
                    ],
                },
                Category {
                    name: "Fuel economy",
                    base: "km/l",
                    // Length per volume: 1 km/l = 1000 m / 0.001 m³.
                    dims: [-2, 0, 0, 0, 0],
                    si: 1e6,
                    reciprocal: true,
                    case_sensitive: false,
                    systems: METRIC_IMPERIAL,
                    units: vec![
                        inverse("l/100km", "litres per 100 kilometres", &["lp100km", "litres-per-100km", "liters-per-100km"], 100.0),
                        metric("km/l", "kilometres per litre", &["kmpl", "kilometres-per-litre", "kilometers-per-liter"], 1.0),
                        imperial("mpg", "miles per US gallon", &["mpg-us", "mi/gal"], 1.609_344 / 3.785_411_784),
                        imperial("mpg-uk", "miles per imperial (UK) gallon", &["mpg-imp", "mi/gal-uk"], 1.609_344 / 4.546_09),
                    ],
                },
                Category {
                    name: "Energy",
                    base: "J",
                    dims: [2, 1, -2, 0, 0],
                    si: 1.0,
                    reciprocal: false,
                    case_sensitive: false,
                    systems: METRIC_IMPERIAL,
                    units: vec![
                        metric("J", "joule", &["joule", "joules"], 1.0),
                        metric("kJ", "kilojoule", &["kilojoule", "kilojoules"], 1e3),
                        metric("MJ", "megajoule", &["megajoule", "megajoules"], 1e6),
                        metric("GJ", "gigajoule", &["gigajoule", "gigajoules"], 1e9),
                        metric("Wh", "watt-hour", &["watt-hour", "watt-hours"], 3600.0),
                        metric("kWh", "kilowatt-hour", &["kilowatt-hour", "kilowatt-hours"], 3.6e6),
                        metric("MWh", "megawatt-hour", &["megawatt-hour", "megawatt-hours"], 3.6e9),
                        metric("cal", "calorie (thermochemical, 4.184 J)", &["calorie", "calories"], 4.184),
                        metric("kcal", "kilocalorie (food calorie)", &["kilocalorie", "kilocalories"], 4184.0),
                        metric("eV", "electronvolt", &["electronvolt", "electronvolts"], 1.602_176_634e-19),
                        imperial("BTU", "British thermal unit (IT)", &["btu", "btus"], 1_055.055_852_62),
                        imperial("therm", "therm (US, 100000 BTU)", &["therms"], 105_480_400.0),
                        imperial("ft-lbf", "foot-pound force", &["ft*lbf", "foot-pound", "foot-pounds"], 1.355_817_948_331_400_4),
                    ],
                },
                Category {
                    name: "Power",
                    base: "W",
                    dims: [2, 1, -3, 0, 0],
                    si: 1.0,
                    reciprocal: false,
                    case_sensitive: false,
                    systems: METRIC_IMPERIAL,
                    units: vec![
                        metric("W", "watt", &["watt", "watts"], 1.0),
                        metric("kW", "kilowatt", &["kilowatt", "kilowatts"], 1e3),
                        metric("MW", "megawatt", &["megawatt", "megawatts"], 1e6),
                        metric("GW", "gigawatt", &["gigawatt", "gigawatts"], 1e9),
                        metric("PS", "metric horsepower (Pferdestärke)", &["cv", "metric-horsepower"], 735.498_75),
                        imperial("hp", "horsepower (mechanical)", &["horsepower", "bhp"], 745.699_871_582_270_2),
                        imperial("BTU/h", "British thermal unit per hour", &["btu/h", "btu/hr"], 1_055.055_852_62 / 3600.0),
                    ],
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_unit_has_a_system() {
        let config = Config::default();
        let systems = |name: &str| -> Vec<(&str, System)> {
            let cat = config.categories.iter().find(|c| c.name == name).unwrap();
            cat.units.iter().map(|u| (u.symbol, u.system)).collect()
        };
        use System::*;
        assert_eq!(
            systems("Length"),
            [
                ("mm", Metric),
                ("cm", Metric),
                ("m", Metric),
                ("km", Metric),
                ("au", Metric),
                ("ly", Metric),
                ("pc", Metric),
                ("in", Imperial),
                ("ft", Imperial),
                ("yd", Imperial),
                ("mi", Imperial)
            ]
        );
        assert_eq!(systems("Temperature"), [("K", Metric), ("C", Metric), ("F", Imperial)]);
        for (symbol, system) in systems(BITS) {
            let want = if symbol.contains('i') {
                Imperial
            } else if symbol.len() == 1 {
                Both
            } else {
                Metric
            };
            assert_eq!(system, want, "{symbol}");
        }
        for cat in &config.categories {
            // Both tabs have units.
            for tab in [Metric, Imperial] {
                // Time's second tab holds the date formats, not units.
                let formats_tab = cat.name == TIME && tab == Imperial;
                assert!(formats_tab || cat.units.iter().any(|u| u.system == tab), "{} has no {tab:?} unit", cat.name);
            }
            assert_eq!(cat.case_sensitive, cat.systems == ["Decimal", "Binary"], "{}", cat.name);
        }
    }

    #[test]
    fn time_units() {
        let config = Config::default();
        let time = config.categories.iter().find(|c| c.name == TIME).unwrap();
        let symbols: Vec<_> = time.units.iter().map(|u| (u.symbol, u.system)).collect();
        use System::*;
        // All the durations on the first tab; the second lists the date formats (in the TUI).
        assert!(symbols.iter().all(|(_, s)| *s == Metric));
        assert_eq!(symbols.iter().map(|(s, _)| *s).collect::<Vec<_>>(), ["ns", "us", "ms", "s", "min", "h", "d", "wk", "mo", "yr"]);
        assert_eq!(time.systems, ["Durations", "Formats"]);
        let yr = time.units.iter().find(|u| u.symbol == "yr").unwrap();
        assert_eq!(yr.factor, 365.25 * 86_400.0);
        let mo = time.units.iter().find(|u| u.symbol == "mo").unwrap();
        assert_eq!(mo.factor * 12.0, yr.factor);
        // Date words and the number words never collide with unit names.
        for word in DATE_WORDS.iter().chain(NUMBER_WORDS) {
            assert!(config.categories.iter().flat_map(|c| &c.units).all(|u| u.symbol != *word && !u.aliases.contains(word)), "{word}");
        }
    }

    #[test]
    fn every_unit_has_a_full_name() {
        let config = Config::default();
        let mut seen = Vec::new();
        for u in config.categories.iter().flat_map(|c| &c.units) {
            assert!(!u.name.is_empty() && u.name != u.symbol, "{}", u.symbol);
            assert!(!seen.contains(&u.name), "name `{}` used twice", u.name);
            seen.push(u.name);
        }
    }

    #[test]
    fn no_name_collisions() {
        let config = Config::default();
        let mut seen: Vec<(String, &str)> = Vec::new();
        for cat in &config.categories {
            for u in &cat.units {
                for name in std::iter::once(&u.symbol).chain(u.aliases.iter()) {
                    // Case-insensitive categories must also be unique ignoring case.
                    let key = if cat.case_sensitive { name.to_string() } else { name.to_lowercase() };
                    if let Some((_, owner)) = seen.iter().find(|(k, _)| *k == key) {
                        assert_eq!(*owner, u.symbol, "name `{name}` is used by both `{owner}` and `{}`", u.symbol);
                    }
                    seen.push((key, u.symbol));
                }
            }
        }
    }

    /// Values can be expressions (`100+20sqft`), so no unit name may start with something an
    /// expression could consume: digits, `.`, operators, brackets, or `x` (multiply).
    #[test]
    fn names_do_not_clash_with_expressions() {
        let config = Config::default();
        for u in config.categories.iter().flat_map(|c| &c.units) {
            for name in std::iter::once(&u.symbol).chain(u.aliases.iter()) {
                let first = name.chars().next().unwrap();
                assert!(!first.is_ascii_digit() && !".+-*/^()[]{}xX".contains(first), "unit name `{name}`");
            }
        }
    }
}
