//! What the tray app asks of Windows besides its windows: the clipboard, starting with Windows, the
//! light or dark theme, downloads without a console window, opening a file or a page.

use crate::win32::*;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

/// `text` on the clipboard.
pub fn copy(hwnd: HWND, text: &str) -> bool {
    let units = wide(text);
    // SAFETY: the clipboard is opened and closed here; the memory handed to it is its own after
    // SetClipboardData succeeds, and freed here otherwise.
    unsafe {
        if OpenClipboard(hwnd) == 0 {
            return false;
        }
        EmptyClipboard();
        let memory = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2);
        let mut ok = false;
        if !memory.is_null() {
            let target = GlobalLock(memory) as *mut u16;
            if !target.is_null() {
                std::ptr::copy_nonoverlapping(units.as_ptr(), target, units.len());
                GlobalUnlock(memory);
                ok = !SetClipboardData(CF_UNICODETEXT, memory).is_null();
            }
            if !ok {
                GlobalFree(memory);
            }
        }
        CloseClipboard();
        ok
    }
}

// ---------------------------------------------------------------- starting with Windows

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// The value the installer writes too (its "Start with Windows" option).
const RUN_VALUE: &str = "Aequalis";
/// The argument of a start with Windows: no input box at once.
pub const AUTOSTART: &str = "--autostart";

pub fn starts_with_windows() -> bool {
    let (key, value) = (wide(RUN_KEY), wide(RUN_VALUE));
    let mut size = 0u32;
    // SAFETY: valid strings; asking only whether the value exists (no data buffer).
    unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) == 0 }
}

/// Starts this program with Windows (the user's Run key), or no more.
pub fn set_starts_with_windows(on: bool) -> bool {
    let (key, value) = (wide(RUN_KEY), wide(RUN_VALUE));
    let mut run: HANDLE = std::ptr::null_mut();
    // SAFETY: valid strings and out pointer; the key is closed below.
    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, KEY_SET_VALUE, &mut run) != 0 {
            return false;
        }
        let done = if on {
            let Ok(exe) = std::env::current_exe() else {
                RegCloseKey(run);
                return false;
            };
            let command = wide(&format!("\"{}\" {AUTOSTART}", exe.display()));
            RegSetValueExW(run, value.as_ptr(), 0, REG_SZ, command.as_ptr().cast(), (command.len() * 2) as u32) == 0
        } else {
            RegDeleteValueW(run, value.as_ptr()) == 0
        };
        RegCloseKey(run);
        done
    }
}

// ---------------------------------------------------------------- the theme

/// Whether apps are dark (Settings › Personalization › Colors).
pub fn dark_theme() -> bool {
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let value = wide("AppsUseLightTheme");
    let (mut light, mut size) = (1u32, 4u32);
    // SAFETY: valid strings; a DWORD and its size.
    let read = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut light as *mut u32).cast(), &mut size)
    };
    read == 0 && light == 0
}

// ---------------------------------------------------------------- downloads

/// Downloads for the library with Windows' own curl, as the command line does, but without the
/// console window a window-less program would otherwise flash for each one.
pub struct Curl;

impl Curl {
    fn command() -> Command {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // From System32 by its path: never a curl.exe found elsewhere first.
        let system = std::env::var_os("SystemRoot").map(|root| Path::new(&root).join("System32").join("curl.exe"));
        let mut curl = Command::new(system.filter(|p| p.exists()).unwrap_or_else(|| "curl.exe".into()));
        curl.creation_flags(CREATE_NO_WINDOW).stdin(Stdio::null()).stderr(Stdio::null());
        curl
    }
}

impl aequalis::http::Fetcher for Curl {
    fn get(&self, url: &str, seconds: u32, strict: bool) -> Option<Vec<u8>> {
        let out = Self::command().args([if strict { "-sfL" } else { "-sL" }, "--max-time", &seconds.to_string(), url]).output().ok()?;
        (out.status.success() || !strict).then_some(out.stdout)
    }

    fn save(&self, url: &str, to: &Path, seconds: u32) -> bool {
        let mut curl = Self::command();
        curl.args(["-sfL", "--max-time", &seconds.to_string(), "-o"]).arg(to).arg(url).stdout(Stdio::null());
        curl.status().is_ok_and(|s| s.success())
    }
}

// ---------------------------------------------------------------- opening things

/// The full app, `aequalis.exe` (the interactive mode), in a console window of its own (Windows
/// Terminal when it is the default). It sits next to this program; else the one on the PATH.
pub fn open_terminal() {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let beside = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join("aequalis.exe"))).filter(|p| p.exists());
    let program = beside.map_or_else(|| "aequalis.exe".to_string(), |p| p.display().to_string());
    // `start` gives a console program a console of its own; the title is the first quoted word.
    let _ = Command::new("cmd.exe").args(["/c", "start", "Aequalis", &program]).creation_flags(CREATE_NO_WINDOW).spawn();
}

/// A page in the browser.
pub fn open_link(url: &str) {
    let (op, url) = (wide("open"), wide(url));
    // SAFETY: valid strings.
    unsafe { ShellExecuteW(std::ptr::null_mut(), op.as_ptr(), url.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) };
}

/// A text file in Notepad (the settings: no program may be set to open .json files).
pub fn edit_file(path: &Path) {
    let (op, notepad, file) = (wide("open"), wide("notepad.exe"), wide(&format!("\"{}\"", path.display())));
    // SAFETY: valid strings.
    unsafe { ShellExecuteW(std::ptr::null_mut(), op.as_ptr(), notepad.as_ptr(), file.as_ptr(), std::ptr::null(), SW_SHOWNORMAL) };
}
