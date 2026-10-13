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
    if crate::settings::current().offline() { Mode::Offline } else { Mode::Online }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(Status::Forced.label(), "offline (forced)");
        assert_eq!(Status::Online.label(), "online");
    }
}
