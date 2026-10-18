//! Calculator mode: the value can be an arithmetic expression, with or without units.
//! Covers the examples from the feature request, run through the real binary.

use std::process::{Command, Output};

fn aequalis(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aequalis")).arg("-v").args(args).output().expect("run aequalis")
}

/// (stdout, stderr) of a successful run.
fn run(args: &[&str]) -> (String, String) {
    let out = aequalis(args);
    assert!(out.status.success(), "{args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    (String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap())
}

#[test]
fn power() {
    assert_eq!(run(&["10**2"]), ("100\n".into(), String::new()));
    assert_eq!(run(&["10^2"]).0, "100\n");
}

#[test]
fn brackets() {
    let (out, err) = run(&["(100+20)/120"]);
    assert_eq!(out, "1\n");
    assert_eq!(err, "aequalis: note: evaluated as (100 + 20) / 120\n");
    assert_eq!(run(&["[100+20]/120"]).0, "1\n");
    assert_eq!(run(&["{100+20}/120"]).0, "1\n");
}

#[test]
fn default_precision() {
    assert_eq!(run(&["1/3"]), ("0.3333\n".into(), String::new()));
    assert_eq!(run(&["1/3", "--precision", "6"]).0, "0.333333\n");
}

#[test]
fn spaced_expression_with_units() {
    let (out, err) = run(&["100", "+", "20", "+", "50", "/", "10", "sqft", "m2"]);
    assert_eq!(out, "11.6129 m2\n");
    assert_eq!(err, "aequalis: note: evaluated as (100 + 20) + (50 / 10)\n");
}

#[test]
fn silent_hides_the_grouping_note() {
    let (out, err) = run(&["-s", "100", "+", "20", "+", "50", "/", "10", "sqft", "m2"]);
    assert_eq!(out, "11.6129 m2\n");
    assert_eq!(err, "");
}

#[test]
fn errors() {
    assert_eq!(aequalis(&["1/0"]).status.code(), Some(1));
    assert_eq!(aequalis(&["(1+2"]).status.code(), Some(2));
    assert_eq!(aequalis(&["1+2", "km"]).status.code(), Some(2));
}

/// The shell-safe spellings reach the program unchanged when typed without quotes in bash.
#[cfg(unix)]
#[test]
fn shell_safe_spellings_unquoted_in_bash() {
    if Command::new("bash").arg("--version").output().is_err() {
        return;
    }
    let bin = env!("CARGO_BIN_EXE_aequalis");
    let out = Command::new("bash")
        .arg("-c")
        .arg(format!("'{bin}' 10^2 && '{bin}' [100+20]/120 && '{bin}' {{100+20}}/120 && '{bin}' 3 x 4"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "100\n1\n1\n12\n");
}
