//! The Windows functions the tray app uses, declared here (no crates), with the structures' sizes
//! checked against the Windows headers at build time.

#![allow(non_snake_case, clippy::upper_case_acronyms)]

use std::ffi::c_void;

pub type HWND = *mut c_void;
pub type HANDLE = *mut c_void;
pub type HDC = *mut c_void;
pub type HFONT = *mut c_void;
pub type HBRUSH = *mut c_void;
pub type HICON = *mut c_void;
pub type HMENU = *mut c_void;
pub type WndProc = unsafe extern "system" fn(HWND, u32, usize, isize) -> isize;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RECT {
    pub fn contains(&self, p: POINT) -> bool {
        p.x >= self.left && p.x < self.right && p.y >= self.top && p.y < self.bottom
    }
}

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: u32,
    pub lpfnWndProc: WndProc,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HANDLE,
    pub hIcon: HICON,
    pub hCursor: HANDLE,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
    pub hIconSm: HICON,
}

#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: usize,
    pub lParam: isize,
    pub time: u32,
    pub pt: POINT,
    pub lPrivate: u32,
}

#[repr(C)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: i32,
    pub rcPaint: RECT,
    pub fRestore: i32,
    pub fIncUpdate: i32,
    pub rgbReserved: [u8; 32],
}

#[repr(C)]
pub struct NOTIFYICONDATAW {
    pub cbSize: u32,
    pub hWnd: HWND,
    pub uID: u32,
    pub uFlags: u32,
    pub uCallbackMessage: u32,
    pub hIcon: HICON,
    pub szTip: [u16; 128],
    pub dwState: u32,
    pub dwStateMask: u32,
    pub szInfo: [u16; 256],
    pub uVersion: u32,
    pub szInfoTitle: [u16; 64],
    pub dwInfoFlags: u32,
    pub guidItem: [u8; 16],
    pub hBalloonIcon: HICON,
}

#[repr(C)]
pub struct NOTIFYICONIDENTIFIER {
    pub cbSize: u32,
    pub hWnd: HWND,
    pub uID: u32,
    pub guidItem: [u8; 16],
}

#[repr(C)]
pub struct MONITORINFO {
    pub cbSize: u32,
    pub rcMonitor: RECT,
    pub rcWork: RECT,
    pub dwFlags: u32,
}

#[repr(C)]
#[derive(Default)]
pub struct SIZE {
    pub cx: i32,
    pub cy: i32,
}

const _: () = assert!(std::mem::size_of::<WNDCLASSEXW>() == 80);
const _: () = assert!(std::mem::size_of::<MSG>() == 48);
const _: () = assert!(std::mem::size_of::<PAINTSTRUCT>() == 72);
const _: () = assert!(std::mem::size_of::<NOTIFYICONDATAW>() == 976);
const _: () = assert!(std::mem::size_of::<MONITORINFO>() == 40);
const _: () = assert!(std::mem::size_of::<NOTIFYICONIDENTIFIER>() == 40);

// Messages.
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_ACTIVATE: u32 = 0x0006;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_QUERYENDSESSION: u32 = 0x0011;
pub const WM_ERASEBKGND: u32 = 0x0014;
pub const WM_SETTINGCHANGE: u32 = 0x001A;
pub const WM_TIMECHANGE: u32 = 0x001E;
pub const WM_SETCURSOR: u32 = 0x0020;
pub const WM_ENDSESSION: u32 = 0x0016;
pub const WM_SETFONT: u32 = 0x0030;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_CHAR: u32 = 0x0102;
pub const WM_COMMAND: u32 = 0x0111;
pub const WM_TIMER: u32 = 0x0113;
pub const WM_CTLCOLOREDIT: u32 = 0x0133;
pub const WM_LBUTTONUP: u32 = 0x0202;
pub const WM_CONTEXTMENU: u32 = 0x007B;
pub const WM_HOTKEY: u32 = 0x0312;
pub const WM_DPICHANGED: u32 = 0x02E0;
pub const WM_NULL: u32 = 0x0000;
pub const WM_USER: u32 = 0x0400;
pub const WM_APP: u32 = 0x8000;
pub const EN_CHANGE: u32 = 0x0300;
pub const EM_SETSEL: u32 = 0x00B1;
pub const EM_SETCUEBANNER: u32 = 0x1501;

