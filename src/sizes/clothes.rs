//! Clothing sizes (`docs/SPEC_CLOTHES.md`): the four charts, in priority order (women's, men's
//! suit, men's trousers, men's shirt); a clothes request read from the request's tokens; every chart
//! that answers it, the first as the result.

use super::{Answer, Error, Label, Person, System, Token, BRAND_NOTE};

/// A clothes chart, in priority order: the technical ones (waist, collar) last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chart {
    /// Dresses, tops and trousers: one numeric chart.
    Women,
    /// Men's suits and jackets, by chest; their letters are the men's letters.
    Suit,
    /// Men's trousers, by waist.
    Trousers,
    /// Men's shirts, by collar.
    Shirt,
}

/// A row of a clothes chart: one size in every system (empty: the chart has none; the men's
/// continental number in EU, IT, FR and DE), and the body measures it stands for in cm (0: none).
#[derive(Debug, PartialEq)]
pub struct ClothesRow {
    pub us: &'static str,
    pub uk: &'static str,
    pub eu: &'static str,
    pub it: &'static str,
    pub fr: &'static str,
    pub de: &'static str,
    pub jp: &'static str,
    pub int: &'static str,
    pub cn: &'static str,
    pub bust: f64,
    pub waist: f64,
    pub hip: f64,
    pub chest: f64,
    pub neck: f64,
}

#[derive(Debug, PartialEq)]
pub struct ClothesChart {
    pub chart: Chart,
    pub person: Person,
    pub rows: &'static [ClothesRow],
}

/// The clothes charts, in priority order.
pub fn clothes_charts() -> &'static [ClothesChart] {
    super::CLOTHES_CHARTS
}

const ALL: [Chart; 4] = [Chart::Women, Chart::Suit, Chart::Trousers, Chart::Shirt];

impl Chart {
    /// As a line names it: `women`, `men's suit`.
    pub fn name(self) -> &'static str {
        match self {
            Chart::Women => "women",
            Chart::Suit => "men's suit",
            Chart::Trousers => "men's trousers",
            Chart::Shirt => "men's shirt",
        }
    }

    /// The note of an answer from this chart.
    fn note(self) -> &'static str {
        match self {
            Chart::Women => "women's clothes",
            Chart::Suit => "men's suits and jackets",
            Chart::Trousers => "men's trousers",
            Chart::Shirt => "men's shirts",
        }
    }

    pub fn rows(self) -> &'static [ClothesRow] {
        clothes_charts().iter().find(|c| c.chart == self).map_or(&[], |c| c.rows)
    }

    /// The column a system reads in this chart (`eu` is DE for women, the men's `it` is EU, their
    /// `uk` US); `None`: the chart has no such system.
    pub fn column(self, s: System) -> Option<System> {
        use System::*;
        Some(match (self, s) {
            (Chart::Women, Eu) => De,
            (Chart::Women, s) => s,
            (_, Uk) => Us,
            (Chart::Suit, It | Fr | De) | (Chart::Trousers, It | Fr | De) | (Chart::Shirt, Fr | De) => Eu,
            (Chart::Suit, Us | Eu | Jp | Int) | (Chart::Trousers, Us | Eu | Int) | (Chart::Shirt, Us | Eu | Int) => s,
            _ => return None,
        })
    }

    /// The systems its details and its line give, in order.
    pub fn shown(self) -> &'static [System] {
        use System::*;
        match self {
            Chart::Women => &[Us, Uk, De, Fr, It, Jp, Int, Cn],
            Chart::Suit => &[Us, Eu, Jp, Int],
            Chart::Trousers => &[Us, Eu, Int],
            Chart::Shirt => &[Eu, Us, Int],
        }
    }

    /// The region's system for this chart (SPEC_CLOTHES §4.2): the country of the device's zone.
    fn region(self) -> System {
        let country = crate::time::device_zone().and_then(crate::time::tz::zone_country).map(|c| c.code);
        match (self, country) {
            (_, Some("US" | "LR" | "MM")) => System::Us,
            (_, Some("GB" | "IE")) => System::Uk,
            (Chart::Women | Chart::Suit | Chart::Trousers, Some("IT")) => System::It,
            (Chart::Women, Some("FR" | "BE" | "ES" | "PT")) => System::Fr,
            (Chart::Women, Some("DE" | "AT" | "CH" | "NL" | "DK" | "SE" | "NO" | "FI" | "IS")) => System::De,
            (Chart::Women | Chart::Suit, Some("JP")) => System::Jp,
            (Chart::Women, Some("CN")) => System::Cn,
            _ => System::Eu,
        }
    }
}

