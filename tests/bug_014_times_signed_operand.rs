//! Bug 014: `2 x - 3` was refused, though `2 * - 3` and `2 x -3` give -6. Found by the fuzzer
//! (`fuzz/`, its check that spaces change nothing in a calculation).
//!
//! `x` is a multiplication only when an operand follows it; a sign followed by a space was not
//! seen as the start of one. Fixed: the operand may have a sign, spaced as after `*`.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

fn run(args: &str) -> (String, i32) {
    let out = Command::new(BIN).args(args.split_whitespace()).env("LC_ALL", "C").output().unwrap();
    (String::from_utf8(out.stdout).unwrap().trim_end().to_string(), out.status.code().unwrap_or(-1))
}

#[test]
fn reported() {
    assert_eq!(run("2 x - 3"), ("-6".into(), 0));
    assert_eq!(run("2 x + 3").0, "6");
    assert_eq!(run("2 x - pi").0, "-6.2832");
    assert_eq!(run("2 x - (1+1)").0, "-4");
    assert_eq!(run("10 x - 3 km mi").0, "-18.6411 mi");
}

#[test]
fn x_unchanged_elsewhere() {
    assert_eq!(run("2 * - 3").0, "-6");
    assert_eq!(run("2x3").0, "6");
    assert_eq!(run("2 x -").1, 1, "no operand: `x` is a unit, unknown");
}
