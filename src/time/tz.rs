//! Named time zones, without crates: the system's zone database (TZif files under
//! `/usr/share/zoneinfo`, or `$TZDIR`; RFC 8536), with the POSIX TZ rule in each file's footer for
//! times after its last transition. And the cities that name them: every country's capital and
//! major cities (`Paris`, `New York`, `Tokyo`), or a zone's own name (`Europe/Paris`).

use super::{civil_from_days, days_from_civil, days_in_month, unaccented, weekday};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// A time zone from the database: its offsets over time.
#[derive(Debug)]
pub struct Tz {
    /// The zone's name: `Europe/Paris`.
    pub name: &'static str,
    /// Transition times (seconds since the epoch, UTC), ascending, and the offset (seconds east of
    /// UTC) from each one on.
    transitions: Vec<(i64, i32)>,
    /// The offset before the first transition.
    initial: i32,
    /// The rule after the last transition (the footer).
    rule: Option<Rule>,
}

impl PartialEq for Tz {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Tz {
    /// The offset east of UTC at `t` (seconds since the epoch, UTC).
    pub fn offset_at(&self, t: i64) -> i32 {
        let after = self.transitions.partition_point(|&(at, _)| at <= t);
        match (after, &self.rule) {
            (0, Some(rule)) if self.transitions.is_empty() => rule.offset_at(t),
            (0, _) => self.initial,
            (n, Some(rule)) if n == self.transitions.len() => rule.offset_at(t),
            (n, _) => self.transitions[n - 1].1,
        }
    }

    /// The UTC time of a wall-clock time here (seconds since the epoch, both), as JavaScript
    /// Temporal's `compatible`: when the clocks go back (an overlap), the earlier reading; when they
    /// go forward (a gap), the time moves on by the gap (02:30 is 03:30), as the C library does.
    pub fn to_utc(&self, wall: i64) -> i64 {
        // The offsets a day before and after: around a change, each is one side of it.
        let (before, after) = (i64::from(self.offset_at(wall - 86_400)), i64::from(self.offset_at(wall + 86_400)));
        let reads = |offset: i64| {
            let t = wall - offset;
            (t + i64::from(self.offset_at(t)) == wall).then_some(t)
        };
        match (reads(before), reads(after)) {
            (Some(a), Some(b)) => a.min(b),
            (Some(t), None) | (None, Some(t)) => t,
            // A gap: read with the offset before it, which lands after it.
            (None, None) => wall - before,
        }
    }

    /// Parses a TZif file.
    fn parse(name: &'static str, data: &[u8]) -> Option<Tz> {
        let header = |at: usize| -> Option<(u8, [usize; 6])> {
            let h = data.get(at..at + 44)?;
            if &h[..4] != b"TZif" {
                return None;
            }
            let count = |i: usize| u32::from_be_bytes(h[20 + 4 * i..24 + 4 * i].try_into().unwrap()) as usize;
            Some((h[4], [count(0), count(1), count(2), count(3), count(4), count(5)]))
        };
        let (version, counts) = header(0)?;
        let block = |time_size: usize, [isut, isstd, leap, time, types, chars]: [usize; 6]| {
            time * time_size + time + types * 6 + chars + leap * (time_size + 4) + isstd + isut
        };
        // Version 2 and later repeat the data with 64-bit times, then the footer rule.
        let (start, time_size, counts) = if version >= b'2' {
            let second = 44 + block(4, counts);
            (second + 44, 8, header(second)?.1)
        } else {
            (44, 4, counts)
        };
        let [_, _, _, time_count, type_count, _] = counts;
        let body = data.get(start..start + block(time_size, counts))?;
        let times: Vec<i64> = (0..time_count)
            .map(|i| {
                let b = &body[i * time_size..(i + 1) * time_size];
                if time_size == 8 {
                    i64::from_be_bytes(b.try_into().unwrap())
                } else {
                    i64::from(i32::from_be_bytes(b.try_into().unwrap()))
                }
            })
            .collect();
        let indices = &body[time_count * time_size..time_count * (time_size + 1)];
        let types_at = time_count * (time_size + 1);
        let offset_of = |t: usize| -> Option<i32> {
            let b = body.get(types_at + t * 6..types_at + t * 6 + 4)?;
            Some(i32::from_be_bytes(b.try_into().unwrap()))
        };
        if type_count == 0 {
            return None;
        }
        let transitions = times.iter().zip(indices).map(|(&t, &i)| Some((t, offset_of(usize::from(i))?))).collect::<Option<Vec<_>>>()?;
        let rule = (version >= b'2')
            .then(|| {
                let footer = &data[start + block(time_size, counts)..];
                let text = std::str::from_utf8(footer).ok()?.trim_matches('\n');
                Rule::parse(text)
            })
            .flatten();
        Some(Tz { name, transitions, initial: offset_of(0)?, rule })
    }
}

// ---------------------------------------------------------------- POSIX TZ rules

/// A POSIX TZ rule: `CET-1CEST,M3.5.0,M10.5.0/3`.
#[derive(Debug, PartialEq)]
struct Rule {
    /// Standard time, seconds east of UTC.
    std: i32,
    /// Daylight saving time: its offset and when it starts and ends (each in the local time in
    /// force before it).
    dst: Option<(i32, Change, Change)>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
struct Change {
    day: RuleDay,
    /// Seconds after local midnight (may be negative or over a day).
    time: i64,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum RuleDay {
    /// `Mm.w.d`: weekday `d` (0 = Sunday) of week `w` (5 = the last) of month `m`.
    Weekday(u32, u32, u32),
    /// `Jn`: day 1–365, February 29 never counted.
    Julian(u32),
    /// `n`: day 0–365, February 29 counted.
    Ordinal(u32),
}

impl Rule {
    fn parse(text: &str) -> Option<Rule> {
        let mut s = text.as_bytes();
        let name = |s: &mut &[u8]| -> Option<()> {
            if s.first() == Some(&b'<') {
                let end = s.iter().position(|&b| b == b'>')?;
                *s = &s[end + 1..];
            } else {
                let n = s.iter().take_while(|b| b.is_ascii_alphabetic()).count();
                if n < 3 {
                    return None;
                }
                *s = &s[n..];
            }
            Some(())
        };
        // `[+-]hh[:mm[:ss]]`, in seconds.
        let clock = |s: &mut &[u8]| -> Option<i64> {
            let sign = match s.first() {
                Some(b'-') => {
                    *s = &s[1..];
                    -1
                }
                Some(b'+') => {
                    *s = &s[1..];
                    1
                }
                _ => 1,
            };
            let mut total = 0i64;
            for (i, unit) in [3600, 60, 1].into_iter().enumerate() {
                if i > 0 {
                    if s.first() != Some(&b':') {
                        break;
                    }
                    *s = &s[1..];
                }
                let n = s.iter().take_while(|b| b.is_ascii_digit()).count();
                if n == 0 {
                    return None;
                }
                total += std::str::from_utf8(&s[..n]).ok()?.parse::<i64>().ok()? * unit;
                *s = &s[n..];
            }
            Some(sign * total)
        };
        let number = |s: &mut &[u8]| -> Option<u32> {
            let n = s.iter().take_while(|b| b.is_ascii_digit()).count();
            let v = std::str::from_utf8(&s[..n]).ok()?.parse().ok()?;
            *s = &s[n..];
            Some(v)
        };
        let change = |s: &mut &[u8]| -> Option<Change> {
            let day = match s.first()? {
                b'M' => {
                    *s = &s[1..];
                    let m = number(s)?;
                    *s = s.strip_prefix(b".")?;
                    let w = number(s)?;
                    *s = s.strip_prefix(b".")?;
                    let d = number(s)?;
                    ((1..=12).contains(&m) && (1..=5).contains(&w) && d <= 6).then_some(RuleDay::Weekday(m, w, d))?
                }
                b'J' => {
                    *s = &s[1..];
                    RuleDay::Julian(number(s).filter(|n| (1..=365).contains(n))?)
                }
                _ => RuleDay::Ordinal(number(s).filter(|&n| n <= 365)?),
            };
            let time = if s.first() == Some(&b'/') {
                *s = &s[1..];
                clock(s)?
            } else {
                7200
            };
            Some(Change { day, time })
        };
        name(&mut s)?;
        // POSIX offsets are west of UTC: `CET-1` is +01:00.
        let std = -clock(&mut s)? as i32;
        if s.is_empty() {
            return Some(Rule { std, dst: None });
        }
        name(&mut s)?;
        let dst = if s.first().is_some_and(|&b| b != b',') { -clock(&mut s)? as i32 } else { std + 3600 };
        // No rule: the US one (the POSIX default of many systems).
        let (start, end) = if s.is_empty() {
            let us = |m, w| Change { day: RuleDay::Weekday(m, w, 0), time: 7200 };
            (us(3, 2), us(11, 1))
        } else {
            s = s.strip_prefix(b",")?;
            let start = change(&mut s)?;
            s = s.strip_prefix(b",")?;
            (start, change(&mut s)?)
        };
        s.is_empty().then_some(Rule { std, dst: Some((dst, start, end)) })
    }

