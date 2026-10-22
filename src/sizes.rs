//! Clothing and shoe sizes (`docs/SPEC_SIZES.md`): the shoe charts and the words of a size request,
//! compiled from `src/data/sizes.json` (`build/tables.rs`).

/// Whose sizes: a chart is for one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Person {
    Men,
    Women,
    Kids,
}

/// A kids' size that starts over: toddler (US `C`) or youth (US `Y`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Age {
    Toddler,
    Youth,
}

/// A word of a size request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Word {
    /// An item (`ITEMS`): shoes, dress…
    Item(usize),
    /// A body measure (`BODY`): foot, collar…
    Body(usize),
    Person(Person),
    Age(Age),
}

/// A row of a shoe chart: one size in every system, as the brand writes it (empty: the chart has no
/// such column), and the foot it stands for.
#[derive(Debug, PartialEq)]
pub struct ShoeRow {
    pub us: &'static str,
    pub uk: &'static str,
    pub eu: &'static str,
    pub jp: &'static str,
    pub foot_cm: f64,
}

/// A brand's shoe chart for one person.
#[derive(Debug, PartialEq)]
pub struct ShoeChart {
    pub person: Person,
    pub brand: &'static str,
    pub rows: &'static [ShoeRow],
}

include!(concat!(env!("OUT_DIR"), "/sizes.rs"));

/// The shoe charts, Nike's before Clarks's for each person.
pub fn shoe_charts() -> &'static [ShoeChart] {
    SHOE_CHARTS
}

/// What `word` is in a size request, accents and case aside; and whether it counts only beside
/// another size word (`kid` and `top` are currencies too, `foot` a unit).
pub fn word(word: &str) -> Option<(Word, bool)> {
    let k = crate::time::tz::key(word);
    SIZE_WORDS.binary_search_by(|(w, ..)| (*w).cmp(k.as_str())).ok().map(|at| (SIZE_WORDS[at].1, SIZE_WORDS[at].2))
}

/// An item's name (`shoes`), and whether it is not answered yet.
pub fn item(i: usize) -> (&'static str, bool) {
    ITEMS[i]
}

/// A body measure (`foot`), its item (`shoes`), and whether it is not answered yet.
pub fn body(i: usize) -> (&'static str, &'static str, bool) {
    BODY[i]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_six_charts() {
        let charts: Vec<(Person, &str, usize)> = shoe_charts().iter().map(|c| (c.person, c.brand, c.rows.len())).collect();
        assert_eq!(
            charts,
            [
                (Person::Men, "Nike", 19),
                (Person::Men, "Clarks", 21),
                (Person::Women, "Nike", 15),
                (Person::Women, "Clarks", 15),
                (Person::Kids, "Nike", 27),
                (Person::Kids, "Clarks", 33),
            ]
        );
        // Nike's men's US 8.5: EU 42, JP 26.5, a 10 3/16 in foot; Clarks's UK 8: US 9, a 263 mm one.
        let row = &shoe_charts()[0].rows[5];
        assert_eq!((row.us, row.uk, row.eu, row.jp), ("8.5", "7.5", "42", "26.5"));
        assert!((row.foot_cm - 25.876).abs() < 0.001, "{}", row.foot_cm);
        let row = &shoe_charts()[1].rows[6];
        assert_eq!((row.us, row.uk, row.eu, row.jp, row.foot_cm), ("9", "8", "42", "", 26.3));
    }

    #[test]
    fn the_words_accents_and_case_aside() {
        let shoes = word("shoes").unwrap().0;
        for w in ["Scarpe", "ZAPATOS", "Schuhe", "ayakkabı", "ayakkabi", "giày", "giay", "buty"] {
            assert_eq!(word(w).map(|(w, _)| w), Some(shoes), "{w}");
        }
        assert_eq!(
            item(match shoes {
                Word::Item(i) => i,
                _ => unreachable!(),
            }),
            ("shoes", false)
        );
        assert_eq!(word("donna"), Some((Word::Person(Person::Women), false)));
        assert_eq!(word("Männer"), Some((Word::Person(Person::Men), false)));
        assert_eq!(word("toddler"), Some((Word::Age(Age::Toddler), false)));
        assert!(matches!(word("fuß"), Some((Word::Body(0), false))));
        // Clothing is recognised, not answered yet.
        let Some((Word::Item(dress), false)) = word("vestito") else { panic!() };
        assert_eq!(item(dress), ("dress", true));
        // A currency or a unit too: only beside another size word.
        assert_eq!(word("kid"), Some((Word::Person(Person::Kids), true)));
        assert!(word("top").unwrap().1 && word("foot").unwrap().1);
        assert_eq!(word("nike"), None);
    }
}
