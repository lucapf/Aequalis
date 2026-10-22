//! The History panel (F5, or History in the toolbar): every request evaluated, newest first,
//! filtered by what is typed. Enter writes the request into the line, Alt+Y copies its result, Del
//! removes it; a click does the same with `↩`, `⧉` and `✕`.

use super::editor::History;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct View {
    pub query: String,
    pub selected: usize,
}

/// The entries (oldest-first indexes into `history`) whose request contains `query`, case aside,
/// the newest first.
pub fn matches(history: &History, query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    (0..history.entries().len()).rev().filter(|&i| q.is_empty() || history.entries()[i].input.to_lowercase().contains(&q)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::editor::Entry;

    #[test]
    fn newest_first_and_filtered() {
        let mut h = History::new(10);
        for input in ["1+1", "10 km mi", "2^8", "5 KM ft"] {
            h.push(Entry { input: input.into(), result: Ok(String::new()), copy: None });
        }
        assert_eq!(matches(&h, ""), [3, 2, 1, 0]);
        assert_eq!(matches(&h, " km"), [3, 1], "case aside");
        assert!(matches(&h, "zzz").is_empty());
    }
}
