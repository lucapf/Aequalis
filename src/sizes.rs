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

// ---------------------------------------------------------------- for the UIs

/// A group of the Sizes class (Measures, the Sizes menu): its name, and the word it writes.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub name: &'static str,
    pub write: &'static str,
}

/// The groups answered so far, in the data's order: Shoes (phase 1).
pub fn groups() -> Vec<Group> {
    ITEMS.iter().filter(|(_, coming)| !coming).map(|&(name, _)| Group { name: group_name(name), write: name }).collect()
}

fn group_name(item: &str) -> &'static str {
    match item {
        "shoes" => "Shoes",
        _ => "Sizes",
    }
}

/// The system keys of the Sizes class, (shown, written): EU, UK, US, IT, FR, DE, JP, CN.
pub const SYSTEM_KEYS: [(&str, &str); 8] = [("EU", "eu"), ("UK", "uk"), ("US", "us"), ("IT", "it"), ("FR", "fr"), ("DE", "de"), ("JP", "jp"), ("CN", "cn")];

/// What a size word means, for a suggestion's label: `shoes`, `foot`, `women`, `toddler`.
fn meaning(word: Word) -> &'static str {
    match word {
        Word::Item(i) => ITEMS[i].0,
        Word::Body(i) => BODY[i].0,
        Word::Person(p) => person_word(p, None),
        Word::Age(a) => person_word(Person::Kids, Some(a)),
    }
}

/// Suggestions while typing (`suggest.rs`): the size words that start with the word typed, from
/// three letters (`scar` → `scarpe - shoes`), as (written, shown); after a shoe size with no system
/// yet (`shoes 42 `), the systems. The position (in chars) the suggestions replace from.
pub fn suggestions(before: &str, typing: bool) -> Option<(usize, Vec<(String, String)>)> {
    let end = before.chars().count();
    if !typing {
        let tokens = tokens(before)?;
        let shoes =
            tokens.iter().any(|t| matches!(t, Token::Word(Word::Item(i), _) if !ITEMS[*i].1) || matches!(t, Token::Word(Word::Body(i), _) if !BODY[*i].2));
        let after_size = matches!(tokens.last(), Some(Token::Number(n)) if n.age.is_none());
        let system = tokens.iter().any(|t| matches!(t, Token::System(..)));
        if !(shoes && after_size && !system) {
            return None;
        }
        let names = ["eu - Europe", "uk - United Kingdom", "us - United States", "jp - Japan (cm)", "cn - China"];
        return Some((end, names.iter().map(|n| (n[..2].to_string(), n.to_string())).collect()));
    }
    let prefix = before.split_whitespace().last()?;
    if prefix.chars().count() < 3 || prefix.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let typed = crate::time::tz::key(prefix);
    let found: Vec<(String, String)> = SIZE_WORDS
        .iter()
        .filter(|(w, ..)| w.starts_with(typed.as_str()))
        .take(crate::suggest::MAX)
        // `shoes`, not `shoes - shoes`.
        .map(|&(w, word, _)| (w.to_string(), if meaning(word) == w { w.to_string() } else { format!("{w} - {}", meaning(word)) }))
        .collect();
    (!found.is_empty()).then(|| (end - prefix.chars().count(), found))
}

// ---------------------------------------------------------------- a size request

/// A shoe size system. Italian, French and German shoe sizes are EU sizes; Chinese ones the EU
/// number, written CN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum System {
    Us,
    Uk,
    Eu,
    Jp,
    Cn,
}

impl System {
    /// In the order the details and the table give them.
    pub const ALL: [System; 5] = [System::Us, System::Uk, System::Eu, System::Jp, System::Cn];

    pub fn name(self) -> &'static str {
        match self {
            System::Us => "US",
            System::Uk => "UK",
            System::Eu => "EU",
            System::Jp => "JP",
            System::Cn => "CN",
        }
    }

    /// A row's size in this system (empty: the chart has none); CN reads EU.
    fn of(self, row: &ShoeRow) -> &'static str {
        match self {
            System::Us => row.us,
            System::Uk => row.uk,
            System::Eu | System::Cn => row.eu,
            System::Jp => row.jp,
        }
    }

    /// `UK 7.5`, `JP 26.5 cm`.
    fn show(self, size: &str) -> String {
        match self {
            System::Jp => format!("JP {size} cm"),
            s => format!("{} {size}", s.name()),
        }
    }
}