impl ClothesRow {
    /// The size in a column (`Chart::column`).
    pub fn of(&self, column: System) -> &'static str {
        match column {
            System::Us => self.us,
            System::Uk => self.uk,
            System::Eu => self.eu,
            System::It => self.it,
            System::Fr => self.fr,
            System::De => self.de,
            System::Jp => self.jp,
            System::Int => self.int,
            System::Cn => self.cn,
        }
    }
}

/// `US 8`, `CN 165/92A`; a letter alone, `M`.
fn show(system: System, size: &str) -> String {
    match system {
        System::Int => size.to_string(),
        s => format!("{} {size}", s.name()),
    }
}

/// `44`, `15.75`.
fn num(v: f64) -> String {
    format!("{}", (v * 100.0).round() / 100.0)
}

/// A cell as a number: a size, or a CN label's bust (`165/92A` → 92).
fn value(column: System, cell: &str) -> Option<f64> {
    match column {
        System::Cn => cell.split_once('/').and_then(|(_, b)| b.trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse().ok()),
        _ => cell.parse().ok(),
    }
}

/// A body measure of a clothes request.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Measure {
    Bust,
    Chest,
    /// `göğüs`, `Brustumfang`: a woman's bust or a man's chest.
    Torso,
    Waist,
    Hip,
    Collar,
}

impl Measure {
    fn of(name: &str) -> Option<Measure> {
        Some(match name {
            "bust" => Measure::Bust,
            "chest" => Measure::Chest,
            "torso" => Measure::Torso,
            "waist" => Measure::Waist,
            "hip" => Measure::Hip,
            "collar" => Measure::Collar,
            _ => return None,
        })
    }

    fn word(self) -> &'static str {
        match self {
            Measure::Bust => "bust",
            Measure::Chest | Measure::Torso => "chest",
            Measure::Waist => "waist",
            Measure::Hip => "hip",
            Measure::Collar => "collar",
        }
    }

    /// The charts that have it.
    fn charts(self) -> &'static [Chart] {
        match self {
            Measure::Bust | Measure::Hip => &[Chart::Women],
            Measure::Chest => &[Chart::Suit],
            Measure::Torso => &[Chart::Women, Chart::Suit],
            Measure::Waist => &[Chart::Women, Chart::Trousers],
            Measure::Collar => &[Chart::Shirt],
        }
    }

    /// Whether, in `chart`, the measure in inches is the size itself (the US column): a suit's chest,
    /// a waist for trousers, a collar.
    fn is_size(self, chart: Chart) -> bool {
        matches!((self, chart), (Measure::Chest | Measure::Torso, Chart::Suit) | (Measure::Waist, Chart::Trousers) | (Measure::Collar, Chart::Shirt))
    }

    /// The row's measure in cm (0: none).
    fn cm(self, row: &ClothesRow) -> f64 {
        match self {
            Measure::Bust => row.bust,
            Measure::Chest => row.chest,
            Measure::Torso => row.bust.max(row.chest),
            Measure::Waist => row.waist,
            Measure::Hip => row.hip,
            Measure::Collar => row.neck,
        }
    }
}

/// What a clothes request reads.
#[derive(Debug, Clone, PartialEq)]
enum Size {
    /// A number in its system (`Query::source`).
    Number(f64),
    Letter(String),
    /// Both ends of `8-10`, `S-M`; the letters may also be one label (`S/M`).
    Range(Box<Size>, Box<Size>, String),
    /// A CN label's bust.
    Cn(f64),
    /// A waist in inches, and the inseam.
    Waist(f64, Option<f64>),
    /// A body measure in cm, and in inches when it was typed so.
    Measure(Measure, f64, Option<f64>),
}

