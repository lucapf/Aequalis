//! City time zones end to end: `Paris` (now there), `10:00AM Paris` (my 10:00 there),
//! `10:00AM New York to Paris` (New York's 10:00 there). The device zone is pinned to New York,
//! the zones come from the system's database (skipped where it is not installed).

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

fn run(args: &str) -> (Result<String, String>, i32) {
    let out = Command::new(BIN)
        .arg("-v")
        .args(args.split_whitespace())
        .env("TZ", "America/New_York")
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let code = out.status.code().unwrap_or(-1);
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (if code == 0 { Ok(text(out.stdout)) } else { Err(text(out.stderr)) }, code)
}

fn ok(args: &str) -> String {
    run(args).0.unwrap_or_else(|e| panic!("{args}: {e}"))
}

fn installed() -> bool {
    std::path::Path::new("/usr/share/zoneinfo/Europe/Paris").exists()
}

/// Today's date in a zone, as the binary sees it (`YYYY-MM-DD`).
fn today(zone: &str) -> String {
    let out = Command::new(BIN).arg("-v").args(["now", "iso-date"]).env("TZ", zone).output().unwrap();
    String::from_utf8(out.stdout).unwrap().trim().rsplit(' ').nth(1).unwrap().to_string()
}

#[test]
fn the_reported_forms() {
    if !installed() {
        return;
    }
    // `Paris`: now there, to the minute.
    let paris = ok("Paris");
    assert!(paris.ends_with(" Paris") && !paris.contains(" = "), "{paris}");
    let iso = paris.split(' ').next().unwrap();
    assert_eq!(iso.len(), "2026-10-11T08:03+02:00".len(), "{iso}");
    // `10:00AM Paris`: 10:00 today here (New York), in Paris.
    let day = today("America/New_York");
    let out = ok("10:00AM Paris");
    assert!(out.starts_with(&format!("{day}T10:00-0")), "{out}");
    assert!(
        out.contains(" iso = ") && out.ends_with("T16:00+02:00 Paris") || out.ends_with("T16:00+01:00 Paris") || out.ends_with("T15:00+01:00 Paris"),
        "{out}"
    );
    // `10:00AM New York to Paris`: the same, named.
    let out = ok("10:00AM New York to Paris");
    assert!(out.starts_with(&format!("{day}T10:00-0")) && out.contains(" New York = "), "{out}");
    // Without AM / PM: 24-hour.
    assert_eq!(ok("10:00 Paris"), ok("10:00AM Paris"));
    assert_eq!(ok("22:00 Paris"), ok("10:00 PM Paris"));
}

#[test]
fn exact_dates_across_dst() {
    if !installed() {
        return;
    }
    // Summer: New York −04:00, Paris +02:00, Tokyo +09:00 (no DST), Sydney +10:00 (winter there).
    assert_eq!(ok("2026-07-01T10:00 New York to Paris"), "2026-07-01T10:00-04:00 New York = 2026-07-01T16:00+02:00 Paris");
    assert_eq!(ok("2026-07-01T12:00Z Sydney"), "2026-07-01T12:00Z iso = 2026-07-01T22:00+10:00 Sydney");
    // Winter: New York −05:00, Paris +01:00, Sydney +11:00 (summer there).
    assert_eq!(ok("2026/12/04 10:00 PM Paris"), "2026-12-04T22:00-05:00 iso = 2026-12-05T04:00+01:00 Paris");
    assert_eq!(ok("2026-12-04T12:00Z Sydney"), "2026-12-04T12:00Z iso = 2026-12-04T23:00+11:00 Sydney");
    // Between the two DST changes (US first, Europe later): a 5-hour gap for three weeks.
    assert_eq!(ok("2026-03-20T10:00 New York to Paris"), "2026-03-20T10:00-04:00 New York = 2026-03-20T15:00+01:00 Paris");
    // The date changes with the time.
    assert_eq!(ok("2026-12-04T22:00 Tokyo to Los Angeles"), "2026-12-04T22:00+09:00 Tokyo = 2026-12-04T05:00-08:00 Los Angeles");
    // Half-hour zones, accents optional, zone names.
    assert_eq!(ok("2026-12-04T12:00Z New Delhi"), "2026-12-04T12:00Z iso = 2026-12-04T17:30+05:30 New Delhi");
    assert_eq!(ok("2026-12-04T12:00Z sao paulo"), "2026-12-04T12:00Z iso = 2026-12-04T09:00-03:00 São Paulo");
    assert_eq!(ok("2026-12-04T12:00Z Europe/Lisbon"), "2026-12-04T12:00Z iso = 2026-12-04T12:00+00:00 Lisbon");
    // Date math, Unix time, a GMT zone, seconds written.
    assert_eq!(ok("2026-12-04T12:00Z + 3h Tokyo"), "2026-12-04T15:00Z iso = 2026-12-05T00:00+09:00 Tokyo");
    assert_eq!(ok("1796338800 unix Tokyo"), "2026-12-03T18:00-05:00 iso = 2026-12-04T08:00+09:00 Tokyo");
    assert_eq!(ok("2026-12-04 10:00 GMT+2 Paris"), "2026-12-04T10:00+02:00 iso = 2026-12-04T09:00+01:00 Paris");
    assert_eq!(ok("2026-12-04T10:00:30 New York to Paris"), "2026-12-04T10:00:30-05:00 New York = 2026-12-04T16:00:30+01:00 Paris");
}

