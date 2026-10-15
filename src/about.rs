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
/// formatted: the CLI's process, the interactive mode's), the last online sync, the exchange rates,
/// the mode.
pub fn facts(memory: &str) -> Vec<(&'static str, String)> {
    let sync = match crate::online::load().last_sync() {
        Some(t) => crate::time::human_moment(t),
        None => "never".to_string(),
    };
    let mode = match crate::network::mode() {
        crate::network::Mode::Online => "online",
        crate::network::Mode::Offline => "offline (the offline setting)",
    };
    // The exchange rates: their date and where they come from (its terms ask to name it).
    let rates = match crate::currency::load().get(crate::currency::RATES_DATE) {
        Some(day) => {
            let day = crate::time::human_day(day.value as i64);
            format!("of {day}, {}", crate::currency::SOURCE)
        }
        None => format!("not synced yet, {}", crate::currency::SOURCE),
    };
    // The city archive: how many, of when, from where (GeoNames, CC BY 4.0), the policy.
    let settings = crate::settings::current();
    let cities = match (settings.cities_enabled(), crate::cities::meta()) {
        (false, _) => "off (the cities.enabled setting)".to_string(),
        (true, Some(m)) => {
            let day = crate::time::human_day(m.built_at);
            format!(
                "{} of {day}, {} (refresh: {})",
                crate::numbers::device().apply(&m.count.to_string()),
                crate::cities::SOURCE,
                settings.get("cities.refresh")
            )
        }
        (true, None) => format!("not downloaded yet, {}", crate::cities::SOURCE),
    };
    vec![
        ("Version", env!("CARGO_PKG_VERSION").to_string()),
        ("Memory", memory.to_string()),
        ("Last online sync", sync),
        ("Exchange rates", rates),
        ("Cities", cities),
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
        assert!(text.contains("\nExchange rates: ") && text.contains("ExchangeRate-API (https://www.exchangerate-api.com)"), "{text}");
        assert!(text.contains("\nCities: ") && text.contains("GeoNames (https://www.geonames.org), CC BY 4.0"), "{text}");
    }
}
