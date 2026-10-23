//! Clothing and shoe sizes (`docs/SPEC_SIZES.md`, `docs/SPEC_CLOTHES.md`): the shoe charts and the
//! words of a size request, compiled from `src/data/sizes.json` (`build/tables.rs`); the clothes in
//! `clothes`. A request that means several things answers every reading, by priority.

mod clothes;

pub use clothes::{clothes_charts, Chart, ClothesChart, ClothesRow};

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
    /// `size`, `taglia`, `Größe`: a size request, shoes and clothes alike.
    Size,
    /// A country or its adjective as a system (`italian`, `USA`): only beside a size word.
    System(System),
    /// Words a size request may have around it (`my`, `what`, `is`): ignored.
    Filler,
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

// ---------------------------------------------------------------- for the UIs

/// A group of the Sizes class (Measures, the Sizes menu): its name, and the word it writes.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub name: &'static str,
    pub write: &'static str,
}

/// The groups of the Sizes class: Shoes and Clothes (SPEC_CLOTHES §7); the item words (`dress`,
/// `suit`) are typed or suggested.
pub fn groups() -> Vec<Group> {
    vec![Group { name: "Shoes", write: "shoes" }, Group { name: "Clothes", write: "clothes" }]
}

/// The people keys of the Sizes class, (shown, written): Men, Women, Kids.
pub const PEOPLE_KEYS: [(&str, &str); 3] = [("Men", "men"), ("Women", "women"), ("Kids", "kids")];

/// The system keys of the Sizes class, (shown, written): EU, UK, US, IT, FR, DE, JP, CN, INT.
pub const SYSTEM_KEYS: [(&str, &str); 9] =
    [("EU", "eu"), ("UK", "uk"), ("US", "us"), ("IT", "it"), ("FR", "fr"), ("DE", "de"), ("JP", "jp"), ("CN", "cn"), ("INT", "int")];

/// The letter keys of the Sizes class, (shown, written): XS to XXL.
pub const LETTER_KEYS: [(&str, &str); 6] = [("XS", "XS"), ("S", "S"), ("M", "M"), ("L", "L"), ("XL", "XL"), ("XXL", "XXL")];

/// Where `line`'s first item word is (`scarpe`, `quần áo`): (first word, how many words).
fn item_at(words: &[&str]) -> Option<(usize, usize)> {
    (0..words.len()).find_map(|i| (1..=3.min(words.len() - i)).rev().find(|&n| matches!(word(&words[i..i + n].join(" ")), Some((Word::Item(_), _)))).map(|n| (i, n)))
}

/// `line` with a Sizes key written (SPEC_CLOTHES §7): a system's code or a letter after it, nothing
/// else (`42` + `eu` → `42 eu `: the answer brings the currency, the shoes and the clothes); a
/// group's word in place of the item word there (`shoes 42 eu` + clothes → `clothes 42 eu `), else
/// after it; a person in place of the one there (`shoes 42 men` + women → `shoes 42 women `).
pub fn write_key(line: &str, key: &str) -> String {
    let kept;
    let line = match word(key) {
        Some((Word::Person(_), _)) => {
            kept = without_people(line.trim());
            kept.as_str()
        }
        _ => line.trim(),
    };
    let join = |parts: &[&str]| format!("{} ", parts.iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join(" "));
    let words: Vec<&str> = line.split_whitespace().collect();
    match (groups().iter().any(|g| g.write == key), item_at(&words)) {
        (true, Some((i, n))) => {
            let mut out: Vec<&str> = words[..i].to_vec();
            out.push(key);
            out.extend(&words[i + n..]);
            join(&out)
        }
        _ => join(&[line, key]),
    }
}

/// `line` without its person words (`men`, `donna`, `trẻ em`): a person key replaces them.
fn without_people(line: &str) -> String {
    let words: Vec<&str> = line.split_whitespace().collect();
    let mut kept = Vec::new();
    let mut i = 0;
    'next: while i < words.len() {
        for n in (1..=3.min(words.len() - i)).rev() {
            if matches!(word(&words[i..i + n].join(" ")), Some((Word::Person(_), _))) {
                i += n;
                continue 'next;
            }
        }
        kept.push(words[i]);
        i += 1;
    }
    kept.join(" ")
}

/// What a size word means, for a suggestion's label: `shoes`, `foot`, `women`, `toddler`.
fn meaning(word: Word) -> &'static str {
    match word {
        Word::Item(i) => ITEMS[i].0,
        Word::Body(i) => BODY[i].0,
        Word::Person(p) => person_word(p, None),
        Word::Age(a) => person_word(Person::Kids, Some(a)),
        Word::Size => "size",
        Word::System(s) => s.code(),
        Word::Filler => "",
    }
}

/// Suggestions while typing (`suggest.rs`): the size words that start with the word typed, from
/// three letters (`scar` → `scarpe - shoes`), as (written, shown); after a shoe size with no system
/// yet (`shoes 42 `), the systems. The position (in chars) the suggestions replace from.
pub fn suggestions(before: &str, typing: bool) -> Option<(usize, Vec<(String, String)>)> {
    let end = before.chars().count();
    if !typing {
        let tokens = tokens(before)?;
        let shoes = tokens.iter().any(|t| matches!(t, Token::Word(Word::Item(i), _) if ITEMS[*i].0 == "shoes") || matches!(t, Token::Word(Word::Body(i), _) if BODY[*i].1 == "shoes"));
        let clothes = tokens.iter().any(|t| {
            matches!(t, Token::Word(Word::Item(i), _) if !ITEMS[*i].1 && ITEMS[*i].0 != "shoes") || matches!(t, Token::Word(Word::Body(i), _) if !BODY[*i].2 && BODY[*i].1 != "shoes")
        });
        let after_size = matches!(tokens.last(), Some(Token::Number(n)) if n.age.is_none());
        let system = tokens.iter().any(|t| matches!(t, Token::System(..)));
        if !((shoes || clothes) && after_size && !system) {
            return None;
        }
        let names: &[&str] = match clothes {
            false => &["eu - Europe", "uk - United Kingdom", "us - United States", "jp - Japan (cm)", "cn - China"],
            true => &["eu - Europe", "uk - United Kingdom", "us - United States", "it - Italy", "fr - France", "de - Germany", "jp - Japan", "cn - China"],
        };
        return Some((end, names.iter().map(|n| (n[..2].to_string(), n.to_string())).collect()));
    }
    let prefix = before.split_whitespace().last()?;
    if prefix.chars().count() < 3 || prefix.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let typed = crate::time::tz::key(prefix);
    // Countries and adjectives only in a size request (`dress 44 ital`): `12 usd eur` is a currency.
    let asking = before.split_whitespace().rev().skip(1).any(|w| matches!(word(w), Some((Word::Item(_) | Word::Body(_) | Word::Person(_) | Word::Size, _))));
    let found: Vec<(String, String)> = SIZE_WORDS
        .iter()
        .filter(|(w, word, _)| w.starts_with(typed.as_str()) && *word != Word::Filler && (asking || !matches!(word, Word::System(_))))
        .take(crate::suggest::MAX)
        // `shoes`, not `shoes - shoes`.
        .map(|&(w, word, _)| (w.to_string(), if meaning(word) == w { w.to_string() } else { format!("{w} - {}", meaning(word)) }))
        .collect();
    (!found.is_empty()).then(|| (end - prefix.chars().count(), found))
}

// ---------------------------------------------------------------- a size request

/// A size system. For shoes, Italian, French and German sizes are EU sizes and Chinese ones the EU
/// number, written CN; INT (the letters) is clothes' only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum System {
    Us,
    Uk,
    Eu,
    Jp,
    Cn,
    It,
    Fr,
    De,
    Int,
}

impl System {
    /// The shoes' systems, in the order the details and the table give them.
    pub const ALL: [System; 5] = [System::Us, System::Uk, System::Eu, System::Jp, System::Cn];

