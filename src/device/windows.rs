//! Windows: the zones' rules from the registry, the ones the system clock follows (kept up to date by
//! Windows Update), and the device's zone from the system settings, so that local time is always the
//! clock's. Zone names (IANA: `Europe/Rome`) and Windows zones (`W. Europe Standard Time`) are matched
//! by the ICU library that ships with Windows (10, version 1903, and later); the rules are never ICU's.
//!
//! The registry keeps a zone's rule per year (most zones: one rule for every year), so offsets before
//! a rule the registry does not record are approximate.

use aequalis::time::{self, days_from_civil, days_in_month, weekday, ZoneRules, ZoneSource};
use std::ffi::{c_char, c_void};
use std::sync::{Mutex, OnceLock};

/// The years spelt out as transitions; after them, the last year's rule (`ZoneRules::rule`).
const FIRST_YEAR: u16 = 1970;
const LAST_YEAR: u16 = 2037;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct SystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

/// `TIME_ZONE_INFORMATION`.
#[repr(C)]
#[derive(Clone, Copy)]
struct ZoneInfo {
    bias: i32,
    standard_name: [u16; 32],
    standard_date: SystemTime,
    standard_bias: i32,
    daylight_name: [u16; 32],
    daylight_date: SystemTime,
    daylight_bias: i32,
}

/// `DYNAMIC_TIME_ZONE_INFORMATION`: a zone of the registry, by its key name.
#[repr(C)]
#[derive(Clone, Copy)]
struct DynamicZoneInfo {
    info: ZoneInfo,
    key_name: [u16; 128],
    dynamic_daylight_time_disabled: u8,
}

// The sizes in the Windows headers: a wrong layout fails the build.
const _: () = assert!(std::mem::size_of::<ZoneInfo>() == 172);
const _: () = assert!(std::mem::size_of::<DynamicZoneInfo>() == 432);

impl DynamicZoneInfo {
    fn empty() -> Self {
        let info = ZoneInfo {
            bias: 0,
            standard_name: [0; 32],
            standard_date: SystemTime::default(),
            standard_bias: 0,
            daylight_name: [0; 32],
            daylight_date: SystemTime::default(),
            daylight_bias: 0,
        };
        DynamicZoneInfo { info, key_name: [0; 128], dynamic_daylight_time_disabled: 0 }
    }
}

#[link(name = "kernel32")]
extern "system" {
    fn GetDynamicTimeZoneInformation(info: *mut DynamicZoneInfo) -> u32;
    fn GetTimeZoneInformationForYear(year: u16, zone: *const DynamicZoneInfo, info: *mut ZoneInfo) -> i32;
    fn GetUserDefaultGeoName(name: *mut u16, len: i32) -> i32;
    fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
}

#[link(name = "advapi32")]
extern "system" {
    fn EnumDynamicTimeZoneInformation(index: u32, info: *mut DynamicZoneInfo) -> u32;
}

// ---------------------------------------------------------------- the system's rules

/// Takes the zones' rules from the registry, unless `TZDIR` names a zone database (the tests, or
/// whoever prefers the IANA files).
pub fn use_system_rules() {
    if std::env::var_os("TZDIR").is_some_and(|d| !d.is_empty()) {
        return;
    }
    time::set_zone_source(Box::new(Registry));
}

/// The registry as the library's zone database.
struct Registry;

impl ZoneSource for Registry {
    fn rules(&self, name: &str) -> Option<ZoneRules> {
        if let Some(offset) = fixed(name) {
            return Some(ZoneRules { initial: offset, transitions: Vec::new(), rule: None });
        }
        // The device's own zone: exactly what the clock uses (automatic DST may be off).
        let device = DEVICE.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match device {
            Some(d) if d.name == name => registry_rules(&d.key, d.dst_disabled),
            _ => registry_rules(&windows_zone(name)?, false),
        }
    }
}

