//! The input box: a window by the notification area, as the Omarchy widget's popup. What it shows of
//! a request is the library's reply for a UI (`reply::view`): the result, a hint, an error, or a
//! question with one button per choice; Enter copies a result (or takes a question's default), Esc
//! or a click elsewhere closes it. Below: notices, what was copied, the recent requests. A click on
//! the title shows About.
//!
//! The window state lives in one place (`STATE`), borrowed only between Windows calls: a call that
//! sends a message back (setting the text, showing the window) is made with nothing borrowed.

use crate::system;
use crate::win32::*;
use aequalis::json::{self, Value};
use aequalis::reply;
use std::cell::{Cell, RefCell};
use std::time::Instant;

pub const CLASS: &str = "Aequalis.Popup";
/// The evaluation, a moment after the last key (several keys typed fast: one evaluation).
const TIMER_EVALUATE: usize = 10;
const EVALUATE_AFTER_MS: u32 = 60;
const RECENT: usize = 6;

/// The window's handles, set once.
#[derive(Clone, Copy)]
struct Handles {
    popup: HWND,
    edit: HWND,
    instance: HANDLE,
}

thread_local! {
    static HANDLES: Cell<Option<Handles>> = const { Cell::new(None) };
    static STATE: RefCell<State> = RefCell::new(State::default());
    /// The edit control's own procedure (it is subclassed for Enter and Esc).
    static EDIT_PROC: Cell<isize> = const { Cell::new(0) };
}

fn handles() -> Handles {
    HANDLES.with(|h| h.get()).expect("the popup is created first")
}

/// What the reply says to show.
#[derive(Default, Clone, Debug)]
struct Reply {
    show: String,
    message: String,
    result: String,
    copy: String,
    details: Vec<String>,
    rates: Option<String>,
    grouping: Option<String>,
    default: Option<String>,
    choices: Vec<Choice>,
}

#[derive(Clone, Debug)]
struct Choice {
    label: String,
    request: String,
}

impl Reply {
    fn parse(text: &str) -> Option<Reply> {
        let v = json::parse(text).ok()?;
        let s = |key: &str| v.get(key).and_then(Value::str).map(String::from);
        let choices = v
            .get("choices")
            .map(|c| c.array().iter().filter_map(|c| Some(Choice { label: c.get("label")?.str()?.into(), request: c.get("request")?.str()?.into() })).collect())
            .unwrap_or_default();
        Some(Reply {
            show: s("show")?,
            message: s("message").unwrap_or_default(),
            result: s("result").unwrap_or_default(),
            copy: s("copy").unwrap_or_default(),
            details: v.get("details").map(Value::strings).unwrap_or_default(),
            rates: s("rates"),
            grouping: s("grouping"),
            default: s("default"),
            choices,
        })
    }
}

/// Something a click does.
#[derive(Clone, Debug)]
enum Action {
    Copy,
    Use(String),
    Download,
    About,
    Link(String),
    /// The full app (the interactive mode) in a terminal.
    Terminal,
}

/// The colours of a theme (the icon's palette: green, ivory, gold).
#[derive(Clone, Copy)]
struct Theme {
    dark: bool,
    bg: u32,
    field: u32,
    border: u32,
    fg: u32,
    muted: u32,
    accent: u32,
    error: u32,
    pill: u32,
}

impl Theme {
    fn new(dark: bool) -> Theme {
        if dark {
            Theme {
                dark,
                bg: rgb(0x18201D),
                field: rgb(0x222B27),
                border: rgb(0x34403A),
                fg: rgb(0xF7F0E6),
                muted: rgb(0x9AA8A1),
                accent: rgb(0xF2B84B),
                error: rgb(0xF28B82),
                pill: rgb(0x2C3832),
            }
        } else {
            Theme {
                dark,
                bg: rgb(0xFBFCFA),
                field: rgb(0xF0F3EF),
                border: rgb(0xD3DAD5),
                fg: rgb(0x1C2421),
                muted: rgb(0x5D6A64),
                accent: rgb(0x1F5B45),
                error: rgb(0xB3261E),
                pill: rgb(0xE4EDE7),
            }
        }
    }
}

struct Fonts {
    title: HFONT,
    body: HFONT,
    big: HFONT,
    small: HFONT,
}