/// A system as typed, and the note an alias brings (`it`: Italian shoe sizes are EU sizes).
fn system(word: &str) -> Option<(System, Option<&'static str>)> {
    Some(match word.to_ascii_lowercase().as_str() {
        "us" => (System::Us, None),
        "uk" => (System::Uk, None),
        "eu" => (System::Eu, None),
        "jp" => (System::Jp, None),
        "cn" => (System::Cn, Some("Chinese shoe sizes are EU sizes")),
        "it" => (System::Eu, Some("Italian shoe sizes are EU sizes")),
        "fr" => (System::Eu, Some("French shoe sizes are EU sizes")),
        "de" => (System::Eu, Some("German shoe sizes are EU sizes")),
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
    /// Read, but no size to give (`no EU 60 in men's shoes`, `only shoes for now`): exit code 1.
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
}

const COMING: &str = "only shoes for now";
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

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Word(Word, bool),
    Number(Number),
    /// A system, its note, as typed (`eu`).
    System(System, Option<&'static str>, String),
    Cm,
    In,
    Connector,
}

/// The request's words, a size word of up to three words first (`trẻ em`, `ao so mi`); `None`
/// when a word is none of a size request's (a brand, a unit, a place).
fn tokens(input: &str) -> Option<Vec<Token>> {
    let words: Vec<&str> = input.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    'next: while i < words.len() {
        for n in (1..=3.min(words.len() - i)).rev() {
            let phrase = words[i..i + n].join(" ");
            // `men's`, `women’s`.
            let plain = phrase.strip_suffix("'s").or_else(|| phrase.strip_suffix("’s")).unwrap_or(&phrase);
            if let Some((w, weak)) = word(&phrase).or_else(|| word(plain)) {
                out.push(Token::Word(w, weak));
                i += n;
                continue 'next;
            }
        }
        let w = words[i];
        out.push(if let Some(n) = number(w) {
            Token::Number(n)
        } else if let Some((s, note)) = system(w) {
            Token::System(s, note, w.to_string())
        } else if w.eq_ignore_ascii_case("cm") {
            Token::Cm
        } else if w.eq_ignore_ascii_case("in") {
            Token::In
        } else if ["to", "->", "→"].iter().any(|c| w.eq_ignore_ascii_case(c)) {
            Token::Connector
        } else {
            return None;
        });
        i += 1;
    }
    Some(out)
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

/// A size request, if `input` is one: a size word (an item, a body measure, a person; `kid`,
/// `top`, `foot` only beside another), and the size. `None` leaves the request to the others.
pub fn request(input: &str) -> Option<Result<Answer, Error>> {
    let tokens = tokens(input)?;
    let words: Vec<(Word, bool)> = tokens.iter().filter_map(|t| if let Token::Word(w, weak) = t { Some((*w, *weak)) } else { None }).collect();
    if words.is_empty() || words.iter().all(|(_, weak)| *weak) && words.len() < 2 {
        return None;
    }
    let numbers: Vec<(usize, &Number)> = tokens.iter().enumerate().filter_map(|(i, t)| if let Token::Number(n) = t { Some((i, n)) } else { None }).collect();
    let (mut item, mut measure, mut person, mut age) = (None, None, None, None);
    for (w, _) in &words {
        match *w {
            Word::Item(i) => item = Some(i),
            Word::Body(i) => measure = Some(i),
            Word::Person(p) if person.is_some_and(|q| q != p) => return Some(Err(Error::Usage("one person at a time: men, women or kids".into()))),
            Word::Person(p) => person = Some(p),
            Word::Age(a) => age = Some(a),
        }
    }
    if age.is_some() {
        match person {
            Some(Person::Men | Person::Women) => return Some(Err(Error::Usage("toddler and youth sizes are kids' sizes".into()))),
            _ => person = Some(Person::Kids),
        }
    }
    // Not shoes (yet): clothing, the other measures, a person word alone (clothing).
    let shoes = item.is_some_and(|i| !self::item(i).1) || measure.is_some_and(|i| !body(i).2);
    let coming = item.is_some_and(|i| self::item(i).1) || measure.is_some_and(|i| body(i).2);
    if coming || !shoes {
        return match (coming, numbers.is_empty()) {
            (false, true) => None,
            _ => Some(Err(Error::Answer(COMING.into()))),
        };
    }
    Some(shoe(&tokens, &numbers, measure.is_some(), person, age))
}

/// A shoe size request (`42 eu men shoes us`, `27 cm men foot`).
fn shoe(tokens: &[Token], numbers: &[(usize, &Number)], foot: bool, person: Option<Person>, age: Option<Age>) -> Result<Answer, Error> {
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
    let systems: Vec<(usize, System, Option<&'static str>)> =
        tokens.iter().enumerate().filter_map(|(i, t)| if let Token::System(s, note, _) = t { Some((i, *s, *note)) } else { None }).collect();
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
    Ok(Answer { text, details, notes, ..Answer::default() })
}

/// Every person's size in the target, the first chart's, Clarks's named when only it has the size:
/// `US 8.5 men`, `US 10 women`, `US 15.5 men (Clarks)`.
fn everyone(from: From, age: Option<Age>, target: System) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    for p in PEOPLE {
        let parts = parts(p, from, if p == Person::Kids { age } else { None }, target);
        // Toddler and youth only when the size is both; else kids.
        let both = parts.len() > 1;
        for part in &parts {
            let (size, brand) = part.first();
            let brand = brand.map(|b| format!(" ({b})")).unwrap_or_default();
            all.push(format!("{} {}{brand}", target.show(size), person_word(p, part.age.filter(|_| both))));
        }
    }
    all
}

/// The shoe sizes beside a currency that could also be one (`42 eu us`, §4.1): every person's, no
/// notes; in the region's system without a target, none when that is the typed one. `None` when
/// the request is no size: an amount in no chart, a code that is no size system.
pub fn beside(input: &str) -> Option<crate::Extra> {
    let mut amount = None;
    let mut systems: Vec<(System, String)> = Vec::new();
    for t in tokens(input)? {
        match t {
            Token::Number(n) if n.age.is_none() && amount.is_none() => amount = Some(n),
            Token::System(s, _, typed) => systems.push((s, typed)),
            Token::Connector | Token::In => {}
            _ => return None,
        }
    }
    let (amount, (source, typed)) = (amount?, systems.first()?.clone());
    let source = if source == System::Cn { System::Eu } else { source };
    let target = match systems.get(1) {
        Some(&(t, _)) => t,
        None if region() == source => return None,
        None => region(),
    };
    let results = everyone(From::Size(source, amount.value), None, target);
    (!results.is_empty()).then(|| crate::Extra { kind: "shoes".into(), from: format!("{} {typed}", amount.text), results })
}

/// Every person's size in the target, the first chart's: `US 8.5 men, US 10 women`.
fn people(from: From, age: Option<Age>, target: System, shown_from: &str, mut notes: Vec<String>) -> Result<Answer, Error> {
    let all = everyone(from, age, target);
    if all.is_empty() {
        return Err(not_found(shown_from, &PEOPLE, from));
    }
    notes.push(BRAND_NOTE.to_string());
    let rest: Vec<String> = all[1..].to_vec();
    Ok(Answer {
        text: all.join(", "),
        result: Some(all[0].clone()),
        extra: if rest.is_empty() { Vec::new() } else { vec![("shoes".to_string(), rest)] },
        notes,
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
    let extra = lines
        .iter()
        .map(|(who, sizes)| {
            let results = System::ALL.iter().zip(sizes).filter(|(_, v)| !v.is_empty()).map(|(s, v)| format!("{} {v}", s.name())).collect();
            (format!("shoes {who}"), results)
        })
        .collect();
    Ok(Answer { text: aligned(&lines), extra, notes, ..Answer::default() })
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
        assert_eq!(request("12 us woman eu").unwrap(), Err(Error::Answer(COMING.into())));
        assert_eq!(request("44 it dress us").unwrap(), Err(Error::Answer(COMING.into())));
        assert_eq!(request("shoes").unwrap(), Err(Error::Incomplete("now the size: 42 eu, 9 us…".into())));
    }

    #[test]
    fn the_shoes_line_beside_a_currency() {
        let line = |input: &str| beside(input).map(|e| e.line());
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
        // Clothing is recognised, not answered yet.
        let Some((Word::Item(dress), false)) = word("vestito") else { panic!() };
        assert_eq!(item(dress), ("dress", true));
        // A currency or a unit too: only beside another size word.
        assert_eq!(word("kid"), Some((Word::Person(Person::Kids), true)));
        assert!(word("top").unwrap().1 && word("foot").unwrap().1);
        assert_eq!(word("nike"), None);
    }
}
