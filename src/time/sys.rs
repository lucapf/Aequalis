//! The C library's local time zone (with DST) and the locale's date format, without crates:
//! `localtime_r`, `mktime`, `strftime`, `strptime`, `nl_langinfo`. 64-bit Linux (glibc, musl) and
//! macOS, whose `struct tm` share the same layout (checked below). Elsewhere local time is UTC
//! and the locale's format is not available.

use super::Civil;

#[cfg(all(any(target_os = "linux", target_os = "macos"), target_pointer_width = "64"))]
mod imp {
    use super::Civil;
    use std::ffi::CStr;
    use std::os::raw::{c_char, c_int, c_long};
    use std::sync::Once;

    #[repr(C)]
    struct Tm {
        tm_sec: c_int,
        tm_min: c_int,
        tm_hour: c_int,
        tm_mday: c_int,
        tm_mon: c_int,
        tm_year: c_int,
        tm_wday: c_int,
        tm_yday: c_int,
        tm_isdst: c_int,
        tm_gmtoff: c_long,
        tm_zone: *const c_char,
    }

    // The size of `struct tm` in the C headers: a wrong layout fails the build.
    const _: () = assert!(size_of::<Tm>() == 56);

    #[cfg(target_os = "linux")]
    mod items {
        pub const LC_TIME: std::os::raw::c_int = 2;
        pub const D_T_FMT: std::os::raw::c_int = 0x20028;
        pub const D_FMT: std::os::raw::c_int = 0x20029;
    }
    #[cfg(target_os = "macos")]
    mod items {
        pub const LC_TIME: std::os::raw::c_int = 5;
        pub const D_T_FMT: std::os::raw::c_int = 1;
        pub const D_FMT: std::os::raw::c_int = 2;
    }
    use items::*;

    extern "C" {
        fn tzset();
        fn localtime_r(time: *const i64, tm: *mut Tm) -> *mut Tm;
        fn mktime(tm: *mut Tm) -> i64;
        fn strftime(s: *mut c_char, max: usize, format: *const c_char, tm: *const Tm) -> usize;
        fn strptime(s: *const c_char, format: *const c_char, tm: *mut Tm) -> *mut c_char;
        fn nl_langinfo(item: c_int) -> *const c_char;
        fn setlocale(category: c_int, locale: *const c_char) -> *const c_char;
    }

    fn zeroed() -> Tm {
        // SAFETY: all-zero is a valid `struct tm` (integers and a null pointer).
        unsafe { std::mem::zeroed() }
    }

    /// Reads the time zone (`TZ`, /etc/localtime) and the locale's time format once.
    fn init() {
        static INIT: Once = Once::new();
        // SAFETY: called once, before the other calls; the CLI and the TUI are single-threaded.
        INIT.call_once(|| unsafe {
            tzset();
            setlocale(LC_TIME, c"".as_ptr());
        });
    }

    pub fn local_offset(secs: i64) -> Option<i32> {
        init();
        let mut tm = zeroed();
        // SAFETY: valid pointers; localtime_r is the thread-safe variant.
        let ok = !unsafe { localtime_r(&secs, &mut tm) }.is_null();
        ok.then_some(tm.tm_gmtoff as i32)
    }

    pub fn local_to_utc(c: &Civil) -> Option<i64> {
        init();
        let mut tm = zeroed();
        tm.tm_year = i32::try_from(c.year - 1900).ok()?;
        tm.tm_mon = c.month as c_int - 1;
        tm.tm_mday = c.day as c_int;
        tm.tm_hour = c.hour as c_int;
        tm.tm_min = c.minute as c_int;
        tm.tm_sec = c.second as c_int;
        // Let the C library decide whether DST applies.
        tm.tm_isdst = -1;
        // SAFETY: a valid `struct tm`.
        Some(unsafe { mktime(&mut tm) })
    }

    pub fn format_local(secs: i64) -> Option<String> {
        init();
        let mut tm = zeroed();
        // SAFETY: valid pointers.
        if unsafe { localtime_r(&secs, &mut tm) }.is_null() {
            return None;
        }
        let mut buf = [0u8; 256];
        // SAFETY: nl_langinfo returns a valid C string; strftime writes at most `buf.len()` bytes.
        let n = unsafe { strftime(buf.as_mut_ptr().cast(), buf.len(), nl_langinfo(D_T_FMT), &tm) };
        (n > 0).then(|| String::from_utf8_lossy(&buf[..n]).into_owned())
    }

    /// The locale's date format (`D_FMT`, e.g. `%d/%m/%Y`).
    pub fn date_format() -> Option<String> {
        init();
        // SAFETY: nl_langinfo returns a valid C string.
        let format = unsafe { CStr::from_ptr(nl_langinfo(D_FMT)) };
        Some(format.to_string_lossy().into_owned())
    }