/// UTC and the `Etc/GMT±n` zones: a fixed offset, no Windows zone needed.
fn fixed(name: &str) -> Option<i32> {
    if ["UTC", "Etc/UTC", "Etc/UCT", "UCT", "Etc/GMT", "GMT", "Etc/Universal", "Universal", "Etc/Zulu", "Zulu"].contains(&name) {
        return Some(0);
    }
    // POSIX signs: `Etc/GMT+5` is five hours west, UTC-05:00.
    let hours: i32 = name.strip_prefix("Etc/GMT")?.parse().ok()?;
    (-14..=12).contains(&hours).then_some(-hours * 3600)
}

/// The rules of the registry zone `key`, as transitions from `FIRST_YEAR` to `LAST_YEAR` and the
/// rule of the year after. `dst_disabled`: the device's clock with automatic DST off.
fn registry_rules(key: &str, dst_disabled: bool) -> Option<ZoneRules> {
    let zone = registry_zone(key)?;
    let year = |y: u16| YearRule::of(&zone, y, dst_disabled);
    let mut transitions: Vec<(i64, i32)> = Vec::new();
    let mut current: Option<i32> = None;
    let mut initial = 0;
    for y in FIRST_YEAR..=LAST_YEAR {
        let rule = year(y)?;
        let (at_start, changes) = rule.changes(i64::from(y));
        match current {
            None => initial = at_start,
            // A new rule from the year's first moment (in the offset before it).
            Some(before) if before != at_start => push(&mut transitions, days_from_civil(i64::from(y), 1, 1) * 86_400 - i64::from(before), at_start),
            _ => {}
        }
        current = Some(at_start);
        for (at, offset) in changes {
            if current != Some(offset) {
                push(&mut transitions, at, offset);
                current = Some(offset);
            }
        }
    }
    Some(ZoneRules { initial, transitions, rule: year(LAST_YEAR + 1)?.posix() })
}

/// Adds a transition, keeping them in order: one at the same time or earlier is replaced.
fn push(transitions: &mut Vec<(i64, i32)>, at: i64, offset: i32) {
    while transitions.last().is_some_and(|&(t, _)| t >= at) {
        transitions.pop();
    }
    transitions.push((at, offset));
}

/// A registry zone by its key name (`W. Europe Standard Time`).
fn registry_zone(key: &str) -> Option<DynamicZoneInfo> {
    (0..)
        .map_while(|index| {
            let mut zone = DynamicZoneInfo::empty();
            // SAFETY: `zone` is a writable DYNAMIC_TIME_ZONE_INFORMATION; 0 is ERROR_SUCCESS.
            (unsafe { EnumDynamicTimeZoneInformation(index, &mut zone) } == 0).then_some(zone)
        })
        .find(|zone| text(&zone.key_name) == key)
}

/// A zone's rule in one year: its standard offset and, if any, its DST with when it starts and ends.
#[derive(Debug, PartialEq)]
struct YearRule {
    std: i32,
    dst: Option<(i32, SystemTime, SystemTime)>,
}

impl YearRule {
    fn of(zone: &DynamicZoneInfo, year: u16, dst_disabled: bool) -> Option<YearRule> {
        let mut info = DynamicZoneInfo::empty().info;
        // SAFETY: valid pointers to the structures the call reads and writes.
        if unsafe { GetTimeZoneInformationForYear(year, zone, &mut info) } == 0 {
            return None;
        }
        Some(YearRule::from_info(&info, dst_disabled))
    }

    /// The offsets east of UTC: Windows' biases are minutes west.
    fn from_info(info: &ZoneInfo, dst_disabled: bool) -> YearRule {
        let std = -(info.bias + info.standard_bias) * 60;
        let dst = -(info.bias + info.daylight_bias) * 60;
        let changes = info.daylight_date.month != 0 && info.standard_date.month != 0;
        YearRule { std, dst: (changes && !dst_disabled && dst != std).then_some((dst, info.daylight_date, info.standard_date)) }
    }

