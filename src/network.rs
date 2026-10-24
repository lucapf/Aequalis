//! Online or offline: the `offline` setting (`settings.rs`; offline: no network at all), and
//! the status shown (from the online data's last sync, `online.rs`). Only the sync of the online
//! data uses the network.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Online,
    Offline,
}

/// What the indicator shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    /// Never synced yet.
    Checking,
    Online,
    Offline,
    /// The `offline` setting.
    Forced,
}

impl Status {
    /// `online`, `offline`, `offline (forced)`, `checking…`.
    pub fn label(self) -> &'static str {
        match self {
            Status::Checking => "not synced",
            Status::Online => "online",
            Status::Offline => "offline",
            Status::Forced => "offline (forced)",
        }
    }
}

/// The mode, from the settings.
pub fn mode() -> Mode {
    if crate::settings::current().offline() {
        Mode::Offline
    } else {
        Mode::Online
    }
}

/// Whether the network may be used: not the `offline` setting.
pub fn allowed() -> bool {
    mode() == Mode::Online
}

/// The status, from the online data's last sync (`online.rs`): no probing, so a fresh file means
/// no network at all. `forced`: the `offline` setting.
pub fn status_of(data: &crate::online::Data, forced: bool) -> Status {
    match (forced, data.attempted_at, data.ok) {
        (true, ..) => Status::Forced,
        (_, None, _) => Status::Checking,
        (_, Some(_), true) => Status::Online,
        // Not every source answered, but some did (a value synced during the last try): online.
        (_, Some(tried), false) if data.values.iter().any(|(_, s)| s.at >= tried) => Status::Online,
        (_, Some(_), false) => Status::Offline,
    }
}

/// The status now, for `--online`: from the newest download tried (the live values or the
/// exchange rates).
pub fn status() -> Status {
    let (live, rates) = (crate::online::load(), crate::currency::load());
    let newest = if rates.attempted_at > live.attempted_at { &*rates } else { &live };
    status_of(newest, mode() == Mode::Offline)
}

/// The online state and the age of the last sync, for people, as every UI says it: `online (2h old)`,
/// `offline (forced, 2d old)`, `online (just synced)`, `never synced`. `status` is `Status::label`, `synced`
/// the `synced_ago`.
pub fn status_line(status: &str, synced: Option<&str>) -> String {
    let age = match synced {
        None => "never synced".to_string(),
        Some("now") => "just synced".to_string(),
        Some(ago) => format!("{ago} old"),
    };
    match status {
        "offline (forced)" => format!("offline (forced, {age})"),
        "not synced" => age,
        other => format!("{other} ({age})"),
    }
}

/// `status_line` of this machine's online data.
pub fn line() -> String {
    status_line(status().label(), synced_ago().as_deref())
}

/// How long ago the online data was last synced, short (`ago`); `None`: never.
pub fn synced_ago() -> Option<String> {
    since_sync(&crate::online::load())
}

/// How long ago `data` was last synced, short (`ago`); `None`: never.
pub fn since_sync(data: &crate::online::Data) -> Option<String> {
    data.last_sync().map(|t| ago(crate::time::unix_now() - t))
}

/// A time ago, short, in its largest whole unit: `now`, `5m`, `2h`, `3d`, `2w`, `4mo`, `1y`.
pub fn ago(seconds: i64) -> String {
    let s = seconds.max(0);
    match s {
        0..60 => "now".to_string(),
        60..3_600 => format!("{}m", s / 60),
        3_600..86_400 => format!("{}h", s / 3_600),
        86_400..604_800 => format!("{}d", s / 86_400),
        604_800..2_592_000 => format!("{}w", s / 604_800),
        2_592_000..31_557_600 => format!("{}mo", s / 2_592_000),
        _ => format!("{}y", s / 31_557_600),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_status_line() {
        use super::status_line as line;
        assert_eq!(line("online", Some("2h")), "online (2h old)");
        assert_eq!(line("online", Some("now")), "online (just synced)");
        assert_eq!(line("offline", Some("3d")), "offline (3d old)");
        assert_eq!(line("offline (forced)", Some("2d")), "offline (forced, 2d old)");
        assert_eq!(line("offline (forced)", None), "offline (forced, never synced)");
        assert_eq!(line("not synced", None), "never synced");
    }

    use super::*;

    #[test]
    fn ages() {
        let ages: Vec<String> = [0, 59, 60, 3_599, 3_600, 86_400, 13 * 86_400, 14 * 86_400, 40 * 86_400, 400 * 86_400].map(ago).into();
        assert_eq!(ages, ["now", "now", "1m", "59m", "1h", "1d", "1w", "2w", "1mo", "1y"]);
    }

    #[test]
    fn labels() {
        assert_eq!(Status::Forced.label(), "offline (forced)");
        assert_eq!(Status::Online.label(), "online");
    }
}