/// A clothes request read.
#[derive(Debug)]
struct Query {
    size: Size,
    /// What is converted, as shown: `IT 44`, `M`, `92 cm bust`.
    shown: String,
    /// The system a number (or a range of them) is in.
    source: Option<System>,
    target: Option<System>,
    /// The charts it may be, in priority order.
    charts: Vec<Chart>,
    inseam: Option<f64>,
}

/// The charts an item word reaches; a letter reaches the men's letters through tops and shirts.
fn reaches(item: &str, letter: bool) -> &'static [Chart] {
    match (item, letter) {
        ("dress", _) | ("top", false) => &[Chart::Women],
        ("top", true) => &[Chart::Women, Chart::Suit],
        ("trousers", _) => &[Chart::Women, Chart::Trousers],
        ("suit", _) | ("shirt", true) => &[Chart::Suit],
        ("shirt", false) => &[Chart::Shirt],
        _ => &ALL,
    }
}

/// The charts a request may be, in priority order: its items' (all of them for `clothes`), its
/// measure's, its label's (a waist: trousers; a CN label: women's), its person's.
fn candidates(items: &[&str], measure: Option<Measure>, person: Option<Person>, size: &Size) -> Vec<Chart> {
    let letter = matches!(size, Size::Letter(_)) || matches!(size, Size::Range(a, ..) if matches!(**a, Size::Letter(_)));
    ALL.into_iter()
        .filter(|c| items.is_empty() || items.iter().any(|i| reaches(i, letter).contains(c)))
        .filter(|c| measure.is_none_or(|m| m.charts().contains(c)))
        .filter(|c| match size {
            Size::Waist(..) => *c == Chart::Trousers,
            Size::Cn(_) => *c == Chart::Women,
            _ => true,
        })
        .filter(|c| match person {
            Some(Person::Men) => *c != Chart::Women,
            Some(Person::Women) => *c == Chart::Women,
            _ => true,
        })
        .collect()
}

/// A label's own system, read from beside it rather than converted to (`int M`, `cn 160/84A`).
fn own_system(size: &Size, s: System) -> bool {
    match size {
        Size::Letter(l) => s == System::Int || (s == System::Jp && (l == "LL" || l == "3L")),
        Size::Cn(_) => s == System::Cn,
        Size::Waist(..) => matches!(s, System::Us | System::Uk),
        _ => false,
    }
}

