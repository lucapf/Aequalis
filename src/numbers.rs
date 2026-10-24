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

/// A request as shown (`NumberFormat::shown`), and how its places map: positions count characters.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub text: String,
    /// For each place of the request (before its first character … after its last): the place in `text`.
    pub at: Vec<usize>,
    /// For each place of `text`: the place in the request (a separator inserted belongs to the digit before it).
    pub back: Vec<usize>,
}

/// Which characters are not part of a number to group: the digits and separators of a date or a time.
fn not_numbers(c: &[char]) -> Vec<bool> {
    let mut kept = vec![false; c.len()];
    let run_end = |from: usize| (from..c.len()).find(|&k| !c[k].is_ascii_digit()).unwrap_or(c.len());
    let mut i = 0;
    while i < c.len() {
        if !c[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        // The runs of digits joined by one separator character, from here.
        let chain = |sep: char| {
            let mut runs = vec![(i, run_end(i))];
            while let Some(&(_, end)) = runs.last() {
                if c.get(end) == Some(&sep) && c.get(end + 1).is_some_and(char::is_ascii_digit) {
                    runs.push((end + 1, run_end(end + 1)));
                } else {
                    break;
                }
            }
            runs
        };
        let fits = |runs: &[(usize, usize)], widths: [(usize, usize); 3]| {
            runs.len() >= 3 && runs.iter().zip(widths).all(|(&(a, b), (lo, hi))| (lo..=hi).contains(&(b - a)))
        };
        let mut marked = None;
        for (sep, widths) in [('-', [(4, 4), (1, 2), (1, 2)]), ('/', [(1, 4), (1, 2), (1, 4)]), ('.', [(1, 4), (1, 2), (1, 4)])] {
            let runs = chain(sep);
            if fits(&runs, widths) {
                marked = Some(runs[2].1);
                break;
            }
        }
        if marked.is_none() {
            let runs = chain(':');
            if runs.len() >= 2 {
                marked = runs.last().map(|r| r.1);
            }
        }
        let end = marked.unwrap_or_else(|| run_end(i));
        if marked.is_some() {
            kept[i..end].iter_mut().for_each(|k| *k = true);
        }
        i = end;
    }
    kept
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
    pub fn apply_to_result(&self, result: &str) -> String {
        match result.split_once(' ') {
            Some((number, unit)) => format!("{} {unit}", self.apply(number)),
            None => self.apply(result),
        }
    }

    /// A request as it is shown while it is typed: its whole numbers of four digits or more with the
    /// thousands separator, its decimal point as this format writes it (`1000+2500.5` is shown `1,000+2,500.5`).
    /// What is written stays as typed (the core reads `1000`); `Shown` maps the places of one to the other.
    /// Dates (`2026-12-04`, `12/04/2026`, `12.04.2026`), times (`10:30`), fractions, exponents (`1E5000`)
    /// and words' digits (`h1000`) are not numbers to group.
    pub fn shown(&self, request: &str) -> Shown {
        let c: Vec<char> = request.chars().collect();
        let kept = not_numbers(&c);
        let (mut text, mut len) = (String::new(), 0usize);
        // `at[p]`: where the place before the request's character `p` is, in the text shown.
        let mut at = vec![0usize; c.len() + 1];
        let mut i = 0;
        while i < c.len() {
            let before = if i > 0 { c[i - 1] } else { ' ' };
            if c[i].is_ascii_digit() && !kept[i] {
                let end = (i..c.len()).find(|&k| !c[k].is_ascii_digit() || kept[k]).unwrap_or(c.len());
                // The digits of a fraction (`.5000`), an exponent (`1E5000`, `1e-5000`), a word's stay as they are.
                let exponent = matches!(before, '+' | '-') && i >= 3 && matches!(c[i - 2], 'e' | 'E') && c[i - 3].is_ascii_digit();
                let whole = !(before == '.' || before.is_alphabetic() || exponent);
                let digits: String = c[i..end].iter().collect();
                let written = if whole && end - i >= 4 { self.group(&digits) } else { digits };
                let mut k = i;
                for ch in written.chars() {
                    text.push(ch);
                    len += 1;
                    if ch.is_ascii_digit() {
                        k += 1;
                        at[k] = len;
                    }
                }
                i = end;
                continue;
            }
            // A decimal point of a number (not a letter's, as in `St.`, nor a date's).
            let point = c[i] == '.' && !kept[i] && !before.is_alphabetic() && (before.is_ascii_digit() || c.get(i + 1).is_some_and(char::is_ascii_digit));
            let written = if point { self.decimal.clone() } else { c[i].to_string() };
            len += written.chars().count();
            text.push_str(&written);
            at[i + 1] = len;
            i += 1;
        }
        // Back: the request's place of each place shown.
        let mut back = vec![0usize; len + 1];
        let mut p = 0;
        for (s, slot) in back.iter_mut().enumerate() {
            while p < c.len() && at[p + 1] <= s {
                p += 1;
            }
            *slot = p;
        }
        Shown { text, at, back }
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
    if let Some(m) = crate::machine::mocked() {
        return m.format;
    }
    static PLAIN: std::sync::OnceLock<NumberFormat> = std::sync::OnceLock::new();
    PLAIN.get_or_init(NumberFormat::default)
}

/// The device's format, read once (or given by the app embedding aequalis: `set_device`).
#[cfg(not(test))]
pub fn device() -> &'static NumberFormat {
    if let Some(m) = crate::machine::mocked() {
        return m.format;
    }
    DEVICE.get_or_init(device_format)
}

#[cfg_attr(test, allow(dead_code))]
static DEVICE: std::sync::OnceLock<NumberFormat> = std::sync::OnceLock::new();

/// The device's format as the app embedding aequalis knows it (the Android app: from its `Locale`),
/// before any number is formatted; later calls are ignored.
pub fn set_device(format: NumberFormat) {
    let _ = DEVICE.set(format);
}

/// Windows: the Region settings, unless the environment names a locale (`LC_ALL=C` in the tests),
/// which then gets the plain format.
#[cfg(windows)]
pub fn device_format() -> NumberFormat {
    let named = ["LC_ALL", "LC_NUMERIC", "LANG"].iter().any(|v| std::env::var_os(v).is_some_and(|v| !v.is_empty()));
    if named {
        return NumberFormat::default();
    }
    crate::windows::number_format().unwrap_or_default()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
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

    fn separators(decimal: &str, thousands: &str) -> NumberFormat {
        NumberFormat { decimal: decimal.into(), thousands: thousands.into(), grouping: vec![3] }
    }

    #[test]
    fn a_request_shown_with_its_numbers_grouped() {
        let en = |s: &str| separators(".", ",").shown(s).text;
        assert_eq!(en("1000"), "1,000");
        assert_eq!(en("999"), "999");
        assert_eq!(en("1234567"), "1,234,567");
        assert_eq!(en("1000+2500"), "1,000+2,500");
        assert_eq!(en("1000km mi"), "1,000km mi");
        assert_eq!(en("(1000 + 20000) * 3"), "(1,000 + 20,000) * 3");
        assert_eq!(en("1234.5678"), "1,234.5678");
        // The device's separators.
        let it = |s: &str| separators(",", ".").shown(s).text;
        assert_eq!(it("1234.5678"), "1.234,5678");
        assert_eq!(it("5."), "5,");
        assert_eq!(it(".5"), ",5");
        assert_eq!(it("12.04.2026"), "12.04.2026");
        assert_eq!(it("St. Louis"), "St. Louis");
        assert_eq!(separators(".", " ").shown("1234").text, "1 234");
        assert_eq!(separators(".", "").shown("1234").text, "1234");
        // Not numbers to group: fractions, exponents, dates, times, words.
        for same in ["1.23456", "0.12345", "1E5000", "1e-5000", "2026-12-04", "12/04/2026", "10:30", "10:00:0000", "h1000", "18E6 unix"] {
            assert_eq!(en(same), same, "{same}");
        }
        assert_eq!(en("1791644400 unix"), "1,791,644,400 unix");
    }

    #[test]
    fn the_places_follow() {
        let s = separators(".", ",").shown("1000+2");
        assert_eq!(s.text, "1,000+2");
        // The cursor before the `+` (4 in the request) is drawn after `1,000`.
        assert_eq!((s.at[0], s.at[4], s.at[6]), (0, 5, 7));
        // A tap after `1,000` is the request's 4; on the separator, the digit before it.
        assert_eq!((s.back[5], s.back[1], s.back[2], s.back[3], s.back[7]), (4, 1, 1, 2, 6));
        let empty = separators(".", ",").shown("");
        assert_eq!((empty.text.as_str(), empty.at, empty.back), ("", vec![0], vec![0]));
    }
}
