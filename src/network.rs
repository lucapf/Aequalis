//! Online or offline: `TALLY_MODE` (`online`, the default, or `offline`: no network at all), and
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
    /// `TALLY_MODE=offline`.
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

/// `TALLY_MODE`, any case; unset or empty: online.
pub fn parse_mode(value: Option<&str>) -> Result<Mode, String> {
    match value.map(str::trim).filter(|v| !v.is_empty()).map(str::to_ascii_lowercase).as_deref() {
        None | Some("online") => Ok(Mode::Online),
        Some("offline") => Ok(Mode::Offline),
        Some(_) => Err(format!("TALLY_MODE must be online or offline (it is `{}`)", value.unwrap_or_default())),
    }
}

/// The mode from the environment.
pub fn mode() -> Result<Mode, String> {
    parse_mode(std::env::var("TALLY_MODE").ok().as_deref())
}

/// Whether the network may be used: the mode is online (an invalid mode counts as online; the
/// command line reports it as an error).
pub fn allowed() -> bool {
    mode() != Ok(Mode::Offline)
}

/// The status, from the online data's last sync (`online.rs`): no probing, so a fresh file means
/// no network at all. `forced`: `TALLY_MODE=offline`.
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

/// The status now, for `--online`.
pub fn status() -> Status {
    status_of(&crate::online::load(), mode() == Ok(Mode::Offline))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes() {
        assert_eq!(parse_mode(None), Ok(Mode::Online));
        assert_eq!(parse_mode(Some("")), Ok(Mode::Online));
        assert_eq!(parse_mode(Some("online")), Ok(Mode::Online));
        assert_eq!(parse_mode(Some("offline")), Ok(Mode::Offline));
        assert_eq!(parse_mode(Some(" OFFLINE ")), Ok(Mode::Offline));
        assert_eq!(parse_mode(Some("maybe")), Err("TALLY_MODE must be online or offline (it is `maybe`)".into()));
    }

    #[test]
    fn labels() {
        assert_eq!(Status::Forced.label(), "offline (forced)");
        assert_eq!(Status::Online.label(), "online");
    }
}