struct State {
    reply: Option<Reply>,
    /// The request `reply` answers, and whether as submitted.
    reply_for: Option<(String, bool)>,
    submitted: bool,
    copied: Option<String>,
    /// The requests copied, the latest first: (request, result).
    recent: Vec<(String, String)>,
    about: bool,
    notices: Vec<String>,
    status: String,
    syncing: bool,
    hotkey: Option<&'static str>,
    hits: Vec<(RECT, Action)>,
    theme: Theme,
    dpi: u32,
    fonts: Option<Fonts>,
    /// The edit control's background (Windows asks for a brush that stays).
    field_brush: HBRUSH,
    icon: HICON,
    /// When it was last hidden: a click on the tray icon that hid it must not show it again.
    hidden_at: Option<Instant>,
    /// The screen corner it opens in, near the notification area: (right, bottom).
    corner: (bool, bool),
    work: RECT,
}

impl Default for State {
    fn default() -> State {
        State {
            reply: None,
            reply_for: None,
            submitted: false,
            copied: None,
            recent: Vec::new(),
            about: false,
            notices: Vec::new(),
            status: String::new(),
            syncing: false,
            hotkey: None,
            hits: Vec::new(),
            theme: Theme::new(false),
            dpi: 96,
            fonts: None,
            field_brush: std::ptr::null_mut(),
            icon: std::ptr::null_mut(),
            hidden_at: None,
            corner: (true, true),
            work: RECT::default(),
        }
    }
}

impl State {
    /// `px` at 96 DPI, on this monitor.
    fn px(&self, px: i32) -> i32 {
        px * self.dpi as i32 / 96
    }

    /// The fonts and the brush for this DPI and theme.
    fn restyle(&mut self, instance: HANDLE) {
        // SAFETY: GDI objects created here and deleted when replaced.
        unsafe {
            if let Some(old) = self.fonts.take() {
                for f in [old.title, old.body, old.big, old.small] {
                    DeleteObject(f);
                }
            }
            if !self.field_brush.is_null() {
                DeleteObject(self.field_brush);
            }
            let face = wide("Segoe UI");
            let font = |px: i32, weight: i32| CreateFontW(-self.px(px), 0, 0, 0, weight, 0, 0, 0, DEFAULT_CHARSET, 0, 0, CLEARTYPE_QUALITY, 0, face.as_ptr());
            self.fonts = Some(Fonts { title: font(16, FW_SEMIBOLD), body: font(14, FW_NORMAL), big: font(22, FW_SEMIBOLD), small: font(12, FW_NORMAL) });
            self.field_brush = CreateSolidBrush(self.theme.field);
            let size = self.px(40);
            self.icon = LoadImageW(instance, resource(1), IMAGE_ICON, size, size, LR_DEFAULTCOLOR);
        }
    }
}

// ---------------------------------------------------------------- creating, showing

/// Creates the window (hidden).
pub fn create(instance: HANDLE, hotkey: Option<&'static str>) {
    let class = wide(CLASS);
    register(instance, &class);
    let title = wide("Aequalis");
    // SAFETY: a registered class; the windows live for the program's life.
    unsafe {
        let popup = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            460,
            200,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        );
        let edit = edit_control(popup, instance);
        // Rounded corners (Windows 11) and its border.
        let round = DWMWCP_ROUND;
        DwmSetWindowAttribute(popup, DWMWA_WINDOW_CORNER_PREFERENCE, (&round as *const u32).cast(), 4);
        HANDLES.with(|h| h.set(Some(Handles { popup, edit, instance })));
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.hotkey = hotkey;
        s.theme = Theme::new(system::dark_theme());
        s.restyle(instance);
    });
    apply_theme();
}

/// The popup's window class (`class`, NUL-terminated), registered.
fn register(instance: HANDLE, class: &[u16]) {
    // SAFETY: a valid class; the procedure lives for the program's life.
    unsafe {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DROPSHADOW,
            lpfnWndProc: proc_,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), resource(IDC_ARROW)),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        RegisterClassExW(&wc);
    }
}

