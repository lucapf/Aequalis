//! The Enter hook in `--autocompletion bash`: it quotes a typed expression such as
//! `aequalis (10+12-3)/4Km Mi`, which bash would otherwise reject because of the `(`.
//! The hook function is called directly (no terminal, no history file is touched).

// The hook is bash's, for a Unix shell.
#![cfg(unix)]

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_aequalis");

/// Loads the bash script, runs the hook on `line` and prints the rewritten line, then runs it.
fn bash_hook(line: &str) -> Option<(String, String)> {
    Command::new("bash").arg("--version").output().ok()?;
    let script = format!(
        r#"eval "$('{BIN}' --autocompletion bash)"
aequalis() {{ '{BIN}' "$@"; }}
READLINE_LINE=$1
_aequalis_quote_line
printf '%s\n' "$READLINE_LINE"
eval "$READLINE_LINE""#
    );
    let out = Command::new("bash").env("HISTFILE", "/dev/null").args(["-c", &script, "hook", line]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let (rewritten, result) = stdout.split_once('\n').unwrap();
    Some((rewritten.to_string(), result.to_string()))
}

#[test]
fn reported_example_is_quoted_and_runs() {
    let Some((line, result)) = bash_hook("aequalis (10+12-3)/4Km Mi") else { return };
    assert_eq!(line, "aequalis '(10+12-3)/4Km Mi'");
    assert_eq!(result, "2.9515 mi\n");
}

#[test]
fn star_and_flags() {
    let Some((line, result)) = bash_hook("aequalis 2 * 3 --precision 1") else { return };
    assert_eq!(line, "aequalis '2 * 3' --precision 1");
    assert_eq!(result, "6\n");
}

#[test]
fn plain_conversion_is_untouched() {
    let Some((line, result)) = bash_hook("aequalis 10 km mi") else { return };
    assert_eq!(line, "aequalis 10 km mi");
    assert_eq!(result, "6.2137 mi\n");
}

#[test]
fn quote_line_flag() {
    let out = Command::new(BIN).arg("-v").args(["--quote-line", "aequalis (1+2)*3 km m"]).output().unwrap();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "aequalis '(1+2)*3 km m'\n");
    let out = Command::new(BIN).arg("-v").args(["--quote-line", "ls -l"]).output().unwrap();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "ls -l\n");
}
