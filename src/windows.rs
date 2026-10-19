//! What Windows does differently, through the system DLLs every program already links (no crates):
//! reading a file by position, a background process, the process's memory, the device's number
//! format, date order and measurement system. The device's time zone is the program's to read
//! (`time::set_device_zone`), as on every system. Each function stands in for its Unix counterpart, and is
//! used only where `cfg(windows)` says so.

use std::os::raw::c_int;

/// `FileExt::read_at` as on Unix (`seek_read`: Windows moves the file's cursor, which no reader of
/// a city archive file relies on).
pub trait FileExt {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<usize>;
}

impl FileExt for std::fs::File {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<usize> {
        std::os::windows::fs::FileExt::seek_read(self, buf, offset)
    }
}

/// `CommandExt::process_group` as on Unix: a process of its own group (Ctrl-C in the console does
/// not stop it), with no console window.
pub trait CommandExt {
    fn process_group(&mut self, pgroup: i32) -> &mut Self;
}

impl CommandExt for std::process::Command {
    fn process_group(&mut self, _pgroup: i32) -> &mut Self {
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::os::windows::process::CommandExt::creation_flags(self, CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)
    }
}

type Handle = *mut std::ffi::c_void;

/// `PROCESS_MEMORY_COUNTERS`.
#[repr(C)]
struct MemoryCounters {
    cb: u32,
    page_fault_count: u32,
    peak_working_set: usize,
    working_set: usize,
    rest: [usize; 6],
}

// The sizes in the Windows headers: a wrong layout fails the build.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<MemoryCounters>() == 72);

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> Handle;
    fn K32GetProcessMemoryInfo(process: Handle, counters: *mut MemoryCounters, cb: u32) -> c_int;
    fn GetLocaleInfoEx(locale: *const u16, kind: u32, data: *mut u16, len: c_int) -> c_int;
}

/// The process's peak and current working set, in KiB.
pub fn memory_kb() -> Option<(u64, u64)> {
    let mut counters = MemoryCounters { cb: 0, page_fault_count: 0, peak_working_set: 0, working_set: 0, rest: [0; 6] };
    let cb = std::mem::size_of::<MemoryCounters>() as u32;
    counters.cb = cb;
    // SAFETY: `counters` is a writable `PROCESS_MEMORY_COUNTERS` of `cb` bytes; the handle is the
    // pseudo handle of this process.
    if unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, cb) } == 0 {
        return None;
    }
    Some((counters.peak_working_set as u64 / 1024, counters.working_set as u64 / 1024))
}

/// The text of a NUL-terminated UTF-16 buffer.
fn wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// One value of the user's locale (`GetLocaleInfoEx`), as text.
fn locale_info(kind: u32) -> Option<String> {
    let mut buf = [0u16; 32];
    // SAFETY: null is the user's default locale; `buf` holds `len` UTF-16 units.
    let n = unsafe { GetLocaleInfoEx(std::ptr::null(), kind, buf.as_mut_ptr(), buf.len() as c_int) };
    (n > 0).then(|| wide(&buf))
}

/// The user's decimal point, thousands separator and grouping (Region settings).
pub fn number_format() -> Option<crate::numbers::NumberFormat> {
    const LOCALE_SDECIMAL: u32 = 0x0E;
    const LOCALE_STHOUSAND: u32 = 0x0F;
    const LOCALE_SGROUPING: u32 = 0x10;
    let decimal = locale_info(LOCALE_SDECIMAL).filter(|d| !d.is_empty())?;
    let thousands = locale_info(LOCALE_STHOUSAND).unwrap_or_default();
    let grouping = grouping(&locale_info(LOCALE_SGROUPING).unwrap_or_default());
    Some(crate::numbers::NumberFormat { decimal, thousands, grouping })
}

/// Windows' grouping (`3;0`: threes, repeated; `3;2;0`: three then twos; `3`: one group of three)
/// as `localeconv`'s (the last size repeats; 127 stops).
fn grouping(windows: &str) -> Vec<u8> {
    let mut sizes: Vec<u8> = windows.split(';').filter_map(|s| s.trim().parse().ok()).collect();
    match sizes.last() {
        Some(0) => {
            sizes.pop();
        }
        Some(_) => sizes.push(127),
        None => {}
    }
    sizes
}

/// The short date of the Region settings (`dd/MM/yyyy`) as a `strftime` format (`%d/%m/%Y`): what
/// matters to aequalis is the order of the day, the month and the year.
pub fn date_format() -> Option<String> {
    const LOCALE_SSHORTDATE: u32 = 0x1F;
    Some(strftime_of(&locale_info(LOCALE_SSHORTDATE)?))
}

/// A Windows date pattern (`d`, `dd`, `ddd`, `dddd`, `M`…`MMMM`, `y`…`yyyy`; the rest is literal) as a
/// `strftime` format.
fn strftime_of(pattern: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let run = chars[i..].iter().take_while(|&&d| d == c).count();
        match (c, run) {
            ('y', 1 | 2) => out.push_str("%y"),
            ('y', _) => out.push_str("%Y"),
            ('M', 1 | 2) => out.push_str("%m"),
            ('M', 3) => out.push_str("%b"),
            ('M', _) => out.push_str("%B"),
            ('d', 1 | 2) => out.push_str("%d"),
            ('d', 3) => out.push_str("%a"),
            ('d', _) => out.push_str("%A"),
            _ => out.extend(std::iter::repeat(c).take(run)),
        }
        i += run;
    }
    out
}

/// The measurement system in the Region settings (`LOCALE_IMEASURE`: 0 metric, 1 US).
pub fn measurement() -> Option<crate::config::System> {
    const LOCALE_IMEASURE: u32 = 0x0D;
    match locale_info(LOCALE_IMEASURE)?.as_str() {
        "0" => Some(crate::config::System::Metric),
        "1" => Some(crate::config::System::Imperial),
        _ => None,
    }
}

/// `%APPDATA%` (`C:\Users\…\AppData\Roaming`), where programs keep their settings.
pub fn app_data() -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").filter(|v| !v.is_empty()).map(std::path::PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping_as_localeconv() {
        assert_eq!(grouping("3;0"), [3], "threes, repeated");
        assert_eq!(grouping("3;2;0"), [3, 2], "India");
        assert_eq!(grouping("3"), [3, 127], "one group, then none");
        assert_eq!(grouping(""), Vec::<u8>::new());
    }

    #[test]
    fn short_dates_as_strftime() {
        assert_eq!(strftime_of("dd/MM/yyyy"), "%d/%m/%Y");
        assert_eq!(strftime_of("M/d/yyyy"), "%m/%d/%Y");
        assert_eq!(strftime_of("yyyy-MM-dd"), "%Y-%m-%d");
        assert_eq!(strftime_of("dd.MM.yy"), "%d.%m.%y");
        assert_eq!(strftime_of("ddd d MMM yyyy"), "%a %d %b %Y");
    }

    #[test]
    fn read_by_position() {
        let path = std::env::temp_dir().join(format!("aequalis-windows-{}", std::process::id()));
        std::fs::write(&path, b"hello world").unwrap();
        let file = std::fs::File::open(&path).unwrap();
        let mut buf = [0u8; 5];
        assert_eq!(file.read_at(&mut buf, 6).unwrap(), 5);
        assert_eq!(&buf, b"world");
        drop(file);
        let _ = std::fs::remove_file(&path);
    }
}