/// Reads a clothes request from its tokens: one size (and an inseam), its system, the system to
/// convert to, the charts it may be.
fn query(tokens: &[Token], items: &[usize], measures: &[usize], person: Option<Person>) -> Result<Query, Error> {
    let mut kinds: Vec<Measure> = measures.iter().filter_map(|&i| Measure::of(super::body(i).0)).collect();
    kinds.dedup();
    let measure = match kinds[..] {
        [] => None,
        [m] => Some(m),
        _ => return Err(Error::Usage("one measure at a time".into())),
    };
    let labels: Vec<usize> = (0..tokens.len()).filter(|&i| matches!(tokens[i], Token::Number(_) | Token::Letter(_) | Token::Range(..) | Token::Cn(..) | Token::Waist(..))).collect();
    let at = match labels[..] {
        [] => return Err(Error::Incomplete("now the size: 44 it, M, 8 us…".into())),
        [one] => one,
        _ => return Err(Error::Usage("one size at a time".into())),
    };
    let trousers = items.iter().any(|&i| super::item(i).0 == "trousers") || measure == Some(Measure::Waist);
    let inseam = tokens.iter().find_map(|t| if let Token::Inseam(v) = t { Some(*v) } else { None });
    let unit = match (tokens.get(at + 1), measure) {
        (Some(Token::Cm), _) => Some(1.0),
        (Some(Token::In), Some(_)) => Some(2.54),
        _ => None,
    };
    let (size, mut shown) = read_size(&tokens[at], unit, measure, trousers)?;
    let size = match size {
        Size::Waist(w, None) => Size::Waist(w, inseam),
        s => s,
    };
    let items: Vec<&str> = items.iter().map(|&i| super::item(i).0).filter(|&i| i != "clothes").collect();
    let charts = candidates(&items, measure, person, &size);
    // The label's system: beside it, before (`IT 44`) or after (`44 it`).
    let systems: Vec<(usize, System)> = tokens.iter().enumerate().filter_map(|(i, t)| if let Token::System(s, ..) = t { Some((i, *s)) } else { None }).collect();
    let numeric = matches!(size, Size::Number(_)) || matches!(&size, Size::Range(a, ..) if matches!(**a, Size::Number(_)));
    let beside = systems.iter().find(|(i, _)| *i + 1 == at).or_else(|| systems.iter().find(|(i, _)| *i == at + 1)).copied();
    let source_at = beside.filter(|&(_, s)| numeric || own_system(&size, s)).map(|(i, _)| i);
    let mut source = source_at.and_then(|i| systems.iter().find(|(j, _)| *j == i)).map(|&(_, s)| s);
    let targets: Vec<System> = systems.iter().filter(|(i, _)| Some(*i) != source_at).map(|&(_, s)| s).collect();
    let target = match targets[..] {
        [] => None,
        [t] => Some(t),
        _ => return Err(Error::Usage("one system to convert to".into())),
    };
    if numeric && source.is_none() {
        let v = match &size {
            Size::Number(v) => *v,
            Size::Range(a, ..) => match **a {
                Size::Number(v) => v,
                _ => 0.0,
            },
            _ => 0.0,
        };
        source = Some(resolve(&charts, v, &shown)?);
    }
    if let (true, Some(s)) = (numeric, source) {
        shown = format!("{} {shown}", s.name());
    }
    if let (Size::Waist(_, Some(i)), false) = (&size, shown.contains('L') || shown.contains('X')) {
        shown = format!("{shown} L{}", num(*i));
    }
    let inseam = match size {
        Size::Waist(_, i) => i,
        _ => inseam,
    };
    Ok(Query { size, shown, source, target, charts, inseam })
}

/// The size a label reads, and how it is shown: a measure with its unit (`92 cm bust`), a waist or
/// a chest or a collar said by its measure word, a number, a letter, a range, a CN label, a waist.
fn read_size(label: &Token, unit: Option<f64>, measure: Option<Measure>, trousers: bool) -> Result<(Size, String), Error> {
    Ok(match (label, unit, measure) {
        (Token::Number(n), Some(k), Some(m)) => {
            let typed = if k == 1.0 { "cm" } else { "in" };
            (Size::Measure(m, n.value * k, (k != 1.0).then_some(n.value)), format!("{} {typed} {}", n.text, m.word()))
        }
        (Token::Number(_), Some(_), None) => return Err(Error::Usage("which measure: `92 cm bust`, `39 cm collar`, `81 cm waist`".into())),
        (Token::Number(n), None, Some(Measure::Waist)) => (Size::Waist(n.value, None), format!("W{}", n.text)),
        (Token::Number(n), None, Some(m @ (Measure::Chest | Measure::Torso))) => (Size::Measure(m, n.value * 2.54, Some(n.value)), format!("{} in chest", n.text)),
        (Token::Number(n), None, Some(Measure::Collar)) if n.value >= 30.0 => (Size::Measure(Measure::Collar, n.value, None), format!("{} cm collar", n.text)),
        (Token::Number(n), None, Some(Measure::Collar)) => (Size::Measure(Measure::Collar, n.value * 2.54, Some(n.value)), format!("{} in collar", n.text)),
        (Token::Number(_), None, Some(m)) => return Err(Error::Usage(format!("the {} in cm or in: `92 cm {}`", m.word(), m.word()))),
        (Token::Number(n), ..) => (Size::Number(n.value), n.text.clone()),
        (Token::Letter(l), ..) => (Size::Letter(l.clone()), l.clone()),
        // `32/34` with trousers: a waist and its inseam (§2.2).
        (Token::Range(Label::Number(a), Label::Number(b), typed), ..) if typed.contains('/') && trousers => (Size::Waist(a.value, Some(b.value)), typed.clone()),
        (Token::Range(a, b, typed), ..) => {
            let one = |l: &Label| match l {
                Label::Number(n) => Size::Number(n.value),
                Label::Letter(l) => Size::Letter(l.clone()),
            };
            (Size::Range(Box::new(one(a)), Box::new(one(b)), typed.to_ascii_uppercase()), typed.to_ascii_uppercase())
        }
        (Token::Cn(b, typed), ..) => (Size::Cn(*b), format!("CN {}", typed.to_ascii_uppercase())),
        (Token::Waist(w, i, typed), ..) => (Size::Waist(*w, *i), typed.to_ascii_uppercase()),
        _ => unreachable!("a label"),
    })
}

