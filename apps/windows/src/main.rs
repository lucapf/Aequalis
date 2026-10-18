//! aequalis-tray: Aequalis in the Windows notification area. Its icon, or Win+Alt+A, opens the input
//! box (as the Omarchy widget does on Linux); the library answers in this process. The device's time
//! zone is read as the command line reads it (src/device.rs: at start, every hour, and when Windows
//! says it changed), with the zones' rules of the Windows registry.

// A window program: no console.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod app;
#[cfg(windows)]
#[path = "../../../src/device.rs"]
mod device;
#[cfg(windows)]
mod popup;
#[cfg(windows)]
mod system;
#[cfg(windows)]
mod win32;

#[cfg(windows)]
fn main() {
    std::process::exit(app::run());
}

#[cfg(not(windows))]
fn main() {
    eprintln!("aequalis-tray is the Windows app; elsewhere, use aequalis");
}
