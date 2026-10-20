//! Raw terminal mode, the alternate screen, the window size and a read that gives up after 0.2 s,
//! through the system library that std already links (no crates). `termios` differs per OS, so
//! only the layouts below are supported: Linux on x86, ARM and RISC-V (the asm-generic layout),
//! and macOS. On Windows the console is switched to the same escape sequences (virtual terminal
//! input and output), so everything above this file is the same.

use std::io::Write;

/// Restores the terminal when dropped.
pub struct Guard(());

impl Drop for Guard {
    fn drop(&mut self) {
        leave();
    }
}

#[cfg(any(
    all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64", target_arch = "arm", target_arch = "aarch64", target_arch = "riscv64")),
    target_os = "macos"
))]
mod sys {
    use std::os::raw::{c_int, c_ulong};

    #[cfg(target_os = "linux")]
    mod abi {
        pub type Flag = u32;
        pub const NCCS: usize = 32;
        pub const VTIME: usize = 5;
        pub const VMIN: usize = 6;
        pub const ISIG: Flag = 0o1;
        pub const ICANON: Flag = 0o2;
        pub const ECHO: Flag = 0o10;
        pub const IEXTEN: Flag = 0o100000;
        pub const ICRNL: Flag = 0o400;
        pub const IXON: Flag = 0o2000;
        pub const TIOCGWINSZ: std::os::raw::c_ulong = 0x5413;

        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct Termios {
            pub c_iflag: Flag,
            pub c_oflag: Flag,
            pub c_cflag: Flag,
            pub c_lflag: Flag,
            pub c_line: u8,
            pub c_cc: [u8; NCCS],
            pub c_ispeed: u32,
            pub c_ospeed: u32,
        }
    }

    #[cfg(target_os = "macos")]
    mod abi {
        pub type Flag = u64;
        pub const NCCS: usize = 20;
        pub const VMIN: usize = 16;
        pub const VTIME: usize = 17;
        pub const ISIG: Flag = 0x80;
        pub const ICANON: Flag = 0x100;
        pub const ECHO: Flag = 0x8;
        pub const IEXTEN: Flag = 0x400;
        pub const ICRNL: Flag = 0x100;
        pub const IXON: Flag = 0x200;
        pub const TIOCGWINSZ: std::os::raw::c_ulong = 0x4008_7468;

        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct Termios {
            pub c_iflag: Flag,
            pub c_oflag: Flag,
            pub c_cflag: Flag,
            pub c_lflag: Flag,
            pub c_cc: [u8; NCCS],
            pub c_ispeed: u64,
            pub c_ospeed: u64,
        }
    }

    pub use abi::*;

    #[repr(C)]
    #[derive(Default)]
    pub struct Winsize {
        pub ws_row: u16,
        pub ws_col: u16,
        pub ws_xpixel: u16,
        pub ws_ypixel: u16,
    }

    pub const TCSANOW: c_int = 0;

    // The sizes of `struct termios` and `struct winsize` in the C headers: a wrong layout fails the build.
    #[cfg(target_os = "linux")]
    const _: () = assert!(size_of::<Termios>() == 60);
    #[cfg(target_os = "macos")]
    const _: () = assert!(size_of::<Termios>() == 72);
    const _: () = assert!(size_of::<Winsize>() == 8);

    extern "C" {
        pub fn tcgetattr(fd: c_int, termios: *mut Termios) -> c_int;
        pub fn tcsetattr(fd: c_int, action: c_int, termios: *const Termios) -> c_int;
        pub fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    }
}

#[cfg(any(
    all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64", target_arch = "arm", target_arch = "aarch64", target_arch = "riscv64")),
    target_os = "macos"
))]
mod imp {
    use super::sys::*;
    use std::mem::MaybeUninit;
    use std::sync::OnceLock;

    static ORIGINAL: OnceLock<Termios> = OnceLock::new();

