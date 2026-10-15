//! The files Tally keeps, all in one folder (`~/.config/tally`, `$XDG_CONFIG_HOME/tally`):
//! the settings, the online data, the exchange rates, the city archive, the notices' state.
//! Each is written atomically: a new file next to it, renamed over it, so a reader (another
//! tally, an interrupted sync) sees the old version or the new one, never a part.

use std::io::Write;
use std::path::{Path, PathBuf};

static APP_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// The folder given by the app embedding tally (`<dir>/tally`: its own storage, there is no
/// `HOME`); set once, before anything is read. The command line doesn't set it.
pub fn set_dir(base: &Path) {
    let _ = APP_DIR.set(base.join("tally"));
}

/// `~/.config/tally` (`$XDG_CONFIG_HOME/tally`), or the app's (`set_dir`).
pub fn dir() -> Option<PathBuf> {
    if let Some(dir) = APP_DIR.get() {
        return Some(dir.clone());
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("tally"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_whole_and_replaced() {
        let dir = std::env::temp_dir().join(format!("tally-files-{}", std::process::id()));
        let path = dir.join("deep").join("data.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
        // Nothing left behind but the file.
        assert_eq!(std::fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