    fn offset_at(&self, t: i64) -> i32 {
        let Some((dst, start, end)) = self.dst else { return self.std };
        let local_days = (t + i64::from(self.std)).div_euclid(86_400);
        let (year, _, _) = civil_from_days(local_days);
        // The changes of that year, in UTC: the start is in standard time, the end in DST.
        let start = day_of(year, start.day) * 86_400 + start.time - i64::from(self.std);
        let end = day_of(year, end.day) * 86_400 + end.time - i64::from(dst);
        let in_dst = if start < end { start <= t && t < end } else { !(end <= t && t < start) };
        if in_dst {
            dst
        } else {
            self.std
        }
    }
}

/// The day (since the epoch) a rule names in `year`.
fn day_of(year: i64, day: RuleDay) -> i64 {
    let jan1 = days_from_civil(year, 1, 1);
    match day {
        RuleDay::Weekday(m, w, d) => {
            let first = days_from_civil(year, m, 1);
            // `weekday`: Monday = 1 … Sunday = 7; POSIX: Sunday = 0.
            let first_wd = i64::from(weekday(first) % 7);
            let mut day = first + (i64::from(d) - first_wd).rem_euclid(7) + 7 * (i64::from(w) - 1);
            while day >= first + i64::from(days_in_month(year, m)) {
                day -= 7;
            }
            day
        }
        RuleDay::Julian(n) => {
            let leap = days_in_month(year, 2) == 29;
            jan1 + i64::from(n) - 1 + i64::from(leap && n >= 60)
        }
        RuleDay::Ordinal(n) => jan1 + i64::from(n),
    }
}

// ---------------------------------------------------------------- loading

static APP_ZONEINFO: OnceLock<String> = OnceLock::new();

/// The zone database given by the app embedding tally: a folder of zone files, or Android's
/// single file (`/apex/com.android.tzdata/etc/tz/tzdata`); set once, before any zone is loaded.
#[cfg_attr(not(feature = "ffi"), allow(dead_code))]
pub fn set_zoneinfo(path: &str) {
    let _ = APP_ZONEINFO.set(path.to_string());
}

/// The zone database: the app's, else `$TZDIR`, else `/usr/share/zoneinfo`.
fn zoneinfo() -> String {
    if let Some(path) = APP_ZONEINFO.get() {
        return path.clone();
    }
    std::env::var("TZDIR").ok().filter(|d| !d.is_empty()).unwrap_or_else(|| "/usr/share/zoneinfo".into())
}

/// A zone file from Android's `tzdata`: every zone in one file. A 12-byte version
/// (`tzdata2026c\0`), the offsets of the index and of the data (big endian), then the index: 52
/// bytes per zone (its name, NUL-padded to 40 bytes, its offset in the data, its length, 4 unused).
fn from_bundle(bundle: &[u8], name: &str) -> Option<Vec<u8>> {
    let int = |at: usize| -> Option<usize> { Some(u32::from_be_bytes(bundle.get(at..at + 4)?.try_into().ok()?) as usize) };
    if !bundle.starts_with(b"tzdata") {
        return None;
    }
    let (index, data) = (int(12)?, int(16)?);
    bundle.get(index..data)?.as_chunks::<52>().0.iter().find_map(|entry| {
        let end = entry[..40].iter().position(|&b| b == 0).unwrap_or(40);
        if &entry[..end] != name.as_bytes() {
            return None;
        }
        let start = data + u32::from_be_bytes(entry[40..44].try_into().ok()?) as usize;
        let len = u32::from_be_bytes(entry[44..48].try_into().ok()?) as usize;
        Some(bundle.get(start..start + len)?.to_vec())
    })
}

/// A zone's TZif data: `<folder>/<name>`, or its entry in a single-file database (read once).
fn zone_data(database: &str, name: &str) -> Option<Vec<u8>> {
    if !std::path::Path::new(database).is_file() {
        return std::fs::read(format!("{database}/{name}")).ok();
    }
    static BUNDLE: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    from_bundle(BUNDLE.get_or_init(|| std::fs::read(database).ok()).as_deref()?, name)
}

/// A zone of the database by its name (`Europe/Paris`), loaded once. `Err`: not installed.
pub fn load(name: &str) -> Result<&'static Tz, String> {
    static CACHE: OnceLock<Mutex<HashMap<String, &'static Tz>>> = OnceLock::new();
    let valid = !name.is_empty()
        && !name.starts_with('/')
        && name.split('/').all(|part| !part.is_empty() && part != ".." && part != ".")
        && name.chars().all(|c| c.is_ascii_alphanumeric() || "/_-+".contains(c));
    if !valid {
        return Err(format!("`{name}` is not a time zone name"));
    }
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(tz) = cache.get(name) {
        return Ok(tz);
    }
    let database = zoneinfo();
    let path = format!("{database}/{name}");
    let data = zone_data(&database, name).ok_or_else(|| format!("the time zone {name} is not installed ({path})"))?;
    let static_name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let tz = Tz::parse(static_name, &data).ok_or_else(|| format!("{path} is not a time zone file"))?;
    let tz: &'static Tz = Box::leak(Box::new(tz));
    cache.insert(name.to_string(), tz);
    Ok(tz)
}

// ---------------------------------------------------------------- cities

/// A city and its time zone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct City {
    /// As shown: `New York`, `São Paulo`.
    pub name: &'static str,
    pub tz: &'static Tz,
}

/// Every country's capital, and major cities: (name, zone). Zones with an older name too: `A|B`,
/// tried in order.
const CITIES: &[(&str, &str)] = &[
    // Europe
    ("Amsterdam", "Europe/Amsterdam"),
    ("Andorra la Vella", "Europe/Andorra"),
    ("Athens", "Europe/Athens"),
    ("Belgrade", "Europe/Belgrade"),
    ("Berlin", "Europe/Berlin"),
    ("Bern", "Europe/Zurich"),
    ("Bratislava", "Europe/Bratislava"),
    ("Brussels", "Europe/Brussels"),
    ("Bucharest", "Europe/Bucharest"),
    ("Budapest", "Europe/Budapest"),
    ("Chisinau", "Europe/Chisinau"),
    ("Copenhagen", "Europe/Copenhagen"),
    ("Dublin", "Europe/Dublin"),
    ("Helsinki", "Europe/Helsinki"),
    ("Kyiv", "Europe/Kyiv|Europe/Kiev"),
    ("Kiev", "Europe/Kyiv|Europe/Kiev"),
    ("Lisbon", "Europe/Lisbon"),
    ("Ljubljana", "Europe/Ljubljana"),
    ("London", "Europe/London"),
    ("Luxembourg", "Europe/Luxembourg"),
    ("Madrid", "Europe/Madrid"),
    ("Minsk", "Europe/Minsk"),
    ("Monaco", "Europe/Monaco"),
    ("Moscow", "Europe/Moscow"),
    ("Nicosia", "Asia/Nicosia"),
    ("Oslo", "Europe/Oslo"),
    ("Paris", "Europe/Paris"),
    ("Podgorica", "Europe/Podgorica"),
    ("Prague", "Europe/Prague"),
    ("Pristina", "Europe/Belgrade"),
    ("Reykjavik", "Atlantic/Reykjavik"),
    ("Riga", "Europe/Riga"),
    ("Rome", "Europe/Rome"),
    ("San Marino", "Europe/San_Marino"),
    ("Sarajevo", "Europe/Sarajevo"),
    ("Skopje", "Europe/Skopje"),
    ("Sofia", "Europe/Sofia"),
    ("Stockholm", "Europe/Stockholm"),
    ("Tallinn", "Europe/Tallinn"),
    ("Tirana", "Europe/Tirane"),
    ("Vaduz", "Europe/Vaduz"),
    ("Valletta", "Europe/Malta"),
    ("Vatican City", "Europe/Vatican"),
    ("Vienna", "Europe/Vienna"),
    ("Vilnius", "Europe/Vilnius"),
    ("Warsaw", "Europe/Warsaw"),
    ("Zagreb", "Europe/Zagreb"),
    ("Ankara", "Europe/Istanbul"),
    ("Tbilisi", "Asia/Tbilisi"),
    ("Yerevan", "Asia/Yerevan"),
    ("Baku", "Asia/Baku"),
    // Asia
    ("Abu Dhabi", "Asia/Dubai"),
    ("Amman", "Asia/Amman"),
    ("Ashgabat", "Asia/Ashgabat"),
    ("Astana", "Asia/Almaty"),
    ("Baghdad", "Asia/Baghdad"),
    ("Bandar Seri Begawan", "Asia/Brunei"),
    ("Bangkok", "Asia/Bangkok"),
    ("Beijing", "Asia/Shanghai"),
    ("Beirut", "Asia/Beirut"),
    ("Bishkek", "Asia/Bishkek"),
    ("Colombo", "Asia/Colombo"),
    ("Sri Jayawardenepura Kotte", "Asia/Colombo"),
    ("Damascus", "Asia/Damascus"),
    ("Dhaka", "Asia/Dhaka"),
    ("Dili", "Asia/Dili"),
    ("Doha", "Asia/Qatar"),
    ("Dushanbe", "Asia/Dushanbe"),
    ("Hanoi", "Asia/Ho_Chi_Minh|Asia/Saigon"),
    ("Islamabad", "Asia/Karachi"),
    ("Jakarta", "Asia/Jakarta"),
    ("Jerusalem", "Asia/Jerusalem"),
    ("Kabul", "Asia/Kabul"),
    ("Kathmandu", "Asia/Kathmandu|Asia/Katmandu"),
    ("Kuala Lumpur", "Asia/Kuala_Lumpur"),
    ("Kuwait City", "Asia/Kuwait"),
    ("Male", "Indian/Maldives"),
    ("Manama", "Asia/Bahrain"),
    ("Manila", "Asia/Manila"),
    ("Muscat", "Asia/Muscat"),
    ("Naypyidaw", "Asia/Yangon|Asia/Rangoon"),
    ("New Delhi", "Asia/Kolkata|Asia/Calcutta"),
    ("Phnom Penh", "Asia/Phnom_Penh"),
    ("Pyongyang", "Asia/Pyongyang"),
    ("Riyadh", "Asia/Riyadh"),
    ("Sanaa", "Asia/Aden"),
    ("Seoul", "Asia/Seoul"),
    ("Singapore", "Asia/Singapore"),
    ("Taipei", "Asia/Taipei"),
    ("Tashkent", "Asia/Tashkent"),
    ("Tehran", "Asia/Tehran"),
    ("Thimphu", "Asia/Thimphu"),
    ("Tokyo", "Asia/Tokyo"),
    ("Ulaanbaatar", "Asia/Ulaanbaatar"),
    ("Vientiane", "Asia/Vientiane"),
    // Africa
    ("Abuja", "Africa/Lagos"),
    ("Accra", "Africa/Accra"),
    ("Addis Ababa", "Africa/Addis_Ababa"),
    ("Algiers", "Africa/Algiers"),
    ("Antananarivo", "Indian/Antananarivo"),
    ("Asmara", "Africa/Asmara"),
    ("Bamako", "Africa/Bamako"),
    ("Bangui", "Africa/Bangui"),
    ("Banjul", "Africa/Banjul"),
    ("Bissau", "Africa/Bissau"),
    ("Brazzaville", "Africa/Brazzaville"),
    ("Cairo", "Africa/Cairo"),
    ("Conakry", "Africa/Conakry"),
    ("Dakar", "Africa/Dakar"),
    ("Djibouti", "Africa/Djibouti"),
    ("Dodoma", "Africa/Dar_es_Salaam"),
    ("Freetown", "Africa/Freetown"),
    ("Gaborone", "Africa/Gaborone"),
    ("Gitega", "Africa/Bujumbura"),
    ("Harare", "Africa/Harare"),
    ("Juba", "Africa/Juba"),
    ("Kampala", "Africa/Kampala"),
    ("Khartoum", "Africa/Khartoum"),
    ("Kigali", "Africa/Kigali"),
    ("Kinshasa", "Africa/Kinshasa"),
    ("Libreville", "Africa/Libreville"),
    ("Lilongwe", "Africa/Blantyre"),
    ("Lome", "Africa/Lome"),
    ("Luanda", "Africa/Luanda"),
    ("Lusaka", "Africa/Lusaka"),
    ("Malabo", "Africa/Malabo"),
    ("Maputo", "Africa/Maputo"),
    ("Maseru", "Africa/Maseru"),
    ("Mbabane", "Africa/Mbabane"),
    ("Mogadishu", "Africa/Mogadishu"),
    ("Monrovia", "Africa/Monrovia"),
    ("Moroni", "Indian/Comoro"),
    ("Nairobi", "Africa/Nairobi"),
    ("N'Djamena", "Africa/Ndjamena"),
    ("Niamey", "Africa/Niamey"),
    ("Nouakchott", "Africa/Nouakchott"),
    ("Ouagadougou", "Africa/Ouagadougou"),
    ("Port Louis", "Indian/Mauritius"),
    ("Porto-Novo", "Africa/Porto-Novo"),
    ("Praia", "Atlantic/Cape_Verde"),
    ("Pretoria", "Africa/Johannesburg"),
    ("Rabat", "Africa/Casablanca"),
    ("São Tomé", "Africa/Sao_Tome"),
    ("Tripoli", "Africa/Tripoli"),
    ("Tunis", "Africa/Tunis"),
    ("Victoria", "Indian/Mahe"),
    ("Windhoek", "Africa/Windhoek"),
    ("Yamoussoukro", "Africa/Abidjan"),
    ("Yaoundé", "Africa/Douala"),
    // Americas
    ("Washington", "America/New_York"),
    ("Ottawa", "America/Toronto"),
    ("Mexico City", "America/Mexico_City"),
    ("Guatemala City", "America/Guatemala"),
    ("Belmopan", "America/Belize"),
    ("San Salvador", "America/El_Salvador"),
    ("Tegucigalpa", "America/Tegucigalpa"),
    ("Managua", "America/Managua"),
    ("San José", "America/Costa_Rica"),
    ("Panama City", "America/Panama"),
    ("Havana", "America/Havana"),
    ("Kingston", "America/Jamaica"),
    ("Port-au-Prince", "America/Port-au-Prince"),
    ("Santo Domingo", "America/Santo_Domingo"),
    ("Nassau", "America/Nassau"),
    ("Bridgetown", "America/Barbados"),
    ("Port of Spain", "America/Port_of_Spain"),
    ("Castries", "America/St_Lucia"),
    ("Kingstown", "America/St_Vincent"),
    ("Saint George's", "America/Grenada"),
    ("Roseau", "America/Dominica"),
    ("Saint John's", "America/Antigua"),
    ("Basseterre", "America/St_Kitts"),
    ("Bogotá", "America/Bogota"),
    ("Caracas", "America/Caracas"),
    ("Quito", "America/Guayaquil"),
    ("Lima", "America/Lima"),
    ("La Paz", "America/La_Paz"),
    ("Sucre", "America/La_Paz"),
    ("Santiago", "America/Santiago"),
    ("Buenos Aires", "America/Argentina/Buenos_Aires"),
    ("Montevideo", "America/Montevideo"),
    ("Asunción", "America/Asuncion"),
    ("Brasília", "America/Sao_Paulo"),
    ("Georgetown", "America/Guyana"),
    ("Paramaribo", "America/Paramaribo"),
    // Oceania
    ("Canberra", "Australia/Sydney"),
    ("Wellington", "Pacific/Auckland"),
    ("Port Moresby", "Pacific/Port_Moresby"),
    ("Suva", "Pacific/Fiji"),
    ("Honiara", "Pacific/Guadalcanal"),
    ("Port Vila", "Pacific/Efate"),
    ("Apia", "Pacific/Apia"),
    ("Nuku'alofa", "Pacific/Tongatapu"),
    ("Tarawa", "Pacific/Tarawa"),
    ("Majuro", "Pacific/Majuro"),
    ("Palikir", "Pacific/Pohnpei|Pacific/Ponape"),
    ("Ngerulmud", "Pacific/Palau"),
    ("Funafuti", "Pacific/Funafuti"),
    ("Yaren", "Pacific/Nauru"),
    // Major cities
    ("New York", "America/New_York"),
    ("Boston", "America/New_York"),
    ("Miami", "America/New_York"),
    ("Atlanta", "America/New_York"),
    ("Chicago", "America/Chicago"),
    ("Houston", "America/Chicago"),
    ("Dallas", "America/Chicago"),
    ("Denver", "America/Denver"),
    ("Phoenix", "America/Phoenix"),
    ("Los Angeles", "America/Los_Angeles"),
    ("San Francisco", "America/Los_Angeles"),
    ("Seattle", "America/Los_Angeles"),
    ("Las Vegas", "America/Los_Angeles"),
    ("Anchorage", "America/Anchorage"),
    ("Honolulu", "Pacific/Honolulu"),
    ("Toronto", "America/Toronto"),
    ("Montreal", "America/Toronto"),
    ("Vancouver", "America/Vancouver"),
    ("São Paulo", "America/Sao_Paulo"),
    ("Rio de Janeiro", "America/Sao_Paulo"),
    ("Sydney", "Australia/Sydney"),
    ("Melbourne", "Australia/Melbourne"),
    ("Brisbane", "Australia/Brisbane"),
    ("Perth", "Australia/Perth"),
    ("Adelaide", "Australia/Adelaide"),
    ("Auckland", "Pacific/Auckland"),
    ("Shanghai", "Asia/Shanghai"),
    ("Hong Kong", "Asia/Hong_Kong"),
    ("Mumbai", "Asia/Kolkata|Asia/Calcutta"),
    ("Kolkata", "Asia/Kolkata|Asia/Calcutta"),
    ("Bangalore", "Asia/Kolkata|Asia/Calcutta"),
    ("Karachi", "Asia/Karachi"),
    ("Dubai", "Asia/Dubai"),
    ("Istanbul", "Europe/Istanbul"),
    ("Osaka", "Asia/Tokyo"),
    ("Ho Chi Minh City", "Asia/Ho_Chi_Minh|Asia/Saigon"),
    ("Tel Aviv", "Asia/Jerusalem"),
    ("Lagos", "Africa/Lagos"),
    ("Johannesburg", "Africa/Johannesburg"),
    ("Cape Town", "Africa/Johannesburg"),
    ("Casablanca", "Africa/Casablanca"),
    ("Milan", "Europe/Rome"),
    ("Barcelona", "Europe/Madrid"),
    ("Munich", "Europe/Berlin"),
    ("Frankfurt", "Europe/Berlin"),
    ("Zurich", "Europe/Zurich"),
    ("Geneva", "Europe/Zurich"),
    ("Saint Petersburg", "Europe/Moscow"),
    // Cities of the other time zones of countries with several (`countries.json`).
    ("Halifax", "America/Halifax"),
    ("St. John's", "America/St_Johns"),
    ("Winnipeg", "America/Winnipeg"),
    ("Regina", "America/Regina"),
    ("Edmonton", "America/Edmonton"),
    ("Cancún", "America/Cancun"),
    ("Mazatlán", "America/Mazatlan"),
    ("Tijuana", "America/Tijuana"),
    ("Manaus", "America/Manaus"),
    ("Rio Branco", "America/Rio_Branco"),
    ("Fernando de Noronha", "America/Noronha"),
    ("Kaliningrad", "Europe/Kaliningrad"),
    ("Samara", "Europe/Samara"),
    ("Yekaterinburg", "Asia/Yekaterinburg"),
    ("Omsk", "Asia/Omsk"),
    ("Novosibirsk", "Asia/Novosibirsk"),
    ("Krasnoyarsk", "Asia/Krasnoyarsk"),
    ("Irkutsk", "Asia/Irkutsk"),
    ("Yakutsk", "Asia/Yakutsk"),
    ("Vladivostok", "Asia/Vladivostok"),
    ("Magadan", "Asia/Magadan"),
    ("Petropavlovsk-Kamchatsky", "Asia/Kamchatka"),
    ("Darwin", "Australia/Darwin"),
    ("Makassar", "Asia/Makassar|Asia/Ujung_Pandang"),
    ("Jayapura", "Asia/Jayapura"),
    ("Hovd", "Asia/Hovd"),
    ("Hanga Roa", "Pacific/Easter"),
    ("Kiritimati", "Pacific/Kiritimati"),
    ("Kanton", "Pacific/Kanton|Pacific/Enderbury"),
    ("Ponta Delgada", "Atlantic/Azores"),
    ("Las Palmas", "Atlantic/Canary"),
    ("Puerto Ayora", "Pacific/Galapagos"),
    ("Lubumbashi", "Africa/Lubumbashi"),
    ("Weno", "Pacific/Chuuk|Pacific/Truk"),
];

// ---------------------------------------------------------------- countries

/// A country (`src/data/countries.json`): its names, and a city per time zone, the most
/// relevant first.
#[derive(Debug)]
pub struct Country {
    /// ISO 3166 code: `IT`.
    pub code: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub cities: Vec<String>,
    /// Its currency's ISO 4217 code (`EUR`).
    pub currency: String,
}

const COUNTRIES: &str = include_str!("../data/countries.json");

pub fn countries() -> &'static [Country] {
    static TABLE: OnceLock<Vec<Country>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let doc = crate::json::parse(COUNTRIES).expect("src/data/countries.json is valid JSON");
        let strings = |v: Option<&crate::json::Value>| v.map(crate::json::Value::strings).unwrap_or_default();
        doc.get("countries")
            .map(|c| c.array())
            .unwrap_or_default()
            .iter()
            .filter_map(|c| {
                Some(Country {
                    code: c.get("code").and_then(|v| v.str()).unwrap_or_default().to_string(),
                    name: c.get("name")?.str()?.to_string(),
                    aliases: strings(c.get("aliases")),
                    cities: strings(c.get("cities")),
                    currency: c.get("currency").and_then(|v| v.str()).unwrap_or_default().to_string(),
                })
            })
            .collect()
    })
}