/// The system of a number typed without one (SPEC_CLOTHES §3.6): the region's when it has the
/// number, else the only system that has it; else the hint lists them.
fn resolve(charts: &[Chart], v: f64, shown: &str) -> Result<System, Error> {
    let has = |c: Chart, s: System| {
        let values: Vec<f64> = c.rows().iter().filter_map(|r| value(s, r.of(s))).collect();
        values.iter().copied().reduce(f64::min).is_some_and(|lo| lo - 1e-9 <= v) && values.iter().copied().reduce(f64::max).is_some_and(|hi| v <= hi + 1e-9)
    };
    let region = charts.first().map_or(System::Eu, |c| c.region());
    if charts.iter().any(|&c| c.column(region).is_some_and(|col| has(c, col))) {
        return Ok(region);
    }
    let order = [System::Eu, System::Uk, System::Us, System::It, System::Fr, System::De, System::Jp, System::Cn];
    let found: Vec<System> = order.into_iter().filter(|&s| charts.iter().any(|&c| c.column(s) == Some(s) && has(c, s))).collect();
    match found[..] {
        [one] => Ok(one),
        [] => Err(Error::Answer(format!("no {shown} in clothes"))),
        _ => Err(Error::Incomplete(format!("which system: {}", found.iter().map(|s| s.code()).collect::<Vec<_>>().join(", ")))),
    }
}

/// The rows a size falls on, in one chart.
#[derive(Debug)]
struct Hit {
    rows: Vec<&'static ClothesRow>,
    /// Between two rows, near their midpoint: `between IT 44 and 46`, `between 92 and 97 cm`.
    between: Option<String>,
    /// A measure beyond the chart: its first or last row.
    beyond: bool,
}

