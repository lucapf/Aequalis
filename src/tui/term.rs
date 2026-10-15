//! Raw terminal mode, the alternate screen and the window size, through the C library that std
//! already links (no crates). `termios` differs per OS, so only the layouts below are supported:
//! Linux on x86, ARM and RISC-V (the asm-generic layout), and macOS.

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
}

#[cfg(not(any(
    all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64", target_arch = "arm", target_arch = "aarch64", target_arch = "riscv64")),
    target_os = "macos"
)))]
mod imp {
    pub fn raw() -> Result<(), String> {
        Err("interactive mode is not supported on this platform".into())
    }

    pub fn restore() {}

    pub fn size() -> Option<(usize, usize)> {
        None
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

fn print_now(s: &str) {
    let mut out = std::io::stdout();
    let _ = out.write_all(s.as_bytes());
    let _ = out.flush();
}