    /// Raw mode: no echo, no line buffering, no signals from Ctrl-C/Ctrl-Z, Enter sends `\r`.
    /// Reads return after 0.2 s without input, so the caller can notice window resizes.
    pub fn raw() -> Result<(), String> {
        let mut t = MaybeUninit::<Termios>::uninit();
        // SAFETY: `Termios` matches this OS's `struct termios`; fd 0 is the terminal.
        if unsafe { tcgetattr(0, t.as_mut_ptr()) } != 0 {
            return Err(format!("cannot read the terminal settings: {}", std::io::Error::last_os_error()));
        }
        // SAFETY: tcgetattr succeeded, so it filled the struct.
        let original = unsafe { t.assume_init() };
        let _ = ORIGINAL.set(original);
        let mut raw = original;
        raw.c_lflag &= !(ECHO | ICANON | ISIG | IEXTEN);
        raw.c_iflag &= !(IXON | ICRNL);
        raw.c_cc[VMIN] = 0;
        raw.c_cc[VTIME] = 2;
        // SAFETY: as above.
        if unsafe { tcsetattr(0, TCSANOW, &raw) } != 0 {
            return Err(format!("cannot switch the terminal to raw mode: {}", std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn restore() {
        if let Some(t) = ORIGINAL.get() {
            // SAFETY: `t` came from tcgetattr.
            unsafe { tcsetattr(0, TCSANOW, t) };
        }
    }

    pub fn size() -> Option<(usize, usize)> {
        let mut ws = Winsize::default();
        // SAFETY: TIOCGWINSZ writes one `struct winsize`.
        let ok = unsafe { ioctl(1, TIOCGWINSZ, &mut ws as *mut Winsize) } == 0;
        (ok && ws.ws_row > 0 && ws.ws_col > 0).then(|| (usize::from(ws.ws_row), usize::from(ws.ws_col)))
    }

    /// What the terminal sent; 0 bytes after 0.2 s without input (`VTIME`).
    pub fn read(buf: &mut [u8]) -> std::io::Result<usize> {
        std::io::Read::read(&mut std::io::stdin(), buf)
    }
}

#[cfg(not(any(
    all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64", target_arch = "arm", target_arch = "aarch64", target_arch = "riscv64")),
    target_os = "macos",
    windows
)))]
mod imp {
    pub fn raw() -> Result<(), String> {
        Err("interactive mode is not supported on this platform".into())
    }

    pub fn restore() {}

    pub fn size() -> Option<(usize, usize)> {
        None
    }

    pub fn read(_: &mut [u8]) -> std::io::Result<usize> {
        Ok(0)
    }
}

/// The Windows console. `SetConsoleMode` gives it what a terminal has: no line editing or echo,
/// no Ctrl-C signal, and the keys as escape sequences (`ENABLE_VIRTUAL_TERMINAL_INPUT`); the
/// screen takes them too (`ENABLE_VIRTUAL_TERMINAL_PROCESSING`). Windows 10 version 1903 or later,
/// in Windows Terminal, PowerShell or the Command Prompt.
#[cfg(windows)]
mod imp {
    use super::win::{self, InputRecord};
    use std::ffi::c_void;
    use std::sync::{Mutex, OnceLock};

    type Handle = *mut c_void;

    const STD_INPUT_HANDLE: u32 = -10i32 as u32;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const ENABLE_PROCESSED_INPUT: u32 = 0x1;
    const ENABLE_LINE_INPUT: u32 = 0x2;
    const ENABLE_ECHO_INPUT: u32 = 0x4;
    const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x200;
    const ENABLE_PROCESSED_OUTPUT: u32 = 0x1;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x4;
    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_TIMEOUT: u32 = 0x102;
    /// The wait for a key before the loop looks at the window size again.
    const WAIT_MS: u32 = 200;
    /// Records read at once.
    const RECORDS: usize = 64;

    #[repr(C)]
    #[derive(Default)]
    struct ScreenBufferInfo {
        size: [i16; 2],
        cursor: [i16; 2],
        attributes: u16,
        /// The visible part of the buffer: left, top, right, bottom (included).
        window: [i16; 4],
        max_window: [i16; 2],
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(which: u32) -> Handle;
        fn GetConsoleMode(console: Handle, mode: *mut u32) -> i32;
        fn SetConsoleMode(console: Handle, mode: u32) -> i32;
        fn GetConsoleScreenBufferInfo(console: Handle, info: *mut ScreenBufferInfo) -> i32;
        fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
        fn ReadConsoleInputW(console: Handle, records: *mut InputRecord, length: u32, read: *mut u32) -> i32;
    }

    /// The input and output modes the console had.
    static ORIGINAL: OnceLock<(u32, u32)> = OnceLock::new();

    /// What was read and not given yet: bytes, and a first half of a surrogate pair.
    static PENDING: Mutex<(Vec<u8>, Option<u16>)> = Mutex::new((Vec::new(), None));

    fn handle(which: u32) -> Handle {
        // SAFETY: GetStdHandle has no preconditions.
        unsafe { GetStdHandle(which) }
    }