// Window styles.
pub const WS_POPUP: u32 = 0x8000_0000;
pub const WS_CHILD: u32 = 0x4000_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;
pub const WS_TABSTOP: u32 = 0x0001_0000;
pub const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
pub const WS_EX_TOPMOST: u32 = 0x0000_0008;
pub const ES_AUTOHSCROLL: u32 = 0x0080;
pub const CS_DROPSHADOW: u32 = 0x0002_0000;
pub const SW_HIDE: i32 = 0;
pub const SW_SHOW: i32 = 5;
pub const SW_SHOWNORMAL: i32 = 1;
pub const SWP_NOACTIVATE: u32 = 0x0010;
pub const HWND_TOPMOST: HWND = -1isize as HWND;
pub const GWLP_WNDPROC: i32 = -4;
pub const WA_INACTIVE: usize = 0;
pub const VK_RETURN: usize = 0x0D;
pub const VK_ESCAPE: usize = 0x1B;

// The notification area.
pub const NIM_ADD: u32 = 0;
pub const NIM_DELETE: u32 = 2;
pub const NIM_SETVERSION: u32 = 4;
pub const NIF_MESSAGE: u32 = 0x1;
pub const NIF_ICON: u32 = 0x2;
pub const NIF_TIP: u32 = 0x4;
pub const NIF_SHOWTIP: u32 = 0x80;
pub const NOTIFYICON_VERSION_4: u32 = 4;
pub const NIN_SELECT: u32 = WM_USER;
pub const NIN_KEYSELECT: u32 = WM_USER + 1;

// Menus.
pub const MF_STRING: u32 = 0x0;
pub const MF_SEPARATOR: u32 = 0x800;
pub const MF_CHECKED: u32 = 0x8;
pub const TPM_RIGHTBUTTON: u32 = 0x2;
pub const TPM_RETURNCMD: u32 = 0x100;
pub const TPM_NONOTIFY: u32 = 0x80;

// Hot keys.
pub const MOD_ALT: u32 = 0x1;
pub const MOD_WIN: u32 = 0x8;
pub const MOD_NOREPEAT: u32 = 0x4000;

// Images, cursors, drawing.
pub const IMAGE_ICON: u32 = 1;
pub const LR_DEFAULTCOLOR: u32 = 0;
pub const IDC_ARROW: usize = 32512;
pub const IDC_HAND: usize = 32649;
pub const DT_LEFT: u32 = 0x0;
pub const DT_VCENTER: u32 = 0x4;
pub const DT_SINGLELINE: u32 = 0x20;
pub const DT_WORDBREAK: u32 = 0x10;
pub const DT_CALCRECT: u32 = 0x400;
pub const DT_NOPREFIX: u32 = 0x800;
pub const DT_END_ELLIPSIS: u32 = 0x8000;
pub const DT_EDITCONTROL: u32 = 0x2000;
pub const TRANSPARENT: i32 = 1;
pub const SRCCOPY: u32 = 0x00CC_0020;
pub const FW_NORMAL: i32 = 400;
pub const FW_SEMIBOLD: i32 = 600;
pub const DEFAULT_CHARSET: u32 = 1;
pub const CLEARTYPE_QUALITY: u32 = 5;
pub const NULL_PEN: i32 = 8;
pub const DC_BRUSH: i32 = 18;
pub const DI_NORMAL: u32 = 3;
pub const MONITOR_DEFAULTTONEAREST: u32 = 2;
pub const SM_CXSMICON: i32 = 49;

// The clipboard, memory.
pub const CF_UNICODETEXT: u32 = 13;
pub const GMEM_MOVEABLE: u32 = 0x2;

// The registry.
pub const HKEY_CURRENT_USER: HANDLE = 0x8000_0001u32 as isize as HANDLE;
pub const KEY_SET_VALUE: u32 = 0x2;
pub const REG_SZ: u32 = 1;
pub const RRF_RT_REG_SZ: u32 = 0x2;
pub const RRF_RT_REG_DWORD: u32 = 0x10;

// DWM.
pub const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
pub const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
pub const DWMWA_BORDER_COLOR: u32 = 34;
pub const DWMWCP_ROUND: u32 = 2;

pub const ERROR_ALREADY_EXISTS: u32 = 183;

