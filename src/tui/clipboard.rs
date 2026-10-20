//! Copying text to the system clipboard, through the desktop's tool: `wl-copy` (Wayland),
//! `xclip` or `xsel` (X11), `pbcopy` (macOS). No crates: the first that runs gets the text on its
//! standard input. On Windows, PowerShell's `Set-Clipboard`, with the text in an environment
//! variable: a variable holds any character, where a pipe to `clip.exe` would be cut to the
//! console's code page.

use std::process::{Command, Stdio};

/// The tools tried here, in order: (program, arguments).
#[cfg(not(windows))]
fn tools() -> Vec<(&'static str, &'static [&'static str])> {
    tools_for(cfg!(target_os = "macos"), std::env::var_os("WAYLAND_DISPLAY").is_some())
}

/// The tools tried on macOS or not, in a Wayland session or not.
#[cfg(not(windows))]
fn tools_for(macos: bool, wayland: bool) -> Vec<(&'static str, &'static [&'static str])> {
    let mut tools: Vec<(&str, &[&str])> = Vec::new();
    if macos {
        tools.push(("pbcopy", &[]));
    }
    if wayland {
        tools.push(("wl-copy", &[]));
    }
    tools.push(("xclip", &["-selection", "clipboard"]));
    tools.push(("xsel", &["--clipboard", "--input"]));
    tools
}

/// Copies `text`; the error says which tools are missing.
#[cfg(windows)]
pub fn copy(text: &str) -> Result<(), String> {
    let copied = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", "Set-Clipboard -Value $env:AEQUALIS_COPY"])
        .env("AEQUALIS_COPY", text)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match copied {
        Ok(status) if status.success() => Ok(()),
        _ => Err("no clipboard tool worked (powershell Set-Clipboard)".to_string()),
    }
}

/// Copies `text`; the error says which tools are missing.
#[cfg(not(windows))]
pub fn copy(text: &str) -> Result<(), String> {
    use std::io::Write;
    for (program, args) in tools() {
        let Ok(mut child) = Command::new(program).args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else {
            continue;
        };
        let written = child.stdin.take().is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
        if written && child.wait().is_ok_and(|s| s.success()) {
            return Ok(());
        }
    }
    let names: Vec<&str> = tools().iter().map(|(p, _)| *p).collect();
    Err(format!("no clipboard tool worked ({})", names.join(", ")))
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_s_tool_first() {
        let names = |macos, wayland| tools_for(macos, wayland).into_iter().map(|(p, _)| p).collect::<Vec<_>>();
        assert_eq!(names(true, false), ["pbcopy", "xclip", "xsel"]);
        assert_eq!(names(false, true), ["wl-copy", "xclip", "xsel"]);
        assert_eq!(names(false, false), ["xclip", "xsel"], "X11");
        assert_eq!(tools_for(false, false)[0].1, ["-selection", "clipboard"]);
    }
}
