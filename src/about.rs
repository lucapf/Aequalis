//! About tally: the name, the tagline, the memory footprint and the last online sync, with the
//! logo: a tally gate (four strokes crossed by a fifth) and the name. Shown by `tally --about`, the
//! interactive mode (`A`) and the widget (clicking its title).

/// The logo, 5 lines.
pub const ART: &str = r"\| | | |    _____     _ _
 |\| | |   |_   _|_ _| | |_  _
 | |\| |     | |/ _` | | | || |
 | | |\|     |_|\__,_|_|_|\_, |
 | | | |\                 |__/";

pub const NAME: &str = "Tally";
pub const TAGLINE: &str = "Ultralight calculator and converter made for humans";

/// The facts below the logo, as (label, text): the version, the memory (`memory`, already
/// formatted: the CLI's process, the interactive mode's), the last online sync, the mode.
pub fn facts(memory: &str) -> Vec<(&'static str, String)> {
    let sync = match crate::online::load().last_sync() {
        Some(t) => crate::time::human(&crate::time::Instant { ns: t as i128 * 1_000_000_000, zone: crate::time::Zone::Local }, false),
        None => "never".to_string(),
    };
    let mode = crate::network::mode().map_or("online", |m| match m {
        crate::network::Mode::Online => "online",
        crate::network::Mode::Offline => "offline (forced: TALLY_MODE=offline)",
    });
    vec![
        ("Version", env!("CARGO_PKG_VERSION").to_string()),
        ("Memory", memory.to_string()),
        ("Last online sync", sync),
        ("Mode", mode.to_string()),
    ]
}

/// This process's memory (now, where known, and peak) and the binary's size:
/// `2.9 MiB now · peak 3.0 MiB · binary 868 KiB`.
pub fn memory() -> String {
    use crate::json::{current_kb, peak_kb, size};
    let mut parts = Vec::new();
    if let Some(now) = current_kb() {
        parts.push(format!("{} now", size(now)));
    }
    parts.push(format!("peak {}", size(peak_kb())));
    if let Some(len) = std::env::current_exe().ok().and_then(|p| std::fs::metadata(p).ok()).map(|m| m.len()) {
        parts.push(format!("binary {}", size(len / 1024)));
    }
    parts.join(" · ")
}

/// `tally --about`: the logo, the name and tagline, then the facts.
pub fn text() -> String {
    let mut out = format!("{ART}\n\n{NAME} — {TAGLINE}\n\n");
    for (label, value) in facts(&memory()) {
        out.push_str(&format!("{label}: {value}\n"));
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_and_facts() {
        assert_eq!(ART.lines().count(), 5);
        assert!(ART.lines().all(|l| l.is_ascii() && l.chars().count() <= 34), "{ART}");
        let text = text();
        assert!(text.contains("Tally — Ultralight calculator and converter made for humans"), "{text}");
        assert!(text.contains("\nVersion: ") && text.contains("\nMemory: ") && text.contains(" peak "), "{text}");
        assert!(text.contains("\nLast online sync: "), "{text}");
    }
}
