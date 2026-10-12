//! Decoding terminal input bytes into keys.

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
    /// A click or a tap (the left button pressed), at a screen row and column from 0.
    Click { row: usize, col: usize },
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
/// still reach command mode and quit. Alt+key arrives the same way, so it acts as Esc, key.
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
                    _ => return Parsed::Skip(3),
                };
                Parsed::Key(key, 3)
            }
        },
        _ => Parsed::Key(Key::Esc, 1),
    }
}

/// `ESC [ <params> <final>`, e.g. `ESC[A` (up), `ESC[3~` (delete), `ESC[1;5C` (Ctrl+right).
fn csi(b: &[u8]) -> Parsed {
    let mut i = 2;
    while b.get(i).is_some_and(|c| (0x20..=0x3f).contains(c)) {
        i += 1;
    }
    let Some(&end) = b.get(i) else {
        return Parsed::Incomplete;
    };
    let params = std::str::from_utf8(&b[2..i]).unwrap_or("");
    // A mouse event (SGR reporting): `ESC[<button;column;rowM`, `m` when released.
    if let Some(mouse) = params.strip_prefix('<') {
        let n: Vec<usize> = mouse.split(';').filter_map(|v| v.parse().ok()).collect();
        return match (end, n.as_slice()) {
            // The left button pressed: no other button, no motion, no wheel.
            (b'M', &[0, col, row]) if col > 0 && row > 0 => Parsed::Key(Key::Click { row: row - 1, col: col - 1 }, i + 1),
            _ => Parsed::Skip(i + 1),
        };
    }
    let first = params.split(';').next().unwrap_or("");
    let key = match end {
        b'A' => Key::Up,
        b'B' => Key::Down,
        b'C' => Key::Right,
        b'D' => Key::Left,
        b'H' => Key::Home,
        b'F' => Key::End,
        b'Z' => Key::BackTab,
        b'~' => match first {
            "1" | "7" => Key::Home,
            "4" | "8" => Key::End,
            "3" => Key::Delete,
            "5" => Key::PageUp,
            "6" => Key::PageDown,
            _ => return Parsed::Skip(i + 1),
        },
        _ => return Parsed::Skip(i + 1),
    };
    Parsed::Key(key, i + 1)
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
        // Modifiers are ignored: Ctrl+right is right.
        assert_eq!(keys(b"\x1b[1;5C"), [Key::Right]);
    }

    #[test]
    fn esc_alt_and_unknown() {
        assert_eq!(keys(b"\x1b"), [Key::Esc]);
        assert_eq!(keys(b"\x1b\x1b[A"), [Key::Esc, Key::Up]);
        // Esc then a key in one read: both keys (Vim style), also for Alt+key.
        assert_eq!(keys(b"\x1bq"), [Key::Esc, Key::Char('q')]);
        assert_eq!(keys(b"\x1bbx"), [Key::Esc, Key::Char('b'), Key::Char('x')]);
        assert_eq!(keys("\x1b²".as_bytes()), [Key::Esc, Key::Char('²')]);
        assert_eq!(keys(b"\x1b[99~\x1b[5;2Px"), [Key::Char('x')]);
        // Mouse: the left button pressed is a click; releases, other buttons, motion and the wheel
        // are skipped.
        assert_eq!(keys(b"\x1b[<0;10;5M"), [Key::Click { row: 4, col: 9 }]);
        assert_eq!(keys(b"\x1b[<0;10;5m\x1b[<2;1;1M\x1b[<32;3;3M\x1b[<64;1;1M\x1b[<65;1;1Mq"), [Key::Char('q')]);
        let (k, used) = decode(b"\x1b[<0;10");
        assert_eq!((k.len(), used), (0, 0), "incomplete: kept for the next read");
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
    }
}
