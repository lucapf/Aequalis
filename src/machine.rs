//! The machine aequalis runs on, as the core reads it: its measurement system (`locale`), its number
//! format (`numbers`), its date order (`time::Order::of_locale`), its time zone (`time::device_zone`,
//! and so the local currency) and its folder (`files::dir`). Each of those readings asks this module
//! first: a `Mock` installed on the current thread is then the only source, whatever the real machine
//! and whatever an app handed in. Otherwise they read the real machine (the `LC_*` variables, the
//! system's settings), or what an app embedding aequalis set once (`set_device_*`), as before.
//!
//! For the tests, so that they give the same results on every laptop and CI runner: a library or TUI
//! test installs a preset (`let _m = Mock::italy().install();`); the binary's tests name one in
//! `AEQUALIS_MACHINE` (`us`, `italy`, `switzerland`, `japan`), read in debug builds only.

use crate::config::System;
use crate::numbers::NumberFormat;
use crate::time::Order;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// A simulated machine: its settings, and a fresh empty folder of its own (removed with it).
#[derive(Debug)]
pub struct Mock {
    pub system: System,
    /// Kept for the whole run: the formatting functions hand out `&'static` formats.
    pub format: &'static NumberFormat,
    pub order: Order,
    /// `Europe/Rome`: the device's zone, and through it the local currency.
    pub zone: &'static str,
    dir: PathBuf,
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<Mock>>> = const { RefCell::new(None) };
}

/// The thread's mock, if one is installed.
pub fn mocked() -> Option<Rc<Mock>> {
    CURRENT.with(|c| c.borrow().clone())
}

/// Keeps a mock installed on its thread; dropping it puts back the real machine (and removes the
/// mock's folder).
#[must_use = "the mock is uninstalled when the guard is dropped"]
pub struct Guard {
    previous: Option<Rc<Mock>>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        CURRENT.with(|c| *c.borrow_mut() = previous);
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn format(decimal: &str, thousands: &str) -> &'static NumberFormat {
    Box::leak(Box::new(NumberFormat { decimal: decimal.into(), thousands: thousands.into(), grouping: vec![3] }))
}

/// A fresh, empty folder, unique in this run.
fn fresh_dir(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("aequalis-machine-{}-{name}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

impl Mock {
    fn new(name: &str, system: System, format: &'static NumberFormat, order: Order, zone: &'static str) -> Mock {
        Mock { system, format, order, zone, dir: fresh_dir(name) }
    }

    /// Imperial, `1,234.5`, month/day, New York (USD).
    pub fn us() -> Mock {
        Mock::new("us", System::Imperial, format(".", ","), Order::MonthFirst, "America/New_York")
    }

    /// Metric, `1.234,5`, day/month, Rome (EUR).
    pub fn italy() -> Mock {
        Mock::new("italy", System::Metric, format(",", "."), Order::DayFirst, "Europe/Rome")
    }

    /// Metric, `1'234.5`, day/month, Zurich (CHF).
    pub fn switzerland() -> Mock {
        Mock::new("switzerland", System::Metric, format(".", "'"), Order::DayFirst, "Europe/Zurich")
    }

    /// Metric, `1,234.5`, year/month/day, Tokyo (JPY).
    pub fn japan() -> Mock {
        Mock::new("japan", System::Metric, format(".", ","), Order::YearFirst, "Asia/Tokyo")
    }

    /// A preset by name (`AEQUALIS_MACHINE`).
    pub fn named(name: &str) -> Option<Mock> {
        match name.trim().to_ascii_lowercase().as_str() {
            "us" => Some(Mock::us()),
            "italy" => Some(Mock::italy()),
            "switzerland" => Some(Mock::switzerland()),
            "japan" => Some(Mock::japan()),
            _ => None,
        }
    }

    pub fn with_system(mut self, system: System) -> Mock {
        self.system = system;
        self
    }

    pub fn with_format(mut self, decimal: &str, thousands: &str) -> Mock {
        self.format = format(decimal, thousands);
        self
    }

    pub fn with_order(mut self, order: Order) -> Mock {
        self.order = order;
        self
    }

    pub fn with_zone(mut self, zone: &'static str) -> Mock {
        self.zone = zone;
        self
    }

    /// The machine's folder (`files::dir`): fresh and empty, removed with the mock.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Simulates this machine on the current thread for the rest of its life: a test's helper, whose
    /// test runs on a thread of its own.
    pub fn keep(self) {
        std::mem::forget(self.install());
    }

    /// Simulates this machine on the current thread until the guard is dropped.
    pub fn install(self) -> Guard {
        let previous = CURRENT.with(|c| c.borrow_mut().replace(Rc::new(self)));
        Guard { previous }
    }
}

/// The binary's tests: the preset named in `AEQUALIS_MACHINE`, installed on the main thread for the
/// whole run. Debug builds only: a shipped binary reads the real machine whatever the environment.
pub fn from_env() -> Option<Guard> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let name = std::env::var("AEQUALIS_MACHINE").ok()?;
    Some(Mock::named(&name)?.install())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mock_is_the_threads_alone() {
        let _italy = Mock::italy().install();
        let other = std::thread::spawn(|| {
            let _us = Mock::us().install();
            (crate::locale::device_system(), crate::files::dir())
        })
        .join()
        .unwrap();
        assert_eq!(other.0, System::Imperial);
        assert_eq!(crate::locale::device_system(), System::Metric, "the other thread's mock is not this one's");
        assert_ne!(other.1, crate::files::dir(), "each its own folder");
    }

    #[test]
    fn the_mock_is_the_only_source() {
        let _m = Mock::us().install();
        assert_eq!(crate::locale::device_system(), System::Imperial);
        assert_eq!(crate::numbers::device().apply("1234567.5"), "1,234,567.5");
        assert_eq!(crate::time::Order::of_locale(), Order::MonthFirst);
        assert_eq!(crate::time::device_zone(), Some("America/New_York"));
        assert_eq!(crate::currency::local().unwrap().map(|l| l.code), Some("USD"));
    }

    #[test]
    fn the_presets_formats_and_orders() {
        let _m = Mock::switzerland().install();
        assert_eq!(crate::numbers::device().apply("1234.5"), "1'234.5");
        drop(_m);
        let _m = Mock::japan().install();
        assert_eq!(crate::time::Order::of_locale(), Order::YearFirst);
        assert_eq!(crate::currency::local().unwrap().map(|l| l.code), Some("JPY"));
    }

    #[test]
    fn a_fresh_folder_gone_with_the_mock() {
        let guard = Mock::italy().install();
        let dir = crate::files::dir().unwrap();
        assert!(dir.is_dir() && std::fs::read_dir(&dir).unwrap().next().is_none(), "fresh and empty");
        drop(guard);
        assert!(!dir.exists(), "removed with the mock");
    }

    /// A shipped binary reads the real machine, whatever `AEQUALIS_MACHINE` says (`cargo test
    /// --release`).
    #[test]
    #[cfg(not(debug_assertions))]
    fn release_builds_ignore_the_variable() {
        std::env::set_var("AEQUALIS_MACHINE", "us");
        assert!(from_env().is_none());
    }

    #[test]
    fn settings_are_the_mocks_own() {
        let _m = Mock::italy().install();
        crate::settings::change("precision", Some("2")).unwrap();
        assert_eq!(crate::settings::current().precision(), 2);
        let other = std::thread::spawn(|| {
            let _m = Mock::italy().install();
            crate::settings::current().precision()
        })
        .join()
        .unwrap();
        assert_eq!(other, 4, "another machine has its own settings");
    }
}