#[link(name = "user32")]
extern "system" {
    pub fn RegisterClassExW(class: *const WNDCLASSEXW) -> u16;
    pub fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: HWND,
        menu: HMENU,
        instance: HANDLE,
        param: *mut c_void,
    ) -> HWND;
    pub fn DefWindowProcW(hwnd: HWND, msg: u32, w: usize, l: isize) -> isize;
    pub fn CallWindowProcW(proc_: isize, hwnd: HWND, msg: u32, w: usize, l: isize) -> isize;
    pub fn SetWindowLongPtrW(hwnd: HWND, index: i32, value: isize) -> isize;
    pub fn GetMessageW(msg: *mut MSG, hwnd: HWND, min: u32, max: u32) -> i32;
    pub fn TranslateMessage(msg: *const MSG) -> i32;
    pub fn DispatchMessageW(msg: *const MSG) -> isize;
    pub fn PostQuitMessage(code: i32);
    pub fn PostMessageW(hwnd: HWND, msg: u32, w: usize, l: isize) -> i32;
    pub fn SendMessageW(hwnd: HWND, msg: u32, w: usize, l: isize) -> isize;
    pub fn ShowWindow(hwnd: HWND, cmd: i32) -> i32;
    pub fn IsWindowVisible(hwnd: HWND) -> i32;
    pub fn SetWindowPos(hwnd: HWND, after: HWND, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
    pub fn MoveWindow(hwnd: HWND, x: i32, y: i32, w: i32, h: i32, repaint: i32) -> i32;
    pub fn SetForegroundWindow(hwnd: HWND) -> i32;
    pub fn AllowSetForegroundWindow(process: u32) -> i32;
    pub fn GetWindowThreadProcessId(hwnd: HWND, process: *mut u32) -> u32;
    pub fn SetFocus(hwnd: HWND) -> HWND;
    pub fn DestroyWindow(hwnd: HWND) -> i32;
    pub fn FindWindowW(class: *const u16, name: *const u16) -> HWND;
    pub fn SetWindowTextW(hwnd: HWND, text: *const u16) -> i32;
    pub fn GetWindowTextW(hwnd: HWND, text: *mut u16, max: i32) -> i32;
    pub fn GetWindowTextLengthW(hwnd: HWND) -> i32;
    pub fn InvalidateRect(hwnd: HWND, rect: *const RECT, erase: i32) -> i32;
    pub fn BeginPaint(hwnd: HWND, ps: *mut PAINTSTRUCT) -> HDC;
    pub fn EndPaint(hwnd: HWND, ps: *const PAINTSTRUCT) -> i32;
    pub fn GetDC(hwnd: HWND) -> HDC;
    pub fn ReleaseDC(hwnd: HWND, hdc: HDC) -> i32;
    pub fn GetClientRect(hwnd: HWND, rect: *mut RECT) -> i32;
    pub fn FillRect(hdc: HDC, rect: *const RECT, brush: HBRUSH) -> i32;
    pub fn DrawTextW(hdc: HDC, text: *const u16, len: i32, rect: *mut RECT, format: u32) -> i32;
    pub fn DrawIconEx(hdc: HDC, x: i32, y: i32, icon: HICON, w: i32, h: i32, step: u32, brush: HBRUSH, flags: u32) -> i32;
    pub fn SetTimer(hwnd: HWND, id: usize, ms: u32, func: *const c_void) -> usize;
    pub fn KillTimer(hwnd: HWND, id: usize) -> i32;
    pub fn RegisterHotKey(hwnd: HWND, id: i32, modifiers: u32, vk: u32) -> i32;
    pub fn UnregisterHotKey(hwnd: HWND, id: i32) -> i32;
    pub fn LoadImageW(instance: HANDLE, name: *const u16, kind: u32, cx: i32, cy: i32, flags: u32) -> HANDLE;
    pub fn LoadCursorW(instance: HANDLE, name: *const u16) -> HANDLE;
    pub fn SetCursor(cursor: HANDLE) -> HANDLE;
    pub fn CreatePopupMenu() -> HMENU;
    pub fn AppendMenuW(menu: HMENU, flags: u32, id: usize, text: *const u16) -> i32;
    pub fn SetMenuDefaultItem(menu: HMENU, item: u32, by_position: u32) -> i32;
    pub fn TrackPopupMenu(menu: HMENU, flags: u32, x: i32, y: i32, reserved: i32, hwnd: HWND, rect: *const RECT) -> i32;
    pub fn DestroyMenu(menu: HMENU) -> i32;
    pub fn GetCursorPos(p: *mut POINT) -> i32;
    pub fn ScreenToClient(hwnd: HWND, p: *mut POINT) -> i32;
    pub fn MonitorFromPoint(p: POINT, flags: u32) -> HANDLE;
    pub fn GetMonitorInfoW(monitor: HANDLE, info: *mut MONITORINFO) -> i32;
    pub fn GetDpiForWindow(hwnd: HWND) -> u32;
    pub fn GetSystemMetricsForDpi(index: i32, dpi: u32) -> i32;
    pub fn RegisterWindowMessageW(name: *const u16) -> u32;
    pub fn OpenClipboard(hwnd: HWND) -> i32;
    pub fn EmptyClipboard() -> i32;
    pub fn SetClipboardData(format: u32, data: HANDLE) -> HANDLE;
    pub fn CloseClipboard() -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn CreateFontW(
        h: i32,
        w: i32,
        escapement: i32,
        orientation: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strike: u32,
        charset: u32,
        out_precision: u32,
        clip_precision: u32,
        quality: u32,
        pitch: u32,
        face: *const u16,
    ) -> HFONT;
    pub fn SelectObject(hdc: HDC, object: HANDLE) -> HANDLE;
    pub fn DeleteObject(object: HANDLE) -> i32;
    pub fn GetStockObject(index: i32) -> HANDLE;
    pub fn SetTextColor(hdc: HDC, color: u32) -> u32;
    pub fn SetBkColor(hdc: HDC, color: u32) -> u32;
    pub fn SetBkMode(hdc: HDC, mode: i32) -> i32;
    pub fn CreateSolidBrush(color: u32) -> HBRUSH;
    pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
    pub fn CreateCompatibleBitmap(hdc: HDC, w: i32, h: i32) -> HANDLE;
    pub fn BitBlt(dst: HDC, x: i32, y: i32, w: i32, h: i32, src: HDC, sx: i32, sy: i32, rop: u32) -> i32;
    pub fn DeleteDC(hdc: HDC) -> i32;
    pub fn RoundRect(hdc: HDC, l: i32, t: i32, r: i32, b: i32, w: i32, h: i32) -> i32;
    pub fn GetTextExtentPoint32W(hdc: HDC, text: *const u16, len: i32, size: *mut SIZE) -> i32;
    pub fn SetDCBrushColor(hdc: HDC, color: u32) -> u32;
}