/// The box: an edit control in `popup`, with its hint, and its own procedure for Enter and Esc.
fn edit_control(popup: HWND, instance: HANDLE) -> HWND {
    let edit_class = wide("EDIT");
    let cue = wide("10 km mi  ·  12+12/3  ·  2026/12/04 - 12d  ·  now New York");
    // SAFETY: a valid parent window; the procedure lives for the program's life.
    unsafe {
        let edit = CreateWindowExW(
            0,
            edit_class.as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL,
            0,
            0,
            100,
            20,
            popup,
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        );
        SendMessageW(edit, EM_SETCUEBANNER, 1, cue.as_ptr() as isize);
        EDIT_PROC.with(|p| p.set(SetWindowLongPtrW(edit, GWLP_WNDPROC, edit_proc as WndProc as usize as isize)));
        edit
    }
}

/// The theme of Windows, again (it changed).
pub fn theme_changed() {
    let h = handles();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.theme = Theme::new(system::dark_theme());
        s.restyle(h.instance);
    });
    apply_theme();
    relayout();
}

fn apply_theme() {
    let h = handles();
    let (dark, border, body) = STATE.with(|s| {
        let s = s.borrow();
        (s.theme.dark as u32, s.theme.border, s.fonts.as_ref().map(|f| f.body))
    });
    // SAFETY: valid window handles and values.
    unsafe {
        DwmSetWindowAttribute(h.popup, DWMWA_USE_IMMERSIVE_DARK_MODE, (&dark as *const u32).cast(), 4);
        DwmSetWindowAttribute(h.popup, DWMWA_BORDER_COLOR, (&border as *const u32).cast(), 4);
        if let Some(body) = body {
            SendMessageW(h.edit, WM_SETFONT, body as usize, 1);
        }
    }
}

pub fn visible() -> bool {
    // SAFETY: a valid window handle.
    unsafe { IsWindowVisible(handles().popup) != 0 }
}

/// Shows it or hides it (the tray icon, the hot key). `anchor`: where the click was, on screen.
pub fn toggle(anchor: POINT) {
    if visible() {
        hide();
        return;
    }
    // The click on the tray icon has just hidden it (it lost the focus): it stays hidden.
    let just_hidden = STATE.with(|s| s.borrow().hidden_at.is_some_and(|t| t.elapsed().as_millis() < 300));
    if !just_hidden {
        show(anchor);
    }
}

/// Shows it by the notification area of the monitor of `anchor`, the request selected.
pub fn show(anchor: POINT) {
    let h = handles();
    // The monitor's working area (without the taskbar), and the corner nearest the anchor.
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, rcMonitor: RECT::default(), rcWork: RECT::default(), dwFlags: 0 };
    // SAFETY: valid pointers.
    unsafe { GetMonitorInfoW(MonitorFromPoint(anchor, MONITOR_DEFAULTTONEAREST), &mut info) };
    let work = info.rcWork;
    let corner = (anchor.x >= (work.left + work.right) / 2, anchor.y >= (work.top + work.bottom) / 2);
    // On the monitor first, so that its DPI is the one read.
    // SAFETY: a valid window handle.
    unsafe { SetWindowPos(h.popup, HWND_TOPMOST, work.left + 8, work.top + 8, 10, 10, SWP_NOACTIVATE) };
    // SAFETY: as above.
    let dpi = unsafe { GetDpiForWindow(h.popup) }.max(96);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.work = work;
        s.corner = corner;
        s.about = false;
        if s.dpi != dpi {
            s.dpi = dpi;
            s.restyle(h.instance);
        }
        s.status = aequalis::network::status().label().to_string();
        s.notices = aequalis::notify::current();
    });
    apply_theme();
    relayout();
    // SAFETY: valid window handles; the program received the input that opens it, so it may take
    // the focus.
    unsafe {
        ShowWindow(h.popup, SW_SHOW);
        SetForegroundWindow(h.popup);
        SetFocus(h.edit);
        SendMessageW(h.edit, EM_SETSEL, 0, -1);
    }
}

/// Shows it with About (the tray menu).
pub fn show_about(anchor: POINT) {
    show(anchor);
    STATE.with(|s| s.borrow_mut().about = true);
    relayout();
}

pub fn hide() {
    let h = handles();
    STATE.with(|s| s.borrow_mut().hidden_at = Some(Instant::now()));
    // SAFETY: a valid window handle.
    unsafe { ShowWindow(h.popup, SW_HIDE) };
}

/// The online data changed (a download ended, or started): the status, the notices, the reply.
pub fn data_changed(syncing: bool) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.syncing = syncing;
        s.status = aequalis::network::status().label().to_string();
        s.notices = aequalis::notify::current();
    });
    if visible() {
        evaluate();
    }
}

