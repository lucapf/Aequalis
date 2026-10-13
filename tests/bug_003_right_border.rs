//! Bug 003: the TUI's right border was missing in real terminals.
//!
//! Every frame line fills the whole width and was followed by `ESC[K` (erase to end of line).
//! After a character is printed in the last column, a terminal keeps the cursor *on* that column
//! ("pending wrap"), so the `ESC[K` erased the last character: the right border `│`. Fixed: lines
//! are full width, so nothing is erased after them.
//!
//! The output of a real pty session is replayed through a small terminal emulator that has the
//! pending-wrap behaviour of xterm-like terminals (Linux only: util-linux `script`).

#![cfg(all(target_os = "linux", feature = "tui"))]

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

const BIN: &str = env!("CARGO_BIN_EXE_tally");
const ROWS: usize = 24;
const COLS: usize = 80;

/// Just enough of a VT100 to draw the TUI: cursor positioning, printing with autowrap and pending
/// wrap, erase in line / display; other sequences (colours, cursor shape, modes) are ignored.
struct Screen {
    cells: Vec<Vec<char>>,
    row: usize,
    col: usize,
    pending_wrap: bool,
}

impl Screen {
    fn new() -> Self {
        Screen { cells: vec![vec![' '; COLS]; ROWS], row: 0, col: 0, pending_wrap: false }
    }

    fn print(&mut self, c: char) {
        if self.pending_wrap {
            self.pending_wrap = false;
            self.col = 0;
            self.row = (self.row + 1).min(ROWS - 1);
        }
        self.cells[self.row][self.col] = c;
        if self.col == COLS - 1 {
            self.pending_wrap = true;
        } else {
            self.col += 1;
        }
    }

    fn csi(&mut self, params: &str, end: char) {
        let nums: Vec<usize> = params.split(';').map(|p| p.parse().unwrap_or(0)).collect();
        match end {
            'H' => {
                self.row = nums.first().copied().unwrap_or(1).max(1) - 1;
                self.col = nums.get(1).copied().unwrap_or(1).max(1) - 1;
                self.pending_wrap = false;
            }
            'K' => {
                self.cells[self.row][self.col..].fill(' ');
                self.pending_wrap = false;
            }
            'J' if params == "2" => self.cells.iter_mut().for_each(|r| r.fill(' ')),
            _ => {}
        }
    }

    /// Replays `out` up to (not including) leaving the alternate screen.
    fn replay(out: &str) -> Screen {
        let mut screen = Screen::new();
        let body = out.split("\x1b[?1049l").next().unwrap();
        let body = body.rsplit("\x1b[?1049h").next().unwrap();
        let mut chars = body.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '\x1b' {
                if !c.is_control() {
                    screen.print(c);
                }
                continue;
            }
            if chars.next_if_eq(&'[').is_none() {
                continue;
            }
            let mut params = String::new();
            while let Some(&p) = chars.peek() {
                if p.is_ascii_digit() || p == ';' || p == '?' || p == ' ' {
                    params.push(p);
                    chars.next();
                } else {
                    break;
                }
            }
            if let Some(end) = chars.next() {
                screen.csi(params.trim(), end);
            }
        }
        screen
    }

    fn line(&self, row: usize) -> String {
        self.cells[row].iter().collect()
    }
}

/// A TUI session at 80×24; `keys` are sent one write each, then Ctrl-C.
fn session(keys: &[&str]) -> Option<Screen> {
    Command::new("script").arg("--version").output().ok()?;
    let mut child = Command::new("script")
        .args(["-qec", &format!("stty rows {ROWS} cols {COLS}; exec '{BIN}'"), "/dev/null"])
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        stdout.read_to_end(&mut out).map(|_| out)
    });
    sleep(Duration::from_millis(500));
    for k in keys {
        stdin.write_all(k.as_bytes()).unwrap();
        stdin.flush().unwrap();
        sleep(Duration::from_millis(250));
    }
    stdin.write_all(b"\x03").unwrap();
    drop(stdin);
    child.wait().unwrap();
    let out = reader.join().unwrap().unwrap();
    Some(Screen::replay(&String::from_utf8_lossy(&out)))
}

/// Every row of the sections (all but the status line) ends with the right border.
fn assert_right_border(screen: &Screen) {
    for row in 0..ROWS - 1 {
        let last = screen.cells[row][COLS - 1];
        assert!("│┐┘".contains(last), "row {row} ends with {last:?}:\n{}", screen.line(row));
    }
}

#[test]
fn reported_right_border_in_input_mode() {
    let Some(screen) = session(&["10 km mi\r", "12+12/3"]) else { return };
    assert_right_border(&screen);
    assert!(screen.line(0).starts_with("┌─ Input mode "), "{}", screen.line(0));
}

#[test]
fn right_border_in_command_mode_and_unit_keys() {
    let Some(screen) = session(&["\x1b"]) else { return };
    assert_right_border(&screen);
    let Some(screen) = session(&["\x1bML"]) else { return };
    assert_right_border(&screen);
    assert!((0..ROWS).any(|r| screen.line(r).contains("Length units")));
}

#[test]
fn right_border_with_the_help() {
    let Some(screen) = session(&["\x1bE"]) else { return };
    assert_right_border(&screen);
}
