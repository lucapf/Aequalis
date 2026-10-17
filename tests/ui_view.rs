//! `--json --as-typed [--submitted] "<request>"`: the reply as a UI shows it (the Omarchy widget),
//! the request as typed, a trailing space included.

use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_tally");

fn view(args: &[&str]) -> String {
    let out = Command::new(BIN).arg("--json").arg("--as-typed").args(args).env("TZ", "Europe/Rome").env("LC_ALL", "C").stdin(Stdio::null()).output().unwrap();
    assert_eq!(String::from_utf8(out.stderr).unwrap(), "");
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn a_word_being_typed_shows_nothing_but_a_result() {
    assert!(view(&["10 km"]).contains(r#""show":"none""#));
    // One argument: its trailing space is kept, and says the word is done.
    assert!(view(&["10 km "]).contains(r#""show":"hint","message":"now the length unit to convert to"#));
    assert!(view(&["--submitted", "10 km"]).contains(r#""show":"hint""#));
    let result = view(&["10 km mi"]);
    assert!(result.contains(r#""show":"result","value":"6.2137""#), "{result}");
    assert!(result.contains(r#""peak_kb":"#), "the memory, as every --json reply");
}

#[test]
fn a_question_with_its_choices() {
    let reply = view(&["--submitted", "12/04/2026 unix"]);
    assert!(reply.contains(r#""message":"Which date is 12/04/2026?""#), "{reply}");
    assert!(reply.contains(r#""default":"12/04/2026 MM/DD/YYYY unix""#), "{reply}");
}