// ---------------------------------------------------------------- the request

fn text() -> String {
    let h = handles();
    // SAFETY: a valid window handle and a buffer of the size given.
    unsafe {
        let len = GetWindowTextLengthW(h.edit);
        let mut buf = vec![0u16; len as usize + 1];
        GetWindowTextW(h.edit, buf.as_mut_ptr(), buf.len() as i32);
        from_wide(&buf)
    }
}

/// The reply to the request as typed.
fn evaluate() {
    let request = text();
    let submitted = STATE.with(|s| s.borrow().submitted);
    let reply = if request.trim().is_empty() { None } else { Reply::parse(&reply::view(&request, submitted)) };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.reply = reply;
        s.reply_for = Some((request, submitted));
    });
    relayout();
}

/// Enter, on what is in the box now (a key typed just before may not be evaluated yet): a result
/// copied (and remembered); a question shown, and on a second Enter its default taken; otherwise
/// what the request lacks (an error or a hint shows even in the middle of a word).
fn commit() {
    let h = handles();
    let request = text();
    // The reply shown is this request's, as submitted: Enter acts on it; otherwise it is made now.
    let current = STATE.with(|s| s.borrow().reply_for.as_ref().is_some_and(|(r, submitted)| *r == request && *submitted));
    if !current {
        // SAFETY: a valid window; the pending evaluation is this one.
        unsafe { KillTimer(h.popup, TIMER_EVALUATE) };
        STATE.with(|s| s.borrow_mut().submitted = true);
        evaluate();
    }
    let reply = STATE.with(|s| s.borrow().reply.clone());
    match reply {
        Some(r) if r.show == "question" && current => {
            if let Some(default) = r.default {
                use_request(&default);
            }
        }
        Some(r) if r.show == "result" => copy_result(h.popup),
        _ => {}
    }
}

/// The result on the clipboard, and among the recent requests.
fn copy_result(popup: HWND) {
    let request = text();
    let Some(reply) = STATE.with(|s| s.borrow().reply.clone()).filter(|r| r.show == "result") else { return };
    if system::copy(popup, &reply.copy) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.copied = Some(reply.copy.clone());
            let input = request.trim().to_string();
            s.recent.retain(|(i, _)| *i != input);
            s.recent.insert(0, (input, reply.result.clone()));
            s.recent.truncate(RECENT);
        });
        relayout();
    }
}

/// `request` in the box, the cursor at its end (a choice, a recent request).
fn use_request(request: &str) {
    let h = handles();
    let text = wide(request);
    // SAFETY: valid handles; the text change is handled by `proc_` (nothing is borrowed here).
    unsafe {
        SetWindowTextW(h.edit, text.as_ptr());
        let end = request.encode_utf16().count();
        SendMessageW(h.edit, EM_SETSEL, end, end as isize);
        SetFocus(h.edit);
    }
}

fn act(action: Action) {
    let h = handles();
    match action {
        Action::Copy => copy_result(h.popup),
        Action::Use(request) => use_request(&request),
        Action::Download => crate::app::download(false),
        Action::About => {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.about = !s.about;
            });
            relayout();
        }
        Action::Link(url) => {
            hide();
            system::open_link(&url);
        }
        Action::Terminal => {
            hide();
            system::open_terminal();
        }
    }
}

// ---------------------------------------------------------------- layout and drawing

/// One pass over the content: measuring it (no drawing), or drawing it; the clickable parts.
struct Pass {
    dc: HDC,
    draw: bool,
    width: i32,
    hits: Vec<(RECT, Action)>,
}

impl Pass {
    /// `text` in `rect`'s width, wrapped (or on one line, cut with an ellipsis): its height.
    fn text(&self, font: HFONT, color: u32, text: &str, rect: RECT, single: bool) -> i32 {
        let units: Vec<u16> = text.encode_utf16().collect();
        let flags = DT_NOPREFIX | DT_LEFT | if single { DT_SINGLELINE | DT_END_ELLIPSIS } else { DT_WORDBREAK | DT_EDITCONTROL };
        let mut measured = rect;
        // SAFETY: a valid DC and buffer.
        unsafe {
            SelectObject(self.dc, font);
            DrawTextW(self.dc, units.as_ptr(), units.len() as i32, &mut measured, flags | DT_CALCRECT);
            if self.draw {
                SetTextColor(self.dc, color);
                let mut target = RECT { bottom: rect.top + (measured.bottom - measured.top), ..rect };
                DrawTextW(self.dc, units.as_ptr(), units.len() as i32, &mut target, flags);
            }
        }
        measured.bottom - measured.top
    }