    pub fn name(self) -> &'static str {
        match self {
            System::Us => "US",
            System::Uk => "UK",
            System::Eu => "EU",
            System::Jp => "JP",
            System::Cn => "CN",
            System::It => "IT",
            System::Fr => "FR",
            System::De => "DE",
            System::Int => "INT",
        }
    }

    /// As typed and written by the keys: `us`, `int`.
    pub fn code(self) -> &'static str {
        match self {
            System::Us => "us",
            System::Uk => "uk",
            System::Eu => "eu",
            System::Jp => "jp",
            System::Cn => "cn",
            System::It => "it",
            System::Fr => "fr",
            System::De => "de",
            System::Int => "int",
        }
    }

    /// A shoe row's size in this system (empty: the chart has none); CN, IT, FR and DE read EU.
    fn of(self, row: &ShoeRow) -> &'static str {
        match self {
            System::Us => row.us,
            System::Uk => row.uk,
            System::Eu | System::Cn | System::It | System::Fr | System::De => row.eu,
            System::Jp => row.jp,
            System::Int => "",
        }
    }

    /// `UK 7.5`, `JP 26.5 cm`.
    fn show(self, size: &str) -> String {
        match self {
            System::Jp => format!("JP {size} cm"),
            s => format!("{} {size}", s.name()),
        }
    }

    /// For shoes: the system a size is read in, and the note an alias brings (`it`: Italian shoe
    /// sizes are EU sizes); `None`: no shoe system (INT).
    fn for_shoes(self) -> Option<(System, Option<&'static str>)> {
        Some(match self {
            System::Cn => (System::Cn, Some("Chinese shoe sizes are EU sizes")),
            System::It => (System::Eu, Some("Italian shoe sizes are EU sizes")),
            System::Fr => (System::Eu, Some("French shoe sizes are EU sizes")),
            System::De => (System::Eu, Some("German shoe sizes are EU sizes")),
            System::Int => return None,
            s => (s, None),
        })
    }
}

/// A system's code as typed (`eu`, `IT`, `int`).
fn system(word: &str) -> Option<System> {
    Some(match word.to_ascii_lowercase().as_str() {
        "us" => System::Us,
        "uk" => System::Uk,
        "eu" => System::Eu,
        "jp" => System::Jp,
        "cn" => System::Cn,
        "it" => System::It,
        "fr" => System::Fr,
        "de" => System::De,
        "int" => System::Int,
        _ => return None,
    })
}

/// Why a size request has no answer.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// Not a request that can be read: exit code 2.
    Usage(String),
    /// On its way: what comes next.
    Incomplete(String),
    /// Read, but no size to give (`no EU 60 in men's shoes`, `kids' clothes: not yet`): exit code 1.
    Answer(String),
}

/// A size request's answer.
#[derive(Debug, Default, PartialEq)]
pub struct Answer {
    /// What is shown: a size (`US 8.5 (Nike), 9 (Clarks)`), the people's line, the table.
    pub text: String,
    /// The result for programs and Copy when `text` says more (the first person's): `US 8.5 men`.
    pub result: Option<String>,
    /// The other results, (kind, results): `("shoes", ["US 10 women"])`.
    pub extra: Vec<(String, Vec<String>)>,
    /// The other systems, `UK 7.5 · EU 42 · JP 26.5 cm · CN 42`.
    pub details: Option<String>,
    pub notes: Vec<String>,
    /// What was converted, as shown: `EU 42`, `27 cm foot`.
    pub from: String,
    /// What Copy gives when every person's size is shown (no person typed): a line each,
    /// `Men US 8.5` / `Women US 10`; `Men US 8.5 · UK 7.5 · EU 42 · JP 26.5 · CN 42`.
    pub copy: Option<String>,
    /// This answer as one line of another's (`US 10 women`, `IT 44 women, IT 48 men's suit`): the
    /// results of `shoes: 42 eu = …`.
    pub line: Vec<String>,
    /// The request's other readings, by priority (`42 eu women us`: the women's shoes after the
    /// clothes): `shoes: EU 42 = US 10 women`.
    pub also: Vec<crate::Extra>,
}

/// A person's label in a copied line: `Men`, `Toddler`.
fn title(who: &str) -> String {
    let mut chars = who.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

const COMING: &str = "only shoes and clothes for now";
const BRAND_NOTE: &str = "sizes vary by brand: check the brand's chart";

/// A size typed: `42`, `9.5`, `9½`, `42⅔`, `4C`, `2Y`.
#[derive(Debug, Clone, PartialEq)]
struct Number {
    value: f64,
    age: Option<Age>,
    text: String,
}

fn number(word: &str) -> Option<Number> {
    let (body, age) = match word.chars().last()? {
        'c' | 'C' => (&word[..word.len() - 1], Some(Age::Toddler)),
        'y' | 'Y' => (&word[..word.len() - 1], Some(Age::Youth)),
        _ => (word, None),
    };
    let (whole, part) = match body.chars().last()? {
        '½' => (&body[..body.len() - '½'.len_utf8()], 0.5),
        '⅓' => (&body[..body.len() - '⅓'.len_utf8()], 1.0 / 3.0),
        '⅔' => (&body[..body.len() - '⅔'.len_utf8()], 2.0 / 3.0),
        '¼' => (&body[..body.len() - '¼'.len_utf8()], 0.25),
        '¾' => (&body[..body.len() - '¾'.len_utf8()], 0.75),
        _ => (body, 0.0),
    };
    let digits = !whole.is_empty() && whole.bytes().all(|b| b.is_ascii_digit() || b == b'.' || b == b',') && whole.as_bytes()[0].is_ascii_digit();
    let value = whole.replace(',', ".").parse::<f64>().ok().filter(|_| digits)? + part;
    (value.is_finite() && value > 0.0).then(|| Number { value, age, text: word.to_string() })
}

/// A plain number, no age: `32`, `15.5`.
fn plain(word: &str) -> Option<Number> {
    number(word).filter(|n| n.age.is_none())
}

/// A letter size, as the charts write it: `m` → `M`, `2XL` → `XXL`; JP's `LL` and `3L`.
fn letter(word: &str) -> Option<String> {
    let up = word.to_ascii_uppercase();
    let up = match up.as_str() {
        "2XL" => "XXL".to_string(),
        "3XL" => "XXXL".to_string(),
        _ => up,
    };
    ["XXS", "XS", "S", "M", "L", "XL", "XXL", "XXXL", "LL", "3L"].contains(&up.as_str()).then_some(up)
}

/// A size label of one word (`44`, `M`).
#[derive(Debug, Clone, PartialEq)]
enum Label {
    Number(Number),
    Letter(String),
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    /// A size word, and whether it counts only beside another.
    Word(Word, bool),
    Number(Number),
    /// A letter size: `M`, `XXL`, `3L`.
    Letter(String),
    /// Two sizes: `8-10`, `S/M`, `32/34` (a waist and its inseam for trousers), as typed.
    Range(Label, Label, String),
    /// A Chinese label, `160/84A`: the bust (after the `/`), as typed.
    Cn(f64, String),
    /// A waist, `W32`, `32W`, `32x34`: the waist and the inseam, in inches.
    Waist(f64, Option<f64>, String),
    /// An inseam alone, `L34`: beside a waist.
    Inseam(f64),
    /// A system, as typed (`eu`, `italian`), and how.
    System(System, String, Written),
    Cm,
    In,
    Connector,
}

/// How a system is written: its code (`it`), its code on the size (`IT44`), a country or an adjective
/// (`italian`: only beside a size word).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Written {
    Code,
    Attached,
    Name,
}

