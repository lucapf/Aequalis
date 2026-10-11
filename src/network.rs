//! Online or offline: `TALLY_MODE` (`online`, the default, or `offline`: no network at all), and
//! whether the live sources can be reached (a TCP connection, std only). Only the Trivia's live
//! values use the network (`trivia.rs`).

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Online,
    Offline,
}

/// What the indicator shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    /// Not known yet (the interactive mode's first check).
    #[cfg_attr(not(feature = "tui"), allow(dead_code))]
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
            Status::Checking => "checking…",
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

/// The live sources (JPL Horizons, CelesTrak), checked for connectivity.
const HOSTS: [&str; 2] = ["ssd.jpl.nasa.gov:443", "celestrak.org:443"];

/// Whether any live source accepts a connection within `timeout`.
pub fn reachable(timeout: Duration) -> bool {
    HOSTS.iter().any(|host| {
        host.to_socket_addrs()
            .map(|mut addrs| addrs.any(|addr| TcpStream::connect_timeout(&addr, timeout).is_ok()))
            .unwrap_or(false)
    })
}

/// The status now: forced offline, else whether a live source can be reached.
pub fn status(timeout: Duration) -> Status {
    match mode() {
        Ok(Mode::Offline) => Status::Forced,
        _ if reachable(timeout) => Status::Online,
        _ => Status::Offline,
    }
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