    /// The height of a line of `font`.
    fn line(&self, font: HFONT) -> i32 {
        let units: Vec<u16> = "Ag".encode_utf16().collect();
        let mut size = SIZE::default();
        // SAFETY: a valid DC and buffer.
        unsafe {
            SelectObject(self.dc, font);
            GetTextExtentPoint32W(self.dc, units.as_ptr(), units.len() as i32, &mut size);
        }
        size.cy
    }

    fn text_width(&self, font: HFONT, text: &str) -> i32 {
        let units: Vec<u16> = text.encode_utf16().collect();
        let mut size = SIZE::default();
        // SAFETY: a valid DC and buffer.
        unsafe {
            SelectObject(self.dc, font);
            GetTextExtentPoint32W(self.dc, units.as_ptr(), units.len() as i32, &mut size);
        }
        size.cx
    }

    fn fill(&self, rect: RECT, color: u32, radius: i32) {
        if !self.draw {
            return;
        }
        // SAFETY: a valid DC; stock objects need no deleting.
        unsafe {
            SetDCBrushColor(self.dc, color);
            SelectObject(self.dc, GetStockObject(DC_BRUSH));
            SelectObject(self.dc, GetStockObject(NULL_PEN));
            RoundRect(self.dc, rect.left, rect.top, rect.right + 1, rect.bottom + 1, radius, radius);
        }
    }

    /// A rounded button with `label`, at (x, y): its rect.
    fn pill(&mut self, s: &State, font: HFONT, label: &str, x: i32, y: i32, action: Action) -> RECT {
        let (pad, height) = (s.px(10), s.px(26));
        let w = (self.text_width(font, label) + 2 * pad).min(self.width - 2 * s.px(16));
        let rect = RECT { left: x, top: y, right: x + w, bottom: y + height };
        self.fill(rect, s.theme.pill, height);
        if self.draw {
            let units: Vec<u16> = label.encode_utf16().collect();
            let mut r = RECT { left: rect.left + pad, right: rect.right - pad, ..rect };
            // SAFETY: a valid DC and buffer.
            unsafe {
                SelectObject(self.dc, font);
                SetTextColor(self.dc, s.theme.fg);
                DrawTextW(self.dc, units.as_ptr(), units.len() as i32, &mut r, DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | DT_END_ELLIPSIS);
            }
        }
        self.hits.push((rect, action));
        rect
    }
}

/// Where the box is, in the window: (left, top, right, bottom) of its frame.
fn field_rect(s: &State, width: i32, title_height: i32) -> RECT {
    let pad = s.px(16);
    let top = pad + title_height + s.px(10);
    RECT { left: pad, top, right: width - pad, bottom: top + s.px(38) }
}

/// The content, measured or drawn: its height.
fn layout(pass: &mut Pass, s: &State) -> i32 {
    let Some(f) = s.fonts.as_ref() else { return 0 };
    let mut y = header(pass, s, f);
    if s.about {
        y = about(pass, s, f, y);
    } else {
        if let Some(r) = &s.reply {
            y = reply(pass, s, f, r, y);
        }
        y = below(pass, s, f, y);
    }
    footer(pass, s, f, y) + s.px(16)
}

/// The full width of the content from `top`, as tall as needed.
fn inner(pass: &Pass, s: &State, top: i32) -> RECT {
    let pad = s.px(16);
    RECT { left: pad, top, right: pass.width - pad, bottom: top + 10_000 }
}

