//! A short, deterministic fuzz run in every `cargo test`: random requests from the engine's own
//! words and from awkward characters, each typed key by key as the Android app sends it (the
//! answer and the suggestions for every prefix), through the functions the app's calls wrap
//! (`convert_request`, `grouping`, `suggest`). Nothing may panic: in the app a panic closes it.
//! Then every input of `fuzz/corpus/regressions.txt`. The long runs are `fuzz/fuzz.py`'s.
//!
//! `cargo test --release --test fuzz -- --ignored` runs it 50 times longer.

use aequalis::config::Config;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// xorshift64*: the same requests on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const WORDS: &[&str] = &[
    "10",
    "-40",
    "0",
    ".5",
    "1e3",
    "1e400",
    "2,5",
    "1.000",
    "99999999999999999999999",
    "12+4/2",
    "(1+2",
    "sqrt",
    "sin(30°)",
    "20%",
    "of",
    "+",
    "-",
    "*",
    "/",
    "^",
    "x",
    "(",
    ")",
    "[",
    "]",
    "usd",
    "euro",
    "€",
    "$",
    "dollar",
    "Italy",
    "USA",
    "UAE",
    "Rome",
    "Paris",
    "New York",
    "Tokio",
    "Springfield,",
    "to",
    "in",
    "->",
    "distance",
    "now",
    "today",
    "date",
    "unix",
    "iso",
    "local",
    "num",
    "wd",
    "5wd",
    "+ 3d",
    "12/04/2026",
    "2026/12/04",
    "2026-12-04T10:00Z",
    "12 Sept 2026",
    "10:00AM",
    "10 pm",
    "GMT+2",
    "UTC-05:30",
    "GMT+0530",
    "DD/MM/YYYY",
    "MM/DD/YYYY-24h",
    "février",
    "März",
    "de",
    "the",
    "41.9,12.5",
    "33.87S 151.21E",
    "m^99",
    "kg*m/s2",
    "l/100km",
    "km/h",
    "ft²",
    "Bar, Montenegro",
];

const AWKWARD: &[&str] =
    &["é", "ß", "İ", "°", "²", "·", "€", "円", "−", "\u{301}", "\u{200b}", "\u{200f}", "\u{a0}", "\u{202f}", "１", "٣", "½", "√", "π", "🙂", "👍🏽", "🇮🇹", "\t"];

fn request(r: &mut Rng, units: &[&'static str]) -> String {
    let mut out = String::new();
    for _ in 0..1 + r.below(7) {
        let word = match r.below(10) {
            0..=4 => r.pick(WORDS),
            5..=7 => units[r.below(units.len())],
            _ => r.pick(AWKWARD),
        };
        if !out.is_empty() && r.below(4) > 0 {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// Typed key by key: every prefix, as the app asks while typing.
fn typed(config: &Config, text: &str) {
    let opts = aequalis::Options { positional: Vec::new(), precision: 4, silent: false, verbose: false };
    let ends: Vec<usize> = text.char_indices().map(|(i, _)| i).skip(1).chain([text.len()]).collect();
    for end in ends {
        let before = &text[..end];
        let words: Vec<&str> = before.split_whitespace().collect();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let _ = aequalis::convert_request(config, &opts, &words);
            let _ = aequalis::grouping(before);
            let _ = aequalis::suggest::suggest(config, before, before.chars().count(), 3);
        }));
        assert!(outcome.is_ok(), "panicked on {before:?} (typing {text:?})");
    }
}

fn run(calls: usize) {
    let config = Config::shared();
    let units: Vec<&'static str> = config.categories.iter().flat_map(|c| c.units.iter().map(|u| u.symbol)).collect();
    let mut r = Rng(0x7a11_7a11_7a11_7a11);
    for _ in 0..calls {
        typed(config, &request(&mut r, &units));
    }
}

#[test]
fn random_requests_never_panic() {
    run(300);
}

#[test]
#[ignore]
fn random_requests_never_panic_long() {
    run(75_000);
}

#[test]
fn the_fuzzers_findings() {
    let corpus = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/fuzz/corpus/regressions.txt")).unwrap();
    let mut n = 0;
    for line in corpus.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let doc = aequalis::json::parse(line).unwrap_or_else(|e| panic!("{line}: {e}"));
        typed(Config::shared(), doc.get("input").and_then(|v| v.str()).unwrap());
        n += 1;
    }
    assert!(n >= 10, "{n} inputs");
}