#[link(name = "kernel32")]
extern "system" {
    pub fn GetModuleHandleW(name: *const u16) -> HANDLE;
    pub fn CreateMutexW(attributes: *mut c_void, owner: i32, name: *const u16) -> HANDLE;
    pub fn GetLastError() -> u32;
    pub fn GlobalAlloc(flags: u32, bytes: usize) -> HANDLE;
    pub fn GlobalLock(memory: HANDLE) -> *mut c_void;
    pub fn GlobalUnlock(memory: HANDLE) -> i32;
    pub fn GlobalFree(memory: HANDLE) -> HANDLE;
}

#[link(name = "advapi32")]
extern "system" {
    pub fn RegOpenKeyExW(key: HANDLE, subkey: *const u16, options: u32, access: u32, result: *mut HANDLE) -> i32;
    pub fn RegSetValueExW(key: HANDLE, name: *const u16, reserved: u32, kind: u32, data: *const u8, size: u32) -> i32;
    pub fn RegDeleteValueW(key: HANDLE, name: *const u16) -> i32;
    pub fn RegCloseKey(key: HANDLE) -> i32;
    pub fn RegGetValueW(key: HANDLE, subkey: *const u16, value: *const u16, flags: u32, kind: *mut u32, data: *mut c_void, size: *mut u32) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    pub fn Shell_NotifyIconW(message: u32, data: *mut NOTIFYICONDATAW) -> i32;
    pub fn Shell_NotifyIconGetRect(id: *const NOTIFYICONIDENTIFIER, rect: *mut RECT) -> i32;
    pub fn ShellExecuteW(hwnd: HWND, op: *const u16, file: *const u16, params: *const u16, dir: *const u16, show: i32) -> HANDLE;
}

#[link(name = "dwmapi")]
extern "system" {
    pub fn DwmSetWindowAttribute(hwnd: HWND, attribute: u32, value: *const c_void, size: u32) -> i32;
}

/// `text` as a NUL-terminated UTF-16 string.
pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

/// The text of a UTF-16 buffer, up to its NUL.
pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// The text of a NUL-terminated UTF-16 string a message points to (at most `max` units read).
///
/// # Safety
/// `p` is null or points to a NUL-terminated string.
pub unsafe fn from_wide_ptr(p: *const u16, max: usize) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    while len < max && *p.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
}

/// `MAKEINTRESOURCE`.
pub fn resource(id: usize) -> *const u16 {
    id as *const u16
}

/// A colour as Windows wants it (`0x00BBGGRR`) from `0xRRGGBB`.
pub const fn rgb(hex: u32) -> u32 {
    ((hex & 0xFF) << 16) | (hex & 0xFF00) | (hex >> 16)
}

/// The low and high words of a message's parameter.
pub fn low(v: isize) -> u32 {
    (v as usize & 0xFFFF) as u32
}

pub fn high(v: isize) -> u32 {
    ((v as usize >> 16) & 0xFFFF) as u32
}

/// A point from a mouse message's parameter (signed: several monitors).
pub fn point(l: isize) -> POINT {
    POINT { x: (l & 0xFFFF) as i16 as i32, y: ((l >> 16) & 0xFFFF) as i16 as i32 }
}
