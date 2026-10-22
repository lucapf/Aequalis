//! The Sizes window (F4, Alt+Z, or 󰩻󱗈 Sizes in the top bar; Sizes in the Measures): the groups of
//! the Sizes class (Shoes), which write their item word, then the system keys, which write their
//! code. ↑↓ (j k) choose, Enter writes.

/// A row: (name, what it is, what it writes).
pub fn rows() -> Vec<(&'static str, &'static str, &'static str)> {
    let groups = aequalis::sizes::groups().into_iter().map(|g| (g.name, group_sample(g.write), g.write));
    let systems = aequalis::sizes::SYSTEM_KEYS.iter().map(|&(name, write)| (name, system_sample(write), write));
    groups.chain(systems).collect()
}

fn group_sample(word: &str) -> &'static str {
    match word {
        "shoes" => "men, women, kids: 42 eu men shoes us",
        _ => "",
    }
}

fn system_sample(code: &str) -> &'static str {
    match code {
        "eu" => "Europe",
        "uk" => "United Kingdom",
        "us" => "United States",
        "it" => "Italy: shoes in EU sizes",
        "fr" => "France: shoes in EU sizes",
        "de" => "Germany: shoes in EU sizes",
        "jp" => "Japan, in cm",
        "cn" => "China: the EU number",
        _ => "",
    }
}

/// The row chosen.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct View {
    pub selected: usize,
}

impl View {
    pub fn step(&mut self, down: bool) {
        let n = rows().len();
        self.selected = if down { (self.selected + 1) % n } else { (self.selected + n - 1) % n };
    }

    /// What the row chosen writes.
    pub fn word(&self) -> Option<&'static str> {
        rows().get(self.selected).map(|r| r.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_then_systems() {
        let rows = rows();
        assert_eq!(rows[0], ("Shoes", "men, women, kids: 42 eu men shoes us", "shoes"));
        assert_eq!(rows.iter().map(|r| r.2).collect::<Vec<_>>(), ["shoes", "eu", "uk", "us", "it", "fr", "de", "jp", "cn"]);
        let mut v = View::default();
        v.step(false);
        assert_eq!(v.word(), Some("cn"), "round");
        v.step(true);
        assert_eq!(v.word(), Some("shoes"));
    }
}