    /// `text` in the locale's date-and-time format, or its date format; seconds since the epoch.
    pub fn parse_local(text: &str) -> Option<i64> {
        init();
        let text = std::ffi::CString::new(text.trim()).ok()?;
        [D_T_FMT, D_FMT].into_iter().find_map(|item| {
            let mut tm = zeroed();
            // SAFETY: valid C strings and `struct tm`.
            let end = unsafe { strptime(text.as_ptr(), nl_langinfo(item), &mut tm) };
            // SAFETY: strptime returns a pointer into `text` (or null).
            let rest = (!end.is_null()).then(|| unsafe { CStr::from_ptr(end) })?;
            if !rest.to_bytes().iter().all(u8::is_ascii_whitespace) {
                return None;
            }
            tm.tm_isdst = -1;
            // SAFETY: a valid `struct tm`.
            Some(unsafe { mktime(&mut tm) })
        })
    }
}

/// Windows: the zone is the core's own (`local_tz`: `TZ`, else the Windows settings), so only the
/// formats are here. The `local` format is the C locale's (`Sun Apr 12 00:00:00 2026`), as with
/// `LC_ALL=C`; the date order is the Region settings', or `%m/%d/%y` when the environment names a locale.
#[cfg(windows)]
mod imp {
    use super::Civil;
    use crate::time::{civil_from_days, days_from_civil, from_civil, weekday, Instant, Zone, NS};

    const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

    pub fn local_offset(_secs: i64) -> Option<i32> {
        Some(0)
    }

    pub fn local_to_utc(_c: &Civil) -> Option<i64> {
        None
    }

    /// `%a %b %e %H:%M:%S %Y`.
    pub fn format_local(secs: i64) -> Option<String> {
        let c = Instant { ns: i128::from(secs) * NS, zone: Zone::Local }.civil();
        let day = weekday(days_from_civil(c.year, c.month, c.day));
        Some(format!("{} {} {:2} {:02}:{:02}:{:02} {}", WEEKDAYS[day as usize - 1], MONTHS[c.month as usize - 1], c.day, c.hour, c.minute, c.second, c.year))
    }

    /// The format of `format_local`: `Sun Apr 12 00:00:00 2026`, read in the device's zone.
    pub fn parse_local(text: &str) -> Option<i64> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let [wday, month, day, time, year] = words[..] else { return None };
        let weekday_ok = WEEKDAYS.iter().any(|w| w.eq_ignore_ascii_case(wday));
        let month = MONTHS.iter().position(|m| m.eq_ignore_ascii_case(month))? as u32 + 1;
        let mut clock = time.split(':').map(|p| p.parse::<u32>().ok());
        let (hour, minute, second) = (clock.next()??, clock.next()??, clock.next()??);
        let c = Civil { year: year.parse().ok()?, month, day: day.parse().ok()?, hour, minute, second, nano: 0 };
        let valid = (1..=31).contains(&c.day)
            && hour < 24
            && minute < 60
            && second < 61
            && civil_from_days(days_from_civil(c.year, c.month, c.day)) == (c.year, c.month, c.day);
        (weekday_ok && valid).then_some(())?;
        let instant = from_civil(&c, Zone::Local).ok()?;
        i64::try_from(instant.ns.div_euclid(NS)).ok()
    }

    pub fn date_format() -> Option<String> {
        let named = ["LC_ALL", "LC_TIME", "LANG"].iter().any(|v| std::env::var_os(v).is_some_and(|v| !v.is_empty()));
        if named {
            return Some("%m/%d/%y".into());
        }
        crate::windows::date_format()
    }
}

#[cfg(not(any(windows, all(any(target_os = "linux", target_os = "macos"), target_pointer_width = "64"))))]
mod imp {
    use super::Civil;

    pub fn local_offset(_secs: i64) -> Option<i32> {
        Some(0)
    }

    pub fn local_to_utc(_c: &Civil) -> Option<i64> {
        None
    }

    pub fn format_local(_secs: i64) -> Option<String> {
        None
    }

    pub fn parse_local(_text: &str) -> Option<i64> {
        None
    }

    pub fn date_format() -> Option<String> {
        None
    }
}

pub fn local_offset(secs: i64) -> Option<i32> {
    imp::local_offset(secs)
}

/// The UTC seconds of a local wall-clock time (DST decided by the C library).
pub fn local_to_utc(c: &Civil) -> Option<i64> {
    imp::local_to_utc(c)
}

pub fn format_local(secs: i64) -> Option<String> {
    imp::format_local(secs)
}

pub fn parse_local(text: &str) -> Option<i64> {
    imp::parse_local(text)
}

/// The locale's date format (`D_FMT`), for the day / month order of the `date` word.
pub fn date_format() -> Option<String> {
    imp::date_format()
}
