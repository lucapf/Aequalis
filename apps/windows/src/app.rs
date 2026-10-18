//! The program: one hidden window that owns the tray icon (a click opens the input box, a right
//! click its menu), the hot key (Win+Alt+A), and the timers: the device's time zone read again every
//! hour (and at once when Windows says it changed), the online data refreshed in the background.
//! One at a time: a second start (the Start menu) opens the first one's box.

use crate::{device, popup, system, win32::*};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

const CLASS: &str = "Aequalis.Tray";
const WM_TRAY: u32 = WM_APP + 1;
/// Another start of the program asks for the box.
const WM_SHOW: u32 = WM_APP + 2;
/// A download ended.
const WM_SYNCED: u32 = WM_APP + 3;
const TIMER_HOURLY: usize = 1;
const TIMER_FIRST_SYNC: usize = 2;
/// The first background refresh, a while after the start (not during the computer's own start).
const FIRST_SYNC_AFTER_MS: u32 = 20_000;
const HOTKEY_ID: i32 = 1;
const HOTKEY_NAME: &str = "Win+Alt+A";
const ICON_ID: u32 = 1;

const MENU_OPEN: usize = 1;
const MENU_DOWNLOAD: usize = 2;
const MENU_SETTINGS: usize = 3;
const MENU_AUTOSTART: usize = 4;
const MENU_ABOUT: usize = 5;
const MENU_QUIT: usize = 6;

/// The window, for the download thread to say it ended.
static MAIN: AtomicIsize = AtomicIsize::new(0);
static SYNCING: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// Explorer's message when the taskbar is created again (after a crash): the icon is added back.
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
    static INSTANCE: Cell<HANDLE> = const { Cell::new(std::ptr::null_mut()) };
}

pub fn run() -> i32 {
    let autostart = std::env::args().any(|a| a == system::AUTOSTART);
    let class = wide(CLASS);
    // SAFETY: a named mutex, kept for the process's life; the other window is only posted to.
    unsafe {
        let name = wide("Local\\Aequalis.Tray");
        CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let other = FindWindowW(class.as_ptr(), std::ptr::null());
            if !other.is_null() {
                let mut process = 0;
                GetWindowThreadProcessId(other, &mut process);
                AllowSetForegroundWindow(process);
                PostMessageW(other, WM_SHOW, 0, 0);
            }
            return 0;
        }
    }
    // The zones' rules (the registry) and the device's zone; downloads with no console window.
    device::start();
    aequalis::http::register(Box::new(system::Curl));

    // SAFETY: the window class and window live for the program's life.
    let hwnd = unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        INSTANCE.with(|i| i.set(instance));
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: proc_,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: LoadImageW(instance, resource(1), IMAGE_ICON, 0, 0, LR_DEFAULTCOLOR),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        RegisterClassExW(&wc);
        let title = wide("Aequalis");
        // A top-level window (never shown), so that it gets the system's broadcasts.
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        );
        MAIN.store(hwnd as isize, Ordering::SeqCst);
        let taskbar = wide("TaskbarCreated");
        TASKBAR_CREATED.with(|t| t.set(RegisterWindowMessageW(taskbar.as_ptr())));
        let hotkey = RegisterHotKey(hwnd, HOTKEY_ID, MOD_WIN | MOD_ALT | MOD_NOREPEAT, u32::from(b'A')) != 0;
        popup::create(instance, hotkey.then_some(HOTKEY_NAME));
        add_icon(hwnd, NIM_ADD);
        SetTimer(hwnd, TIMER_HOURLY, device::CHECK_EVERY.as_millis() as u32, std::ptr::null());
        SetTimer(hwnd, TIMER_FIRST_SYNC, FIRST_SYNC_AFTER_MS, std::ptr::null());
        hwnd
    };
    // Started by hand (the Start menu, the installer's last page): the box opens at once.
    if !autostart {
        popup::show(anchor(hwnd));
    }
    let mut msg = MSG { hwnd: std::ptr::null_mut(), message: 0, wParam: 0, lParam: 0, time: 0, pt: POINT::default(), lPrivate: 0 };
    // SAFETY: the usual message loop.
    unsafe {
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    0
}

/// The tray icon: added, or added again (`NIM_ADD`), with the current monitor's size.
fn add_icon(hwnd: HWND, message: u32) {
    let instance = INSTANCE.with(|i| i.get());
    // SAFETY: an all-zero NOTIFYICONDATAW is valid; its fields are set before the call.
    unsafe {
        let mut data: NOTIFYICONDATAW = std::mem::zeroed();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = ICON_ID;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        data.uCallbackMessage = WM_TRAY;
        let size = GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForWindow(hwnd).max(96));
        data.hIcon = LoadImageW(instance, resource(1), IMAGE_ICON, size, size, LR_DEFAULTCOLOR);
        let tip = wide(&format!("Aequalis  ·  {HOTKEY_NAME}"));
        data.szTip[..tip.len().min(127)].copy_from_slice(&tip[..tip.len().min(127)]);
        Shell_NotifyIconW(message, &mut data);
        data.uVersion = NOTIFYICON_VERSION_4;
        Shell_NotifyIconW(NIM_SETVERSION, &mut data);
    }
}

fn remove_icon(hwnd: HWND) {
    // SAFETY: as in `add_icon`.
    unsafe {
        let mut data: NOTIFYICONDATAW = std::mem::zeroed();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = ICON_ID;
        Shell_NotifyIconW(NIM_DELETE, &mut data);
    }
}