/// The title (About on a click), the online status, and the box's frame (the edit control sits
/// in it): where what follows starts.
fn header(pass: &mut Pass, s: &State, f: &Fonts) -> i32 {
    let (t, pad, w) = (s.theme, s.px(16), pass.width);
    pass.text(f.title, t.fg, "Aequalis", inner(pass, s, pad), true);
    let title_h = pass.line(f.title);
    let title_w = pass.text_width(f.title, "Aequalis");
    pass.hits.push((RECT { left: pad, top: pad, right: pad + title_w, bottom: pad + title_h }, Action::About));
    let status = format!("●  {}", s.status);
    let status_w = pass.text_width(f.small, &status);
    let status_color = if s.status == "online" { t.accent } else { t.muted };
    let status_top = pad + (title_h - pass.line(f.small)) / 2;
    pass.text(f.small, status_color, &status, RECT { left: w - pad - status_w, top: status_top, right: w - pad, bottom: pad + title_h }, true);
    let field = field_rect(s, w, title_h);
    pass.fill(RECT { left: field.left - 1, top: field.top - 1, right: field.right + 1, bottom: field.bottom + 1 }, t.border, s.px(10));
    pass.fill(field, t.field, s.px(8));
    field.bottom + s.px(12)
}

/// About, from `y`: the icon, the name and tagline, the facts, the link.
fn about(pass: &mut Pass, s: &State, f: &Fonts, mut y: i32) -> i32 {
    let (t, pad, gap) = (s.theme, s.px(16), s.px(10));
    let icon = s.px(40);
    if pass.draw {
        // SAFETY: a valid DC and icon.
        unsafe { DrawIconEx(pass.dc, pad, y, s.icon, icon, icon, 0, std::ptr::null_mut(), DI_NORMAL) };
    }
    let text_left = pad + icon + s.px(12);
    let head = format!("{} — {}", aequalis::about::NAME, aequalis::about::TAGLINE);
    let h = pass.text(f.body, t.fg, &head, RECT { left: text_left, ..inner(pass, s, y) }, false);
    y += h.max(icon) + gap;
    for (label, value) in aequalis::about::facts(&aequalis::about::memory()) {
        y += pass.text(f.small, t.muted, &format!("{label}: {value}"), inner(pass, s, y), false) + s.px(2);
    }
    y += gap;
    let link = "github.com/lucapf/aequalis";
    let h = pass.text(f.small, t.accent, link, inner(pass, s, y), true);
    pass.hits.push((RECT { left: pad, top: y, right: pad + pass.text_width(f.small, link), bottom: y + h }, Action::Link(format!("https://{link}"))));
    y + h + gap
}

/// The reply, from `y`: the result with its Copy button and lines, a hint, an error, or a
/// question's choices as buttons.
fn reply(pass: &mut Pass, s: &State, f: &Fonts, r: &Reply, mut y: i32) -> i32 {
    let (t, pad, gap, w) = (s.theme, s.px(16), s.px(10), pass.width);
    match r.show.as_str() {
        "result" => {
            let copy_w = pass.text_width(f.small, "Copy") + s.px(20);
            let text = format!("= {}", r.result);
            let h = pass.text(f.big, t.fg, &text, RECT { right: w - pad - copy_w - s.px(8), ..inner(pass, s, y) }, false);
            pass.pill(s, f.small, "Copy", w - pad - copy_w, y + (pass.line(f.big) - s.px(26)) / 2, Action::Copy);
            y += h.max(s.px(26)) + s.px(4);
            for line in r.grouping.iter().chain(r.rates.iter()).chain(r.details.iter()) {
                y += pass.text(f.small, t.muted, line, inner(pass, s, y), false) + s.px(2);
            }
            y + gap
        }
        "hint" => y + pass.text(f.body, t.muted, &r.message, inner(pass, s, y), false) + gap,
        "error" => y + pass.text(f.body, t.error, &r.message, inner(pass, s, y), false) + gap,
        "question" => {
            y += pass.text(f.body, t.muted, &r.message, inner(pass, s, y), false) + s.px(8);
            let mut x = pad;
            for choice in &r.choices {
                let width = pass.text_width(f.body, &choice.label) + s.px(20);
                if x > pad && x + width > w - pad {
                    x = pad;
                    y += s.px(26) + s.px(6);
                }
                let rect = pass.pill(s, f.body, &choice.label, x, y, Action::Use(choice.request.clone()));
                x = rect.right + s.px(6);
            }
            y + s.px(26) + gap
        }
        _ => y,
    }
}