/// The built-in names, normalised once (`squeeze`): every lookup is then a hash lookup instead of
/// a scan that normalises each name again.
struct Names {
    /// A city's name → its index in `CITIES` (the first city with that name).
    cities: HashMap<String, usize>,
    /// Each city's normalised name, in `CITIES` order (misspellings are measured against them).
    city_keys: Vec<String>,
    /// A country's name or alias → its index in `countries()` (the first with that name).
    countries: HashMap<String, usize>,
    /// Every country name and alias, normalised, with the country's index.
    country_keys: Vec<(String, usize)>,
    /// A country's ISO code (`IT`) → its index in `countries()`.
    codes: HashMap<&'static str, usize>,
    /// A time zone → the country it belongs to (the first whose cities are in it).
    zones: HashMap<&'static str, usize>,
    /// The most words in a city name (`Sri Jayawardenepura Kotte`) and in a country name (`Saint
    /// Vincent and the Grenadines`).
    longest_city: usize,
    longest_country: usize,
}

fn names() -> &'static Names {
    static NAMES: OnceLock<Names> = OnceLock::new();
    NAMES.get_or_init(|| {
        // Each name normalised once: its words counted, then squeezed.
        let mut longest_city = 1;
        let city_keys: Vec<String> = CITIES
            .iter()
            .map(|(n, _)| {
                let k = key(n);
                longest_city = longest_city.max(k.split(' ').count());
                k.replace(' ', "")
            })
            .collect();
        let mut cities = HashMap::new();
        for (i, k) in city_keys.iter().enumerate() {
            cities.entry(k.clone()).or_insert(i);
        }
        let mut country_keys = Vec::new();
        let mut by_name = HashMap::new();
        let mut codes = HashMap::new();
        let mut zones = HashMap::new();
        let mut longest_country = 1;
        for (i, c) in countries().iter().enumerate() {
            codes.entry(c.code.as_str()).or_insert(i);
            for name in std::iter::once(&c.name).chain(&c.aliases) {
                let k = key(name);
                longest_country = longest_country.max(k.split(' ').count());
                let k = k.replace(' ', "");
                by_name.entry(k.clone()).or_insert(i);
                country_keys.push((k, i));
            }
            for city in &c.cities {
                if let Some(&ci) = cities.get(&squeeze(city)) {
                    for zone in CITIES[ci].1.split('|') {
                        zones.entry(zone).or_insert(i);
                    }
                }
            }
        }
        Names { longest_city, longest_country, cities, city_keys, countries: by_name, country_keys, codes, zones }
    })
}

