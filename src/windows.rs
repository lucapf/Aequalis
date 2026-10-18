//! What Windows does differently, through the system DLLs every program already links (no crates):
//! reading a file by position, a background process, the process's memory, the device's time zone,
//! number format and measurement system. Each function stands in for its Unix counterpart, and is
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

/// `DYNAMIC_TIME_ZONE_INFORMATION`: only the key name is read.
#[repr(C)]
struct ZoneInformation {
    bias: i32,
    standard_name: [u16; 32],
    standard_date: [u16; 8],
    standard_bias: i32,
    daylight_name: [u16; 32],
    daylight_date: [u16; 8],
    daylight_bias: i32,
    key_name: [u16; 128],
    dynamic_daylight_disabled: u8,
}

// The sizes in the Windows headers: a wrong layout fails the build.
const _: () = assert!(std::mem::size_of::<ZoneInformation>() == 432);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<MemoryCounters>() == 72);

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> Handle;
    fn K32GetProcessMemoryInfo(process: Handle, counters: *mut MemoryCounters, cb: u32) -> c_int;
    fn GetDynamicTimeZoneInformation(info: *mut ZoneInformation) -> u32;
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

/// The time zone in the Windows settings, by its IANA name (`W. Europe Standard Time` →
/// `Europe/Berlin`); `None` when Windows names one this table doesn't know.
pub fn local_zone() -> Option<String> {
    let mut info = ZoneInformation {
        bias: 0,
        standard_name: [0; 32],
        standard_date: [0; 8],
        standard_bias: 0,
        daylight_name: [0; 32],
        daylight_date: [0; 8],
        daylight_bias: 0,
        key_name: [0; 128],
        dynamic_daylight_disabled: 0,
    };
    // SAFETY: `info` is a writable `DYNAMIC_TIME_ZONE_INFORMATION`.
    if unsafe { GetDynamicTimeZoneInformation(&mut info) } == u32::MAX {
        return None;
    }
    iana_zone(&wide(&info.key_name)).map(str::to_string)
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
/// matters to tally is the order of the day, the month and the year.
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

/// The zone database next to the program (`<folder of aequalis.exe>\zoneinfo`): Windows has none of
/// its own in the IANA format, so the release ships it.
pub fn zoneinfo() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join("zoneinfo").to_string_lossy().into_owned())
}

/// `%APPDATA%` (`C:\Users\…\AppData\Roaming`), where programs keep their settings.
pub fn app_data() -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").filter(|v| !v.is_empty()).map(std::path::PathBuf::from)
}

/// The IANA zone of a Windows time zone (CLDR `windowsZones.xml`, the `001` territory of each).
pub fn iana_zone(windows: &str) -> Option<&'static str> {
    ZONES.iter().find(|(w, _)| *w == windows).map(|&(_, iana)| iana)
}