/// Under the reply, from `y`: the notices (a new city archive, out-of-date data) with a Download
/// button, the download running, what was copied, and the recent requests.
fn below(pass: &mut Pass, s: &State, f: &Fonts, mut y: i32) -> i32 {
    let (t, pad, gap, w) = (s.theme, s.px(16), s.px(10), pass.width);
    for notice in &s.notices {
        let button_w = pass.text_width(f.small, "Download") + s.px(20);
        let h = pass.text(f.small, t.accent, notice, RECT { right: w - pad - button_w - s.px(8), ..inner(pass, s, y + s.px(4)) }, false);
        pass.pill(s, f.small, "Download", w - pad - button_w, y, Action::Download);
        y += (h + s.px(4)).max(s.px(26)) + s.px(6);
    }
    if s.syncing {
        y += pass.text(f.small, t.muted, "Downloading…", inner(pass, s, y), true) + s.px(6);
    }
    if let Some(copied) = &s.copied {
        y += pass.text(f.small, t.accent, &format!("Copied {copied}"), inner(pass, s, y), true) + gap;
    }
    if !s.recent.is_empty() {
        y += pass.text(f.small, t.muted, "Recent", inner(pass, s, y), true) + s.px(4);
        for (input, result) in &s.recent {
            let line = format!("{input}   =   {result}");
            let h = pass.text(f.body, t.fg, &line, inner(pass, s, y), true);
            pass.hits.push((RECT { left: pad, top: y, right: w - pad, bottom: y + h }, Action::Use(input.clone())));
            y += h + s.px(4);
        }
        y += s.px(6);
    }
    y
}

/// The footer from `y` (the keys), and at its right the button for the full app (the interactive
/// mode, in a terminal): where it ends.
fn footer(pass: &mut Pass, s: &State, f: &Fonts, y: i32) -> i32 {
    let (pad, w) = (s.px(16), pass.width);
    let footer = match s.hotkey {
        Some(key) => format!("Enter copies · Esc closes · {key} opens"),
        None => "Enter copies · Esc closes".to_string(),
    };
    let button = "Full app";
    let button_w = pass.text_width(f.small, button) + s.px(20);
    let h = pass.text(f.small, s.theme.muted, &footer, RECT { right: w - pad - button_w - s.px(8), ..inner(pass, s, y) }, true);
    pass.pill(s, f.small, button, w - pad - button_w, y - s.px(2), Action::Terminal);
    y + h.max(s.px(24))
}

/// The width of the window here.
fn width(s: &State) -> i32 {
    s.px(460)
}

/// Measures the content again: the window's height, its place in its corner, the box's place.
fn relayout() {
    let h = handles();
    // SAFETY: a valid window; the DC is released below.
    let dc = unsafe { GetDC(h.popup) };
    let (rect, field) = STATE.with(|s| {
        let s = s.borrow();
        let w = width(&s);
        let mut pass = Pass { dc, draw: false, width: w, hits: Vec::new() };
        let height = layout(&mut pass, &s).min(s.work.bottom - s.work.top - s.px(16));
        let margin = s.px(12);
        let x = if s.corner.0 { s.work.right - w - margin } else { s.work.left + margin };
        let y = if s.corner.1 { s.work.bottom - height - margin } else { s.work.top + margin };
        let fonts = s.fonts.as_ref().expect("the fonts are made with the window");
        let frame = field_rect(&s, w, pass.line(fonts.title));
        let inset = s.px(10);
        let line = pass.line(fonts.body);
        let field = RECT { left: frame.left + inset, top: frame.top + (frame.bottom - frame.top - line) / 2, right: frame.right - inset, bottom: 0 };
        (RECT { left: x, top: y, right: x + w, bottom: y + height }, RECT { bottom: field.top + line, ..field })
    });
    // SAFETY: valid handles.
    unsafe {
        ReleaseDC(h.popup, dc);
        SetWindowPos(h.popup, HWND_TOPMOST, rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top, SWP_NOACTIVATE);
        MoveWindow(h.edit, field.left, field.top, field.right - field.left, field.bottom - field.top, 1);
        InvalidateRect(h.popup, std::ptr::null(), 0);
    }
}

