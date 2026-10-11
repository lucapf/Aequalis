//! Bug 001: `tally 100sqft mq` failed with "unknown unit `mq`".
//!
//! `mq` (Italian *metro quadro*) is a common notation for square metres, but it was
//! not an alias of `m2`. Fixed by adding `mq` (and `mmq`, `cmq`, `kmq`) as aliases.

use std::process::{Command, Output};

fn tally(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tally")).args(args).output().expect("run tally")
}

fn stdout(args: &[&str]) -> String {
    let out = tally(args);
    assert!(out.status.success(), "{args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn reported_command() {
    assert_eq!(stdout(&["100sqft", "mq"]), "9.2903 m2\n");
}

#[test]
fn separate_value_and_unit() {
    assert_eq!(stdout(&["100", "sqft", "mq"]), "9.2903 m2\n");
}

#[test]
fn mq_as_source_unit() {
    assert_eq!(stdout(&["1", "mq", "sqft"]), "10.7639 ft2\n");
    assert_eq!(stdout(&["50mq", "cmq"]), "500000 cm2\n");
}

#[test]
fn related_aliases() {
    assert_eq!(stdout(&["1", "kmq", "mq"]), "1000000 m2\n");
    assert_eq!(stdout(&["1", "cmq", "mmq"]), "100 mm2\n");
}

#[test]
fn case_insensitive() {
    assert_eq!(stdout(&["100", "SQFT", "MQ"]), "9.2903 m2\n");
}

#[test]
fn listed_with_m2() {
    let list = stdout(&["--list"]);
    assert!(list.lines().any(|l| l.trim_start().starts_with("m2") && l.contains("mq")));
}

#[test]
fn mq_is_not_a_length() {
    let out = tally(&["1", "mq", "m"]);
    assert_eq!(out.status.code(), Some(1));
}
