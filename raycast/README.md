# Tally for Raycast

[Tally](https://github.com/lucapf/tally) in Raycast: an ultralight calculator and converter made
for humans. Type a request and the answer appears as you type; Enter copies it.

- Units: `10 km mi`, `100 km/h mph`, `1 bar psi`
- Calculator: `12+4/2`, `sqrt 16`, `150 + 20%`
- Currencies: `100 usd eur` (European Central Bank reference rates)
- Dates: `12 Sept 2026 + 5wd`, `now unix`
- Time zones: `10:00AM Paris to New York`, `now Tokyo`
- Distances: `Rome to Paris`

When a request can be read in more than one way (`12/04/2026 unix`, `now usa`, a misspelt city),
the readings are listed: choose one and it is written in for you.

## Install Tally first

The extension runs the `tally` command. Download `tally-<version>-aarch64-macos.tar.gz` from the
[releases](https://github.com/lucapf/tally/releases) and copy `tally` to `~/.local/bin` (or
`/opt/homebrew/bin`). If it lives elsewhere, set its path in the extension's preferences.

## Preferences

- **tally Command**: the path of the tally binary (default: found automatically).
- **Offline**: never use the network (`TALLY_MODE=offline`); exchange rates and live values stay
  as last synced.