    /// The offset at the year's start, and the year's changes (UTC seconds, the offset from then on),
    /// in order. DST starts in standard time and ends in DST, each in the local time before it.
    fn changes(&self, year: i64) -> (i32, Vec<(i64, i32)>) {
        let Some((dst, start, end)) = self.dst else { return (self.std, Vec::new()) };
        let starts = local_seconds(year, start) - i64::from(self.std);
        let ends = local_seconds(year, end) - i64::from(dst);
        let mut changes = vec![(starts, dst), (ends, self.std)];
        changes.sort();
        // In the south, DST spans the new year.
        (if starts < ends { self.std } else { dst }, changes)
    }

    /// As a POSIX TZ rule (`<STD>-1<DST>-2,M3.5.0/2,M10.5.0/3`); `None` when a change is a fixed date,
    /// which a POSIX rule cannot repeat.
    fn posix(&self) -> Option<String> {
        let Some((dst, start, end)) = self.dst else { return Some(format!("<STD>{}", clock(-i64::from(self.std)))) };
        let change = |c: SystemTime| (c.year == 0).then(|| format!("M{}.{}.{}/{}", c.month, c.day, c.day_of_week, clock(time_of_day(c))));
        Some(format!("<STD>{}<DST>{},{},{}", clock(-i64::from(self.std)), clock(-i64::from(dst)), change(start)?, change(end)?))
    }
}

/// `[-]h[:mm[:ss]]`.
fn clock(seconds: i64) -> String {
    let sign = if seconds < 0 { "-" } else { "" };
    let s = seconds.abs();
    match (s % 3600 / 60, s % 60) {
        (0, 0) => format!("{sign}{}", s / 3600),
        (m, 0) => format!("{sign}{}:{m:02}", s / 3600),
        (m, sec) => format!("{sign}{}:{m:02}:{sec:02}", s / 3600),
    }
}

/// A change's local time in `year`, seconds since the epoch: `day` is the week of the month (5: the
/// last) of the weekday `day_of_week` (0: Sunday), or with `year` set, a fixed date.
fn local_seconds(year: i64, c: SystemTime) -> i64 {
    let month = u32::from(c.month);
    let day = if c.year == 0 {
        let first = days_from_civil(year, month, 1);
        // `weekday`: Monday = 1 … Sunday = 7; Windows: Sunday = 0.
        let first_wd = i64::from(weekday(first) % 7);
        let mut day = first + (i64::from(c.day_of_week) - first_wd).rem_euclid(7) + 7 * (i64::from(c.day) - 1);
        while day >= first + i64::from(days_in_month(year, month)) {
            day -= 7;
        }
        day
    } else {
        days_from_civil(year, month, u32::from(c.day))
    };
    day * 86_400 + time_of_day(c)
}

/// Seconds after midnight; Windows writes midnight as 23:59:59.999.
fn time_of_day(c: SystemTime) -> i64 {
    i64::from(c.hour) * 3600 + i64::from(c.minute) * 60 + i64::from(c.second) + i64::from(c.milliseconds >= 500)
}

// ---------------------------------------------------------------- the device's zone

/// The device's zone as last read: its IANA name, its registry key, automatic DST off.
#[derive(Clone, Debug)]
struct Device {
    name: String,
    key: String,
    dst_disabled: bool,
}

static DEVICE: Mutex<Option<Device>> = Mutex::new(None);

/// The zone in the Windows settings, by its IANA name for the user's region (`W. Europe Standard
/// Time` in Italy: `Europe/Rome`). Remembered, so that the zone's rules are the clock's.
pub fn device_zone() -> Option<String> {
    let mut zone = DynamicZoneInfo::empty();
    // SAFETY: `zone` is a writable DYNAMIC_TIME_ZONE_INFORMATION; u32::MAX is TIME_ZONE_ID_INVALID.
    if unsafe { GetDynamicTimeZoneInformation(&mut zone) } == u32::MAX {
        return None;
    }
    let key = text(&zone.key_name);
    let name = iana_zone(&key, region().as_deref()).or_else(|| iana_zone(&key, None))?;
    let device = Device { name: name.clone(), key, dst_disabled: zone.dynamic_daylight_time_disabled != 0 };
    *DEVICE.lock().unwrap_or_else(|e| e.into_inner()) = Some(device);
    Some(name)
}

