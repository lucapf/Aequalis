//! The Trivia window (`R`): the entries of `trivia.rs` by section (the tabs), one per row, with
//! the selected one's description below. Enter writes its value into the expression. The
//! spacecraft's distances are retrieved in the background the first time it opens.

use crate::trivia::{self, Entry};
use std::sync::{Arc, Mutex};

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

/// Where a spacecraft's distance stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LiveState {
    /// Not asked (tests, or before the window opens) or being retrieved.
    Pending,
    /// Retrieved (au), or `None`: offline, so it is estimated.
    Done(Option<f64>),
}

/// The spacecraft's distances, retrieved once in the background.
#[derive(Default)]
pub struct Live {
    result: Arc<Mutex<Option<Vec<(&'static str, Option<f64>)>>>>,
    started: bool,
    seen: bool,
}

impl Live {
    /// Starts retrieving them (once).
    pub fn start(&mut self) {
        if std::mem::replace(&mut self.started, true) {
            return;
        }
        let result = Arc::clone(&self.result);
        std::thread::spawn(move || {
            let fetched = trivia::fetch_all();
            if let Ok(mut r) = result.lock() {
                *r = Some(fetched);
            }
        });
    }

    pub fn state(&self, name: &str) -> LiveState {
        match self.result.lock().ok().and_then(|r| r.clone()) {
            Some(all) => LiveState::Done(all.iter().find(|(n, _)| *n == name).and_then(|(_, au)| *au)),
            None => LiveState::Pending,
        }
    }

    /// True once, when the distances have arrived: the screen is redrawn.
    pub fn arrived(&mut self) -> bool {
        let done = self.result.lock().is_ok_and(|r| r.is_some());
        done && !std::mem::replace(&mut self.seen, true)
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

    #[test]
    fn nothing_is_retrieved_unless_started() {
        let mut live = Live::default();
        assert_eq!(live.state("Voyager 1"), LiveState::Pending);
        assert!(!live.arrived());
    }
}