/// The nearest row to `wanted` among `rows` (value ascending), the larger at the midpoint, with
/// the "between" note near the midpoint (SPEC_SIZES §4.4); a measure beyond the chart takes its
/// first or last row.
fn nearest(rows: &[(&'static ClothesRow, f64)], wanted: f64, measure: bool, note: &dyn Fn(f64, f64) -> String) -> Option<Hit> {
    let (first, last) = (rows.first()?.1, rows.last()?.1);
    let at = |v: f64| rows.iter().filter(|(_, x)| (x - v).abs() < 1e-9).map(|(r, _)| *r).collect::<Vec<_>>();
    if wanted < first - 1e-9 || wanted > last + 1e-9 {
        return measure.then(|| Hit { rows: at(if wanted < first { first } else { last }), between: None, beyond: true });
    }
    let exact = at(wanted);
    if !exact.is_empty() {
        return Some(Hit { rows: exact, between: None, beyond: false });
    }
    let i = rows.windows(2).position(|w| w[0].1 < wanted && wanted < w[1].1)?;
    let (lo, hi) = (rows[i].1, rows[i + 1].1);
    let mid = (lo + hi) / 2.0;
    let between = ((wanted - mid).abs() <= (hi - lo) / 4.0 + 1e-9).then(|| note(lo, hi));
    Some(Hit { rows: at(if wanted < mid { lo } else { hi }), between, beyond: false })
}

/// Where a size falls in a chart; `None`: the chart has no such size or system.
fn hit(chart: Chart, size: &Size, source: Option<System>) -> Option<Hit> {
    let rows = chart.rows();
    let column_values = |col: System| rows.iter().filter_map(|r| value(col, r.of(col)).map(|v| (r, v))).collect::<Vec<_>>();
    match size {
        Size::Number(v) => {
            let s = source?;
            let col = chart.column(s)?;
            nearest(&column_values(col), *v, false, &|lo, hi| format!("between {} {} and {}", s.name(), num(lo), num(hi)))
        }
        Size::Letter(l) => {
            let col = chart.column(if l == "LL" || l == "3L" { System::Jp } else { System::Int })?;
            let found: Vec<&ClothesRow> = rows.iter().filter(|r| r.of(col).eq_ignore_ascii_case(l)).collect();
            (!found.is_empty()).then_some(Hit { rows: found, between: None, beyond: false })
        }
        Size::Range(a, b, typed) => {
            // `S/M`: the women's label first.
            let label: Vec<&ClothesRow> = rows.iter().filter(|r| r.int.eq_ignore_ascii_case(typed)).collect();
            if !label.is_empty() {
                return Some(Hit { rows: label, between: None, beyond: false });
            }
            let (ha, hb) = (hit(chart, a, source)?, hit(chart, b, source)?);
            let index = |r: &&ClothesRow| rows.iter().position(|x| std::ptr::eq(x, *r));
            let from = ha.rows.iter().filter_map(index).min()?;
            let to = hb.rows.iter().filter_map(index).max()?;
            (from <= to).then(|| Hit { rows: rows[from..=to].iter().collect(), between: None, beyond: false })
        }
        Size::Cn(b) => nearest(&column_values(chart.column(System::Cn)?), *b, false, &|lo, hi| format!("between CN /{} and /{}", num(lo), num(hi))),
        Size::Waist(w, _) if chart == Chart::Trousers => nearest(&column_values(System::Us), *w, false, &|lo, hi| format!("between W{} and W{}", num(lo), num(hi))),
        Size::Waist(..) => None,
        Size::Measure(m, cm, inches) => {
            if !m.charts().contains(&chart) {
                return None;
            }
            // The measure that is the size, in inches (a suit's chest in cm too): the US column.
            match inches {
                Some(_) | None if m.is_size(chart) && (inches.is_some() || chart == Chart::Suit) => {
                    let i = inches.unwrap_or(cm / 2.54);
                    nearest(&column_values(System::Us), i, true, &|lo, hi| format!("between US {} and {}", num(lo), num(hi)))
                }
                _ => {
                    let values: Vec<(&ClothesRow, f64)> = rows.iter().map(|r| (r, m.cm(r))).filter(|(_, v)| *v > 0.0).collect();
                    nearest(&values, *cm, true, &|lo, hi| format!("between {} and {} cm", num(lo), num(hi)))
                }
            }
        }
    }
}

/// The sizes of `rows` in a column: `44`, a range `44–46`; `None` when the chart has none.
fn sizes_of(rows: &[&ClothesRow], column: System) -> Option<String> {
    let mut all: Vec<&str> = Vec::new();
    for r in rows {
        let s = r.of(column);
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

/// A clothes request's answer: one chart's size with its other systems, a line per chart, or, with
/// no target and several charts, a line per chart with every system.
pub(super) fn answer(tokens: &[Token], items: &[usize], measures: &[usize], person: Option<Person>) -> Result<Answer, Error> {
    let q = query(tokens, items, measures, person)?;
    let hits: Vec<(Chart, Hit)> = q
        .charts
        .iter()
        .filter(|c| q.target.is_none_or(|t| c.column(t).is_some()))
        .filter_map(|&c| hit(c, &q.size, q.source).map(|h| (c, h)))
        .collect();
    let answer = match (q.target, &hits[..]) {
        (_, []) => return Err(not_found(&q)),
        (t, [(c, h)]) => one(&q, *c, h, t.unwrap_or_else(|| c.region())),
        (Some(t), many) => several(&q, many, Some(t)),
        // A measure has no system to show beside the others: each chart's region (§4.3).
        (None, many) if matches!(q.size, Size::Measure(..)) => several(&q, many, None),
        (None, many) => table(&q, many),
    }?;
    Ok(Answer { from: q.shown.clone(), ..answer })
}

/// The notes every answer from these charts has: `EU sizes as in DE`, the inseam, the brand note.
fn closing(q: &Query, charts: &[Chart], target: Option<System>) -> Vec<String> {
    let mut notes = Vec::new();
    if charts.contains(&Chart::Women) && (target == Some(System::Eu) || q.source == Some(System::Eu)) {
        notes.push("EU sizes as in DE".to_string());
    }
    if let Some(i) = q.inseam {
        notes.push(match target {
            Some(t) if t != System::Us && t != System::Uk => format!("inseam L{} has no {} size", num(i), t.name()),
            _ => format!("inseam L{}", num(i)),
        });
    }
    notes.push(BRAND_NOTE.to_string());
    notes
}

/// One chart's answer: the size in the target, the other systems, the notes.
fn one(q: &Query, chart: Chart, h: &Hit, target: System) -> Result<Answer, Error> {
    let col = chart.column(target).ok_or_else(|| not_found(q))?;
    let size = sizes_of(&h.rows, col).ok_or_else(|| Error::Answer(format!("no {} size for {} in {}", target.name(), q.shown, chart.note())))?;
    let text = show(target, &size);
    let others: Vec<String> = chart.shown().iter().filter(|&&s| s != col).filter_map(|&s| sizes_of(&h.rows, s).map(|v| show(s, &v))).collect();
    let mut notes = vec![chart.note().to_string()];
    notes.extend(h.between.clone());
    if h.beyond {
        notes.push("beyond the chart".to_string());
    }
    notes.extend(closing(q, &[chart], Some(target)));
    // As a line, nothing when the region's system is the typed one.
    let same = q.target.is_none() && q.source.and_then(|s| chart.column(s)) == Some(col);
    Ok(Answer {
        line: if same { Vec::new() } else { vec![format!("{text} {}", chart.name())] },
        text,
        details: (!others.is_empty()).then(|| others.join(" · ")),
        notes,
        ..Answer::default()
    })
}

/// Several charts: a line each, `IT 44 women`, in priority order; in the target, else in each
/// chart's region.
fn several(q: &Query, hits: &[(Chart, Hit)], target: Option<System>) -> Result<Answer, Error> {
    let lines: Vec<(Chart, String)> = hits
        .iter()
        .filter_map(|(c, h)| {
            let t = target.unwrap_or_else(|| c.region());
            sizes_of(&h.rows, c.column(t)?).map(|s| (*c, show(t, &s)))
        })
        .collect();
    if lines.is_empty() {
        return Err(not_found(q));
    }
    let all: Vec<String> = lines.iter().map(|(c, s)| format!("{s} {}", c.name())).collect();
    let mut notes: Vec<String> = hits.iter().filter_map(|(c, h)| h.between.as_ref().map(|b| format!("{b} ({})", c.name()))).collect();
    if hits.iter().any(|(_, h)| h.beyond) {
        notes.push("beyond the chart".to_string());
    }
    let charts: Vec<Chart> = lines.iter().map(|(c, _)| *c).collect();
    notes.extend(closing(q, &charts, target));
    Ok(Answer {
        text: all.join("\n"),
        result: Some(all[0].clone()),
        extra: lines[1..].iter().map(|(c, s)| (format!("clothes {}", c.name()), vec![s.clone()])).collect(),
        notes,
        copy: Some(lines.iter().map(|(c, s)| format!("{} {s}", super::title(c.name()))).collect::<Vec<_>>().join("\n")),
        line: all,
        ..Answer::default()
    })
}

/// Several charts, no target: a line per chart with every system it has.
fn table(q: &Query, hits: &[(Chart, Hit)]) -> Result<Answer, Error> {
    let lines: Vec<(Chart, Vec<String>)> =
        hits.iter().map(|(c, h)| (*c, c.shown().iter().filter_map(|&s| sizes_of(&h.rows, s).map(|v| show(s, &v))).collect())).collect();
    let width = lines.iter().map(|(c, _)| c.name().chars().count()).max().unwrap_or(0);
    let text = lines.iter().map(|(c, cells)| format!("{:<width$}  {}", c.name(), cells.join(" · "))).collect::<Vec<_>>().join("\n");
    // As a line: each chart's region, left out where it is the typed system.
    let line = hits
        .iter()
        .filter_map(|(c, h)| {
            let region = c.region();
            let col = c.column(region)?;
            (q.source.and_then(|s| c.column(s)) != Some(col)).then_some(())?;
            sizes_of(&h.rows, col).map(|s| format!("{} {}", show(region, &s), c.name()))
        })
        .collect();
    let mut notes: Vec<String> = hits.iter().filter_map(|(c, h)| h.between.as_ref().map(|b| format!("{b} ({})", c.name()))).collect();
    let charts: Vec<Chart> = lines.iter().map(|(c, _)| *c).collect();
    notes.extend(closing(q, &charts, None));
    Ok(Answer {
        text,
        extra: lines.iter().map(|(c, cells)| (format!("clothes {}", c.name()), cells.clone())).collect(),
        copy: Some(lines.iter().map(|(c, cells)| format!("{} {}", super::title(c.name()), cells.join(" · "))).collect::<Vec<_>>().join("\n")),
        notes,
        line,
        ..Answer::default()
    })
}

/// A size no chart has: `no IT 70 in clothes (IT 36–54 women, IT 44–60 men's suit, …)`.
fn not_found(q: &Query) -> Error {
    let ranges: Vec<String> = match q.source {
        Some(s) => q
            .charts
            .iter()
            .filter_map(|&c| {
                let col = c.column(s)?;
                let values: Vec<&str> = c.rows().iter().map(|r| r.of(col)).filter(|v| !v.is_empty()).collect();
                Some(format!("{} {}–{} {}", s.name(), values.first()?, values.last()?, c.name()))
            })
            .collect(),
        None => Vec::new(),
    };
    let whose = match q.charts[..] {
        [c] => c.note(),
        _ => "clothes",
    };
    match ranges.is_empty() {
        true => Error::Answer(format!("no {} in {whose}", q.shown)),
        false => Error::Answer(format!("no {} in {whose} ({})", q.shown, ranges.join(", "))),
    }
}

/// The clothes line beside another answer (`42 eu us`, `M it`, SPEC_CLOTHES §3.3): a size and
/// systems only; every chart that has it, in the target or each chart's region; none when nothing
/// would change (the region's system is the typed one).
pub(super) fn beside(tokens: &[Token]) -> Option<crate::Extra> {
    let mut label = None;
    let mut systems: Vec<(System, String)> = Vec::new();
    for t in tokens {
        match t {
            Token::Number(n) if n.age.is_none() && label.is_none() => label = Some(Label::Number(n.clone())),
            Token::Letter(l) if label.is_none() => label = Some(Label::Letter(l.clone())),
            Token::System(s, typed, false) => systems.push((*s, typed.clone())),
            Token::Connector | Token::In => {}
            _ => return None,
        }
    }
    let (size, source, targets, from) = match label? {
        Label::Number(n) => {
            let (s, typed) = systems.first()?;
            (Size::Number(n.value), Some(*s), &systems[1..], format!("{} {typed}", n.text))
        }
        Label::Letter(l) => (Size::Letter(l.clone()), None, &systems[..], l),
    };
    if targets.len() > 1 {
        return None;
    }
    let results: Vec<String> = ALL
        .iter()
        .filter_map(|&c| {
            let target = targets.first().map_or_else(|| c.region(), |t| t.0);
            let col = c.column(target)?;
            // Nothing to say where the target reads the typed system's column (`42 eu fr`: trousers).
            if source.and_then(|s| c.column(s)) == Some(col) {
                return None;
            }
            let h = hit(c, &size, source)?;
            sizes_of(&h.rows, col).map(|s| format!("{} {}", show(target, &s), c.name()))
        })
        .collect();
    (!results.is_empty()).then(|| crate::Extra { kind: "clothes".into(), from, results })
}
