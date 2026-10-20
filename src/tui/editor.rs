//! The expression bar (a one-line editor) and the session history.

use std::collections::VecDeque;

#[derive(Debug, Default)]
pub struct Line {
    chars: Vec<char>,
    /// Position in `chars`.
    cursor: usize,
}

impl Line {
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// Replaces the whole line; the cursor goes to the end.
    pub fn set(&mut self, text: &str) {
        self.chars = text.chars().collect();
        self.cursor = self.chars.len();
    }

    /// Replaces `start..end` with `text`; the cursor goes after it.
    pub fn replace(&mut self, start: usize, end: usize, text: &str) {
        self.chars.splice(start..end, text.chars());
        self.cursor = start + text.chars().count();
    }

    pub fn insert(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.chars.len());
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.chars.len();
    }

    /// `w`: the start of the next word.
    pub fn word_forward(&mut self) {
        let rest = &self.chars[self.cursor..];
        let word = rest.iter().take_while(|c| !c.is_whitespace()).count();
        let spaces = rest[word..].iter().take_while(|c| c.is_whitespace()).count();
        self.cursor += word + spaces;
    }

    /// `b`: the start of this or the previous word.
    pub fn word_back(&mut self) {
        let before = &self.chars[..self.cursor];
        let spaces = before.iter().rev().take_while(|c| c.is_whitespace()).count();
        let word = before[..before.len() - spaces].iter().rev().take_while(|c| !c.is_whitespace()).count();
        self.cursor -= spaces + word;
    }

    /// Ctrl-U.
    pub fn kill_to_start(&mut self) {
        self.chars.drain(..self.cursor);
        self.cursor = 0;
    }

    /// Ctrl-K.
    pub fn kill_to_end(&mut self) {
        self.chars.truncate(self.cursor);
    }

    /// Ctrl-W: the word before the cursor, and the spaces after it.
    pub fn kill_word(&mut self) {
        let spaces = self.chars[..self.cursor].iter().rev().take_while(|c| c.is_whitespace()).count();
        let word = self.chars[..self.cursor - spaces].iter().rev().take_while(|c| !c.is_whitespace()).count();
        let start = self.cursor - spaces - word;
        self.chars.drain(start..self.cursor);
        self.cursor = start;
    }
}

#[derive(Debug, PartialEq)]
pub struct Entry {
    pub input: String,
    /// The printed result, or the error message.
    pub result: Result<String, String>,
}

/// Evaluated expressions, oldest first, capped at `cap` entries. Browsing highlights one; the bar
/// is not changed (Enter writes the expression into it, `C` copies the result).
pub struct History {
    entries: VecDeque<Entry>,
    cap: usize,
    /// The entry highlighted while browsing.
    pos: Option<usize>,
}

impl History {
    pub fn new(cap: usize) -> Self {
        History { entries: VecDeque::new(), cap, pos: None }
    }

    pub fn entries(&self) -> &VecDeque<Entry> {
        &self.entries
    }

    pub fn selected(&self) -> Option<usize> {
        self.pos
    }

