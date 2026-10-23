//! Bug 017: `1 us d` converted 1 US dollar to the local currency, while `1 us -> d`, `1 us in d`
//! and `1 us to d` converted 1 microsecond to days. Found by the fuzzer (`fuzz/`).
//!
//! An ISO code was looked up with the spaces squeezed out, so the two words `us d` read as `USD`.
//! Fixed: a code is one word (`U.S.D.` still is one); `us d` is two units, as with a connector.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

fn run(args: &str) -> (String, String, i32) {
    let out = Command::new(BIN).args(args.split_whitespace()).env("LC_ALL", "C").output().unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

#[test]
fn reported() {
    let (plain, err, code) = run("1 us d");
    assert_eq!(code, 0, "{err}");
    assert!(plain.ends_with(" d"), "{plain}");
    for connector in ["->", "in", "to"] {
        assert_eq!(run(&format!("1 us {connector} d")).0, plain, "{connector}");
    }
}

#[test]
fn round_trip() {
    assert_eq!(run("86400000000000 us d").0, "1000 d");
}