/// The country whose ISO code is `code` (`IT`), when it is one of `countries.json` (no
/// territories: Bermuda, the Cayman Islands…).
pub fn country_by_code(code: &str) -> Option<&'static Country> {
    names().codes.get(code).map(|&i| &countries()[i])
}

/// A name as compared, spaces aside too: `New York`, `new-york` and `NewYork` are `newyork`.
pub(crate) fn squeeze(name: &str) -> String {
    key(name).replace(' ', "")
}

/// The country a name gives (any case, accents optional, spaces and dots aside: `usa`, `U.S.`,
/// `Turkiye`).
pub fn country(name: &str) -> Option<&'static Country> {
    names().countries.get(&squeeze(name)).map(|&i| &countries()[i])
}

/// The country a time zone belongs to (`Europe/Rome`: Italy), from its cities.
pub fn zone_country(zone: &str) -> Option<&'static Country> {
    names().zones.get(zone).map(|&i| &countries()[i])
}

/// How far a misspelt name is from a place's names (Levenshtein, case, accents and spaces aside).
pub fn distance(name: &str, names: &[&str]) -> usize {
    let wanted = squeeze(name);
    names.iter().map(|n| levenshtein(&wanted, &squeeze(n))).min().unwrap_or(usize::MAX)
}

/// The candidates nearest `wanted` (normalised), within `limit` letters: only those at the best
/// distance, in their order. `None` when one is `wanted` exactly (it is no misspelling).
pub(crate) fn nearest<'a, T: Copy>(wanted: &str, candidates: impl IntoIterator<Item = (&'a str, T)>, limit: usize) -> Option<(usize, Vec<T>)> {
    let len = wanted.chars().count();
    let mut best: Option<(usize, Vec<T>)> = None;
    for (k, item) in candidates {
        // Lengths further apart than the limit can't be within it.
        if k.chars().count().abs_diff(len) > limit {
            continue;
        }
        let Some(d) = levenshtein_within(wanted, k, limit) else { continue };
        if d == 0 {
            return None;
        }
        match &mut best {
            Some((b, items)) if d == *b => items.push(item),
            Some((b, _)) if d > *b => {}
            _ => best = Some((d, vec![item])),
        }
    }
    best
}