/// The user's region (`IT`), from the Region settings.
fn region() -> Option<String> {
    let mut buf = [0u16; 16];
    // SAFETY: `buf` holds `len` UTF-16 units.
    let n = unsafe { GetUserDefaultGeoName(buf.as_mut_ptr(), buf.len() as i32) };
    (n > 0).then(|| text(&buf)).filter(|r| r.len() == 2)
}

// ---------------------------------------------------------------- names, through ICU

/// The parts of ICU (icu.dll) that match zone names: `ucal_getWindowsTimeZoneID`,
/// `ucal_getTimeZoneIDForWindowsID`.
struct Icu {
    to_windows: unsafe extern "C" fn(*const u16, i32, *mut u16, i32, *mut i32) -> i32,
    from_windows: unsafe extern "C" fn(*const u16, i32, *const c_char, *mut u16, i32, *mut i32) -> i32,
}

/// ICU, loaded once from System32 only (never from the current folder).
fn icu() -> Option<&'static Icu> {
    static ICU: OnceLock<Option<Icu>> = OnceLock::new();
    ICU.get_or_init(|| {
        const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x800;
        let name: Vec<u16> = "icu.dll\0".encode_utf16().collect();
        // SAFETY: a NUL-terminated name; the module stays loaded for the process's life.
        let module = unsafe { LoadLibraryExW(name.as_ptr(), std::ptr::null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32) };
        if module.is_null() {
            return None;
        }
        // SAFETY: NUL-terminated names; the functions have these C signatures (ICU's ucal.h).
        unsafe {
            let to_windows = GetProcAddress(module, c"ucal_getWindowsTimeZoneID".as_ptr());
            let from_windows = GetProcAddress(module, c"ucal_getTimeZoneIDForWindowsID".as_ptr());
            if to_windows.is_null() || from_windows.is_null() {
                return None;
            }
            Some(Icu { to_windows: std::mem::transmute(to_windows), from_windows: std::mem::transmute(from_windows) })
        }
    })
    .as_ref()
}

/// The Windows zone of an IANA zone (`Europe/Rome` → `W. Europe Standard Time`).
fn windows_zone(name: &str) -> Option<String> {
    let icu = icu()?;
    let id: Vec<u16> = name.encode_utf16().collect();
    let mut out = [0u16; 128];
    let mut status = 0i32;
    // SAFETY: `id` holds `len` units; `out` has room for 128; ICU sets `status` (> 0: an error).
    let n = unsafe { (icu.to_windows)(id.as_ptr(), id.len() as i32, out.as_mut_ptr(), out.len() as i32, &mut status) };
    (status <= 0 && n > 0).then(|| text(&out))
}

/// The IANA zone of a Windows zone, for a region (`IT`; `None`: the zone's main one).
fn iana_zone(key: &str, region: Option<&str>) -> Option<String> {
    let icu = icu()?;
    let id: Vec<u16> = key.encode_utf16().collect();
    let region = region.and_then(|r| std::ffi::CString::new(r).ok());
    let mut out = [0u16; 128];
    let mut status = 0i32;
    // SAFETY: as in `windows_zone`; the region is null or a NUL-terminated string.
    let n = unsafe {
        (icu.from_windows)(
            id.as_ptr(),
            id.len() as i32,
            region.as_ref().map_or(std::ptr::null(), |r| r.as_ptr()),
            out.as_mut_ptr(),
            out.len() as i32,
            &mut status,
        )
    };
    (status <= 0 && n > 0).then(|| text(&out))
}