/// A word as a size label, or a pair of them: `44`, `M`, `8-10`, `S/M`, `160/84A`, `W32`, `32x34`.
fn label_token(w: &str) -> Option<Token> {
    if let Some(n) = number(w) {
        return Some(Token::Number(n));
    }
    if let Some(l) = letter(w) {
        return Some(Token::Letter(l));
    }
    let lower = w.to_ascii_lowercase();
    // `W32`, `32W`, `L34`.
    let digits = |s: &str| plain(s).map(|n| n.value);
    if let Some(v) = lower.strip_prefix('w').and_then(digits).or_else(|| lower.strip_suffix('w').and_then(digits)) {
        return Some(Token::Waist(v, None, w.to_string()));
    }
    if let Some(v) = lower.strip_prefix('l').and_then(digits) {
        return Some(Token::Inseam(v));
    }
    if let Some((a, b)) = lower.split_once('x') {
        if let (Some(a), Some(b)) = (digits(a), digits(b)) {
            return Some(Token::Waist(a, Some(b), w.to_string()));
        }
    }
    // `160/84A`, `165/92`: a height of three digits, then the bust.
    if let Some((h, rest)) = w.split_once('/') {
        let bust = rest.trim_end_matches(|c: char| c.is_ascii_alphabetic());
        if h.len() == 3 && h.bytes().all(|b| b.is_ascii_digit()) && (2..=3).contains(&bust.len()) && rest.len() - bust.len() <= 1 {
            if let Some(b) = digits(bust) {
                return Some(Token::Cn(b, w.to_string()));
            }
        }
    }
    let (a, b) = w.split_once('-').or_else(|| w.split_once('/'))?;
    let one = |s: &str| plain(s).map(Label::Number).or_else(|| letter(s).map(Label::Letter));
    match (one(a)?, one(b)?) {
        (l @ Label::Number(_), r @ Label::Number(_)) | (l @ Label::Letter(_), r @ Label::Letter(_)) => Some(Token::Range(l, r, w.to_string())),
        _ => None,
    }
}

/// A size and its system written as one word: `IT44`, `44it`, `eu42.5`; the system as typed, and
/// whether it comes first.
fn attached(w: &str) -> Option<(System, String, Number, bool)> {
    let lower = w.to_ascii_lowercase();
    let cut = lower.len().checked_sub(2)?;
    if lower.len() > 2 && lower.is_char_boundary(2) {
        if let (Some(s), Some(n)) = (system(&lower[..2]), number(&w[2..])) {
            return Some((s, lower[..2].to_string(), n, true));
        }
    }
    if cut > 0 && lower.is_char_boundary(cut) {
        if let (Some(s), Some(n)) = (system(&lower[cut..]), number(&w[..cut])) {
            return Some((s, lower[cut..].to_string(), n, false));
        }
    }
    None
}

/// The request's words, a size word of up to three words first (`trẻ em`, `ao so mi`); `None`
/// when a word is none of a size request's (a brand, a unit, a place). Punctuation around words is
/// dropped (`44 it,`, `us?`).
fn tokens(input: &str) -> Option<Vec<Token>> {
    let words: Vec<&str> = input
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| matches!(c, ',' | '?' | '!' | ':' | ';' | '¿' | '¡')))
        .filter(|w| !w.is_empty())
        .collect();
    // `taille` is French for waist beside a measure (`70 cm taille`), else for size (SPEC_CLOTHES §2.3).
    let measured = words.iter().any(|w| {
        let lower = w.to_ascii_lowercase();
        lower == "cm" || lower == "in" || lower.ends_with('"') || ["cm", "in"].iter().any(|u| lower.strip_suffix(u).and_then(plain).is_some())
    });
    let mut out = Vec::new();
    let mut i = 0;
    'next: while i < words.len() {
        for n in (1..=3.min(words.len() - i)).rev() {
            let phrase = words[i..i + n].join(" ");
            // `men's`, `women’s`.
            let plain = phrase.strip_suffix("'s").or_else(|| phrase.strip_suffix("’s")).unwrap_or(&phrase);
            if let Some((w, weak)) = word(&phrase).or_else(|| word(plain)) {
                out.push(match w {
                    Word::System(s) => Token::System(s, phrase, Written::Name),
                    Word::Body(_) if !measured && crate::time::tz::key(&phrase) == "taille" => Token::Word(Word::Size, false),
                    w => Token::Word(w, weak),
                });
                i += n;
                continue 'next;
            }
            // A country (`Italy`, `United States`) as its system; not a code (`us` is the system).
            if phrase.chars().count() > 3 {
                if let Some(s) = crate::time::tz::country(&phrase).and_then(|c| country_system(c.code)) {
                    out.push(Token::System(s, phrase, Written::Name));
                    i += n;
                    continue 'next;
                }
            }
        }
        let w = words[i];
        let lower = w.to_ascii_lowercase();
        // `92cm`, `36"`, `36in`.
        let unit = [("cm", Token::Cm), ("\"", Token::In), ("″", Token::In), ("in", Token::In)]
            .into_iter()
            .find_map(|(u, t)| lower.strip_suffix(u).and_then(plain).map(|n| (n, t)));
        if let Some(t) = label_token(w) {
            out.push(t);
        } else if let Some((n, unit)) = unit {
            out.extend([Token::Number(n), unit]);
        } else if let Some((s, typed, n, before)) = attached(w) {
            let s = Token::System(s, typed, Written::Attached);
            if before {
                out.extend([s, Token::Number(n)]);
            } else {
                out.extend([Token::Number(n), s]);
            }
        } else if let Some(s) = system(w) {
            out.push(Token::System(s, w.to_string(), Written::Code));
        } else if lower == "cm" {
            out.push(Token::Cm);
        } else if lower == "in" {
            out.push(Token::In);
        } else if ["to", "->", "→", "a", "en", "nach", "=", "em", "ke", "sang", "na"].contains(&lower.as_str()) {
            out.push(Token::Connector);
        } else {
            return None;
        }
        i += 1;
    }
    Some(pronouns(out))
}

/// `it` and `de` as words, not systems, dropped: the English pronoun beside a filler or before a
/// connector, away from a size (`what is it in us`); the Spanish and Portuguese "of" between a size
/// word and the size (`talla de 44`).
fn pronouns(tokens: Vec<Token>) -> Vec<Token> {
    let label = |t: Option<&Token>| matches!(t, Some(Token::Number(_) | Token::Letter(_) | Token::Range(..) | Token::Cn(..) | Token::Waist(..)));
    let word = |t: Option<&Token>| matches!(t, Some(Token::Word(Word::Size | Word::Item(_), _)));
    (0..tokens.len())
        .filter(|&i| {
            let (before, after) = (i.checked_sub(1).and_then(|j| tokens.get(j)), tokens.get(i + 1));
            match &tokens[i] {
                Token::System(System::It, _, Written::Code) => {
                    label(before) || label(after) || !(matches!(before, Some(Token::Word(Word::Filler, _))) || matches!(after, Some(Token::In | Token::Connector)))
                }
                Token::System(System::De, _, Written::Code) => !(word(before) && label(after)),
                _ => true,
            }
        })
        .map(|i| tokens[i].clone())
        .collect()
}

/// The size system of a country (`IT` → IT), for the countries that have one.
fn country_system(code: &str) -> Option<System> {
    Some(match code {
        "US" => System::Us,
        "GB" => System::Uk,
        "IT" => System::It,
        "FR" => System::Fr,
        "DE" => System::De,
        "JP" => System::Jp,
        "CN" => System::Cn,
        _ => return None,
    })
}

/// What a size is read from: a size in a system, or a foot's length in cm.
#[derive(Debug, Clone, Copy, PartialEq)]
enum From {
    Size(System, f64),
    Foot(f64),
}

/// The rows a size or a foot falls on, in one chart.
#[derive(Debug)]
struct Hit {
    rows: Vec<&'static ShoeRow>,
    age: Option<Age>,
    /// Between two rows, near their midpoint: the note, `between EU 42 and 42.5`.
    between: Option<String>,
    /// A foot longer or shorter than the chart: its last or first row.
    beyond: bool,
}

/// What a chart says of a size.
#[derive(Debug)]
enum Found {
    /// The chart has no such system (Clarks: no JP).
    Lacks,
    Outside,
    Hits(Vec<Hit>),
}