fn paint(hwnd: HWND) {
    let mut ps = PAINTSTRUCT { hdc: std::ptr::null_mut(), fErase: 0, rcPaint: RECT::default(), fRestore: 0, fIncUpdate: 0, rgbReserved: [0; 32] };
    // SAFETY: BeginPaint/EndPaint around drawing into an off-screen bitmap of the window's size.
    unsafe {
        let dc = BeginPaint(hwnd, &mut ps);
        let mut client = RECT::default();
        GetClientRect(hwnd, &mut client);
        let (w, h) = (client.right, client.bottom);
        let memory = CreateCompatibleDC(dc);
        let bitmap = CreateCompatibleBitmap(dc, w, h);
        let old = SelectObject(memory, bitmap);
        SetBkMode(memory, TRANSPARENT);
        let hits = STATE.with(|s| {
            let s = s.borrow();
            SetDCBrushColor(memory, s.theme.bg);
            FillRect(memory, &client, GetStockObject(DC_BRUSH));
            let mut pass = Pass { dc: memory, draw: true, width: w, hits: Vec::new() };
            layout(&mut pass, &s);
            pass.hits
        });
        STATE.with(|s| s.borrow_mut().hits = hits);
        BitBlt(dc, 0, 0, w, h, memory, 0, 0, SRCCOPY);
        SelectObject(memory, old);
        DeleteObject(bitmap);
        DeleteDC(memory);
        EndPaint(hwnd, &ps);
    }
}

fn hit(p: POINT) -> Option<Action> {
    STATE.with(|s| s.borrow().hits.iter().rev().find(|(r, _)| r.contains(p)).map(|(_, a)| a.clone()))
}

// ---------------------------------------------------------------- messages

unsafe extern "system" fn proc_(hwnd: HWND, msg: u32, w: usize, l: isize) -> isize {
    match msg {
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_ACTIVATE if w & 0xFFFF == WA_INACTIVE => {
            // A click elsewhere: it closes, as a flyout does.
            if IsWindowVisible(hwnd) != 0 {
                hide();
            }
            0
        }
        WM_COMMAND if high(w as isize) == EN_CHANGE => {
            edited(hwnd);
            0
        }
        WM_TIMER if w == TIMER_EVALUATE => {
            KillTimer(hwnd, TIMER_EVALUATE);
            evaluate();
            0
        }
        WM_CTLCOLOREDIT => edit_colors(w as HDC),
        WM_SETCURSOR => {
            let mut p = POINT::default();
            GetCursorPos(&mut p);
            ScreenToClient(hwnd, &mut p);
            if hit(p).is_some() {
                SetCursor(LoadCursorW(std::ptr::null_mut(), resource(IDC_HAND)));
                return 1;
            }
            DefWindowProcW(hwnd, msg, w, l)
        }
        WM_LBUTTONUP => {
            if let Some(action) = hit(point(l)) {
                act(action);
            }
            0
        }
        WM_DPICHANGED => {
            rescaled(high(w as isize));
            0
        }
        // Alt+F4: closes the box, not the program.
        WM_CLOSE => {
            hide();
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

/// The request was edited: what was shown for the last one goes, and it is evaluated once the
/// typing pauses.
fn edited(hwnd: HWND) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.submitted = false;
        s.copied = None;
        s.about = false;
    });
    // SAFETY: a valid window.
    unsafe { SetTimer(hwnd, TIMER_EVALUATE, EVALUATE_AFTER_MS, std::ptr::null()) };
}

/// The edit control's colors, into its DC: the brush of its background.
fn edit_colors(dc: HDC) -> isize {
    let (fg, bg, brush) = STATE.with(|s| {
        let s = s.borrow();
        (s.theme.fg, s.theme.field, s.field_brush)
    });
    // SAFETY: the edit control's DC, given with the message.
    unsafe {
        SetTextColor(dc, fg);
        SetBkColor(dc, bg);
    }
    brush as isize
}

/// The window moved to a screen of another scale (`dpi`): fonts, theme and layout again.
fn rescaled(dpi: u32) {
    let h = handles();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.dpi = dpi.max(96);
        s.restyle(h.instance);
    });
    apply_theme();
    relayout();
}

/// The edit control: Enter commits, Esc closes; the rest is the control's.
unsafe extern "system" fn edit_proc(hwnd: HWND, msg: u32, w: usize, l: isize) -> isize {
    match (msg, w) {
        (WM_KEYDOWN, VK_RETURN) => {
            commit();
            0
        }
        (WM_KEYDOWN, VK_ESCAPE) => {
            hide();
            0
        }
        // No beep for them.
        (WM_CHAR, 0x0D | 0x1B) => 0,
        _ => CallWindowProcW(EDIT_PROC.with(|p| p.get()), hwnd, msg, w, l),
    }
}
