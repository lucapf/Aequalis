//! Memory: every run of tally stays below 10 MB (its peak, `peak_kb` in the `--json` reply):
//! requests of every kind, and installing the city archive (an external merge sort keeps its
//! index out of memory). docs/REVIEW.md has the measurements on the real archive.

mod common;

use std::path::PathBuf;
use std::process::Command;

const LIMIT_KB: u64 = 10 * 1024;

/// The peak memory a `--json` reply reports.
fn peak_kb(reply: &str) -> u64 {
    let at = reply.rfind("\"peak_kb\":").unwrap_or_else(|| panic!("no peak_kb in {reply}")) + "\"peak_kb\":".len();
    reply[at..].trim_end().trim_end_matches('}').parse().unwrap()
}

#[test]
fn requests_stay_below_10_mb() {
    let dir = common::config("memory-requests", &[]);
    for request in [
        "10 km mi",
        "12+4/2",
        "sqrt 16",
        "now Tokyo",
        "10:00AM Paris to New York",
        "Rome Paris",
        "10:00am jappan romme",
        "2026/12/04 + 5wd",
        "12/04/2026 unix",
        "12 usd eur",
        "12 dollar euro",
    ] {
        let args: Vec<&str> = std::iter::once("--json").chain(request.split(' ')).collect();
        let reply = common::out(&common::tally(&dir, &args));
        let kb = peak_kb(&reply);
        assert!(kb > 0 && kb < LIMIT_KB, "{request}: {kb} KB");
    }
}

#[test]
fn installing_the_city_archive_stays_below_10_mb() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cities-sample.json");
    let serve = std::env::temp_dir().join(format!("tally-memory-archive-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&serve);
    std::fs::create_dir_all(&serve).unwrap();
    let gz = std::fs::File::create(serve.join("cities.json.gz")).unwrap();
    assert!(Command::new("gzip").arg("-c").arg(&fixture).stdout(gz).status().unwrap().success());
    std::fs::write(serve.join("cities.version"), "1 sample\n").unwrap();
    let url = format!("file://{}", serve.join("cities.json.gz").display());
    let dir = common::config("memory-archive", &[("offline", "false"), ("cities.refresh", "manual"), ("rates.refresh", "manual"), ("live.refresh", "manual"), ("cities.url", &url)]);
    let reply = common::out(&common::tally(&dir, &["--json", "--sync", "cities"]));
    assert!(reply.starts_with(r#"{"ok":true,"result":"9 cities""#), "{reply}");
    let kb = peak_kb(&reply);
    assert!(kb > 0 && kb < LIMIT_KB, "{kb} KB");
}