    pub fn push(&mut self, entry: Entry) {
        self.pos = None;
        if self.cap == 0 {
            return;
        }
        if self.entries.len() == self.cap {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// The entry highlighted, if browsing.
    pub fn highlighted(&self) -> Option<&Entry> {
        self.entries.get(self.pos?)
    }

    /// Highlights the next older entry (the newest first); stays on the oldest.
    pub fn up(&mut self) {
        self.pos = match self.pos {
            None if self.entries.is_empty() => None,
            None => Some(self.entries.len() - 1),
            Some(p) => Some(p.saturating_sub(1)),
        };
    }

    /// Highlights the next newer entry; after the newest, none (browsing ends).
    pub fn down(&mut self) {
        self.pos = self.pos.and_then(|p| (p + 1 < self.entries.len()).then_some(p + 1));
    }

    pub fn stop_browsing(&mut self) {
        self.pos = None;
    }

    /// Removes the highlighted entry; the one after it is highlighted, else the new last, else
    /// browsing ends.
    pub fn remove_highlighted(&mut self) {
        let Some(p) = self.pos else { return };
        self.entries.remove(p);
        self.pos = (!self.entries.is_empty()).then(|| p.min(self.entries.len() - 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, cursor: usize) -> Line {
        let mut l = Line::default();
        l.set(text);
        l.cursor = cursor;
        l
    }

    #[test]
    fn editing() {
        let mut l = Line::default();
        for c in "10 km".chars() {
            l.insert(c);
        }
        l.home();
        l.insert('1');
        assert_eq!((l.text(), l.cursor()), ("110 km".into(), 1));
        l.delete();
        l.end();
        l.backspace();
        l.left();
        l.right();
        l.right();
        assert_eq!((l.text(), l.cursor()), ("10 k".into(), 4));
    }

    #[test]
    fn kills() {
        let mut l = line("100 + 20 sqft  ", 15);
        l.kill_word();
        assert_eq!((l.text(), l.cursor()), ("100 + 20 ".into(), 9));
        let mut l = line("100 km mi", 4);
        l.kill_to_end();
        assert_eq!(l.text(), "100 ");
        let mut l = line("100 km mi", 4);
        l.kill_to_start();
        assert_eq!((l.text(), l.cursor()), ("km mi".into(), 0));
        let mut l = line("", 0);
        l.kill_word();
        l.backspace();
        l.delete();
        assert!(l.is_empty());
    }

    #[test]
    fn word_motions() {
        let mut l = line("10 km  mi", 0);
        l.word_forward();
        assert_eq!(l.cursor(), 3);
        l.word_forward();
        assert_eq!(l.cursor(), 7);
        l.word_forward();
        assert_eq!(l.cursor(), 9);
        l.word_back();
        assert_eq!(l.cursor(), 7);
        l.left();
        l.word_back();
        assert_eq!(l.cursor(), 3);
        l.home();
        l.word_back();
        assert_eq!(l.cursor(), 0);
    }

    #[test]
    fn replace_word() {
        let mut l = line("100 sq", 6);
        l.replace(4, 6, "sqft");
        assert_eq!((l.text(), l.cursor()), ("100 sqft".into(), 8));
    }

    fn entry(input: &str) -> Entry {
        Entry { input: input.into(), result: Ok(String::new()) }
    }

    #[test]
    fn browse_highlights() {
        let mut h = History::new(10);
        h.up();
        assert_eq!(h.selected(), None, "nothing to browse");
        for e in ["1+1", "10 km mi", "2^8"] {
            h.push(entry(e));
        }
        let shown = |h: &History| h.highlighted().map(|e| e.input.clone());
        h.up();
        assert_eq!(shown(&h).as_deref(), Some("2^8"));
        h.up();
        h.up();
        h.up();
        assert_eq!((shown(&h).as_deref(), h.selected()), (Some("1+1"), Some(0)), "stays on the oldest");
        h.down();
        assert_eq!(shown(&h).as_deref(), Some("10 km mi"));
        h.down();
        h.down();
        assert_eq!(h.selected(), None, "after the newest: none");
        h.down();
        assert_eq!(h.selected(), None);
    }

    #[test]
    fn remove_highlighted_entry() {
        let mut h = History::new(10);
        for e in ["a", "b", "c"] {
            h.push(entry(e));
        }
        h.remove_highlighted();
        assert_eq!(h.entries().len(), 3, "nothing highlighted: nothing removed");
        h.up();
        h.up();
        h.remove_highlighted();
        let inputs = |h: &History| h.entries().iter().map(|e| e.input.clone()).collect::<Vec<_>>();
        assert_eq!((inputs(&h), h.highlighted().map(|e| e.input.as_str())), (vec!["a".into(), "c".into()], Some("c")));
        h.remove_highlighted();
        assert_eq!((inputs(&h), h.highlighted().map(|e| e.input.as_str())), (vec!["a".into()], Some("a")));
        h.remove_highlighted();
        assert_eq!((h.entries().len(), h.selected()), (0, None));
    }

    #[test]
    fn push_ends_browsing() {
        let mut h = History::new(10);
        h.push(entry("1"));
        h.up();
        h.push(entry("2"));
        assert_eq!(h.selected(), None);
        h.up();
        assert_eq!(h.highlighted().map(|e| e.input.as_str()), Some("2"));
    }

    #[test]
    fn cap() {
        let mut h = History::new(2);
        for e in ["a", "b", "c"] {
            h.push(entry(e));
        }
        let inputs: Vec<_> = h.entries().iter().map(|e| e.input.as_str()).collect();
        assert_eq!(inputs, ["b", "c"]);
        let mut none = History::new(0);
        none.push(entry("a"));
        assert!(none.entries().is_empty());
    }
}
