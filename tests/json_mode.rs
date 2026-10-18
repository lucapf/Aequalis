//! `--json`: one JSON object per request on stdout, nothing on stderr, never a prompt; used by the
//! Omarchy bar widget. TZ and the locale are pinned.

use std::io::Write;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

fn json(args: &str) -> (String, String, i32) {
    let out = Command::new(BIN)
        .arg("-v")
        .arg("--json")
        .args(args.split_whitespace())
        .env("TZ", "Europe/Rome")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap();
    (without_peak(&text(out.stdout)), text(out.stderr), out.status.code().unwrap_or(-1))
}

/// Each line without its `"peak_kb":N` (the process's memory varies), checking it is there.
fn without_peak(out: &str) -> String {
    out.lines()
        .map(|line| {
            let (object, kb) = line.rsplit_once(",\"peak_kb\":").unwrap_or_else(|| panic!("no peak_kb: {line}"));
            let kb: u64 = kb.strip_suffix('}').unwrap().parse().unwrap();
            assert!((100..1_000_000).contains(&kb), "{kb} KiB");
            format!("{object}}}\n")
        })
        .collect()
}

#[test]
fn results_with_notes_and_warnings() {
    assert_eq!(json("10 km mi"), ("{\"ok\":true,\"result\":\"6.2137 mi\",\"notes\":[],\"warnings\":[]}\n".into(), String::new(), 0));
    let (out, err, _) = json("2026/12/04 - 12d");
    assert_eq!(out, "{\"ok\":true,\"result\":\"2026/11/22 YYYY/MM/DD\",\"notes\":[\"read `2026/12/04` as YYYY/MM/DD (year first)\"],\"warnings\":[]}\n");
    assert_eq!(err, "", "notes go into the JSON, not to stderr");
    assert!(json("1 GB GiB").0.contains("\"warnings\":[\"mixing decimal (GB"));
    assert!(json("-s 1 GB GiB").0.contains("\"warnings\":[]"), "-s still applies");
    assert_eq!(json("1/3 --precision 2").0, "{\"ok\":true,\"result\":\"0.33\",\"notes\":[],\"warnings\":[]}\n");
}

#[test]
fn errors_keep_their_exit_codes() {
    assert_eq!(json("10 foo m"), ("{\"ok\":false,\"exit\":1,\"error\":\"unknown unit `foo`\"}\n".into(), String::new(), 1));
    let (out, _, code) = json("10 km mi m");
    assert!(out.starts_with("{\"ok\":false,\"exit\":2,\"error\":\"expected <value>"));
    assert_eq!(code, 2);
    // Not finished yet (a unit so far): what comes next, marked so a UI shows a hint, not an error.
    let (out, _, code) = json("10 km");
    assert_eq!(out, "{\"ok\":false,\"exit\":2,\"error\":\"now the length unit to convert to: in, ft, yd…\",\"incomplete\":true}\n");
    assert_eq!(code, 2);
}

#[test]
fn ambiguous_dates_carry_their_choices() {
    let (out, err, code) = json("12/04/2026 unix");
    assert_eq!((err.as_str(), code), ("", 2), "never asked");
    assert!(out.contains("\"ambiguous\":{\"kind\":\"date\",\"text\":\"12/04/2026\",\"choices\":[{\"pattern\":\"MM/DD/YYYY\",\"date\":\"2026-12-04\",\"request\":\"12/04/2026 MM/DD/YYYY unix\"},{\"pattern\":\"DD/MM/YYYY\",\"date\":\"2026-04-12\",\"request\":\"12/04/2026 DD/MM/YYYY unix\"}]}}"), "{out}");
}

#[test]
fn one_line_per_piped_request() {
    let mut child = Command::new(BIN).arg("-v").args(["--json", "m"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"1 km\n2 km\nbad\n").unwrap();
    let out = child.wait_with_output().unwrap();
    let lines: Vec<String> = String::from_utf8(out.stdout).unwrap().lines().map(String::from).collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("\"result\":\"1000 m\"") && lines[1].contains("\"result\":\"2000 m\""));
    assert!(lines[2].starts_with("{\"ok\":false"));
    assert_eq!(out.status.code(), Some(2), "the most severe exit code");
}

#[test]
fn peak_memory_is_aequalis_s_own() {
    // Started by a large process (the Omarchy shell is ~360 MiB), the reply reports aequalis's peak,
    // not its parent's (`getrusage`'s `ru_maxrss` survives `exec` on Linux).
    let big = vec![1u8; 200 << 20];
    let out = Command::new(BIN).arg("-v").args(["--json", "10", "km", "mi"]).stdin(Stdio::null()).output().unwrap();
    assert_eq!(big.iter().map(|&b| b as usize).sum::<usize>(), 200 << 20, "keep the memory touched");
    let line = String::from_utf8(out.stdout).unwrap();
    let kb: u64 = line.trim_end().rsplit(':').next().unwrap().strip_suffix('}').unwrap().parse().unwrap();
    assert!(kb < 50_000, "{kb} KiB: {line}");
}