    pub fn raw() -> Result<(), String> {
        const NO_CONSOLE: &str = "interactive mode needs a console: run aequalis in Windows Terminal, PowerShell or the Command Prompt";
        let (input, output) = (handle(STD_INPUT_HANDLE), handle(STD_OUTPUT_HANDLE));
        let (mut in_mode, mut out_mode) = (0u32, 0u32);
        // SAFETY: the handles are the process's own; the modes are written to local variables.
        if unsafe { GetConsoleMode(input, &mut in_mode) } == 0 || unsafe { GetConsoleMode(output, &mut out_mode) } == 0 {
            return Err(NO_CONSOLE.into());
        }
        let _ = ORIGINAL.set((in_mode, out_mode));
        let old_console = "this console cannot draw the screen: use Windows 10 version 1903 or later";
        // SAFETY: as above.
        if unsafe { SetConsoleMode(output, out_mode | ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING) } == 0 {
            return Err(old_console.into());
        }
        let raw_in = (in_mode & !(ENABLE_PROCESSED_INPUT | ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT)) | ENABLE_VIRTUAL_TERMINAL_INPUT;
        // SAFETY: as above.
        if unsafe { SetConsoleMode(input, raw_in) } == 0 {
            restore();
            return Err(old_console.into());
        }
        Ok(())
    }

    pub fn restore() {
        if let Some(&(in_mode, out_mode)) = ORIGINAL.get() {
            // SAFETY: the modes came from GetConsoleMode.
            unsafe {
                SetConsoleMode(handle(STD_INPUT_HANDLE), in_mode);
                SetConsoleMode(handle(STD_OUTPUT_HANDLE), out_mode);
            }
        }
    }

    /// Rows and columns of the visible window (not of the buffer, which may hold more lines).
    pub fn size() -> Option<(usize, usize)> {
        let mut info = ScreenBufferInfo::default();
        // SAFETY: the call fills one CONSOLE_SCREEN_BUFFER_INFO, which `ScreenBufferInfo` matches.
        let ok = unsafe { GetConsoleScreenBufferInfo(handle(STD_OUTPUT_HANDLE), &mut info) } != 0;
        let [left, top, right, bottom] = info.window;
        let (rows, cols) = (i32::from(bottom) - i32::from(top) + 1, i32::from(right) - i32::from(left) + 1);
        (ok && rows > 0 && cols > 0).then_some((rows as usize, cols as usize))
    }

    /// What the console sent, as bytes of UTF-8: waits 0.2 s for a key, and 0 bytes after it. The
    /// keys come from the console's own records (`ReadConsoleInputW`), not from a read of the
    /// handle, which would wait for a key while the console had only a resize or a focus to tell.
    pub fn read(buf: &mut [u8]) -> std::io::Result<usize> {
        let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
        if pending.0.is_empty() {
            let input = handle(STD_INPUT_HANDLE);
            // SAFETY: the handle is the console's input.
            match unsafe { WaitForSingleObject(input, WAIT_MS) } {
                WAIT_OBJECT_0 => {}
                WAIT_TIMEOUT => return Ok(0),
                _ => return Err(std::io::Error::last_os_error()),
            }
            let mut records = [InputRecord::default(); RECORDS];
            let mut read = 0u32;
            // SAFETY: `records` has room for RECORDS of them, which is what is asked for.
            if unsafe { ReadConsoleInputW(input, records.as_mut_ptr(), RECORDS as u32, &mut read) } == 0 {
                return Err(std::io::Error::last_os_error());
            }
            let units = win::key_units(&records[..read as usize]);
            let (bytes, high) = &mut *pending;
            win::utf16_to_utf8(&units, high, bytes);
        }
        let n = buf.len().min(pending.0.len());
        buf[..n].copy_from_slice(&pending.0[..n]);
        pending.0.drain(..n);
        Ok(n)
    }
}

/// What reading the Windows console needs that is plain data: the console's records, and UTF-16
/// to UTF-8. Apart from the console so that the tests run anywhere.
#[cfg(any(windows, test))]
mod win {
    /// One `INPUT_RECORD` of the console (20 bytes): its kind, and the 16 bytes of the event as four
    /// words. A key event: the key is down (word 0); the repeat count (low half of word 1) and the
    /// virtual key (high half); the scan code (low half of word 2) and the character as a UTF-16
    /// unit (high half); the modifiers (word 3).
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct InputRecord {
        pub kind: u16,
        _padding: u16,
        pub event: [u32; 4],
    }

    pub const KEY_EVENT: u16 = 1;

    impl InputRecord {
        #[cfg(test)]
        pub fn key(down: bool, repeat: u16, unit: u16) -> Self {
            InputRecord { kind: KEY_EVENT, _padding: 0, event: [u32::from(down), u32::from(repeat), u32::from(unit) << 16, 0] }
        }

        #[cfg(test)]
        pub fn other(kind: u16) -> Self {
            InputRecord { kind, _padding: 0, event: [1, 1, 0x41 << 16, 0] }
        }
    }