/// The countries whose names (or aliases) are nearest a misspelt one, within 2 letters (`Jappan`:
/// Japan), case, accents and spaces aside; none for an exact name or fewer than 3 letters. Short
/// aliases (`US`, `UK`) only match exactly.
pub fn near_countries(name: &str) -> Vec<&'static Country> {
    let wanted = squeeze(name);
    let len = wanted.chars().count();
    if len < 3 {
        return Vec::new();
    }
    let candidates = names().country_keys.iter().map(|(k, i)| (k.as_str(), *i));
    let Some((d, mut found)) = nearest(&wanted, candidates, 2) else { return Vec::new() };
    if len <= d + 2 {
        return Vec::new();
    }
    found.dedup();
    found.into_iter().map(|i| &countries()[i]).collect()
}

/// Every country name and alias, normalised (`squeeze`), with its country: for misspellings.
pub(crate) fn country_keys() -> impl Iterator<Item = (&'static str, &'static Country)> {
    names().country_keys.iter().map(|(k, i)| (k.as_str(), &countries()[*i]))
}

/// The number of words of the longest country name (`Saint Vincent and the Grenadines`).
pub fn longest_country() -> usize {
    names().longest_country
}

/// A city name as compared: lowercase, no accents, `- _ . '` as spaces, one space between words.
/// One pass, one allocation: every lookup normalises its text.
pub(crate) fn key(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut space = false;
    let mut push = |c: char| {
        if c.is_whitespace() || matches!(c, '-' | '_' | '.' | '\'' | '’') {
            space = !out.is_empty();
        } else {
            if space {
                out.push(' ');
                space = false;
            }
            out.push(c);
        }
    };
    for c in name.chars() {
        // ASCII (most names) without the Unicode tables.
        if c.is_ascii() {
            push(c.to_ascii_lowercase());
        } else {
            c.to_lowercase().map(unaccented).for_each(&mut push);
        }
    }
    out
}