/// Where the box opens: by the tray icon (else by the mouse).
fn anchor(hwnd: HWND) -> POINT {
    let mut rect = RECT::default();
    let id = NOTIFYICONIDENTIFIER { cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32, hWnd: hwnd, uID: ICON_ID, guidItem: [0; 16] };
    // SAFETY: valid pointers.
    if unsafe { Shell_NotifyIconGetRect(&id, &mut rect) } == 0 {
        return POINT { x: (rect.left + rect.right) / 2, y: (rect.top + rect.bottom) / 2 };
    }
    let mut p = POINT::default();
    // SAFETY: a valid pointer.
    unsafe { GetCursorPos(&mut p) };
    p
}

/// Downloads the online data in the background (`auto`: what the refresh settings say is due, as
/// the command line's background sync), then tells the window.
pub fn download(auto: bool) {
    if SYNCING.swap(true, Ordering::SeqCst) {
        return;
    }
    if !auto {
        popup::data_changed(true);
    }
    std::thread::spawn(move || {
        let _ = aequalis::online::sync(auto);
        SYNCING.store(false, Ordering::SeqCst);
        // SAFETY: posting to the window (it may be gone: then nothing happens).
        unsafe { PostMessageW(MAIN.load(Ordering::SeqCst) as HWND, WM_SYNCED, 0, 0) };
    });
}

/// The settings file in Notepad (written first with the defaults, if there is none yet).
fn edit_settings() {
    let Some(path) = aequalis::settings::path() else { return };
    if !path.exists() {
        let _ = aequalis::settings::save_to(&path, &aequalis::settings::Settings::default());
    }
    system::edit_file(&path);
}

fn menu(hwnd: HWND, at: POINT) {
    // SAFETY: a menu created, shown and destroyed here; the window takes the foreground first, so
    // that the menu closes on a click elsewhere.
    let command = unsafe {
        let menu = CreatePopupMenu();
        let item = |id: usize, flags: u32, text: &str| {
            let text = wide(text);
            AppendMenuW(menu, flags, id, text.as_ptr());
        };
        item(MENU_OPEN, MF_STRING, &format!("Open Aequalis\t{HOTKEY_NAME}"));
        item(0, MF_SEPARATOR, "");
        item(MENU_DOWNLOAD, MF_STRING, "Download the online data now");
        item(MENU_SETTINGS, MF_STRING, "Settings…");
        item(MENU_AUTOSTART, MF_STRING | if system::starts_with_windows() { MF_CHECKED } else { 0 }, "Start with Windows");
        item(0, MF_SEPARATOR, "");
        item(MENU_ABOUT, MF_STRING, "About Aequalis");
        item(MENU_QUIT, MF_STRING, "Quit");
        SetMenuDefaultItem(menu, MENU_OPEN as u32, 0);
        SetForegroundWindow(hwnd);
        let command = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY, at.x, at.y, 0, hwnd, std::ptr::null());
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
        command as usize
    };
    match command {
        MENU_OPEN => popup::show(anchor(hwnd)),
        MENU_DOWNLOAD => download(false),
        MENU_SETTINGS => edit_settings(),
        MENU_AUTOSTART => {
            system::set_starts_with_windows(!system::starts_with_windows());
        }
        MENU_ABOUT => popup::show_about(anchor(hwnd)),
        // SAFETY: a valid window.
        MENU_QUIT => unsafe {
            DestroyWindow(hwnd);
        },
        _ => {}
    }
}

unsafe extern "system" fn proc_(hwnd: HWND, msg: u32, w: usize, l: isize) -> isize {
    match msg {
        WM_TRAY => {
            // NOTIFYICON_VERSION_4: the event in the low word, where it happened in `w`.
            let at = point(w as isize);
            match low(l) {
                NIN_SELECT | NIN_KEYSELECT => popup::toggle(at),
                WM_CONTEXTMENU => menu(hwnd, at),
                _ => {}
            }
            0
        }
        WM_HOTKEY if w == HOTKEY_ID as usize => {
            popup::toggle(anchor(hwnd));
            0
        }
        WM_SHOW => {
            popup::show(anchor(hwnd));
            0
        }
        WM_SYNCED => {
            popup::data_changed(false);
            0
        }
        WM_TIMER => {
            if w == TIMER_FIRST_SYNC {
                KillTimer(hwnd, TIMER_FIRST_SYNC);
            } else {
                device::check();
            }
            download(true);
            0
        }
        // The date, the time or the time zone changed (a setting, a trip): the zone is read again.
        WM_TIMECHANGE => {
            device::check();
            popup::data_changed(SYNCING.load(Ordering::SeqCst));
            0
        }
        // Light or dark.
        WM_SETTINGCHANGE => {
            let area = from_wide_ptr(l as *const u16, 64);
            if area == "ImmersiveColorSet" {
                popup::theme_changed();
            }
            0
        }
        WM_QUERYENDSESSION => 1,
        // Windows shutting down, or an installer replacing the program.
        WM_ENDSESSION if w != 0 => {
            DestroyWindow(hwnd);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            remove_icon(hwnd);
            UnregisterHotKey(hwnd, HOTKEY_ID);
            PostQuitMessage(0);
            0
        }
        _ if msg != 0 && msg == TASKBAR_CREATED.with(|t| t.get()) => {
            add_icon(hwnd, NIM_ADD);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