#[test]
fn errors() {
    if !installed() {
        return;
    }
    for (args, message) in [
        ("Paris 10:00", "put the city at the end"),
        ("10:00 Paris unix", "put the city at the end"),
        ("10:00AM Par", "`Par` is not a city"),
        ("10:00 Atlantis", "`Atlantis` is not a city"),
        ("10:00AM", "a time alone takes a city"),
        ("10:00AM New York to", "now the city to convert to"),
        ("Paris to", "now the city to convert to: `Paris to Rome`"),
        ("10:00 GMT+2 London to Paris", "two time zones"),
        ("2026-12-04T10:00Z New York to Paris", "has its own time zone"),
        ("2026-12-25 - 2026-12-04 Paris", "a difference of dates is a duration"),
    ] {
        let (out, code) = run(args);
        assert_eq!(code, 2, "{args}");
        let err = out.unwrap_err();
        assert!(err.contains(message), "{args}: {err}");
    }
    // An ambiguous date keeps the cities in the suggested request.
    let err = run("12/04/2026 10:00 Paris").0.unwrap_err();
    assert!(err.contains("`12/04/2026 10:00 MM/DD/YYYY-24h Paris`"), "{err}");
    // Not cities: units and numbers are unchanged.
    assert_eq!(ok("10 km mi"), "6.2137 mi");
    assert_eq!(ok("5 h min"), "300 min");
}

#[test]
fn misspelt_cities_are_suggested() {
    if !installed() {
        return;
    }
    // Within 2 letters (case and spaces aside): the city is suggested, with the request to run.
    for (args, city, request) in [
        ("10:00 Tokio", "Tokyo", "10:00 Tokyo"),
        ("Tokio", "Tokyo", "Tokyo"),
        ("10:00AM New Yrok to Paris", "New York", "10:00AM New York to Paris"),
        ("10:00 pariss", "Paris", "10:00 Paris"),
        ("2026/12/04 10:00 PM Bejing", "Beijing", "2026/12/04 10:00 PM Beijing"),
    ] {
        let (out, code) = run(args);
        assert_eq!(code, 2, "{args}");
        let err = out.unwrap_err();
        assert!(err.contains(&format!("is not a city: did you mean {city}? (`{request}`)")), "{args}: {err}");
    }
    // Spaces aside, the same name.
    assert!(ok("-v 2026-12-04T10:00 NewYork to Paris").starts_with("2026-12-04T10:00-05:00 New York = "));
    // More than 2 letters off: not a city.
    assert!(run("10:00 Gotham").0.unwrap_err().contains("`Gotham` is not a city (`10:00AM Paris`"));
    // Units are not misspelt cities.
    assert_eq!(ok("5 h min"), "300 min");
    // --json: the choices, to click.
    let out = Command::new(BIN).args(["--json", "10:00", "Tokio"]).output().unwrap();
    let json = String::from_utf8(out.stdout).unwrap();
    // `note`: what was misspelt, for the question.
    assert!(json.contains(r#""ambiguous":{"kind":"city","text":"Tokio","note":"`Tokio` is not a city","choices":[{"pattern":"Tokyo","date":"Asia/Tokyo","request":"10:00 Tokyo"}]}"#), "{json}");
}

#[test]
fn misspelt_city_asked_at_a_terminal() {
    use std::io::{Read, Write};
    if !installed() {
        return;
    }
    // A terminal for the binary: util-linux's `script` (`-qec <command> <file>`), or the BSD one
    // on macOS (`-q <file> <command>…`). No `script` (Windows): nothing to check.
    let request = ["2026-12-04T10:00", "New", "York", "to", "Tokio"];
    let args: Vec<String> = match cfg!(target_os = "linux") {
        true => vec!["-qec".into(), format!("'{BIN}' {}", request.join(" ")), "/dev/null".into()],
        false => ["-q", "/dev/null", BIN].into_iter().chain(request).map(String::from).collect(),
    };
    let Ok(mut child) =
        Command::new("script").args(&args).env("TZ", "America/New_York").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn()
    else {
        return;
    };
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (sent, received) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut buf = [0; 4096];
        while let Ok(n @ 1..) = stdout.read(&mut buf) {
            let _ = sent.send(String::from_utf8_lossy(&buf[..n]).into_owned());
        }
    });
    // Waits, however slow the machine, for `text` in what the terminal shows.
    let mut out = String::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let wait = |out: &mut String, text: &str| {
        while !out.contains(text) {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match received.recv_timeout(left) {
                Ok(chunk) => out.push_str(&chunk),
                Err(_) => panic!("no `{text}`: {out}"),
            }
        }
    };
    // Enter once the question is shown: `\n`, a line's end whether or not the terminal turns `\r`
    // into one (macOS's `script` leaves its terminal raw when its input is no terminal). The input
    // stays open until the answer: its end would be the answer (`^D`) if it came first.
    wait(&mut out, "Enter = 1]: ");
    stdin.write_all(b"\n").unwrap();
    wait(&mut out, "GMT+9 Tokyo");
    drop(stdin);
    child.wait().unwrap();
    reader.join().unwrap();
    out.extend(received.try_iter());
    let out = out.replace('\r', "");
    assert!(out.contains("`Tokio` is not a city. Did you mean:\n  1) Tokyo        Asia/Tokyo\n"), "{out}");
    assert!(out.contains("Which city? [1-1, Enter = 1]: "), "{out}");
    assert!(out.contains("05 Dec 26 12:00 AM GMT+9 Tokyo"), "Enter: Tokyo: {out}");
}