/// The index in `CITIES` of the city a name gives.
fn city_index(name: &str) -> Option<usize> {
    names().cities.get(&squeeze(name)).copied()
}

/// The city a name gives (any case, accents optional: `new york`, `Sao Paulo`), or a zone's own
/// name (`Europe/Paris`, `America/New_York`). `None`: not a city; `Some(Err)`: its zone is not
/// installed.
pub fn city(name: &str) -> Option<Result<City, String>> {
    // Spaces aside: `NewYork` is New York.
    if let Some(i) = city_index(name) {
        let (display, zones) = CITIES[i];
        let mut error = String::new();
        for zone in zones.split('|') {
            match load(zone) {
                Ok(tz) => return Some(Ok(City { name: display, tz })),
                Err(e) if error.is_empty() => error = e,
                Err(_) => {}
            }
        }
        return Some(Err(error));
    }
    // A zone name: `Region/City`, shown as its last part (`New_York` → `New York`).
    if name.contains('/') && name.starts_with(|c: char| c.is_ascii_uppercase()) {
        let tz = load(name).ok()?;
        let shown = tz.name.rsplit('/').next().unwrap().replace('_', " ");
        return Some(Ok(City { name: Box::leak(shown.into_boxed_str()), tz }));
    }
    None
}

/// The name of the place `name` is, as shown (`new york` → `New York`), case, accents and spaces
/// aside; without loading its time zone.
pub fn place_name(name: &str) -> Option<&'static str> {
    city_index(name).map(|i| CITIES[i].0)
}