/// The text of a NUL-terminated UTF-16 buffer.
fn text(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The offset of `rules` at `t`, for the years spelt out as transitions.
    fn offset(rules: &ZoneRules, t: i64) -> i32 {
        rules.transitions.iter().take_while(|&&(at, _)| at <= t).last().map_or(rules.initial, |&(_, o)| o)
    }

    // 15 January and 15 July 2026, noon UTC.
    const JAN: i64 = 1_768_478_400;
    const JUL: i64 = 1_784_116_800;

    #[test]
    fn fixed_zones() {
        assert_eq!(fixed("UTC"), Some(0));
        assert_eq!(fixed("Etc/GMT+5"), Some(-5 * 3600), "POSIX signs: west");
        assert_eq!(fixed("Etc/GMT-14"), Some(14 * 3600));
        assert_eq!(fixed("Etc/GMT+13"), None);
        assert_eq!(fixed("Europe/Rome"), None);
    }

    #[test]
    fn changes_on_their_days() {
        // The last Sunday of March 2026 is the 29th, at 02:00.
        let last_sunday = SystemTime { month: 3, day: 5, day_of_week: 0, hour: 2, ..SystemTime::default() };
        assert_eq!(local_seconds(2026, last_sunday), days_from_civil(2026, 3, 29) * 86_400 + 7200);
        // The first Sunday of April 2026 is the 5th; midnight as Windows writes it.
        let first_sunday = SystemTime { month: 4, day: 1, day_of_week: 0, hour: 23, minute: 59, second: 59, milliseconds: 999, ..SystemTime::default() };
        assert_eq!(local_seconds(2026, first_sunday), days_from_civil(2026, 4, 6) * 86_400);
        assert_eq!(clock(-3600), "-1");
        assert_eq!(clock(19_800), "5:30");
        assert_eq!(clock(86_400), "24");
    }

    #[test]
    fn names_both_ways() {
        assert_eq!(windows_zone("Europe/Rome").as_deref(), Some("W. Europe Standard Time"));
        assert_eq!(iana_zone("W. Europe Standard Time", Some("IT")).as_deref(), Some("Europe/Rome"));
        assert_eq!(iana_zone("W. Europe Standard Time", None).as_deref(), Some("Europe/Berlin"));
        assert_eq!(windows_zone("Nowhere/Atlantis"), None);
    }

    #[test]
    fn the_registry_rules() {
        let rome = registry_rules("W. Europe Standard Time", false).unwrap();
        assert_eq!((offset(&rome, JAN), offset(&rome, JUL)), (3600, 7200));
        assert_eq!(rome.rule.as_deref(), Some("<STD>-1<DST>-2,M3.5.0/2,M10.5.0/3"));
        // The south: DST across the new year.
        let sydney = registry_rules("AUS Eastern Standard Time", false).unwrap();
        assert_eq!((offset(&sydney, JAN), offset(&sydney, JUL)), (11 * 3600, 10 * 3600));
        // No DST at all.
        let dubai = registry_rules("Arabian Standard Time", false).unwrap();
        assert_eq!((offset(&dubai, JAN), offset(&dubai, JUL)), (4 * 3600, 4 * 3600));
        assert_eq!(dubai.rule.as_deref(), Some("<STD>-4"));
        // Automatic DST off: standard time all year, as the clock then shows.
        let rome_standard = registry_rules("W. Europe Standard Time", true).unwrap();
        assert_eq!(offset(&rome_standard, JUL), 3600);
        assert!(rome.transitions.windows(2).all(|w| w[0].0 < w[1].0), "in order");
        assert_eq!(registry_rules("Nowhere Standard Time", false), None);
    }

    #[test]
    fn the_device_zone_is_the_clocks() {
        // What the system clock uses now, against the device zone's rules: always the same offset.
        #[link(name = "kernel32")]
        extern "system" {
            fn GetTimeZoneInformation(info: *mut ZoneInfo) -> u32;
        }
        let name = device_zone().expect("the device's zone");
        let rules = Registry.rules(&name).expect("its rules");
        let mut info = DynamicZoneInfo::empty().info;
        // SAFETY: a writable TIME_ZONE_INFORMATION. 2: TIME_ZONE_ID_DAYLIGHT.
        let id = unsafe { GetTimeZoneInformation(&mut info) };
        let clock = -(info.bias + if id == 2 { info.daylight_bias } else { info.standard_bias }) * 60;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        assert_eq!(offset(&rules, now), clock, "{name}");
    }
}
