//! Number formatting with the device's decimal and thousands separators: the interactive mode's
//! preview and trivia, and distances everywhere (`geo.rs`).
//!
//! The separators come from the C library, as for any program on the device: `setlocale(LC_NUMERIC,
//! "")` (from `LC_ALL`, `LC_NUMERIC`, `LANG`) and `localeconv()`. On macOS the region setting
//! (`defaults read -g AppleLocale`) is tried first. A locale that is not installed (on Linux it must
//! be generated) falls back to the plain format: `.` and no grouping.

/// Decimal point, thousands separator and digit grouping (`localeconv`'s `grouping`: sizes from the
/// right; the last one repeats; 127 or more stops grouping).
#[derive(Debug, Clone, PartialEq)]
pub struct NumberFormat {
    pub decimal: String,
    pub thousands: String,
    pub grouping: Vec<u8>,
}

impl Default for NumberFormat {
    /// The plain format, as printed on the command line: `1234567.89`.
    fn default() -> Self {
        NumberFormat { decimal: ".".into(), thousands: String::new(), grouping: Vec::new() }
    }
}

impl NumberFormat {
    /// Reformats a number as printed by `convert::format_value` (`-1234567.5`).
    pub fn apply(&self, number: &str) -> String {
        let (sign, digits) = match number.strip_prefix('-') {
            Some(rest) => ("-", rest),
            None => ("", number),
        };
        let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
        if !int.bytes().all(|b| b.is_ascii_digit()) {
            return number.to_string();
        }
        let mut out = format!("{sign}{}", self.group(int));
        if !frac.is_empty() {
            out.push_str(&self.decimal);
            out.push_str(frac);
        }
        out
    }

    /// Reformats the leading number of a result such as `1073.7418 MB`.
    #[cfg_attr(not(feature = "tui"), allow(dead_code))]
    pub fn apply_to_result(&self, result: &str) -> String {
        match result.split_once(' ') {
            Some((number, unit)) => format!("{} {unit}", self.apply(number)),
            None => self.apply(result),
        }
    }

    fn group(&self, int: &str) -> String {
        let first = self.grouping.first().copied().unwrap_or(0);
        if self.thousands.is_empty() || first == 0 || first >= 127 {
            return int.to_string();
        }
        let mut groups: Vec<&str> = Vec::new();
        let (mut end, mut size, mut next) = (int.len(), usize::from(first), 1);
        while end > size {
            groups.push(&int[end - size..end]);
            end -= size;
            match self.grouping.get(next).copied() {
                // No more grouping: the rest in one piece.
                Some(g) if g >= 127 => break,
                // 0 or the end of the list: the last size repeats.
                Some(0) | None => {}
                Some(g) => size = usize::from(g),
            }
            next += 1;
        }
        groups.push(&int[..end]);
        groups.reverse();
        groups.join(&self.thousands)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod sys {
    use std::os::raw::{c_char, c_int};

    #[cfg(target_os = "linux")]
    pub const LC_NUMERIC: c_int = 1;
    #[cfg(target_os = "macos")]
    pub const LC_NUMERIC: c_int = 4;

    /// The first fields of `struct lconv`, in the same order on glibc, musl and macOS; only these are read.
    #[repr(C)]
    pub struct Lconv {
        pub decimal_point: *const c_char,
        pub thousands_sep: *const c_char,
        pub grouping: *const c_char,
    }

    extern "C" {
        pub fn setlocale(category: c_int, locale: *const c_char) -> *const c_char;
        pub fn localeconv() -> *const Lconv;
    }
}

/// The device's number format (see the module docs). Call once, before other threads start:
/// `setlocale` is process-wide. It is put back to "C" afterwards.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn device_format() -> NumberFormat {
    use std::ffi::{CStr, CString};
    // The macOS region first (the C library there ignores it), then the environment's locale.
    #[cfg(target_os = "macos")]
    let preferred = macos_locale().and_then(|locale| CString::new(format!("{locale}.UTF-8")).ok());
    #[cfg(not(target_os = "macos"))]
    let preferred: Option<CString> = None;
    let candidates: Vec<CString> = preferred.into_iter().chain(CString::new("").ok()).collect();
    let read = |ptr: *const std::os::raw::c_char| {
        if ptr.is_null() {
            return Vec::new();
        }
        // SAFETY: localeconv's strings are valid, NUL-terminated, until the next setlocale.
        unsafe { CStr::from_ptr(ptr) }.to_bytes().to_vec()
    };
    let mut format = NumberFormat::default();
    for locale in &candidates {
        // SAFETY: valid C strings; called before any other thread uses the C locale.
        if unsafe { sys::setlocale(sys::LC_NUMERIC, locale.as_ptr()) }.is_null() {
            continue;
        }
        // SAFETY: localeconv returns a pointer to a static struct whose first fields are `Lconv`.
        let conv = unsafe { &*sys::localeconv() };
        let decimal = String::from_utf8(read(conv.decimal_point)).unwrap_or_default();
        format = NumberFormat {
            decimal: if decimal.is_empty() { ".".into() } else { decimal },
            thousands: String::from_utf8(read(conv.thousands_sep)).unwrap_or_default(),
            grouping: read(conv.grouping),
        };
        break;
    }
    let c = CString::new("C").unwrap();
    // SAFETY: as above.
    unsafe { sys::setlocale(sys::LC_NUMERIC, c.as_ptr()) };
    format
}

/// The device's format, read once; the plain one in the unit tests, which must not depend on the
/// machine's locale.
#[cfg(test)]
pub fn device() -> &'static NumberFormat {
    static PLAIN: std::sync::OnceLock<NumberFormat> = std::sync::OnceLock::new();
    PLAIN.get_or_init(NumberFormat::default)
}