/// The places' names, as shown.
pub fn place_names() -> impl Iterator<Item = &'static str> {
    CITIES.iter().map(|(n, _)| *n)
}

/// The built-in cities nearest a misspelt name, within 2 letters, as indexes in `CITIES`; none
/// for an exact name or fewer than 3 letters.
fn near_indexes(name: &str) -> Vec<usize> {
    let wanted = squeeze(name);
    if wanted.chars().count() < 3 {
        return Vec::new();
    }
    let candidates = names().city_keys.iter().enumerate().map(|(i, k)| (k.as_str(), i));
    nearest(&wanted, candidates, 2).map_or_else(Vec::new, |(_, found)| found)
}

/// The places nearest a misspelt name, within 2 letters (`Rmoe`: Rome), as `near` does.
pub fn near_names(name: &str) -> Vec<&'static str> {
    // An exact name is its own suggestion.
    if let Some(i) = city_index(name) {
        return vec![CITIES[i].0];
    }
    let mut names: Vec<&str> = near_indexes(name).into_iter().map(|i| CITIES[i].0).collect();
    names.dedup();
    names
}

/// The distance between two words: the fewest letters added, removed or changed (Levenshtein).
/// `levenshtein`, when it is `limit` or less: it stops once a row has no value within the limit
/// (the values only grow from there), so a name far off costs a few letters, not the whole table.
pub(crate) fn levenshtein_within(a: &str, b: &str, limit: usize) -> Option<usize> {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        let mut lowest = row[0];
        for (j, &cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (above + 1).min(row[j] + 1).min(diagonal + usize::from(ca != cb));
            diagonal = above;
            lowest = lowest.min(row[j + 1]);
        }
        if lowest > limit {
            return None;
        }
    }
    Some(row[b.len()]).filter(|&d| d <= limit)
}

pub(crate) fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (above + 1).min(row[j] + 1).min(diagonal + usize::from(ca != cb));
            diagonal = above;
        }
    }
    row[b.len()]
}

/// The cities whose names are nearest a misspelt one, within 2 letters (`Tokio`: Tokyo), case,
/// accents and spaces aside. Several only at the same distance; none for an exact name or fewer
/// than 3 letters.
pub fn near(name: &str) -> Vec<City> {
    near_indexes(name)
        .into_iter()
        .filter_map(|i| {
            let (display, zones) = CITIES[i];
            zones.split('|').find_map(|z| load(z).ok()).map(|tz| City { name: display, tz })
        })
        .collect()
}