    /// The characters typed (UTF-16 units) in the records: the key presses with a character, each
    /// as many times as held. Releases and the console's other events (resize, focus, menu, mouse)
    /// say nothing for the keyboard.
    pub fn key_units(records: &[InputRecord]) -> Vec<u16> {
        let mut units = Vec::new();
        for r in records.iter().filter(|r| r.kind == KEY_EVENT && r.event[0] != 0) {
            let (repeat, unit) = ((r.event[1] & 0xffff) as u16, (r.event[2] >> 16) as u16);
            if unit != 0 {
                units.extend(std::iter::repeat_n(unit, usize::from(repeat.max(1))));
            }
        }
        units
    }

    /// Appends `units` as UTF-8 to `out`. A high surrogate that ends the units waits in `high` for
    /// its pair; one without a pair, and a pair's second half alone, are dropped.
    pub fn utf16_to_utf8(units: &[u16], high: &mut Option<u16>, out: &mut Vec<u8>) {
        let mut buf = [0u8; 4];
        for &unit in units {
            let ch = match (high.take(), unit) {
                (Some(h), 0xdc00..=0xdfff) => char::from_u32(0x10000 + ((u32::from(h) - 0xd800) << 10) + (u32::from(unit) - 0xdc00)),
                (_, 0xd800..=0xdbff) => {
                    *high = Some(unit);
                    None
                }
                (_, 0xdc00..=0xdfff) => None,
                _ => char::from_u32(u32::from(unit)),
            };
            if let Some(ch) = ch {
                out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            }
        }
    }
}

/// Switches to raw mode and the alternate screen until the guard is dropped.
pub fn enter() -> Result<Guard, String> {
    imp::raw()?;
    print_now("\x1b[?1049h");
    Ok(Guard(()))
}

/// Leaves the alternate screen, shows the cursor (default shape) and restores the terminal settings.
pub fn leave() {
    print_now("\x1b[0 q\x1b[?1049l\x1b[?25h");
    imp::restore();
}

/// Rows and columns of the terminal, if known.
pub fn size() -> Option<(usize, usize)> {
    imp::size()
}

/// What the terminal sent (the bytes of the keys): 0 bytes after 0.2 s without input, so the
/// caller can notice a resize, and an error when the terminal is gone.
pub fn read(buf: &mut [u8]) -> std::io::Result<usize> {
    imp::read(buf)
}

fn print_now(s: &str) {
    let mut out = std::io::stdout();
    let _ = out.write_all(s.as_bytes());
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::win::*;

    fn bytes(units: &[u16]) -> Vec<u8> {
        let (mut out, mut high) = (Vec::new(), None);
        utf16_to_utf8(units, &mut high, &mut out);
        out
    }

    #[test]
    fn keys_are_the_presses_with_a_character() {
        let records = [
            InputRecord::key(true, 1, u16::from(b'a')),
            InputRecord::key(false, 1, u16::from(b'a')),
            // Held: three times; a key with no character (Shift alone); other events of the console.
            InputRecord::key(true, 3, u16::from(b'b')),
            InputRecord::key(true, 1, 0),
            InputRecord::other(4),
            InputRecord::other(2),
            InputRecord::key(true, 0, u16::from(b'c')),
        ];
        assert_eq!(key_units(&records), "abbbc".encode_utf16().collect::<Vec<_>>());
        assert!(key_units(&[]).is_empty());
    }

    #[test]
    fn escape_sequences_arrive_as_characters() {
        // The up arrow in VT input mode: ESC [ A, one record each.
        let records: Vec<InputRecord> = "\x1b[A".encode_utf16().map(|u| InputRecord::key(true, 1, u)).collect();
        assert_eq!(bytes(&key_units(&records)), b"\x1b[A");
    }

    #[test]
    fn utf16_becomes_utf8() {
        assert_eq!(bytes(&"m² ° € 𝄞".encode_utf16().collect::<Vec<_>>()), "m² ° € 𝄞".as_bytes());
        // A pair split between two reads is joined; a lone half is dropped.
        let (mut out, mut high) = (Vec::new(), None);
        let units: Vec<u16> = "𝄞".encode_utf16().collect();
        utf16_to_utf8(&units[..1], &mut high, &mut out);
        assert!(out.is_empty() && high.is_some());
        utf16_to_utf8(&units[1..], &mut high, &mut out);
        assert_eq!((out.as_slice(), high), ("𝄞".as_bytes(), None));
        assert_eq!(bytes(&[0xd834, u16::from(b'x')]), b"x", "a high half with no low half");
        assert_eq!(bytes(&[0xdd1e, u16::from(b'x')]), b"x", "a low half with no high half");
    }

    #[test]
    fn the_record_is_the_size_of_the_console_s() {
        assert_eq!(size_of::<InputRecord>(), 20);
    }
}
