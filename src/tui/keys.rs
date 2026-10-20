//! Decoding terminal input bytes into keys and mouse reports.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Key {
    Char(char),
    /// Ctrl + a letter, as the lower-case letter.
    Ctrl(char),
    Enter,
    Tab,
    BackTab,
    Backspace,
    Delete,
    Esc,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    /// A function key, `F(1)` to `F(12)`.
    F(u8),
    /// Alt + a character, when the terminal sent them in one write (`ESC` then the character).
    Alt(char),
    /// Alt or Ctrl with ← →: a word to the left or right.
    WordLeft,
    WordRight,
    Mouse(Mouse),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Button {
    Left,
    Middle,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseKind {
    Down(Button),
    Up,
    Drag(Button),
    ScrollUp,
    ScrollDown,
}

/// A mouse report: what happened and where, from 0 at the top left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mouse {
    pub kind: MouseKind,
    pub row: usize,
    pub col: usize,
}

enum Parsed {
    Key(Key, usize),
    /// Bytes that are not a key we use (unknown sequences).
    Skip(usize),
    Incomplete,
}

/// Decodes the complete keys at the start of `buf` and returns them with the number of bytes used.
/// An incomplete escape sequence or UTF-8 character at the end is left for the next read; a lone
/// ESC is the Esc key, since terminals send a whole sequence in one write. ESC followed by any other
/// key is Esc and then that key, as in Vim: Esc then `q` typed quickly (or bundled by ssh) must
/// still reach command mode and quit. Alt+key arrives the same way, so it is `Alt(key)`, which a
/// key without an Alt binding must treat as Esc, key.
pub fn decode(buf: &[u8]) -> (Vec<Key>, usize) {
    let mut keys = Vec::new();
    let mut i = 0;
    while i < buf.len() {
        match parse(&buf[i..]) {
            Parsed::Key(key, n) => {
                keys.push(key);
                i += n;
            }
            Parsed::Skip(n) => i += n,
            Parsed::Incomplete => break,
        }
    }
    (keys, i)
}

fn parse(b: &[u8]) -> Parsed {
    match b[0] {
        b'\r' | b'\n' => Parsed::Key(Key::Enter, 1),
        b'\t' => Parsed::Key(Key::Tab, 1),
        0x7f | 0x08 => Parsed::Key(Key::Backspace, 1),
        0x1b => escape(b),
        c @ 1..=26 => Parsed::Key(Key::Ctrl(char::from(b'a' + c - 1)), 1),
        c if c < 0x20 => Parsed::Skip(1),
        _ => utf8(b),
    }
}

fn escape(b: &[u8]) -> Parsed {
    match b.get(1) {
        Some(b'[') => csi(b),
        Some(b'O') => match b.get(2) {
            None => Parsed::Incomplete,
            Some(c) => {
                let key = match c {
                    b'A' => Key::Up,
                    b'B' => Key::Down,
                    b'C' => Key::Right,
                    b'D' => Key::Left,
                    b'H' => Key::Home,
                    b'F' => Key::End,
                    b'P'..=b'S' => Key::F(c - b'P' + 1),
                    _ => return Parsed::Skip(3),
                };
                Parsed::Key(key, 3)
            }
        },
        // Alt + a printable character; ESC before a control character, or ESC ESC, is the Esc key.
        Some(&c) if c >= 0x20 && c != 0x7f => match utf8(&b[1..]) {
            Parsed::Key(Key::Char(ch), n) => Parsed::Key(Key::Alt(ch), n + 1),
            Parsed::Skip(n) => Parsed::Skip(n + 1),
            _ => Parsed::Incomplete,
        },
        _ => Parsed::Key(Key::Esc, 1),
    }
}