/// The number of words of the longest city name (`Sri Jayawardenepura Kotte`).
pub fn longest_name() -> usize {
    names().longest_city
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(y: i64, m: u32, d: u32, h: i64) -> i64 {
        days_from_civil(y, m, d) * 86_400 + h * 3600
    }

    #[test]
    fn posix_rules() {
        let paris = Rule::parse("CET-1CEST,M3.5.0,M10.5.0/3").unwrap();
        assert_eq!(paris.std, 3600);
        // 2026: DST from Sunday 29 March 01:00 UTC to Sunday 25 October 01:00 UTC.
        assert_eq!(paris.offset_at(secs(2026, 3, 29, 1) - 1), 3600);
        assert_eq!(paris.offset_at(secs(2026, 3, 29, 1)), 7200);
        assert_eq!(paris.offset_at(secs(2026, 10, 25, 1) - 1), 7200);
        assert_eq!(paris.offset_at(secs(2026, 10, 25, 1)), 3600);
        // Southern hemisphere: DST across the new year.
        let sydney = Rule::parse("AEST-10AEDT,M10.1.0,M4.1.0/3").unwrap();
        assert_eq!(sydney.offset_at(secs(2026, 1, 15, 0)), 11 * 3600);
        assert_eq!(sydney.offset_at(secs(2026, 7, 15, 0)), 10 * 3600);
        // US rules, quoted names, no DST, half hours.
        let ny = Rule::parse("EST5EDT,M3.2.0,M11.1.0").unwrap();
        assert_eq!(ny.offset_at(secs(2026, 7, 1, 12)), -4 * 3600);
        assert_eq!(ny.offset_at(secs(2026, 12, 1, 12)), -5 * 3600);
        assert_eq!(Rule::parse("<+0530>-5:30").unwrap(), Rule { std: 19_800, dst: None });
        assert_eq!(Rule::parse("<-03>3").unwrap().std, -3 * 3600);
        assert_eq!(Rule::parse("IST-5:30").unwrap().offset_at(0), 19_800);
        // Julian and ordinal days.
        assert_eq!(day_of(2024, RuleDay::Julian(60)), days_from_civil(2024, 3, 1));
        assert_eq!(day_of(2024, RuleDay::Ordinal(59)), days_from_civil(2024, 2, 29));
        assert_eq!(day_of(2026, RuleDay::Weekday(3, 5, 0)), days_from_civil(2026, 3, 29), "the last Sunday");
        for bad in ["", "C-1", "CET", "CET-1CEST,M13.1.0,M10.5.0", "CET-1CEST,M3.5.0"] {
            assert_eq!(Rule::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn zone_database() {
        // Skipped where the database is not installed.
        let Ok(paris) = load("Europe/Paris") else { return };
        assert_eq!(paris.offset_at(secs(2026, 7, 1, 12)), 7200);
        assert_eq!(paris.offset_at(secs(2026, 12, 1, 12)), 3600);
        assert_eq!(paris.offset_at(secs(2040, 7, 1, 12)), 7200, "after the last transition: the rule");
        assert_eq!(paris.offset_at(secs(1950, 1, 1, 12)), 3600);
        let wall = secs(2026, 7, 1, 14);
        assert_eq!(paris.to_utc(wall), wall - 7200);
        // 29 March 2026, 02:30 doesn't exist (02:00 → 03:00): 03:30 CEST, 01:30Z.
        let gap = secs(2026, 3, 29, 2) + 1800;
        assert_eq!(paris.to_utc(gap), secs(2026, 3, 29, 1) + 1800);
        // 25 October 2026, 02:30 happens twice (03:00 → 02:00): the first, CEST, 00:30Z.
        let overlap = secs(2026, 10, 25, 2) + 1800;
        assert_eq!(paris.to_utc(overlap), secs(2026, 10, 25, 0) + 1800);
        assert!(std::ptr::eq(paris, load("Europe/Paris").unwrap()), "loaded once");
        let Ok(kolkata) = load("Asia/Kolkata") else { return };
        assert_eq!(kolkata.offset_at(secs(2026, 1, 1, 0)), 19_800);
        for bad in ["../etc/passwd", "/etc/passwd", "Europe/../../etc", "", "Nowhere/City"] {
            assert!(load(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn cities() {
        let Ok(_) = load("Europe/Paris") else { return };
        let name = |s: &str| city(s).map(|c| c.map(|c| (c.name, c.tz.name)));
        assert_eq!(name("Paris"), Some(Ok(("Paris", "Europe/Paris"))));
        assert_eq!(name("new york"), Some(Ok(("New York", "America/New_York"))));
        assert_eq!(name("Sao Paulo"), Some(Ok(("São Paulo", "America/Sao_Paulo"))));
        assert_eq!(name("PORT-AU-PRINCE"), Some(Ok(("Port-au-Prince", "America/Port-au-Prince"))));
        assert_eq!(name("Europe/Rome"), Some(Ok(("Rome", "Europe/Rome"))));
        assert_eq!(name("America/New_York"), Some(Ok(("New York", "America/New_York"))));
        assert_eq!(name("Atlantis"), None);
        assert_eq!(name("km"), None);
        // Every city's zone is installed (or its older name).
        for (n, _) in CITIES {
            assert!(matches!(city(n), Some(Ok(_))), "{n}: {:?}", city(n));
        }
        // Misspelt names: the nearest, within 2 letters, case and spaces aside.
        let near_names = |s: &str| near(s).into_iter().map(|c| c.name).collect::<Vec<_>>();
        assert_eq!(near_names("Tokio"), ["Tokyo"]);
        assert_eq!(near_names("new yrok"), ["New York"]);
        assert_eq!(near_names("NewYork"), Vec::<&str>::new(), "spaces aside: the same name");
        assert_eq!(near_names("Parris"), ["Paris"]);
        assert_eq!(near_names("Lisbona"), ["Lisbon"]);
        assert_eq!(near_names("Bejing"), ["Beijing"]);
        assert_eq!(near_names("Atlantis"), ["Atlanta"], "2 letters");
        assert!(near_names("Gotham").is_empty(), "too far");
        assert!(near_names("Paris").is_empty(), "exact");
        assert!(near_names("km").is_empty(), "too short");
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("tokio", "tokyo"), 1);
        // Within a limit: the distance, or none past it (stopping early).
        assert_eq!(levenshtein_within("tokio", "tokyo", 1), Some(1));
        assert_eq!(levenshtein_within("kitten", "sitting", 2), None);
        assert_eq!(levenshtein_within("kitten", "sitting", 3), Some(3));
        assert_eq!(levenshtein_within("rome", "rome", 0), Some(0));
        assert_eq!(levenshtein_within("sanfransisco", "sanfrancisco", 2), Some(1));
        assert_eq!(levenshtein_within("abcdef", "uvwxyz", 2), None);
        // Names are distinct.
        let mut keys: Vec<String> = CITIES.iter().map(|(n, _)| key(n)).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), CITIES.len());
    }
}