const ZONES: &[(&str, &str)] = &[
    ("Dateline Standard Time", "Etc/GMT+12"),
    ("UTC-11", "Etc/GMT+11"),
    ("Aleutian Standard Time", "America/Adak"),
    ("Hawaiian Standard Time", "Pacific/Honolulu"),
    ("Marquesas Standard Time", "Pacific/Marquesas"),
    ("Alaskan Standard Time", "America/Anchorage"),
    ("UTC-09", "Etc/GMT+9"),
    ("Pacific Standard Time (Mexico)", "America/Tijuana"),
    ("UTC-08", "Etc/GMT+8"),
    ("Pacific Standard Time", "America/Los_Angeles"),
    ("US Mountain Standard Time", "America/Phoenix"),
    ("Mountain Standard Time (Mexico)", "America/Mazatlan"),
    ("Mountain Standard Time", "America/Denver"),
    ("Yukon Standard Time", "America/Whitehorse"),
    ("Central America Standard Time", "America/Guatemala"),
    ("Central Standard Time", "America/Chicago"),
    ("Easter Island Standard Time", "Pacific/Easter"),
    ("Central Standard Time (Mexico)", "America/Mexico_City"),
    ("Canada Central Standard Time", "America/Regina"),
    ("SA Pacific Standard Time", "America/Bogota"),
    ("Eastern Standard Time (Mexico)", "America/Cancun"),
    ("Eastern Standard Time", "America/New_York"),
    ("Haiti Standard Time", "America/Port-au-Prince"),
    ("Cuba Standard Time", "America/Havana"),
    ("US Eastern Standard Time", "America/Indiana/Indianapolis"),
    ("Turks And Caicos Standard Time", "America/Grand_Turk"),
    ("Paraguay Standard Time", "America/Asuncion"),
    ("Atlantic Standard Time", "America/Halifax"),
    ("Venezuela Standard Time", "America/Caracas"),
    ("Central Brazilian Standard Time", "America/Cuiaba"),
    ("SA Western Standard Time", "America/La_Paz"),
    ("Pacific SA Standard Time", "America/Santiago"),
    ("Newfoundland Standard Time", "America/St_Johns"),
    ("Tocantins Standard Time", "America/Araguaina"),
    ("E. South America Standard Time", "America/Sao_Paulo"),
    ("SA Eastern Standard Time", "America/Cayenne"),
    ("Argentina Standard Time", "America/Argentina/Buenos_Aires"),
    ("Greenland Standard Time", "America/Nuuk"),
    ("Montevideo Standard Time", "America/Montevideo"),
    ("Magallanes Standard Time", "America/Punta_Arenas"),
    ("Saint Pierre Standard Time", "America/Miquelon"),
    ("Bahia Standard Time", "America/Bahia"),
    ("UTC-02", "Etc/GMT+2"),
    ("Mid-Atlantic Standard Time", "Etc/GMT+2"),
    ("Azores Standard Time", "Atlantic/Azores"),
    ("Cape Verde Standard Time", "Atlantic/Cape_Verde"),
    ("UTC", "Etc/UTC"),
    ("GMT Standard Time", "Europe/London"),
    ("Greenwich Standard Time", "Atlantic/Reykjavik"),
    ("Sao Tome Standard Time", "Africa/Sao_Tome"),
    ("Morocco Standard Time", "Africa/Casablanca"),
    ("W. Europe Standard Time", "Europe/Berlin"),
    ("Central Europe Standard Time", "Europe/Budapest"),
    ("Romance Standard Time", "Europe/Paris"),
    ("Central European Standard Time", "Europe/Warsaw"),
    ("W. Central Africa Standard Time", "Africa/Lagos"),
    ("Jordan Standard Time", "Asia/Amman"),
    ("GTB Standard Time", "Europe/Bucharest"),
    ("Middle East Standard Time", "Asia/Beirut"),
    ("Egypt Standard Time", "Africa/Cairo"),
    ("E. Europe Standard Time", "Europe/Chisinau"),
    ("Syria Standard Time", "Asia/Damascus"),
    ("West Bank Standard Time", "Asia/Hebron"),
    ("South Africa Standard Time", "Africa/Johannesburg"),
    ("FLE Standard Time", "Europe/Kyiv"),
    ("Israel Standard Time", "Asia/Jerusalem"),
    ("South Sudan Standard Time", "Africa/Juba"),
    ("Kaliningrad Standard Time", "Europe/Kaliningrad"),
    ("Sudan Standard Time", "Africa/Khartoum"),
    ("Libya Standard Time", "Africa/Tripoli"),
    ("Namibia Standard Time", "Africa/Windhoek"),
    ("Arabic Standard Time", "Asia/Baghdad"),
    ("Turkey Standard Time", "Europe/Istanbul"),
    ("Arab Standard Time", "Asia/Riyadh"),
    ("Belarus Standard Time", "Europe/Minsk"),
    ("Russian Standard Time", "Europe/Moscow"),
    ("E. Africa Standard Time", "Africa/Nairobi"),
    ("Volgograd Standard Time", "Europe/Volgograd"),
    ("Iran Standard Time", "Asia/Tehran"),
    ("Arabian Standard Time", "Asia/Dubai"),
    ("Astrakhan Standard Time", "Europe/Astrakhan"),
    ("Azerbaijan Standard Time", "Asia/Baku"),
    ("Russia Time Zone 3", "Europe/Samara"),
    ("Mauritius Standard Time", "Indian/Mauritius"),
    ("Saratov Standard Time", "Europe/Saratov"),
    ("Georgian Standard Time", "Asia/Tbilisi"),
    ("Caucasus Standard Time", "Asia/Yerevan"),
    ("Afghanistan Standard Time", "Asia/Kabul"),
    ("West Asia Standard Time", "Asia/Tashkent"),
    ("Qyzylorda Standard Time", "Asia/Qyzylorda"),
    ("Ekaterinburg Standard Time", "Asia/Yekaterinburg"),
    ("Pakistan Standard Time", "Asia/Karachi"),
    ("India Standard Time", "Asia/Kolkata"),
    ("Sri Lanka Standard Time", "Asia/Colombo"),
    ("Nepal Standard Time", "Asia/Kathmandu"),
    ("Central Asia Standard Time", "Asia/Bishkek"),
    ("Bangladesh Standard Time", "Asia/Dhaka"),
    ("Omsk Standard Time", "Asia/Omsk"),
    ("Myanmar Standard Time", "Asia/Yangon"),
    ("SE Asia Standard Time", "Asia/Bangkok"),
    ("Altai Standard Time", "Asia/Barnaul"),
    ("W. Mongolia Standard Time", "Asia/Hovd"),
    ("North Asia Standard Time", "Asia/Krasnoyarsk"),
    ("N. Central Asia Standard Time", "Asia/Novosibirsk"),
    ("Tomsk Standard Time", "Asia/Tomsk"),
    ("China Standard Time", "Asia/Shanghai"),
    ("North Asia East Standard Time", "Asia/Irkutsk"),
    ("Singapore Standard Time", "Asia/Singapore"),
    ("W. Australia Standard Time", "Australia/Perth"),
    ("Taipei Standard Time", "Asia/Taipei"),
    ("Ulaanbaatar Standard Time", "Asia/Ulaanbaatar"),
    ("Aus Central W. Standard Time", "Australia/Eucla"),
    ("Transbaikal Standard Time", "Asia/Chita"),
    ("Tokyo Standard Time", "Asia/Tokyo"),
    ("North Korea Standard Time", "Asia/Pyongyang"),
    ("Korea Standard Time", "Asia/Seoul"),
    ("Yakutsk Standard Time", "Asia/Yakutsk"),
    ("Cen. Australia Standard Time", "Australia/Adelaide"),
    ("AUS Central Standard Time", "Australia/Darwin"),
    ("E. Australia Standard Time", "Australia/Brisbane"),
    ("AUS Eastern Standard Time", "Australia/Sydney"),
    ("West Pacific Standard Time", "Pacific/Port_Moresby"),
    ("Tasmania Standard Time", "Australia/Hobart"),
    ("Vladivostok Standard Time", "Asia/Vladivostok"),
    ("Lord Howe Standard Time", "Australia/Lord_Howe"),
    ("Bougainville Standard Time", "Pacific/Bougainville"),
    ("Russia Time Zone 10", "Asia/Srednekolymsk"),
    ("Magadan Standard Time", "Asia/Magadan"),
    ("Norfolk Standard Time", "Pacific/Norfolk"),
    ("Sakhalin Standard Time", "Asia/Sakhalin"),
    ("Central Pacific Standard Time", "Pacific/Guadalcanal"),
    ("Russia Time Zone 11", "Asia/Kamchatka"),
    ("New Zealand Standard Time", "Pacific/Auckland"),
    ("UTC+12", "Etc/GMT-12"),
    ("Fiji Standard Time", "Pacific/Fiji"),
    ("Kamchatka Standard Time", "Asia/Kamchatka"),
    ("Chatham Islands Standard Time", "Pacific/Chatham"),
    ("UTC+13", "Etc/GMT-13"),
    ("Tonga Standard Time", "Pacific/Tongatapu"),
    ("Samoa Standard Time", "Pacific/Apia"),
    ("Line Islands Standard Time", "Pacific/Kiritimati"),
];

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
    fn zones_named() {
        assert_eq!(iana_zone("W. Europe Standard Time"), Some("Europe/Berlin"));
        assert_eq!(iana_zone("Nowhere Standard Time"), None);
        let mut names: Vec<_> = ZONES.iter().map(|(w, _)| w).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), ZONES.len(), "each Windows zone once");
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
