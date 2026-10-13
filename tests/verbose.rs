//! By default only the result: no notes (stderr, the TUI's status line, `--json`) and no ISO side
//! (`<iso> iso = …`). `-v` / `--verbose` adds both. `TZ` and the locale are pinned.

mod common;

use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_tally");

/// (stdout, stderr) of a run.
fn run(args: &str) -> (String, String) {
    let out = Command::new(BIN)
        .args(args.split_whitespace())
        .env("TZ", "Europe/Rome")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr))
}

#[test]
fn dates_show_only_the_result() {
    for (request, quiet, verbose) in [
        ("2026/04/12 date unix", "1775944800 unix", "2026-04-12T00:00+02:00 iso = 1775944800 unix"),
        ("1775944800 unix iso-week", "2026-W15-7 iso-week", "2026-04-12T00:00+02:00 iso = 2026-W15-7 iso-week"),
        // A date alone: for people (the `format.date`, `format.datetime` settings).
        ("2026/12/04 10:00 PM", "04 Dec 26 10:00 PM GMT+1", "2026-12-04T22:00+01:00 iso = 2026/12/04 10:00:00 PM YYYY/MM/DD-12h"),
        ("1796338800 unix", "04 Dec 26", "2026-12-04T00:00+01:00 iso = 1796338800 unix"),
        ("12 Sept 2026", "12 Sept 26", "2026-09-12T00:00+02:00 iso = 12/09/2026 DD/MM/YYYY"),
        ("2026/12/04 + 5wd", "2026/12/11 YYYY/MM/DD", "2026/12/11 YYYY/MM/DD"),
    ] {
        let (out, err) = run(request);
        assert_eq!((out.as_str(), err.as_str()), (quiet, ""), "{request}");
        for flag in ["-v", "--verbose"] {
            assert_eq!(run(&format!("{flag} {request}")).0, verbose, "{flag} {request}");
        }
    }
}

#[test]
fn notes_only_with_verbose() {
    // How a date was read, how an expression was grouped.
    assert_eq!(run("2026/12/04 unix").1, "");
    assert_eq!(run("-v 2026/12/04 unix").1, "tally: note: read `2026/12/04` as YYYY/MM/DD (year first)");
    assert_eq!(run("100 + 20 + 50 / 10"), ("125".into(), "".into()));
    assert_eq!(run("-v 100 + 20 + 50 / 10"), ("125".into(), "tally: note: evaluated as (100 + 20) + (50 / 10)".into()));
    assert_eq!(run("-v -s 100 + 20 + 50 / 10").1, "", "--silent wins");
    // Warnings are not notes: still shown.
    assert!(run("1 GB GiB").1.starts_with("tally: warning: mixing decimal"));
    // In --json too.
    assert!(run("--json 2026/12/04 unix").0.contains("\"result\":\"1796338800 unix\",\"notes\":[]"));
    assert!(run("--json -v 2026/12/04 unix").0.contains("\"notes\":[\"read `2026/12/04` as YYYY/MM/DD (year first)\"]"));
}

#[test]
fn cities_show_only_the_time_there() {
    if !std::path::Path::new("/usr/share/zoneinfo/Europe/Paris").exists() {
        return;
    }
    assert_eq!(run("2026-12-04T10:00 New York to Paris").0, "04 Dec 26 4:00 PM GMT+1 Paris");
    assert_eq!(run("-v 2026-12-04T10:00 New York to Paris").0, "2026-12-04T10:00-05:00 New York = 2026-12-04T16:00+01:00 Paris");
    assert_eq!(run("2026-12-04T10:00Z Tokyo").0, "04 Dec 26 7:00 PM GMT+9 Tokyo");
    assert_eq!(run("-v 2026-12-04T10:00Z Tokyo").0, "2026-12-04T10:00Z iso = 2026-12-04T19:00+09:00 Tokyo");
}

/// (stdout) with the human formats set.
fn with_formats(date: Option<&str>, datetime: Option<&str>, args: &str) -> String {
    let mut settings = Vec::new();
    if let Some(f) = date {
        settings.push(("format.date", f));
    }
    if let Some(f) = datetime {
        settings.push(("format.datetime", f));
    }
    let dir = common::config(&format!("formats-{}", settings.len() + date.map_or(0, str::len) * 7 + datetime.map_or(0, str::len) * 13), &settings);
    let mut cmd = Command::new(BIN);
    cmd.args(args.split_whitespace()).env("TZ", "Europe/Rome").env("XDG_CONFIG_HOME", &dir);
    String::from_utf8(cmd.output().unwrap().stdout).unwrap().trim_end().to_string()
}

#[test]
fn dates_for_people() {
    // The defaults: `DD MMM YY` and `DD MMM YY H24:mm tz`.
    assert_eq!(with_formats(None, None, "12 Sept 2026"), "12 Sept 26");
    assert_eq!(with_formats(None, None, "2026-09-12 20:00"), "12 Sept 26 8:00 PM GMT+2");
    assert_eq!(with_formats(None, None, "2026-12-04 + 1d"), "05 Dec 26", "an ISO date after date math");
    assert_eq!(with_formats(None, None, "2026-12-04T15:00Z + 1h"), "04 Dec 26 4:00 PM GMT");
    assert_eq!(with_formats(None, None, "2026-12-04 10:00 GMT+5:30"), "04 Dec 26 10:00 AM GMT+5:30");
    assert_eq!(with_formats(None, None, "today").len(), "11 Oct 26".len(), "today: the date");
    assert!(with_formats(None, None, "now").ends_with(" GMT+2") || with_formats(None, None, "now").ends_with(" GMT+1"), "now: with the time");
    // Unchanged: an explicit target, and date math on a written pattern (its own format).
    assert_eq!(with_formats(None, None, "2026-12-04T22:00 iso"), "2026-12-04T22:00+01:00 iso");
    assert_eq!(with_formats(None, None, "2026/12/04 + 5wd"), "2026/12/11 YYYY/MM/DD");
    // Configured.
    assert_eq!(with_formats(Some("dddd D MMMM YYYY"), None, "12 Sept 2026"), "Saturday 12 September 2026");
    assert_eq!(with_formats(None, Some("ddd DD/MM/YYYY hh:mm A ZZ [at] tz"), "2026/12/04 10:00 PM"), "Fri 04/12/2026 10:00 PM +01:00 at GMT+1");
    assert_eq!(with_formats(None, Some("YYYY-MM-DD H:mm:ss"), "2026-12-04 9:05:07"), "2026-12-04 9:05:07");
    assert_eq!(with_formats(Some(""), None, "12 Sept 2026"), "", "empty: rejected (the setting needs a pattern)");
    assert_eq!(with_formats(None, Some("h:mm A tz"), "10:00 PM Paris"), with_formats(None, Some("h:mm A tz"), "22:00 Paris"));
}