/// The device's format, read once (or given by the app embedding tally: `set_device`).
#[cfg(not(test))]
pub fn device() -> &'static NumberFormat {
    DEVICE.get_or_init(device_format)
}

#[cfg_attr(test, allow(dead_code))]
static DEVICE: std::sync::OnceLock<NumberFormat> = std::sync::OnceLock::new();

/// The device's format as the app embedding tally knows it (the Android app: from its `Locale`),
/// before any number is formatted; later calls are ignored.
pub fn set_device(format: NumberFormat) {
    let _ = DEVICE.set(format);
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn device_format() -> NumberFormat {
    NumberFormat::default()
}

/// The region in macOS settings (`it_IT`, or `en_IT@rg=itzzzz` → `en_IT`).
#[cfg(target_os = "macos")]
fn macos_locale() -> Option<String> {
    let out = std::process::Command::new("/usr/bin/defaults").args(["read", "-g", "AppleLocale"]).output().ok()?;
    let locale = String::from_utf8(out.stdout).ok()?;
    let locale = locale.trim().split('@').next()?.to_string();
    (out.status.success() && !locale.is_empty()).then_some(locale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(decimal: &str, thousands: &str, grouping: &[u8]) -> NumberFormat {
        NumberFormat { decimal: decimal.into(), thousands: thousands.into(), grouping: grouping.to_vec() }
    }

    #[test]
    fn plain_format_is_unchanged() {
        let plain = NumberFormat::default();
        for n in ["0", "16", "-40", "1073.7418", "100000000", "0.3333"] {
            assert_eq!(plain.apply(n), n);
        }
    }

    #[test]
    fn us_italian_french_swiss() {
        let us = format(".", ",", &[3, 3]);
        assert_eq!(us.apply("100000000"), "100,000,000");
        assert_eq!(us.apply("1073.7418"), "1,073.7418");
        assert_eq!(us.apply("-1234567.5"), "-1,234,567.5");
        assert_eq!(us.apply("999"), "999");
        assert_eq!(us.apply("0.3333"), "0.3333");
        let it = format(",", ".", &[3, 3]);
        assert_eq!(it.apply("1073.7418"), "1.073,7418");
        let fr = format(",", "\u{202f}", &[3]);
        assert_eq!(fr.apply("1234567.89"), "1\u{202f}234\u{202f}567,89");
        let ch = format(".", "'", &[3, 3]);
        assert_eq!(ch.apply("1234567"), "1'234'567");
    }

    #[test]
    fn grouping_rules() {
        // India: the first group is 3, then 2.
        assert_eq!(format(".", ",", &[3, 2]).apply("100000000"), "10,00,00,000");
        // 127 stops grouping.
        assert_eq!(format(".", ",", &[3, 127]).apply("100000000"), "100000,000");
        // No separator or no grouping: digits as they are.
        assert_eq!(format(",", "", &[3]).apply("1234.5"), "1234,5");
        assert_eq!(format(",", ".", &[]).apply("1234.5"), "1234,5");
    }

    #[test]
    fn results_with_units() {
        let it = format(",", ".", &[3, 3]);
        assert_eq!(it.apply_to_result("1073.7418 MB"), "1.073,7418 MB");
        assert_eq!(it.apply_to_result("16"), "16");
        assert_eq!(it.apply_to_result("100000000 kb"), "100.000.000 kb");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reads_the_c_library() {
        // Only checks that the call works and gives a sane format; the values depend on the machine.
        let f = device_format();
        assert!(!f.decimal.is_empty());
    }
}
