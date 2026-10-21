//! Ctrl+K: search units, currencies, settings, trivia, cities and earlier requests (`find.rs`).

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Search {
    pub query: String,
    pub selected: usize,
}