/// `ESC [ <params> <final>`, e.g. `ESC[A` (up), `ESC[3~` (delete), `ESC[1;5C` (Ctrl+right),
/// `ESC[15~` (F5), `ESC[<0;12;5M` (a mouse press), `ESC[M` and three bytes (the old mouse report).
fn csi(b: &[u8]) -> Parsed {
    // The Linux console's F1 to F5: `ESC [ [ A` to `ESC [ [ E`.
    if b.get(2) == Some(&b'[') {
        return match b.get(3) {
            None => Parsed::Incomplete,
            Some(c @ b'A'..=b'E') => Parsed::Key(Key::F(c - b'A' + 1), 4),
            Some(_) => Parsed::Skip(3),
        };
    }
    let mut i = 2;
    while b.get(i).is_some_and(|c| (0x20..=0x3f).contains(c)) {
        i += 1;
    }
    let Some(&end) = b.get(i) else {
        return Parsed::Incomplete;
    };
    let params = std::str::from_utf8(&b[2..i]).unwrap_or("");
    if end == b'M' && params.is_empty() {
        return x10_mouse(b);
    }
    if params.starts_with('<') && matches!(end, b'M' | b'm') {
        return match sgr_mouse(&params[1..], end == b'm') {
            Some(mouse) => Parsed::Key(Key::Mouse(mouse), i + 1),
            None => Parsed::Skip(i + 1),
        };
    }
    let mut fields = params.split(';');
    let first = fields.next().unwrap_or("");
    // Alt or Ctrl with an arrow: xterm's modifier 3 (Alt) and 5 (Ctrl).
    let word = matches!(fields.next(), Some("3" | "5"));
    let key = match end {
        b'A' => Key::Up,
        b'B' => Key::Down,
        b'C' if word => Key::WordRight,
        b'C' => Key::Right,
        b'D' if word => Key::WordLeft,
        b'D' => Key::Left,
        b'H' => Key::Home,
        b'F' => Key::End,
        b'Z' => Key::BackTab,
        b'P'..=b'S' if first == "1" => Key::F(end - b'P' + 1),
        b'~' => match first {
            "1" | "7" => Key::Home,
            "4" | "8" => Key::End,
            "3" => Key::Delete,
            "5" => Key::PageUp,
            "6" => Key::PageDown,
            _ => match first.parse::<u8>() {
                Ok(n @ 11..=15) => Key::F(n - 10),
                Ok(n @ 17..=21) => Key::F(n - 11),
                Ok(n @ (23 | 24)) => Key::F(n - 12),
                _ => return Parsed::Skip(i + 1),
            },
        },
        _ => return Parsed::Skip(i + 1),
    };
    Parsed::Key(key, i + 1)
}

/// The report of the SGR mode (1006): `<button;column;row` and `M` for a press, `m` for a release.
fn sgr_mouse(params: &str, release: bool) -> Option<Mouse> {
    let mut fields = params.split(';').map(|f| f.parse::<usize>().ok());
    let (code, col, row) = (fields.next()??, fields.next()??, fields.next()??);
    if fields.next().is_some() || col == 0 || row == 0 {
        return None;
    }
    mouse(code, release, row - 1, col - 1)
}

/// The report of the old mode: `ESC [ M` then the button, column and row, each plus 32.
fn x10_mouse(b: &[u8]) -> Parsed {
    let Some(&[code, col, row]) = b.get(3..6).map(|s| <&[u8; 3]>::try_from(s).unwrap()) else {
        return Parsed::Incomplete;
    };
    let (code, col, row) = (usize::from(code.saturating_sub(32)), usize::from(col.saturating_sub(32)), usize::from(row.saturating_sub(32)));
    // Here a release is the button code 3 (without motion or the wheel), there is no `m`.
    let release = code & 0b11 == 3 && code & (32 | 64) == 0;
    match mouse(code, release, row.saturating_sub(1), col.saturating_sub(1)) {
        Some(mouse) => Parsed::Key(Key::Mouse(mouse), 6),
        None => Parsed::Skip(6),
    }
}