fn row_age(row: &ShoeRow) -> Option<Age> {
    match row.us.chars().last() {
        Some('C') => Some(Age::Toddler),
        Some('Y') => Some(Age::Youth),
        _ => None,
    }
}

/// A size as a number: `12.5C` → 12.5.
fn numeric(label: &str) -> Option<f64> {
    label.trim_end_matches(['C', 'Y']).parse().ok()
}

/// `0.3` → `0.3`, `27.94` → `27.9`: a foot's length for people.
fn cm(value: f64) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    crate::convert::format_value(rounded, 1)
}

/// Where `from` falls in `chart`: the nearest row (the larger at the midpoint), every row with
/// that size (a size written twice: a range), for kids' US and UK sizes in each age they start
/// over in (`age`: only that one).
fn lookup(chart: &ShoeChart, from: From, age: Option<Age>) -> Found {
    let label = |row: &ShoeRow| match from {
        From::Size(s, _) => s.of(row),
        From::Foot(_) => "",
    };
    let value = |row: &ShoeRow| match from {
        From::Size(s, _) => numeric(s.of(row)),
        From::Foot(_) => Some(row.foot_cm),
    };
    if let From::Size(s, _) = from {
        if chart.rows.iter().all(|r| s.of(r).is_empty()) {
            return Found::Lacks;
        }
    }
    let wanted = match from {
        From::Size(_, v) | From::Foot(v) => v,
    };
    let kids = chart.person == Person::Kids;
    let starts_over = kids && matches!(from, From::Size(System::Us | System::Uk, _));
    // Kids' US and UK sizes start over (13.5C → 1Y, UK 13.5 → 1): a group each time they do.
    let all: Vec<(&'static ShoeRow, f64)> =
        chart.rows.iter().filter(|r| !kids || age.is_none() || row_age(r) == age).filter_map(|r| value(r).map(|v| (r, v))).collect();
    let mut groups: Vec<Vec<(&'static ShoeRow, f64)>> = Vec::new();
    for (i, &(r, v)) in all.iter().enumerate() {
        match groups.last_mut() {
            Some(g) if !(starts_over && i > 0 && v < all[i - 1].1) => g.push((r, v)),
            _ => groups.push(vec![(r, v)]),
        }
    }
    let mut hits = Vec::new();
    for rows in groups {
        let (Some(&(_, first)), Some(&(_, last))) = (rows.first(), rows.last()) else { continue };
        let at = |v: f64| rows.iter().filter(|(_, x)| (x - v).abs() < 1e-9).map(|(r, _)| *r).collect::<Vec<_>>();
        let note = |lo: f64, hi: f64| match from {
            From::Foot(_) => format!("between {} and {} cm", cm(lo), cm(hi)),
            From::Size(s, _) => format!("between {} {} and {}", s.name(), label(at(lo)[0]), label(at(hi)[0])),
        };
        // A kids' size said by its age, or one that starts over: which age it is.
        let hit = |rows: Vec<&'static ShoeRow>, between, beyond| Hit {
            age: if kids && (starts_over || age.is_some()) { row_age(rows[0]) } else { None },
            rows,
            between,
            beyond,
        };
        if wanted < first - 1e-9 || wanted > last + 1e-9 {
            if matches!(from, From::Foot(_)) {
                hits.push(hit(at(if wanted < first { first } else { last }), None, true));
            }
            continue;
        }
        let exact = at(wanted);
        if !exact.is_empty() {
            hits.push(hit(exact, None, false));
            continue;
        }
        let Some(i) = rows.windows(2).position(|w| w[0].1 < wanted && wanted < w[1].1) else { continue };
        let (lo, hi) = (rows[i].1, rows[i + 1].1);
        let mid = (lo + hi) / 2.0;
        let pick = if wanted < mid { lo } else { hi };
        let between = ((wanted - mid).abs() <= (hi - lo) / 4.0 + 1e-9).then(|| note(lo, hi));
        hits.push(hit(at(pick), between, false));
    }
    if hits.is_empty() {
        Found::Outside
    } else {
        Found::Hits(hits)
    }
}

/// The sizes of `rows` in `system`: `8.5`, a range `6.5–7`; `None` when the chart has none.
fn sizes_of(rows: &[&ShoeRow], system: System) -> Option<String> {
    let mut all: Vec<&str> = Vec::new();
    for r in rows {
        let s = system.of(r);
        if !s.is_empty() && !all.contains(&s) {
            all.push(s);
        }
    }
    match all[..] {
        [] => None,
        [one] => Some(one.to_string()),
        [first, .., last] => Some(format!("{first}–{last}")),
    }
}

/// One person's charts, Nike's first.
fn charts(person: Person) -> impl Iterator<Item = &'static ShoeChart> {
    SHOE_CHARTS.iter().filter(move |c| c.person == person)
}

const PEOPLE: [Person; 3] = [Person::Men, Person::Women, Person::Kids];

fn possessive(person: Person) -> &'static str {
    match person {
        Person::Men => "men's",
        Person::Women => "women's",
        Person::Kids => "kids'",
    }
}

fn person_word(person: Person, age: Option<Age>) -> &'static str {
    match (person, age) {
        (_, Some(Age::Toddler)) => "toddler",
        (_, Some(Age::Youth)) => "youth",
        (Person::Men, None) => "men",
        (Person::Women, None) => "women",
        (Person::Kids, None) => "kids",
    }
}

