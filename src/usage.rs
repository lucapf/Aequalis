//! How often each unit, currency and city was used, for the interactive mode's board: what is used
//! most comes first. Kept in `~/.config/aequalis/usage.json` (`{"units": {"km": 12}, …}`); a state
//! the program keeps, not a setting (the Settings window clears it).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What is counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Unit,
    Currency,
    City,
}

impl Kind {
    fn key(self) -> &'static str {
        match self {
            Kind::Unit => "units",
            Kind::Currency => "currencies",
            Kind::City => "cities",
        }
    }
}

const KINDS: [Kind; 3] = [Kind::Unit, Kind::Currency, Kind::City];

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Usage {
    counts: BTreeMap<(usize, String), u64>,
}

impl Usage {
    fn index(kind: Kind) -> usize {
        KINDS.iter().position(|k| *k == kind).unwrap_or(0)
    }

    /// `name` was used once more.
    pub fn hit(&mut self, kind: Kind, name: &str) {
        let count = self.counts.entry((Self::index(kind), name.to_string())).or_insert(0);
        *count = count.saturating_add(1);
    }

    /// How many times `name` was used.
    pub fn hits(&self, kind: Kind, name: &str) -> u64 {
        self.counts.get(&(Self::index(kind), name.to_string())).copied().unwrap_or(0)
    }

    /// Nothing counted yet.
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// `names` ordered by use, the most used first; those used as often keep their order.
    pub fn ranked<'a>(&self, kind: Kind, names: &[&'a str]) -> Vec<&'a str> {
        let mut ranked: Vec<&str> = names.to_vec();
        ranked.sort_by_key(|n| std::cmp::Reverse(self.hits(kind, n)));
        ranked
    }

    /// The most used of one kind, with their counts, the most first (at most `limit`).
    pub fn top(&self, kind: Kind, limit: usize) -> Vec<(String, u64)> {
        let mut all: Vec<(String, u64)> = self.counts.iter().filter(|((k, _), _)| *k == Self::index(kind)).map(|((_, n), c)| (n.clone(), *c)).collect();
        all.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        all.truncate(limit);
        all
    }

    /// The file's content.
    pub fn to_json(&self) -> String {
        let section = |kind: Kind| {
            let items: Vec<String> = self
                .counts
                .iter()
                .filter(|((k, _), _)| *k == Self::index(kind))
                .map(|((_, name), count)| format!("{}: {count}", crate::json::string(name)))
                .collect();
            format!("  {}: {{{}}}", crate::json::string(kind.key()), items.join(", "))
        };
        format!("{{\n{}\n}}\n", KINDS.map(section).join(",\n"))
    }

    /// Reads a file's content (a bad file is no usage at all).
    pub fn from_json(text: &str) -> Usage {
        let mut usage = Usage::default();
        let Ok(doc) = crate::json::parse(text) else { return usage };
        for kind in KINDS {
            let Some(section) = doc.get(kind.key()) else { continue };
            for (name, value) in section.members() {
                if let Some(count) = value.number().filter(|n| *n >= 1.0) {
                    usage.counts.insert((Self::index(kind), name.clone()), count as u64);
                }
            }
        }
        usage
    }
}

/// `~/.config/aequalis/usage.json`.
pub fn path() -> Option<PathBuf> {
    crate::files::dir().map(|d| d.join("usage.json"))
}

pub fn load_from(path: &Path) -> Usage {
    std::fs::read_to_string(path).map(|text| Usage::from_json(&text)).unwrap_or_default()
}

pub fn save_to(path: &Path, usage: &Usage) -> std::io::Result<()> {
    crate::files::write_atomic(path, usage.to_json().as_bytes())
}

/// The usage kept in the file in use.
pub fn load() -> Usage {
    path().map(|p| load_from(&p)).unwrap_or_default()
}

pub fn save(usage: &Usage) {
    if let Some(path) = path() {
        let _ = save_to(&path, usage);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counted_and_ranked() {
        let mut u = Usage::default();
        assert!(u.is_empty());
        for name in ["km", "km", "mi", "km", "mi", "ft"] {
            u.hit(Kind::Unit, name);
        }
        u.hit(Kind::Currency, "km");
        assert_eq!((u.hits(Kind::Unit, "km"), u.hits(Kind::Unit, "mi"), u.hits(Kind::Unit, "m")), (3, 2, 0));
        assert_eq!(u.hits(Kind::Currency, "km"), 1, "a kind of its own");
        // The most used first; the ones not used, in the order given.
        assert_eq!(u.ranked(Kind::Unit, &["m", "ft", "mi", "km", "cm"]), ["km", "mi", "ft", "m", "cm"]);
        assert_eq!(u.top(Kind::Unit, 2), [("km".to_string(), 3), ("mi".to_string(), 2)]);
    }

    #[test]
    fn kept_in_a_file() {
        let mut u = Usage::default();
        u.hit(Kind::Unit, "km");
        u.hit(Kind::Unit, "km");
        u.hit(Kind::Currency, "EUR");
        u.hit(Kind::City, "New York");
        assert_eq!(Usage::from_json(&u.to_json()), u);
        let dir = std::env::temp_dir().join(format!("aequalis-usage-{}", std::process::id()));
        let file = dir.join("usage.json");
        save_to(&file, &u).unwrap();
        assert_eq!(load_from(&file), u);
        let _ = std::fs::remove_dir_all(&dir);
        // A missing or broken file is no usage.
        assert!(load_from(&dir.join("none.json")).is_empty());
        assert!(Usage::from_json("not json").is_empty());
    }
}
