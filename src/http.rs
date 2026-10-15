//! Downloads. The command line runs `curl`; an app that embeds tally (the Android app, `ffi.rs`)
//! has no `curl`, so it registers its own `Fetcher` once, at start.

use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

/// Downloads for tally: the body of `url` (`None`: no answer, or with `strict` an HTTP error), or
/// `url` saved to the file `to`. At most `seconds`.
pub trait Fetcher: Send + Sync {
    fn get(&self, url: &str, seconds: u32, strict: bool) -> Option<Vec<u8>>;
    fn save(&self, url: &str, to: &Path, seconds: u32) -> bool;
}

static FETCHER: OnceLock<Box<dyn Fetcher>> = OnceLock::new();

/// Uses `fetcher` for every download from now on (once: later calls are ignored).
pub fn register(fetcher: Box<dyn Fetcher>) {
    let _ = FETCHER.set(fetcher);
}

/// The body of `url`; with `strict`, `None` on an HTTP error too (`curl -f`).
pub fn get(url: &str, seconds: u32, strict: bool) -> Option<Vec<u8>> {
    if let Some(f) = FETCHER.get() {
        return f.get(url, seconds, strict);
    }
    let time = seconds.to_string();
    let out = Command::new("curl").args([if strict { "-sfL" } else { "-sL" }, "--max-time", &time, url]).output().ok()?;
    (out.status.success() || !strict).then_some(out.stdout)
}

/// `url` saved to the file `to` (an HTTP error is a failure).
pub fn save(url: &str, to: &Path, seconds: u32) -> bool {
    if let Some(f) = FETCHER.get() {
        return f.save(url, to, seconds);
    }
    Command::new("curl")
        .args(["-sfL", "--max-time", &seconds.to_string(), "-o"])
        .arg(to)
        .arg(url)
        .status()
        .is_ok_and(|s| s.success())
}
