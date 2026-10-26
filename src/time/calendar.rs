//! The calendar: civil dates and days (proleptic Gregorian, years -9999 to 9999), ISO weeks,
//! instants and their zones, the device's zone, and now.

use super::*;

pub fn is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

pub fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from((m + 9) % 12); // March = 0
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (era * 400 + yoe + i64::from(m <= 2), m, d)
}

/// ISO weekday: Monday = 1 … Sunday = 7.
pub fn weekday(days: i64) -> u32 {
    (days + 3).rem_euclid(7) as u32 + 1
}

/// ISO week date of a day: (week-based year, week, weekday).
pub fn iso_week(days: i64) -> (i64, u32, u32) {
    let wd = weekday(days);
    let thursday = days - i64::from(wd) + 4;
    let (year, _, _) = civil_from_days(thursday);
    let week = ((thursday - days_from_civil(year, 1, 1)) / 7 + 1) as u32;
    (year, week, wd)
}

pub(super) fn weeks_in_year(year: i64) -> u32 {
    iso_week(days_from_civil(year, 12, 28)).1
}

pub(super) fn days_from_iso_week(year: i64, week: u32, wd: u32) -> i64 {
    let jan4 = days_from_civil(year, 1, 4);
    jan4 - i64::from(weekday(jan4) - 1) + i64::from(week - 1) * 7 + i64::from(wd - 1)
}

impl Instant {
    /// Offset from UTC in seconds at this instant.
    pub fn offset(&self) -> i32 {
        match self.zone {
            Zone::Fixed(offset) => offset,
            Zone::Local => match local_tz() {
                Some(tz) => tz.offset_at(self.ns.div_euclid(NS) as i64),
                None => sys::local_offset(self.ns.div_euclid(NS) as i64).unwrap_or(0),
            },
            Zone::Named(tz) => tz.offset_at(self.ns.div_euclid(NS) as i64),
        }
    }

    /// The wall-clock date and time in the instant's zone.
    pub fn civil(&self) -> Civil {
        let local = self.ns + i128::from(self.offset()) * NS;
        let (days, rem) = (local.div_euclid(DAY_NS), local.rem_euclid(DAY_NS));
        let (year, month, day) = civil_from_days(days as i64);
        let secs = (rem / NS) as u32;
        Civil { year, month, day, hour: secs / 3600, minute: secs / 60 % 60, second: secs % 60, nano: (rem % NS) as u32 }
    }

    pub(super) fn in_range(self) -> Result<Self, TimeError> {
        let min = i128::from(days_from_civil(MIN_YEAR, 1, 1) - 1) * DAY_NS;
        let max = i128::from(days_from_civil(MAX_YEAR + 1, 1, 1) + 1) * DAY_NS;
        if (min..max).contains(&self.ns) {
            Ok(self)
        } else {
            Err(TimeError::Range)
        }
    }
}

pub(super) static DEVICE_ZONE: std::sync::RwLock<Option<&'static tz::Tz>> = std::sync::RwLock::new(None);

/// Sets the device's time zone (`Europe/Rome`), as the program running aequalis reads it from the
/// system: aequalis does not look for it itself. A program sets it at start, and a long-running one
/// again from time to time (the device may change zone); the zone's rules are read again each time.
/// `Err`: a zone the database does not know, and the zone in use stays.
pub fn set_device_zone(name: &str) -> Result<(), String> {
    let zone = tz::reload(name)?;
    *DEVICE_ZONE.write().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(zone);
    Ok(())
}

/// The device's time zone, by name, as the program set it (`set_device_zone`); `None`: not set.
pub fn device_zone() -> Option<&'static str> {
    local_tz().map(|tz| tz.name)
}

/// The device's zone (`set_device_zone`): local time through the same engine as the cities' (the
/// same answer for `02:30` in a DST gap either way). `None`: no zone was set, and the C library's
/// local time is used (UTC where there is none).
pub(super) fn local_tz() -> Option<&'static tz::Tz> {
    if let Some(zone) = crate::machine::mocked().and_then(|m| m.zone) {
        return tz::load(zone).ok();
    }
    *DEVICE_ZONE.read().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The instant of a wall-clock time in `zone`.
pub(super) fn from_civil(c: &Civil, zone: Zone) -> Result<Instant, TimeError> {
    if !(MIN_YEAR..=MAX_YEAR).contains(&c.year) {
        return Err(TimeError::Range);
    }
    let days = days_from_civil(c.year, c.month, c.day);
    let wall = i128::from(days) * 86_400 + i128::from(c.hour * 3600 + c.minute * 60 + c.second);
    let secs = match zone {
        Zone::Fixed(offset) => wall - i128::from(offset),
        Zone::Local => match local_tz() {
            Some(tz) => i128::from(tz.to_utc(wall as i64)),
            None => i128::from(sys::local_to_utc(c).unwrap_or(wall as i64)),
        },
        Zone::Named(tz) => i128::from(tz.to_utc(wall as i64)),
    };
    Instant { ns: secs * NS + i128::from(c.nano), zone }.in_range()
}

pub(super) static FIXED_NOW: std::sync::OnceLock<i64> = std::sync::OnceLock::new();

/// Freezes "now" at `unix_seconds` for the whole process: the hidden `--now` option, for tests
/// whose data has a date (exchange rates, the city archive, the Trivia's estimates). Set once,
/// before anything; later calls are ignored. File times (the sync's lock, the rates' cache) stay
/// real: they are about files, not dates.
pub fn set_now(unix_seconds: i64) {
    let _ = FIXED_NOW.set(unix_seconds);
}

/// The frozen "now" (`set_now`), if any: passed on to the background sync.
pub fn fixed_now() -> Option<i64> {
    FIXED_NOW.get().copied()
}

/// Now: the clock, or the frozen time (the thread's mock clock, `set_now`). Every date of aequalis comes from here.
pub fn now() -> Instant {
    if let Some(seconds) = crate::machine::mocked().and_then(|m| m.now) {
        return Instant { ns: i128::from(seconds) * NS, zone: Zone::Local };
    }
    if let Some(&seconds) = FIXED_NOW.get() {
        return Instant { ns: i128::from(seconds) * NS, zone: Zone::Local };
    }
    let ns = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as i128,
        Err(e) => -(e.duration().as_nanos() as i128),
    };
    Instant { ns, zone: Zone::Local }
}