/// `code` is the button in the low two bits, then 4 Shift, 8 Alt, 16 Ctrl, 32 motion, 64 the wheel.
fn mouse(code: usize, release: bool, row: usize, col: usize) -> Option<Mouse> {
    let button = match code & 0b11 {
        0 => Button::Left,
        1 => Button::Middle,
        _ => Button::Right,
    };
    let kind = match (code & 64 != 0, code & 32 != 0, release) {
        (true, _, _) if code & 1 == 0 => MouseKind::ScrollUp,
        (true, _, _) => MouseKind::ScrollDown,
        (_, _, true) => MouseKind::Up,
        // Motion with no button held (`code & 3 == 3`) is not asked for: nothing to report.
        (_, true, _) if code & 0b11 == 3 => return None,
        (_, true, _) => MouseKind::Drag(button),
        _ => MouseKind::Down(button),
    };
    Some(Mouse { kind, row, col })
}

fn utf8(b: &[u8]) -> Parsed {
    let len = match b[0] {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => return Parsed::Skip(1),
    };
    if b.len() < len {
        return Parsed::Incomplete;
    }
    match std::str::from_utf8(&b[..len]) {
        Ok(s) => Parsed::Key(Key::Char(s.chars().next().unwrap()), len),
        Err(_) => Parsed::Skip(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(b: &[u8]) -> Vec<Key> {
        let (keys, used) = decode(b);
        assert_eq!(used, b.len(), "not all bytes used for {b:?}");
        keys
    }

    #[test]
    fn plain_keys() {
        assert_eq!(keys(b"1+2"), [Key::Char('1'), Key::Char('+'), Key::Char('2')]);
        assert_eq!(keys(b"\r\n\t\x7f\x08"), [Key::Enter, Key::Enter, Key::Tab, Key::Backspace, Key::Backspace]);
        assert_eq!(keys(b"\x01\x03\x04\x17"), [Key::Ctrl('a'), Key::Ctrl('c'), Key::Ctrl('d'), Key::Ctrl('w')]);
    }

    #[test]
    fn escape_sequences() {
        assert_eq!(keys(b"\x1b[A\x1b[B\x1b[C\x1b[D"), [Key::Up, Key::Down, Key::Right, Key::Left]);
        assert_eq!(keys(b"\x1bOA\x1bOH\x1bOF"), [Key::Up, Key::Home, Key::End]);
        assert_eq!(keys(b"\x1b[H\x1b[F\x1b[1~\x1b[4~\x1b[7~\x1b[8~"), [Key::Home, Key::End, Key::Home, Key::End, Key::Home, Key::End]);
        assert_eq!(keys(b"\x1b[3~\x1b[5~\x1b[6~\x1b[Z"), [Key::Delete, Key::PageUp, Key::PageDown, Key::BackTab]);
        // Shift and the other modifiers are ignored; Alt and Ctrl move by words.
        assert_eq!(keys(b"\x1b[1;2C\x1b[1;5C\x1b[1;3D"), [Key::Right, Key::WordRight, Key::WordLeft]);
        assert_eq!(keys(b"\x1b[1;5A"), [Key::Up]);
    }

    #[test]
    fn esc_alt_and_unknown() {
        assert_eq!(keys(b"\x1b"), [Key::Esc]);
        assert_eq!(keys(b"\x1b\x1b[A"), [Key::Esc, Key::Up]);
        // Esc then a key in one read is Alt+key; the app turns an unbound one into Esc, key (Vim style).
        assert_eq!(keys(b"\x1bq"), [Key::Alt('q')]);
        assert_eq!(keys(b"\x1bbx"), [Key::Alt('b'), Key::Char('x')]);
        assert_eq!(keys("\x1b²".as_bytes()), [Key::Alt('²')]);
        // Esc before a control key is still Esc, then that key.
        assert_eq!(keys(b"\x1b\r\x1b\x7f"), [Key::Esc, Key::Enter, Key::Esc, Key::Backspace]);
        assert_eq!(keys(b"\x1b[99~\x1b[5;2Px"), [Key::Char('x')]);
    }

    #[test]
    fn function_keys() {
        assert_eq!(keys(b"\x1bOP\x1bOQ\x1bOR\x1bOS"), [Key::F(1), Key::F(2), Key::F(3), Key::F(4)]);
        assert_eq!(keys(b"\x1b[11~\x1b[12~\x1b[13~\x1b[14~\x1b[15~"), [Key::F(1), Key::F(2), Key::F(3), Key::F(4), Key::F(5)]);
        assert_eq!(keys(b"\x1b[17~\x1b[18~\x1b[19~\x1b[20~\x1b[21~"), [Key::F(6), Key::F(7), Key::F(8), Key::F(9), Key::F(10)]);
        assert_eq!(keys(b"\x1b[23~\x1b[24~"), [Key::F(11), Key::F(12)]);
        // With a modifier (Shift+F1), and the Linux console's own codes.
        assert_eq!(keys(b"\x1b[1;2P\x1b[15;5~"), [Key::F(1), Key::F(5)]);
        assert_eq!(keys(b"\x1b[[A\x1b[[E"), [Key::F(1), Key::F(5)]);
        assert_eq!(keys(b"\x1b[16~\x1b[22~"), []);
    }

    fn mouse(kind: MouseKind, row: usize, col: usize) -> Key {
        Key::Mouse(Mouse { kind, row, col })
    }

    #[test]
    fn mouse_reports() {
        // SGR: columns and rows count from 1; `m` is the release.
        assert_eq!(keys(b"\x1b[<0;12;5M"), [mouse(MouseKind::Down(Button::Left), 4, 11)]);
        assert_eq!(keys(b"\x1b[<0;12;5m"), [mouse(MouseKind::Up, 4, 11)]);
        assert_eq!(keys(b"\x1b[<2;1;1M"), [mouse(MouseKind::Down(Button::Right), 0, 0)]);
        assert_eq!(keys(b"\x1b[<32;3;4M"), [mouse(MouseKind::Drag(Button::Left), 3, 2)]);
        assert_eq!(keys(b"\x1b[<64;3;4M\x1b[<65;3;4M"), [mouse(MouseKind::ScrollUp, 3, 2), mouse(MouseKind::ScrollDown, 3, 2)]);
        // Modifiers (Ctrl here) do not change what was pressed.
        assert_eq!(keys(b"\x1b[<16;3;4M"), [mouse(MouseKind::Down(Button::Left), 3, 2)]);
        // Motion with no button, and malformed reports, are dropped.
        assert_eq!(keys(b"\x1b[<35;3;4M\x1b[<0;0;4M\x1b[<0;3M\x1b[<;3;4Mq"), [Key::Char('q')]);
        // The old report: three bytes, each plus 32 (a column past 95 is no longer ASCII).
        assert_eq!(
            keys(b"\x1b[M #\"\x1b[M!\"\"\x1b[M`!!"),
            [mouse(MouseKind::Down(Button::Left), 1, 2), mouse(MouseKind::Down(Button::Middle), 1, 1), mouse(MouseKind::ScrollUp, 0, 0),]
        );
        assert_eq!(keys(b"\x1b[M#!!"), [mouse(MouseKind::Up, 0, 0)]);
        assert_eq!(keys(&[0x1b, b'[', b'M', b' ', 200, 40]), [mouse(MouseKind::Down(Button::Left), 7, 167)]);
    }

    #[test]
    fn utf8() {
        assert_eq!(keys("m²°".as_bytes()), [Key::Char('m'), Key::Char('²'), Key::Char('°')]);
    }

    #[test]
    fn incomplete_input_is_kept() {
        let (k, used) = decode(b"1\x1b[");
        assert_eq!((k, used), (vec![Key::Char('1')], 1));
        let (k, used) = decode(&"x²".as_bytes()[..2]);
        assert_eq!((k, used), (vec![Key::Char('x')], 1));
        assert_eq!(decode(b"\x1bO"), (vec![], 0));
        assert_eq!(decode(b"\x1b[<0;12"), (vec![], 0));
        assert_eq!(decode(b"\x1b[M #"), (vec![], 0));
        assert_eq!(decode(b"\x1b[["), (vec![], 0));
        assert_eq!(decode(&"\x1b²".as_bytes()[..2]), (vec![], 0));
    }
}
