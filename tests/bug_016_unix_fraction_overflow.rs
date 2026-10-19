//! Bug 016: `.110000000000000000000000000159 unix` crashed the fuzz build (`attempt to multiply with
//! overflow`, in `time::scaled`); a release build wrapped around silently. Found by the fuzzer (`fuzz/`).
//!
//! The fraction of a timestamp is rounded half up as `(fraction × scale × 2 + den) / (2 × den)`; with
//! 30 fractional digits the doubling left the i128. Fixed: every step is checked, and the timestamp
//! is then not a time.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

fn run(args: &str) -> (String, String, i32) {
    let out = Command::new(BIN).args(args.split_whitespace()).env("LC_ALL", "C").output().unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap().trim_end().to_string();
    (text(out.stdout), text(out.stderr), out.status.code().unwrap_or(-1))
}

#[test]
fn reported() {
    for request in [".110000000000000000000000000159 unix", "3.141000000000000000000000000000000159 unix"] {
        let (_, err, code) = run(request);
        assert_ne!(code, 101, "{request}: {err}");
        assert!(!err.contains("panicked"), "{request}: {err}");
    }
}
