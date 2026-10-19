//! The files Aequalis keeps, all in one folder (`~/.config/aequalis`, `$XDG_CONFIG_HOME/aequalis`,
//! `%APPDATA%\aequalis` on Windows):
//! the settings, the online data, the exchange rates, the city archive, the notices' state.
//! Each is written atomically: a new file next to it, renamed over it, so a reader (another
//! aequalis, an interrupted sync) sees the old version or the new one, never a part.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

static APP_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// The folder given by the app embedding aequalis (`<dir>/aequalis`: its own storage, there is no
/// `HOME`); set once, before anything is read. The command line doesn't set it.
pub fn set_dir(base: &Path) {
    let _ = APP_DIR.set(base.join("aequalis"));
}

/// `~/.config/aequalis` (`$XDG_CONFIG_HOME/aequalis`; on Windows `%APPDATA%\aequalis`), or the app's
/// (`set_dir`).
pub fn dir() -> Option<PathBuf> {
    if let Some(dir) = APP_DIR.get() {
        return Some(dir.clone());
    }
    let xdg = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(PathBuf::from);
    // Windows: not `HOME`, which only some shells set (the same aequalis would keep two folders).
    #[cfg(windows)]
    let base = xdg.or_else(crate::windows::app_data)?;
    #[cfg(not(windows))]
    let base = xdg.or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("aequalis"))
}

/// Writes `bytes` to `path` atomically: to `<name>.<pid>.tmp` next to it, synced, then renamed
/// over it. Creates the folder if needed.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let name = path.file_name().map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
    let tmp = path.with_file_name(format!("{name}.{}.tmp", std::process::id()));
    let mut file = std::fs::File::create(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)
}

/// What tells one version of a file from the next: when it was last written, and its length (two
/// writes within the clock's step still differ in most cases). `None`: no such file.
pub type Stamp = (SystemTime, u64);

pub fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// What was read from a file, kept until the file changes (a background sync writes a new
/// version): a long-running program (the interactive mode, the apps) reads it once per version, a
/// request once.
pub struct Cached<T> {
    kept: Mutex<Option<(PathBuf, Stamp, Arc<T>)>>,
}

impl<T> Default for Cached<T> {
    fn default() -> Self {
        Cached::new()
    }
}

impl<T> Cached<T> {
    pub const fn new() -> Self {
        Cached { kept: Mutex::new(None) }
    }

    /// What `read` makes of the file at `path`, read again only when its stamp changed; `None` when
    /// there is no file, or `read` gives nothing (then asked again next time).
    pub fn get(&self, path: &Path, read: impl FnOnce() -> Option<T>) -> Option<Arc<T>> {
        let stamp = stamp(path)?;
        let mut kept = self.kept.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((p, s, value)) = &*kept {
            if p == path && *s == stamp {
                return Some(Arc::clone(value));
            }
        }
        let value = Arc::new(read()?);
        *kept = Some((path.to_path_buf(), stamp, Arc::clone(&value)));
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kept_until_the_file_changes() {
        let dir = std::env::temp_dir().join(format!("aequalis-cached-{}", std::process::id()));
        let path = dir.join("data.txt");
        let cache: Cached<String> = Cached::new();
        let reads = std::cell::Cell::new(0);
        let read = || {
            reads.set(reads.get() + 1);
            std::fs::read_to_string(&path).ok()
        };
        assert_eq!(cache.get(&path, read), None, "no file");
        write_atomic(&path, b"one").unwrap();
        assert_eq!(cache.get(&path, read).as_deref().map(String::as_str), Some("one"));
        assert_eq!(cache.get(&path, read).as_deref().map(String::as_str), Some("one"));
        assert_eq!(reads.get(), 1, "read once while the file stays the same");
        write_atomic(&path, b"three").unwrap();
        assert_eq!(cache.get(&path, read).as_deref().map(String::as_str), Some("three"));
        assert_eq!(reads.get(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn written_whole_and_replaced() {
        let dir = std::env::temp_dir().join(format!("aequalis-files-{}", std::process::id()));
        let path = dir.join("deep").join("data.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
        // Nothing left behind but the file.
        assert_eq!(std::fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