/// One answer of one person, for one age: each chart's size, the row the details come from.
#[derive(Debug)]
struct Part {
    age: Option<Age>,
    /// (brand, size), Nike's first; a chart left out has none.
    sizes: Vec<(&'static str, String)>,
    /// Whether a chart left out has the size's system but not the size: then the other is named.
    named: bool,
    /// The first chart's rows: the details and the table.
    rows: Vec<&'static ShoeRow>,
    brand: &'static str,
    /// (brand, note) of each chart between two rows.
    between: Vec<(&'static str, String)>,
    beyond: bool,
}

/// A person's parts: one, or one per age a kids' size starts over in. Empty: no chart has the size.
fn parts(person: Person, from: From, age: Option<Age>, target: System) -> Vec<Part> {
    let found: Vec<(&'static ShoeChart, Found)> = charts(person).map(|c| (c, lookup(c, from, age))).collect();
    let ages: Vec<Option<Age>> = {
        let mut ages = Vec::new();
        for (_, f) in &found {
            if let Found::Hits(hits) = f {
                for h in hits {
                    if !ages.contains(&h.age) {
                        ages.push(h.age);
                    }
                }
            }
        }
        ages
    };
    let mut out = Vec::new();
    for a in ages {
        let (mut sizes, mut named, mut between, mut beyond, mut first) = (Vec::new(), false, Vec::new(), false, None);
        for (chart, f) in &found {
            let hit = match f {
                Found::Hits(hits) => hits.iter().find(|h| h.age == a),
                _ => None,
            };
            let size = hit.and_then(|h| sizes_of(&h.rows, target));
            match (hit, &size) {
                (Some(h), Some(s)) => {
                    sizes.push((chart.brand, s.clone()));
                    if let Some(note) = &h.between {
                        between.push((chart.brand, note.clone()));
                    }
                    beyond |= h.beyond;
                    first.get_or_insert((h.rows.clone(), chart.brand));
                }
                // The chart has the systems but not this size: the other chart answers alone, named.
                _ if !matches!(f, Found::Lacks) && chart.rows.iter().any(|r| !target.of(r).is_empty()) => named = true,
                _ => {}
            }
        }
        if let Some((rows, brand)) = first {
            out.push(Part { age: a, sizes, named, rows, brand, between, beyond });
        }
    }
    out
}

impl Part {
    /// `8.5 (Nike), 9 (Clarks)`, `8` (the charts agree), `15.5 (Clarks)` (only Clarks has it).
    fn merged(&self) -> String {
        match &self.sizes[..] {
            [(_, a), (_, b)] if a == b => a.clone(),
            [(_, a)] if !self.named => a.clone(),
            sizes => sizes.iter().map(|(brand, s)| format!("{s} ({brand})")).collect::<Vec<_>>().join(", "),
        }
    }

    /// Whether the result names a chart.
    fn split(&self) -> bool {
        self.merged().contains(" (")
    }

    /// The first chart's size alone, and its chart when only Clarks's has the size: the people's
    /// line and the shoes line beside a currency.
    fn first(&self) -> (&str, Option<&'static str>) {
        let (brand, s) = &self.sizes[0];
        (s, (*brand != "Nike" && self.named).then_some(*brand))
    }
}

/// The range of a size in the charts, for an error: `EU 38–51`.
fn range(people: &[Person], system: System) -> String {
    let rows: Vec<&ShoeRow> = people.iter().flat_map(|&p| charts(p)).flat_map(|c| c.rows.iter()).filter(|r| !system.of(r).is_empty()).collect();
    let eu = |r: &&ShoeRow| numeric(r.eu).unwrap_or(0.0);
    let lo = rows.iter().min_by(|a, b| eu(a).total_cmp(&eu(b)));
    let hi = rows.iter().max_by(|a, b| eu(a).total_cmp(&eu(b)));
    match (lo, hi) {
        (Some(lo), Some(hi)) => format!("{} {}–{}", system.name(), system.of(lo), system.of(hi)),
        _ => String::new(),
    }
}

/// The region's shoe system (§4.2): the country of the device's time zone.
pub fn region() -> System {
    let country = crate::time::device_zone().and_then(crate::time::tz::zone_country).map(|c| c.code);
    match country {
        Some("US" | "LR" | "MM") => System::Us,
        Some("GB" | "IE") => System::Uk,
        Some("JP") => System::Jp,
        Some("CN") => System::Cn,
        _ => System::Eu,
    }
}

/// A size request, if `input` is one: a size word (an item, a body measure, a person, `size`;
/// `kid`, `top`, `foot` only beside another), and the size. `None` leaves the request to the others.
/// A request that means several things answers each reading, the first that answers as the result
/// and the others as lines (SPEC_CLOTHES §3.4): a person word alone is clothes then shoes, `size`
/// shoes then clothes.
pub fn request(input: &str) -> Option<Result<Answer, Error>> {
    let tokens = tokens(input)?;
    let words: Vec<(Word, bool)> =
        tokens.iter().filter_map(|t| if let Token::Word(w, weak) = t { Some((*w, *weak)) } else { None }).filter(|(w, _)| *w != Word::Filler).collect();
    if words.is_empty() || words.iter().all(|(_, weak)| *weak) && words.len() < 2 {
        return None;
    }
    let numbers: Vec<(usize, &Number)> = tokens.iter().enumerate().filter_map(|(i, t)| if let Token::Number(n) = t { Some((i, n)) } else { None }).collect();
    let (mut items, mut measures, mut person, mut age, mut size) = (Vec::new(), Vec::new(), None, None, false);
    for (w, _) in &words {
        match *w {
            Word::Item(i) => items.push(i),
            Word::Body(i) => measures.push(i),
            Word::Person(p) if person.is_some_and(|q| q != p) => return Some(Err(Error::Usage("one person at a time: men, women or kids".into()))),
            Word::Person(p) => person = Some(p),
            Word::Age(a) => age = Some(a),
            Word::Size => size = true,
            Word::System(_) | Word::Filler => {}
        }
    }
    if age.is_some() {
        match person {
            Some(Person::Men | Person::Women) => return Some(Err(Error::Usage("toddler and youth sizes are kids' sizes".into()))),
            _ => person = Some(Person::Kids),
        }
    }
    let coming = items.iter().any(|&i| self::item(i).1) || measures.iter().any(|&i| body(i).2);
    let shoes = items.iter().any(|&i| self::item(i).0 == "shoes") || measures.iter().any(|&i| body(i).1 == "shoes");
    let clothes = items.iter().any(|&i| !self::item(i).1 && self::item(i).0 != "shoes") || measures.iter().any(|&i| !body(i).2 && body(i).1 != "shoes");
    let labels = tokens.iter().any(|t| matches!(t, Token::Number(_) | Token::Letter(_) | Token::Range(..) | Token::Cn(..) | Token::Waist(..)));
    if coming {
        return labels.then(|| Err(Error::Answer(COMING.into())));
    }
    let clothes_answer = || match person {
        Some(Person::Kids) => Err(Error::Answer("kids' clothes: not yet".into())),
        _ => clothes::answer(&tokens, &items, &measures, person),
    };
    let shoe_answer = || shoe(&tokens, &numbers, measures.iter().any(|&i| body(i).1 == "shoes"), person, age);
    Some(match (shoes, clothes) {
        (true, true) => Err(Error::Usage("shoes or clothes, one at a time".into())),
        (true, false) => shoe_answer(),
        (false, true) => clothes_answer(),
        // A person, an age or `size` alone, and a size: every reading.
        (false, false) if !labels => return None,
        (false, false) if age.is_some() => shoe_answer(),
        (false, false) if size => by_priority(vec![("shoes", shoe_answer()), ("clothes", clothes_answer())]),
        (false, false) => by_priority(vec![("clothes", clothes_answer()), ("shoes", shoe_answer())]),
    })
}

/// The readings of a request, by priority: the first that answers is the answer, the others that
/// answer its lines (`also`); when none answers, the first one's error.
fn by_priority(readings: Vec<(&str, Result<Answer, Error>)>) -> Result<Answer, Error> {
    let mut main: Option<Answer> = None;
    let mut first_error = None;
    for (kind, reading) in readings {
        match (reading, &mut main) {
            (Ok(a), None) => main = Some(a),
            (Ok(a), Some(_)) if a.line.is_empty() => {}
            (Ok(a), Some(m)) => m.also.push(crate::Extra { kind: kind.to_string(), from: a.from.clone(), results: a.line }),
            (Err(e), _) => {
                first_error.get_or_insert(e);
            }
        }
    }
    main.ok_or_else(|| first_error.unwrap_or_else(|| Error::Incomplete("now the size: 42 eu, M, 9 us…".into())))
}

/// A shoe size request (`42 eu men shoes us`, `27 cm men foot`).
fn shoe(tokens: &[Token], numbers: &[(usize, &Number)], foot: bool, person: Option<Person>, age: Option<Age>) -> Result<Answer, Error> {
    if tokens.iter().any(|t| matches!(t, Token::Letter(_) | Token::Range(..) | Token::Cn(..) | Token::Waist(..) | Token::Inseam(_))) {
        return Err(Error::Usage("shoe sizes are numbers: 42 eu, 9.5 us, 4C".into()));
    }
    let (at, n) = match numbers {
        [] => return Err(Error::Incomplete("now the size: 42 eu, 9 us…".into())),
        [one] => *one,
        _ => return Err(Error::Usage("one size at a time".into())),
    };
    let next = tokens.get(at + 1);
    let unit = match next {
        Some(Token::Cm) => Some(1.0),
        Some(Token::In) if foot => Some(2.54),
        _ => None,
    };
    let age = age.or(n.age);
    let mut notes: Vec<&'static str> = Vec::new();
    let mut systems: Vec<(usize, System, Option<&'static str>)> = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        if let Token::System(s, typed, _) = t {
            match s.for_shoes() {
                Some((s, note)) => systems.push((i, s, note)),
                None => return Err(Error::Usage(format!("`{typed}` sizes are clothes sizes: shoes are eu, uk, us, jp, cn"))),
            }
        }
    }
    let (from, shown_from, source_at) = match (foot, unit) {
        (true, Some(k)) => {
            let unit = if k == 1.0 { "cm" } else { "in" };
            (From::Foot(n.value * k), format!("{} {unit} foot", n.text), None)
        }
        (true, None) => return Err(Error::Usage("the foot's length, in cm or in: `27 cm men foot`".into())),
        (false, Some(_)) => return Err(Error::Usage("cm and in measure a foot: `27 cm men foot`".into())),
        (false, None) => {
            // Its system: next to it, before it (`US 9`) or after (`42 eu`); else the only one it can be.
            let beside = systems.iter().find(|(i, ..)| *i + 1 == at).or_else(|| systems.iter().find(|(i, ..)| *i == at + 1));
            let source = match (beside, n.age) {
                (Some(&(i, s, note)), None) => {
                    notes.extend(note);
                    (s, Some(i))
                }
                (Some(&(i, System::Us, _)), Some(_)) => (System::Us, Some(i)),
                (Some(_), Some(_)) => return Err(Error::Usage(format!("`{}` is a US size", n.text))),
                (None, Some(_)) => (System::Us, None),
                (None, None) => match only_system(person, n.value) {
                    Some(s) => (s, None),
                    None => return Err(Error::Incomplete("which system: eu, uk, us…".into())),
                },
            };
            let s = if source.0 == System::Cn { System::Eu } else { source.0 };
            (From::Size(s, n.value), format!("{} {}", source.0.name(), n.text), source.1)
        }
    };
    let targets: Vec<&(usize, System, Option<&'static str>)> = systems.iter().filter(|(i, ..)| Some(*i) != source_at).collect();
    let target = match targets[..] {
        [] => None,
        [&(_, s, note)] => {
            notes.extend(note);
            Some(s)
        }
        _ => return Err(Error::Usage("one system to convert to".into())),
    };
    let mut notes: Vec<String> = notes.into_iter().map(String::from).collect();
    notes.dedup();
    let answer = match (person, target) {
        (Some(p), target) => one_person(p, from, age, target.unwrap_or_else(region), &shown_from, notes),
        (None, Some(t)) => people(from, age, t, &shown_from, notes),
        (None, None) => table(from, age, &shown_from, notes),
    };
    answer.map(|a| Answer { from: shown_from, ..a })
}

/// The one system a size without one can be in (`42`: EU only), among the people's charts.
fn only_system(person: Option<Person>, value: f64) -> Option<System> {
    let people: Vec<Person> = person.map_or(PEOPLE.to_vec(), |p| vec![p]);
    let mut found: Vec<System> = Vec::new();
    for s in [System::Us, System::Uk, System::Eu, System::Jp] {
        let has = people.iter().flat_map(|&p| charts(p)).any(|c| matches!(lookup(c, From::Size(s, value), None), Found::Hits(_)));
        if has {
            found.push(s);
        }
    }
    match found[..] {
        [one] => Some(one),
        _ => None,
    }
}

/// The error of a size no chart has: `no EU 60 in men's shoes (EU 38–51)`.
fn not_found(shown_from: &str, people: &[Person], from: From) -> Error {
    let whose = match people {
        [p] => format!("{} shoes", possessive(*p)),
        _ => "shoes".to_string(),
    };
    let range = match from {
        From::Size(s, _) => format!(" ({})", range(people, s)),
        From::Foot(_) => String::new(),
    };
    Error::Answer(format!("no {shown_from} in {whose}{range}"))
}

/// The "between" notes (§4.4): with two charts, one when both are between the same rows, else each
/// named; with one, unnamed.
fn between_notes(parts: &[Part]) -> Vec<String> {
    let mut notes: Vec<String> = Vec::new();
    for p in parts {
        let found: Vec<String> = match &p.between[..] {
            [] => Vec::new(),
            [(_, a), (_, b)] if a == b => vec![a.clone()],
            [(_, note)] if p.sizes.len() == 1 => vec![note.clone()],
            all => all.iter().map(|(brand, note)| format!("{note} ({brand})")).collect(),
        };
        for n in found {
            if !notes.contains(&n) {
                notes.push(n);
            }
        }
    }
    notes
}

/// One person's answer: the size in the target (both charts where they differ), the other systems,
/// the notes.
fn one_person(person: Person, from: From, age: Option<Age>, target: System, shown_from: &str, system_notes: Vec<String>) -> Result<Answer, Error> {
    let parts = parts(person, from, age, target);
    if parts.is_empty() {
        return Err(not_found(shown_from, &[person], from));
    }
    let text = match &parts[..] {
        [one] => target.show(&one.merged()),
        many => many.iter().map(|p| format!("{} {}", target.show(&p.merged()), person_word(person, p.age))).collect::<Vec<_>>().join(", "),
    };
    let details = match &parts[..] {
        [one] => {
            let others: Vec<String> = System::ALL.iter().filter(|&&s| s != target).filter_map(|&s| sizes_of(&one.rows, s).map(|v| s.show(&v))).collect();
            (!others.is_empty()).then(|| others.join(" · "))
        }
        _ => None,
    };
    let split = parts.iter().any(Part::split);
    let mut notes = vec![match (split, details.is_some()) {
        (true, true) => format!("{} shoes (details: {})", possessive(person), parts[0].brand),
        _ => format!("{} shoes", possessive(person)),
    }];
    notes.extend(between_notes(&parts));
    if parts.iter().any(|p| p.beyond) {
        notes.push("beyond the chart".to_string());
    }
    notes.extend(system_notes);
    notes.push(BRAND_NOTE.to_string());
    // As the shoes line beside a currency: the first chart's size, Clarks's named when only it has it.
    let line = parts
        .iter()
        .map(|p| {
            let (size, brand) = p.first();
            let brand = brand.map(|b| format!(" ({b})")).unwrap_or_default();
            format!("{} {}{brand}", target.show(size), person_word(person, p.age.filter(|_| parts.len() > 1)))
        })
        .collect();
    Ok(Answer { text, details, notes, line, ..Answer::default() })
}

/// Every person's size in the target, the first chart's, Clarks's named when only it has the size:
/// `US 8.5 men`, `US 10 women`, `US 15.5 men (Clarks)`.
fn everyone(from: From, age: Option<Age>, target: System) -> Vec<String> {
    people_sizes(from, age, target).into_iter().map(|(who, size, brand)| format!("{size} {who}{brand}")).collect()
}

/// Every person's size in the target, as (person, size, chart): (`men`, `US 8.5`, ``), (`men`,
/// `US 15.5`, ` (Clarks)`).
fn people_sizes(from: From, age: Option<Age>, target: System) -> Vec<(&'static str, String, String)> {
    let mut all = Vec::new();
    for p in PEOPLE {
        let parts = parts(p, from, if p == Person::Kids { age } else { None }, target);
        // Toddler and youth only when the size is both; else kids.
        let both = parts.len() > 1;
        for part in &parts {
            let (size, brand) = part.first();
            let brand = brand.map(|b| format!(" ({b})")).unwrap_or_default();
            all.push((person_word(p, part.age.filter(|_| both)), target.show(size), brand));
        }
    }
    all
}

/// The size readings of a request that has no size word (`42 eu us`, `M it`, §3.3 of both specs):
/// the shoes line, then the clothes line; none when the request is no size (an amount in no chart,
/// a code that is no size system, a word that is no size's).
pub fn beside(input: &str) -> Vec<crate::Extra> {
    let Some(tokens) = tokens(input) else { return Vec::new() };
    [shoes_beside(&tokens), clothes::beside(&tokens)].into_iter().flatten().collect()
}

/// What comes next after a size with nothing to convert it to (`42 de` where DE sizes are the
/// region's, a letter alone, `XXL`): every prefix of a size request is a hint or an answer
/// (SPEC_CLOTHES §3.7). `None`: no size typed that way.
pub fn hint(input: &str) -> Option<String> {
    let tokens = tokens(input)?;
    let label = |t: &Token| matches!(t, Token::Number(_) | Token::Letter(_) | Token::Range(..) | Token::Cn(..) | Token::Waist(..));
    let code = |t: &Token| matches!(t, Token::System(_, _, Written::Code | Written::Attached));
    let fits = tokens.iter().all(|t| label(t) || code(t) || matches!(t, Token::Inseam(_) | Token::Connector));
    let (labels, systems) = (tokens.iter().filter(|t| label(t)).count(), tokens.iter().filter(|t| code(t)).count());
    // A plain number alone is a calculation.
    let number = matches!(tokens[..], [Token::Number(Number { age: None, .. })]);
    (fits && labels == 1 && systems <= 1 && !number).then(|| "now the system to convert to: eu, uk, us, it, fr, de, jp, cn, int".to_string())
}

/// The shoe sizes beside a currency that could also be one (`42 eu us`, SPEC_SIZES §4.1): every
/// person's, no notes; in the region's system without a target, none when that is the typed one.
fn shoes_beside(tokens: &[Token]) -> Option<crate::Extra> {
    let mut amount = None;
    let mut systems: Vec<(System, String)> = Vec::new();
    for t in tokens {
        match t {
            Token::Number(n) if n.age.is_none() && amount.is_none() => amount = Some(n),
            Token::System(s, typed, Written::Code | Written::Attached) => systems.push((s.for_shoes()?.0, typed.clone())),
            Token::Connector | Token::In => {}
            _ => return None,
        }
    }
    let (amount, (source, typed)) = (amount?, systems.first()?.clone());
    let source = if source == System::Cn { System::Eu } else { source };
    let target = match systems.get(1) {
        Some(&(t, _)) => t,
        None => region(),
    };
    if target == source || target == System::Cn && source == System::Eu {
        return None;
    }
    let results = everyone(From::Size(source, amount.value), None, target);
    (!results.is_empty()).then(|| crate::Extra { kind: "shoes".into(), from: format!("{} {typed}", amount.text), results })
}

/// Every person's size in the target, the first chart's: `US 8.5 men, US 10 women`.
fn people(from: From, age: Option<Age>, target: System, shown_from: &str, mut notes: Vec<String>) -> Result<Answer, Error> {
    let sizes = people_sizes(from, age, target);
    if sizes.is_empty() {
        return Err(not_found(shown_from, &PEOPLE, from));
    }
    notes.push(BRAND_NOTE.to_string());
    let all: Vec<String> = sizes.iter().map(|(who, size, brand)| format!("{size} {who}{brand}")).collect();
    let copy = sizes.iter().map(|(who, size, brand)| format!("{} {size}{brand}", title(who))).collect::<Vec<_>>().join("\n");
    let rest: Vec<String> = all[1..].to_vec();
    Ok(Answer {
        text: all.join(", "),
        result: Some(all[0].clone()),
        extra: if rest.is_empty() { Vec::new() } else { vec![("shoes".to_string(), rest)] },
        notes,
        copy: Some(copy),
        line: all,
        ..Answer::default()
    })
}

/// Every person's size in every system, the first chart's, as a table lined up on the decimal point.
fn table(from: From, age: Option<Age>, shown_from: &str, mut notes: Vec<String>) -> Result<Answer, Error> {
    let mut lines: Vec<(&'static str, Vec<String>)> = Vec::new();
    for p in PEOPLE {
        // Any system: the row is the same; the sizes are read from it.
        let parts = parts(p, from, if p == Person::Kids { age } else { None }, System::Eu);
        let both = parts.len() > 1;
        for part in &parts {
            lines.push((person_word(p, part.age.filter(|_| both)), System::ALL.iter().map(|&s| sizes_of(&part.rows, s).unwrap_or_default()).collect()));
        }
    }
    if lines.is_empty() {
        return Err(not_found(shown_from, &PEOPLE, from));
    }
    notes.push(BRAND_NOTE.to_string());
    let extra: Vec<(String, Vec<String>)> = lines
        .iter()
        .map(|(who, sizes)| {
            let results = System::ALL.iter().zip(sizes).filter(|(_, v)| !v.is_empty()).map(|(s, v)| format!("{} {v}", s.name())).collect();
            (format!("shoes {who}"), results)
        })
        .collect();
    let copy = lines.iter().zip(&extra).map(|((who, _), (_, results))| format!("{} {}", title(who), results.join(" · "))).collect::<Vec<_>>().join("\n");
    Ok(Answer { text: aligned(&lines), extra, notes, copy: Some(copy), line: everyone(from, age, region()), ..Answer::default() })
}

/// The table: a column per system, the sizes lined up on their decimal point.
fn aligned(lines: &[(&str, Vec<String>)]) -> String {
    let split = |v: &str| v.find('.').map_or((v.to_string(), String::new()), |i| (v[..i].to_string(), v[i..].to_string()));
    let label = lines.iter().map(|(who, _)| who.chars().count()).max().unwrap_or(0);
    let mut columns: Vec<Vec<String>> = Vec::new();
    for (c, system) in System::ALL.iter().enumerate() {
        let parts: Vec<(String, String)> = lines.iter().map(|(_, sizes)| split(&sizes[c])).collect();
        let whole = parts.iter().map(|(w, _)| w.chars().count()).max().unwrap_or(0);
        let tail = parts.iter().map(|(_, t)| t.chars().count()).max().unwrap_or(0);
        let width = (whole + tail).max(system.name().len());
        let mut column = vec![format!("{:>width$}", system.name())];
        column.extend(parts.iter().map(|(w, t)| format!("{:>width$}", format!("{w:>whole$}{t:<tail$}"))));
        columns.push(column);
    }
    let mut out = Vec::new();
    for row in 0..=lines.len() {
        let who = if row == 0 { "" } else { lines[row - 1].0 };
        let cells: Vec<&str> = columns.iter().map(|c| c[row].as_str()).collect();
        out.push(format!("{who:<label$}  {}", cells.join("   ")).trim_end().to_string());
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_as_typed() {
        let n = |w: &str| number(w).map(|n| (n.value, n.age));
        assert_eq!(n("42"), Some((42.0, None)));
        assert_eq!(n("9.5"), Some((9.5, None)));
        assert_eq!(n("9,5"), Some((9.5, None)));
        assert_eq!(n("9½"), Some((9.5, None)));
        assert_eq!(n("12.5C"), Some((12.5, Some(Age::Toddler))));
        assert_eq!(n("2y"), Some((2.0, Some(Age::Youth))));
        assert!((n("42⅔").unwrap().0 - 42.667).abs() < 0.001);
        for not in ["eu", "1e5", "-3", "C", ".5", "0", "½"] {
            assert_eq!(n(not), None, "{not}");
        }
    }

    #[test]
    fn not_a_size_request() {
        for input in ["42 eu us", "42 in", "10 km mi", "42 kid us", "42 top eu", "27 cm foot", "42 eu nike shoes us", "now", "shoes in Rome"] {
            assert!(request(input).is_none(), "{input}");
        }
        assert_eq!(request("34 bra eu").unwrap(), Err(Error::Answer(COMING.into())));
        assert_eq!(request("7 hat us").unwrap(), Err(Error::Answer(COMING.into())));
        assert_eq!(request("shoes").unwrap(), Err(Error::Incomplete("now the size: 42 eu, 9 us…".into())));
        assert_eq!(request("clothes").unwrap(), Err(Error::Incomplete("now the size: 44 it, M, 8 us…".into())));
    }

    #[test]
    fn the_shoes_line_beside_a_currency() {
        let line = |input: &str| beside(input).into_iter().find(|e| e.kind == "shoes").map(|e| e.line());
        assert_eq!(line("42 eu us").as_deref(), Some("shoes: 42 eu = US 8.5 men, US 10 women"));
        assert_eq!(line("36 eu us").as_deref(), Some("shoes: 36 eu = US 5.5 women, US 4Y kids"));
        assert_eq!(line("50 eu us").as_deref(), Some("shoes: 50 eu = US 15.5 men (Clarks)"));
        assert_eq!(line("9 us uk").as_deref(), Some("shoes: 9 us = UK 8 men, UK 6.5 women, UK 8.5 kids"));
        for none in ["100 eu us", "42 eu jpy", "42 uk us", "12+3 eu us", "42 eu men us"] {
            assert_eq!(line(none), None, "{none}");
        }
        // No target: the region's system; none when it is the typed one.
        let us = crate::machine::Mock::us().install();
        assert_eq!(line("42 eu").as_deref(), Some("shoes: 42 eu = US 8.5 men, US 10 women"));
        drop(us);
        let _italy = crate::machine::Mock::italy().install();
        assert_eq!(line("42 eu"), None);
        let _dublin = crate::machine::Mock::italy().with_zone("Europe/Dublin").install();
        assert_eq!(line("42 eu").as_deref(), Some("shoes: 42 eu = UK 7.5 men, UK 7.5 women"));
    }

    #[test]
    fn a_sizes_key_writes_its_word() {
        // A system or a letter: its code alone, the answer brings everything.
        assert_eq!(write_key("", "eu"), "eu ");
        assert_eq!(write_key("42", "eu"), "42 eu ");
        assert_eq!(write_key("42 eu", "us"), "42 eu us ");
        assert_eq!(write_key("42 eu men shoes", "us"), "42 eu men shoes us ");
        assert_eq!(write_key("clothes", "M"), "clothes M ");
        assert_eq!(write_key("27 cm men foot", "us"), "27 cm men foot us ");
        // A group: in place of the item word there, else after.
        assert_eq!(write_key("", "shoes"), "shoes ");
        assert_eq!(write_key("42 eu", "shoes"), "42 eu shoes ");
        assert_eq!(write_key("shoes 42", "shoes"), "shoes 42 ");
        assert_eq!(write_key("shoes 42 eu", "clothes"), "clothes 42 eu ");
        assert_eq!(write_key("scarpe 42", "clothes"), "clothes 42 ");
        assert_eq!(write_key("44 it vestito", "shoes"), "44 it shoes ");
        // A person in place of the one there.
        assert_eq!(write_key("42", "men"), "42 men ");
        assert_eq!(write_key("shoes 42 men", "women"), "shoes 42 women ");
        assert_eq!(write_key("scarpe donna 38", "kids"), "scarpe 38 kids ");
        assert_eq!(write_key("42 eu trẻ em shoes", "men"), "42 eu shoes men ");
    }

    #[test]
    fn the_table_on_the_decimal_point() {
        let lines =
            [("men", ["8.5", "7.5", "42", "26.5", "42"].map(String::from).to_vec()), ("women", ["10", "7.5", "42", "27", "42"].map(String::from).to_vec())];
        assert_eq!(aligned(&lines), "         US    UK   EU     JP   CN\nmen     8.5   7.5   42   26.5   42\nwomen  10     7.5   42   27     42");
    }

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
        // Clothing is answered; bras and hats are recognised, not answered yet.
        let Some((Word::Item(dress), false)) = word("vestito") else { panic!() };
        assert_eq!(item(dress), ("dress", false));
        let Some((Word::Item(bra), false)) = word("reggiseno") else { panic!() };
        assert_eq!(item(bra), ("bra", true));
        // The size, system and filler words.
        assert_eq!(word("taglia"), Some((Word::Size, false)));
        assert_eq!(word("Größe"), Some((Word::Size, false)));
        assert_eq!(word("italiana"), Some((Word::System(System::It), false)));
        assert_eq!(word("américain"), Some((Word::System(System::Us), false)));
        assert_eq!(word("what's"), Some((Word::Filler, false)));
        // A currency or a unit too: only beside another size word.
        assert_eq!(word("kid"), Some((Word::Person(Person::Kids), true)));
        assert!(word("top").unwrap().1 && word("foot").unwrap().1);
        assert_eq!(word("nike"), None);
    }

    #[test]
    fn clothes_labels() {
        let t = label_token;
        assert_eq!(t("M"), Some(Token::Letter("M".into())));
        assert_eq!(t("2xl"), Some(Token::Letter("XXL".into())));
        assert_eq!(t("3L"), Some(Token::Letter("3L".into())));
        assert!(matches!(t("W32"), Some(Token::Waist(w, None, _)) if w == 32.0));
        assert!(matches!(t("32w"), Some(Token::Waist(w, None, _)) if w == 32.0));
        assert!(matches!(t("32x34"), Some(Token::Waist(w, Some(l), _)) if w == 32.0 && l == 34.0));
        assert!(matches!(t("L34"), Some(Token::Inseam(l)) if l == 34.0));
        assert!(matches!(t("160/84A"), Some(Token::Cn(b, _)) if b == 84.0));
        assert!(matches!(t("165/92"), Some(Token::Cn(b, _)) if b == 92.0));
        assert!(matches!(t("8-10"), Some(Token::Range(Label::Number(_), Label::Number(_), _))));
        assert!(matches!(t("S/M"), Some(Token::Range(Label::Letter(_), Label::Letter(_), _))));
        for not in ["12/04/2026", "-3", "S-10", "km", "W", "x34", "MM"] {
            assert_eq!(t(not), None, "{not}");
        }
    }

    #[test]
    fn attached_systems_countries_and_fillers() {
        let systems = |input: &str| {
            tokens(input).unwrap().into_iter().filter_map(|t| if let Token::System(s, _, w) = t { Some((s, w == Written::Name)) } else { None }).collect::<Vec<_>>()
        };
        assert_eq!(systems("IT44 dress US"), [(System::It, false), (System::Us, false)]);
        assert_eq!(systems("44it dress"), [(System::It, false)]);
        assert_eq!(systems("dress 44 italian to American"), [(System::It, true), (System::Us, true)]);
        assert_eq!(systems("dress 44 Italy to United States"), [(System::It, true), (System::Us, true)]);
        // `it` and `de` as words: the pronoun, the Spanish "of".
        assert_eq!(systems("what is it in us dress 44 it"), [(System::Us, false), (System::It, false)]);
        assert_eq!(systems("talla de 44 vestido us"), [(System::Us, false)]);
        assert_eq!(systems("M women top it"), [(System::It, false)]);
        // Punctuation and fillers.
        assert!(tokens("my size is 44 it, what is it in us?").is_some());
        assert!(tokens("dress 44 nike us").is_none(), "a brand is no word of a size request");
        // `taille`: waist beside a measure, else size.
        assert!(tokens("70 cm taille").unwrap().iter().any(|t| matches!(t, Token::Word(Word::Body(_), _))));
        assert!(tokens("taille 44 it").unwrap().iter().any(|t| matches!(t, Token::Word(Word::Size, _))));
    }

    #[test]
    fn words_that_are_units_or_currencies_count_only_beside_another() {
        // A word that is also a unit or a currency must not make a size request by itself: weak, or
        // a system or filler (they never do).
        let config = crate::config::Config::default();
        for (w, word, weak) in SIZE_WORDS {
            let other = crate::convert::find_unit(&config, w).is_some() || crate::currency::exact(w).is_some();
            assert!(!other || *weak || matches!(word, Word::System(_) | Word::Filler), "`{w}` is a unit or a currency too");
        }
    }

    #[test]
    fn the_four_clothes_charts() {
        let charts: Vec<(Chart, Person, usize)> = clothes_charts().iter().map(|c| (c.chart, c.person, c.rows.len())).collect();
        assert_eq!(charts, [(Chart::Women, Person::Women, 10), (Chart::Suit, Person::Men, 9), (Chart::Trousers, Person::Men, 11), (Chart::Shirt, Person::Men, 11)]);
        // Women's US 8: IT 44, EU = DE 38, a 93 cm bust; a men's suit 40: EU = IT = FR 50, 101.6 cm.
        let w = &clothes_charts()[0].rows[4];
        assert_eq!((w.us, w.uk, w.eu, w.de, w.fr, w.it, w.int, w.cn, w.bust), ("8", "12", "38", "38", "40", "44", "M", "165/92A", 93.0));
        let s = &clothes_charts()[1].rows[3];
        assert_eq!((s.us, s.uk, s.eu, s.it, s.fr, s.jp, s.int), ("40", "40", "50", "50", "50", "L", "L"));
        assert!((s.chest - 101.6).abs() < 1e-9);
        let shirt = &clothes_charts()[3].rows[4];
        assert_eq!((shirt.eu, shirt.us, shirt.it, shirt.neck), ("40", "15.75", "", 40.0));
    }
}
