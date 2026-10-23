//! Downloads. The command line runs `curl`; an app that embeds aequalis (the Android app, `ffi.rs`)
//! has no `curl`, so it registers its own `Fetcher` once, at start.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

/// Downloads for aequalis: the body of `url` (`None`: no answer, or with `strict` an HTTP error), or
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

/// A download on its way, as curl's progress meter says it: bytes received, the size when known,
/// the current speed in bytes a second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub received: u64,
    pub total: Option<u64>,
    pub speed: u64,
}

type Observer = Box<dyn Fn(Progress) + Send>;

pub(crate) static OBSERVER: Mutex<Option<Observer>> = Mutex::new(None);

/// Told each download's progress from now on (`None`: no more): a sync by hand writes it for the
/// interactive mode (`online::track`). curl then runs with its progress meter.
pub fn observe(observer: Option<Observer>) {
    *OBSERVER.lock().unwrap_or_else(|e| e.into_inner()) = observer;
}

fn observed() -> bool {
    OBSERVER.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}

/// The body of `url`; with `strict`, `None` on an HTTP error too (`curl -f`).
pub fn get(url: &str, seconds: u32, strict: bool) -> Option<Vec<u8>> {
    if let Some(f) = FETCHER.get() {
        return f.get(url, seconds, strict);
    }
    let time = seconds.to_string();
    if observed() {
        let (ok, body) = with_meter(&[if strict { "-fL" } else { "-L" }, "--max-time", &time, url])?;
        return (ok || !strict).then_some(body);
    }
    let out = Command::new("curl").args([if strict { "-sfL" } else { "-sL" }, "--max-time", &time, url]).output().ok()?;
    (out.status.success() || !strict).then_some(out.stdout)
}

/// `url` saved to the file `to` (an HTTP error is a failure).
pub fn save(url: &str, to: &Path, seconds: u32) -> bool {
    if let Some(f) = FETCHER.get() {
        return f.save(url, to, seconds);
    }
    let time = seconds.to_string();
    if observed() {
        let Some(to) = to.to_str() else { return false };
        return with_meter(&["-fL", "--max-time", &time, "-o", to, url]).is_some_and(|(ok, _)| ok);
    }
    Command::new("curl").args(["-sfL", "--max-time", &time, "-o"]).arg(to).arg(url).status().is_ok_and(|s| s.success())
}

/// curl with its progress meter on stderr, each update told to the observer: whether it succeeded,
/// and its stdout.
fn with_meter(args: &[&str]) -> Option<(bool, Vec<u8>)> {
    let mut child = Command::new("curl").args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let body = std::thread::spawn(move || {
        let mut out = Vec::new();
        stdout.read_to_end(&mut out).map(|_| out)
    });
    let mut stderr = child.stderr.take()?;
    let (mut buf, mut line) = ([0u8; 4096], Vec::new());
    while let Ok(n @ 1..) = stderr.read(&mut buf) {
        for &b in &buf[..n] {
            if b != b'\r' && b != b'\n' {
                line.push(b);
                continue;
            }
            if let Some(p) = meter(&String::from_utf8_lossy(&line)) {
                if let Some(observer) = OBSERVER.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                    observer(p);
                }
            }
            line.clear();
        }
    }
    let ok = child.wait().ok()?.success();
    Some((ok, body.join().ok()?.ok()?))
}

/// A line of curl's progress meter (`% Total % Received % Xferd Average Speed Time Time Time
/// Current`): ` 45 20.4M   45 9440k    0     0  1567k      0  0:00:13  0:00:06  0:00:07 1612k`. Its
/// headings and messages are no progress.
fn meter(line: &str) -> Option<Progress> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() != 12 || fields[0].parse::<u32>().is_err() {
        return None;
    }
    let total = size(fields[1])?;
    Some(Progress { received: size(fields[3])?, total: (total > 0).then_some(total), speed: size(fields[11])? })
}

/// One of curl's sizes: `512`, `9440k`, `20.4M`, `1.2G` (1024-based).
fn size(text: &str) -> Option<u64> {
    let (number, unit) = match text.char_indices().last()? {
        (i, c @ ('k' | 'M' | 'G' | 'T')) => (&text[..i], c),
        _ => (text, ' '),
    };
    let scale = match unit {
        'k' => 1024.0,
        'M' => 1024.0 * 1024.0,
        'G' => 1024.0 * 1024.0 * 1024.0,
        'T' => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };
    number.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0).map(|v| (v * scale) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_progress_meter() {
        let p = meter(" 45 20.4M   45 9440k    0     0  1567k      0  0:00:13  0:00:06  0:00:07 1612k").unwrap();
        assert_eq!(p, Progress { received: 9440 * 1024, total: Some((20.4 * 1024.0 * 1024.0) as u64), speed: 1612 * 1024 });
        // Its start, the size not known yet.
        let p = meter("  0     0    0     0    0     0      0      0 --:--:-- --:--:-- --:--:--     0").unwrap();
        assert_eq!(p, Progress { received: 0, total: None, speed: 0 });
        for not in ["  % Total    % Received % Xferd  Average Speed   Time    Time     Time  Current", "curl: (22) The requested URL returned error: 404", ""] {
            assert_eq!(meter(not), None, "{not}");
        }
        assert_eq!((size("512"), size("1.5k"), size("2G")), (Some(512), Some(1536), Some(2 * 1024 * 1024 * 1024)));
        assert_eq!(size("abc"), None);
    }
}
