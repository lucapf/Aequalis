//! The Measures window (`M`, or Measures in the top bar): the unit classes (`Config::categories`,
//! and the Earth distance), one per row. ↑↓ (j k) choose, a class's letter jumps to it, Enter
//! opens its units (the picker); Esc in the units comes back here.

use aequalis::config::{Category, Config};

/// Each unit class's letter in this list (upper case, each its own) and its unit buttons per row
/// in the unit keys, by the class's name: every class of `Config` has one (test
/// `every_class_once_with_its_own_letter`).
const CLASSES: [(&str, char, usize); 13] = [
    (aequalis::config::TIME, 'T', 5),
    (aequalis::config::BITS, 'B', 5),
    ("Length", 'L', 4),
    ("Area", 'A', 5),
    ("Volume", 'V', 5),
    ("Angle", 'G', 4),
    ("Mass", 'M', 4),
    ("Temperature", 'K', 4),
    ("Speed", 'S', 4),
    ("Pressure", 'P', 4),
    ("Fuel economy", 'F', 4),
    ("Energy", 'E', 5),
    ("Power", 'W', 4),
];

fn class(category: &Category) -> (char, usize) {
    CLASSES.iter().find(|(name, ..)| *name == category.name).map_or(('?', 4), |&(_, key, columns)| (key, columns))
}

/// The class's letter in this list.
pub fn key(category: &Category) -> char {
    class(category).0
}

/// The class's unit buttons per row in the unit keys.
pub fn columns(category: &Category) -> usize {
    class(category).1
}

/// A row: a category (index into `Config::categories`), or the Earth distance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Item {
    Category(usize),
    EarthDistance,
}

/// A measure that is no unit class: the distance between two places on the Earth (`geo.rs`),
/// listed with the classes; and its letter.
pub const EARTH_DISTANCE: &str = "Earth distance";
pub const EARTH_DISTANCE_KEY: char = 'D';

/// The rows, in the categories' order, the Earth distance after Length.
pub fn items(config: &Config) -> Vec<Item> {
    let mut items: Vec<Item> = (0..config.categories.len()).map(Item::Category).collect();
    let at = config.categories.iter().position(|c| c.name == "Length").map_or(items.len(), |i| i + 1);
    items.insert(at, Item::EarthDistance);
    items
}

/// A row's name and letter.
pub fn label(config: &Config, item: Item) -> (&'static str, char) {
    match item {
        Item::Category(i) => (config.categories[i].name, key(&config.categories[i])),
        Item::EarthDistance => (EARTH_DISTANCE, EARTH_DISTANCE_KEY),
    }
}

/// The row with the letter `c` (case aside).
pub fn by_key(config: &Config, c: char) -> Option<Item> {
    items(config).into_iter().find(|&i| label(config, i).1 == c.to_ascii_uppercase())
}

/// The window's state: the row chosen.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct View {
    pub selected: usize,
}

impl View {
    /// The window with `item` chosen (back from its units).
    pub fn at(config: &Config, item: Item) -> View {
        View { selected: items(config).iter().position(|&i| i == item).unwrap_or(0) }
    }

    pub fn item(&self, config: &Config) -> Item {
        items(config)[self.selected]
    }

    /// The next / previous row, circular.
    pub fn step(&mut self, config: &Config, forward: bool) {
        let n = items(config).len();
        self.selected = if forward { (self.selected + 1) % n } else { (self.selected + n - 1) % n };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_once_with_its_own_letter() {
        let config = Config::default();
        let items = items(&config);
        assert_eq!(items.len(), config.categories.len() + 1);
        let names: Vec<&str> = items.iter().map(|&i| label(&config, i).0).collect();
        assert_eq!(&names[..4], [aequalis::config::TIME, aequalis::config::BITS, "Length", EARTH_DISTANCE]);
        let mut keys: Vec<char> = items.iter().map(|&i| label(&config, i).1).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), items.len(), "the letters differ");
        // Every class in the table, an upper-case letter and 4 or 5 buttons per row each.
        for c in &config.categories {
            let (key, columns) = class(c);
            assert!(key.is_ascii_uppercase() && (4..=5).contains(&columns), "{}: not in CLASSES", c.name);
        }
        assert_eq!(by_key(&config, 'k').map(|i| label(&config, i).0), Some("Temperature"));
        assert_eq!(by_key(&config, 'd'), Some(Item::EarthDistance));
    }

    #[test]
    fn moving_around() {
        let config = Config::default();
        let mut v = View::default();
        v.step(&config, false);
        assert_eq!(v.selected, items(&config).len() - 1, "circular");
        v.step(&config, true);
        assert_eq!(v.item(&config), Item::Category(0));
        assert_eq!(View::at(&config, Item::EarthDistance).item(&config), Item::EarthDistance);
    }
}
