//! The Trivia window (`R`): the entries of `trivia.rs` by section (the tabs), one per row, with
//! the selected one's description below. Enter opens its popup. The live values come from the
//! online data (`online.rs`).

use crate::trivia::{self, Entry};

/// The section shown and the entry selected in it; and its popup (Enter), scrolled down by
/// `Some(lines)`.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub section: usize,
    pub selected: usize,
    pub popup: Option<usize>,
}

impl View {
    pub fn new() -> Self {
        View { section: 0, selected: 0, popup: None }
    }

    pub fn entries(&self) -> Vec<&'static Entry> {
        let name = trivia::sections()[self.section];
        trivia::entries().iter().filter(|e| e.section == name).collect()
    }

    pub fn entry(&self) -> Option<&'static Entry> {
        self.entries().get(self.selected).copied()
    }

    /// The next / previous section, circular, from its first entry.
    pub fn switch(&mut self, forward: bool) {
        let n = trivia::sections().len();
        self.section = if forward { (self.section + 1) % n } else { (self.section + n - 1) % n };
        self.selected = 0;
    }

    /// The next / previous entry, circular.
    pub fn step(&mut self, down: bool) {
        let n = self.entries().len().max(1);
        self.selected = if down { (self.selected + 1) % n } else { (self.selected + n - 1) % n };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_and_entries_are_circular() {
        let mut v = View::new();
        assert_eq!(v.entry().unwrap().name, "Distance between places");
        v.step(false);
        assert_eq!(v.entry().unwrap().name, "Mount Everest", "above the first: the last");
        v.switch(false);
        assert_eq!((trivia::sections()[v.section], v.selected), ("Physics", 0));
        v.switch(true);
        v.switch(true);
        assert_eq!(v.entry().unwrap().name, "Voyager 1", "the Voyagers first");
    }

}
