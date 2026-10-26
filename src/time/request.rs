//! Date requests: what is one, and how it is read and answered (cities, countries, zones,
//! misspelt places).

use super::*;

/// Whether a request is a date request (see `request`): it starts with `now` / `today` or an ISO
/// date, or its second word is an input format (`1791644400 unix`, `20261010 iso`), or it is
/// text in the locale's format followed by `local`.
pub fn is_date_request(input: &str) -> bool {
    // A city (`Paris`), or a time of day first (`10:00AM Par…`, being typed).
    let words: Vec<&str> = input.split_whitespace().collect();
    if names_city(input) || leading_time(&words).is_some() {
        return true;
    }
    match take_zone(input) {
        // A zone word (`2026/12/04 GMT+2`) makes it a date; a wrong one is reported as a date error.
        Ok((rest, Some(_))) => rest.is_empty() || looks_like_date(&rest) || read_date(&rest, None, Order::of_locale()).is_some(),
        Ok((rest, None)) => looks_like_date(&rest),
        Err(_) => true,
    }
}

pub(super) fn looks_like_date(input: &str) -> bool {
    let words: Vec<&str> = input.split_whitespace().collect();
    let Some(first) = words.first() else { return false };
    // `… num` asks for a number; `2026-12-02/12` is arithmetic (bug 005).
    if words.last().is_some_and(|w| crate::config::is_number_word(w)) || arithmetic_after_iso(input) {
        return false;
    }
    starts_with_keyword(first)
        || leading_iso(first).is_some()
        || words.get(1).is_some_and(|w| unix_scale(w).is_some() || *w == "iso")
        || (words[1..].contains(&"local") || words[1..].contains(&"date")) && !first.starts_with('-')
        || words[1..].iter().any(|w| Pattern::parse(w).is_some())
        || guessed(&words).is_some()
        || written_date_then_math(input).is_some()
        // A date with a month name, alone or before a format (also an impossible one: `31 Feb 2026`
        // is a wrong date, not a unit conversion).
        || numeric_from_named(input.trim()).is_some()
        || !first.starts_with('-') && timed_date(&words).is_some()
        || words.len() > 1 && numeric_from_named(&words[..words.len() - 1].join(" ")).is_some()
}

/// A written date followed by date math (`2026/12/04 - 12d`): subtracting or adding a duration
/// makes the value before it a date (bug 006). The byte position of that `+` / `-` in `text`
/// (trimmed at the start), at the first one after which a duration follows and before which a
/// written date stands.
pub(super) fn written_date_then_math(text: &str) -> Option<usize> {
    let text = text.trim_start();
    text.char_indices().filter(|&(_, c)| c == '+' || c == '-').map(|(i, _)| i).find(|&i| {
        let date = text[..i].trim();
        !date.is_empty() && starts_with_duration(&text[i + 1..]) && read_date(date, None, Order::of_locale()).is_some()
    })
}

/// Whether `text` starts with a duration: a number, optional spaces, a time unit (`12d`, `3 hours`).
pub(super) fn starts_with_duration(text: &str) -> bool {
    let config = Config::shared();
    let text = text.trim_start();
    let n = text.bytes().take_while(|b| b.is_ascii_digit() || *b == b'.').count();
    let unit = text[n..].trim_start();
    let unit = &unit[..unit.find(|c: char| c.is_whitespace() || c == '+' || c == '-').unwrap_or(unit.len())];
    n > 0 && time_unit(config, unit).is_some()
}

/// Whether `text` is a date value on its own: `now`, an ISO date, a date request, or a written
/// date (`12/04/26 3:20PM`). Used to tell an input format from an output one.
pub fn is_date_value(text: &str) -> bool {
    is_date_request(text) || read_date(text, None, Order::of_locale()).is_some()
}

/// Whether `text` is only a number expression (`1-10-10`, `2026-12-02`, `79 / 10 / 1`): digits,
/// operators and brackets, no word. Alone it is arithmetic, never a date on its way to a format
/// (`2026-12-02-2026-12-01` = -27); a date needs a format, a unit or a keyword beside it.
pub fn is_number_expression(text: &str) -> bool {
    let t = text.trim();
    t.chars().any(|c| c.is_ascii_digit()) && t.chars().all(|c| c.is_ascii_digit() || " .+-*/^()[]{}xX".contains(c))
}

/// A written date followed by a single output format (`12/04/26 3:20PM unix`): the date is read
/// with a guessed order.
pub(super) fn guessed(words: &[&str]) -> Option<Reading> {
    let (last, before) = words.split_last()?;
    if before.is_empty() || !is_format(last) {
        return None;
    }
    read_date(&before.join(" "), None, Order::of_locale())
}

/// An ISO date shape followed by `/`, `*`, `^`, `x` or `(`: arithmetic, not a date
/// (`2026-12-02/12` = 2026 − 12 − 2/12). Date math only uses `+` and `-`.
pub(super) fn arithmetic_after_iso(input: &str) -> bool {
    let t = input.trim_start();
    let Some(first) = t.split_whitespace().next() else { return false };
    let Some((_, len)) = leading_iso(first) else { return false };
    t[len..].trim_start().starts_with(['/', '*', '^', 'x', 'X', '('])
}

/// Whether a request writes seconds: `10:00:30`, `T150405` (ISO basic), a Unix timestamp with a
/// fraction aside.
pub(super) fn writes_seconds(text: &str) -> bool {
    let b = text.as_bytes();
    let digit = |i: usize| b.get(i).is_some_and(u8::is_ascii_digit);
    (0..b.len()).any(|i| {
        (b[i] == b':' && digit(i + 1) && digit(i + 2) && b.get(i + 3) == Some(&b':')) || ((b[i] == b'T' || b[i] == b't') && (1..=6).all(|k| digit(i + k)))
    })
}

/// A written date with a time at the start of `words` (`2026/12/04 10:00 PM`, `2026-12-04 22:00`,
/// `2026-12-04T10:00PM`): how many words, and the reading. Not an ISO literal on its own
/// (`2026-12-04T22:00Z`), which keeps its ISO format.
pub(super) fn timed_date(words: &[&str]) -> Option<(usize, Reading)> {
    (1..=words.len().min(8)).rev().find_map(|n| {
        if n == 1 && parse_iso(words[0]).is_some() {
            return None;
        }
        let reading = read_date(&words[..n].join(" "), None, Order::of_locale())?;
        (reading.pattern.clock != Clock::None).then_some((n, reading))
    })
}

/// For the stdin rule: a first argument that is a date value.
pub fn starts_date(token: &str) -> bool {
    starts_with_keyword(token)
        || leading_iso(token).is_some()
        || gmt_zone(token).is_some()
        // A month name first (`September 12, 2026`, also quoted as one argument).
        || token.split(|c: char| !c.is_alphabetic()).next().is_some_and(|w| month_named(w).is_some())
}

pub(super) fn starts_with_keyword(word: &str) -> bool {
    ["now", "today"].iter().any(|k| word.strip_prefix(k).is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '+', '-'])))
}

/// The longest prefix of `word` that is an unambiguous ISO date, and its length.
pub(super) fn leading_iso(word: &str) -> Option<(Iso, usize)> {
    (8..=word.len().min(40)).rev().filter(|&n| word.is_char_boundary(n)).find_map(|n| parse_iso(&word[..n]).filter(Iso::unambiguous).map(|iso| (iso, n)))
}

pub(super) struct Parser<'a> {
    s: &'a str,
    pos: usize,
    /// How guessed dates were read, for the user to check.
    notes: Vec<String>,
    /// The format the first date was written in: the default target.
    input_format: Option<String>,
    /// The zone of dates written without one: the `GMT±N` word, else the device's.
    zone: Zone,
    /// The `GMT±N` word, when given (a date with its own offset too is an error).
    zone_word: Option<String>,
    /// The cities of the request (`10:00 New York to Paris`), taken out of it.
    cities: Option<Cities>,
    /// `--verbose`: the date in ISO too (`<iso> iso = <value> <format>`), not only the result.
    verbose: bool,
}

/// The cities of a request: the target (shown in its zone) and, before `to`, the source (the zone
/// the request is written in), with the words they were written as.
#[derive(Debug, Clone)]
pub(super) struct Cities {
    source: Option<tz::City>,
    target: tz::City,
    text: String,
}

/// Takes the cities out of a request: a city at the end is the target (`10:00AM Paris`), and one
/// before it the source, with or without `to` (`10:00AM New York to Paris`, `10:00AM New York
/// Paris`). `Ok(None)`: no city. A city elsewhere, or
/// a zone that is not installed, is an error.
pub(super) fn take_cities(input: &str) -> Result<(String, Option<Cities>), TimeError> {
    let words: Vec<&str> = input.split_whitespace().collect();
    // The cities found: (first word, word count, city).
    let mut found = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let longest = tz::longest_name().min(words.len() - i);
        let hit = (1..=longest).rev().find_map(|n| tz::city(&words[i..i + n].join(" ")).map(|c| (n, c)));
        match hit {
            Some((n, city)) => {
                found.push((i, n, city.map_err(TimeError::Usage)?));
                i += n;
            }
            None => i += 1,
        }
    }
    let Some(&(t_at, t_len, target)) = found.last() else { return Ok((input.to_string(), None)) };
    // Being typed: `10:00AM New York to`.
    if t_at + t_len + 1 == words.len() && ["to", "->"].contains(&words[t_at + t_len]) {
        let other = if target.name == "Paris" { "Rome" } else { "Paris" };
        return Err(TimeError::Incomplete(format!("now the city to convert to: `{} to {other}`", target.name)));
    }
    let misplaced = || format!("put the city at the end (`10:00 Paris`), or `<city> [to] <city>`: `{}`", words[found[0].0]);
    if t_at + t_len != words.len() {
        return usage(misplaced());
    }
    let (source, start) = match found[..found.len() - 1] {
        [] => (None, t_at),
        [(s_at, s_len, source)] if s_at + s_len + 1 == t_at && ["to", "->"].contains(&words[t_at - 1]) => (Some(source), s_at),
        // `to` is optional: the source right before the target.
        [(s_at, s_len, source)] if s_at + s_len == t_at => (Some(source), s_at),
        _ => return usage(misplaced()),
    };
    // `to` with no city before it: `10:00 to Paris` is `10:00 Paris`.
    let start = if source.is_none() && start > 0 && ["to", "->"].contains(&words[start - 1]) { start - 1 } else { start };
    let text = words[start..].join(" ");
    Ok((words[..start].join(" "), Some(Cities { source, target, text })))
}

/// `now` followed by two or more cities and nothing else (`now Rome Tokyo`, `now Rome to Tokyo`):
/// with `now` every city is a target, not a source (there is no time written in a zone to
/// convert), so each shows the time now there, without repeating a city. `None`: any other request, one city included.
fn now_in_cities(input: &str) -> Option<Result<Vec<tz::City>, TimeError>> {
    let words: Vec<&str> = input.split_whitespace().collect();
    if words.first() != Some(&"now") {
        return None;
    }
    let mut cities: Vec<tz::City> = Vec::new();
    let (mut i, mut named) = (1, 0);
    while i < words.len() {
        // `to` between cities is optional, and means nothing here: all of them are targets.
        if !cities.is_empty() && ["to", "->"].contains(&words[i]) {
            i += 1;
            continue;
        }
        let longest = tz::longest_name().min(words.len() - i);
        let (n, found) = (1..=longest).rev().find_map(|n| tz::city(&words[i..i + n].join(" ")).map(|c| (n, c)))?;
        match found {
            Ok(city) if !cities.iter().any(|c| c.name == city.name) => cities.push(city),
            Ok(_) => {}
            Err(msg) => return Some(Err(TimeError::Usage(msg))),
        }
        (i, named) = (i + n, named + 1);
    }
    (named >= 2).then_some(Ok(cities))
}

/// A time of day alone (`10:00AM`, `10 pm`, `22:00`, `9:30:15`), on `day` (year, month, day).
pub(super) fn read_time_only(text: &str, (y, m, d): (i64, u32, u32)) -> Option<Reading> {
    // `10AM`, `10 pm`: on the hour.
    let t = text.trim();
    let n = t.bytes().take_while(u8::is_ascii_digit).count();
    let after = t[n..].trim_start();
    let meridiem = ["am", "pm", "a.m.", "p.m."].contains(&after.to_ascii_lowercase().as_str());
    let t = if (1..=2).contains(&n) && meridiem { format!("{}:00 {after}", &t[..n]) } else { t.to_string() };
    if !t.contains(':') {
        return None;
    }
    let reading = read_date(&format!("{}/{m:02}/{d:02} {t}", year(y)), Some(Fields::Ymd4), Order::YearFirst)?;
    (reading.pattern.clock != Clock::None).then_some(reading)
}

impl<'a> Parser<'a> {
    /// A zone is given for the whole request: a `GMT±N` word or a source city (`New York to …`).
    pub(super) fn zone_given(&self) -> bool {
        self.zone_word.is_some() || self.cities.as_ref().is_some_and(|c| c.source.is_some())
    }

    /// The zone of a date read with `offset` (its own, if any): its own, else the request's.
    pub(super) fn zone_for(&self, offset: Option<i32>, text: &str) -> Result<Zone, TimeError> {
        match (offset, self.zone_given()) {
            (Some(_), true) => usage(format!("`{text}` has its own time zone, and a GMT zone or a city is given too")),
            (Some(o), false) => Ok(Zone::Fixed(o)),
            (None, _) => Ok(self.zone),
        }
    }

    pub(super) fn rest(&self) -> &'a str {
        &self.s[self.pos..]
    }

    pub(super) fn skip_ws(&mut self) {
        self.pos = self.s.len() - self.rest().trim_start().len();
    }

    /// The next whitespace-separated word, without consuming it.
    pub(super) fn word(&self) -> &'a str {
        self.rest().split_whitespace().next().unwrap_or("")
    }

    pub(super) fn take_word(&mut self) -> &'a str {
        self.skip_ws();
        let w = self.word();
        self.pos += w.len();
        w
    }

    /// A date value; `first` allows the forms that need an input-format word. Each form is tried
    /// in turn (`None`: not that form); only the one that matches takes its words.
    pub(super) fn value(&mut self, first: bool) -> Result<Instant, TimeError> {
        self.skip_ws();
        let rest = self.rest();
        let words: Vec<&'a str> = rest.split_whitespace().collect();
        if first {
            if let Some(found) = self.with_input_word(&words) {
                return found;
            }
        }
        if let Some(found) = self.keyword(rest) {
            return found;
        }
        if first {
            if let Some(found) = self.time_of_day(&words).or_else(|| self.timed(&words)) {
                return found;
            }
        }
        if let Some(found) = self.iso_word(first) {
            return found;
        }
        if first {
            let found = self.stamp(&words).or_else(|| self.local(&words)).or_else(|| self.guessed_first(rest, &words)).or_else(|| self.written_alone(rest));
            if let Some(found) = found {
                return found;
            }
        }
        match self.longest_date(&words, first) {
            Some(found) => found,
            None => self.not_a_date(&words),
        }
    }

    fn take_words(&mut self, n: usize) {
        for _ in 0..n {
            self.take_word();
        }
    }

    /// A date read from `text` by a guess: asked about when it has several readings, noted, its
    /// input format set (when given), then in its zone.
    fn date_read(&mut self, text: &str, reading: &Reading, format: Option<String>) -> Result<Instant, TimeError> {
        self.unambiguous(text, reading)?;
        self.note(text, reading);
        if format.is_some() {
            self.input_format = format;
        }
        from_civil(&reading.civil, self.zone_for(reading.offset, text)?)
    }

    /// `<text> date`: a loosely written date (checked first: `2026-04-12 date` is one too); or
    /// `<text> <pattern> <format>`: read with that pattern exactly.
    fn with_input_word(&mut self, words: &[&'a str]) -> Option<Result<Instant, TimeError>> {
        if let Some(i) = words.iter().position(|w| *w == "date").filter(|&i| i > 0) {
            let text = words[..i].join(" ");
            self.take_words(i + 1);
            let order = Order::of_locale();
            let Some(reading) = read_date(&text, None, order) else {
                return Some(usage(format!("`{text}` is not a date ({})", order.describe())));
            };
            let format = reading.pattern.name();
            return Some(self.date_read(&text, &reading, Some(format)));
        }
        // With a city the city is the target, already taken out: the pattern may come last
        // (`12/04/2026 10:00 MM/DD/YYYY-24h Paris`, the request a question's choice writes: bug 013).
        let end = if self.cities.is_some() { words.len() } else { words.len().saturating_sub(1) };
        let (i, pattern) = (1..end).find_map(|i| Some((i, Pattern::parse(words[i])?)))?;
        let text = words[..i].join(" ");
        self.take_words(i + 1);
        let Some(reading) = read_with_pattern(&text, pattern) else {
            return Some(usage(format!("`{text}` is not a date in the {} format", pattern.name())));
        };
        self.input_format = Some(pattern.name());
        Some(self.zone_for(reading.offset, &text).and_then(|zone| from_civil(&reading.civil, zone)))
    }

    /// `now`, `today`: in the request's zone (`now GMT+2` shows +02:00; `today` starts there).
    fn keyword(&mut self, rest: &str) -> Option<Result<Instant, TimeError>> {
        for keyword in ["now", "today"] {
            if let Some(after) = rest.strip_prefix(keyword) {
                if after.is_empty() || after.starts_with([' ', '+', '-']) {
                    self.pos += keyword.len();
                    let now = Instant { zone: self.zone, ..now() };
                    if keyword == "now" {
                        return Some(Ok(now));
                    }
                    let c = now.civil();
                    return Some(from_civil(&Civil { year: c.year, month: c.month, day: c.day, ..Civil::default() }, self.zone));
                }
            }
        }
        None
    }

    /// A time of day first. With a city (`10:00AM Paris`): today, in the request's zone; without
    /// one, an error: a time of day alone takes a city.
    fn time_of_day(&mut self, words: &[&'a str]) -> Option<Result<Instant, TimeError>> {
        if self.cities.is_none() {
            let n = leading_time(words)?;
            return Some(match words[n..].join(" ") {
                after if after.is_empty() => usage("a time alone takes a city: `10:00AM Paris`"),
                after => usage(format!("`{after}` is not a city (`10:00AM Paris`, `10:00AM New York to Paris`)")),
            });
        }
        let c = Instant { zone: self.zone, ..now() }.civil();
        let (n, reading) = (1..=words.len().min(3)).rev().find_map(|n| Some((n, read_time_only(&words[..n].join(" "), (c.year, c.month, c.day))?)))?;
        self.take_words(n);
        Some(from_civil(&reading.civil, self.zone))
    }

    /// A written date with a time (`2026/12/04 10:00 PM`, `2026-12-04 22:00`): read whole, before
    /// its date part is taken for an ISO date. Year first with `-` is ISO-like: shown in ISO.
    fn timed(&mut self, words: &[&'a str]) -> Option<Result<Instant, TimeError>> {
        let (n, reading) = timed_date(words)?;
        let text = words[..n].join(" ");
        self.take_words(n);
        let iso_like = reading.pattern.fields == Fields::Ymd4 && text.as_bytes().get(4) == Some(&b'-');
        let format = if iso_like { "iso".to_string() } else { reading.pattern.name() };
        Some(self.date_read(&text, &reading, Some(format)))
    }

    /// An ISO date or time first (`2026-12-04`, `2026-12-04T10:00Z`).
    fn iso_word(&mut self, first: bool) -> Option<Result<Instant, TimeError>> {
        let word = self.word();
        let (iso, len) = leading_iso(word)?;
        self.pos += len;
        if first {
            self.input_format = Some(iso_format(&iso).to_string());
        }
        Some(self.zone_for(iso.offset, &word[..len]).and_then(|zone| iso_instant_in(&iso, zone)))
    }

    /// `<number> unix…` is a timestamp and `<literal> iso` an ISO date, the word being the input
    /// format. Anything else before it (`2026/12/04 unix`) is a written date with its target:
    /// guessed later (bug 004).
    fn stamp(&mut self, words: &[&'a str]) -> Option<Result<Instant, TimeError>> {
        let [value, format, ..] = words[..] else { return None };
        if let Some(ns) = unix_scale(format).and_then(|scale| scaled(value, scale)) {
            self.take_words(2);
            self.input_format = Some(format.to_string());
            return Some(Instant { ns, zone: self.zone }.in_range());
        }
        let iso = (format == "iso").then(|| parse_iso(value)).flatten()?;
        self.take_words(2);
        self.input_format = Some(iso_format(&iso).to_string());
        Some(self.zone_for(iso.offset, value).and_then(|zone| iso_instant_in(&iso, zone)))
    }

    /// `<text> local`: a date in the device's own format, in its time zone.
    fn local(&mut self, words: &[&'a str]) -> Option<Result<Instant, TimeError>> {
        let i = words.iter().position(|w| *w == "local")?;
        let text = words[..i].join(" ");
        self.take_words(i + 1);
        if self.zone_given() {
            return Some(usage("the local format is read in the device's time zone: remove the GMT zone"));
        }
        let Some(secs) = sys::parse_local(&text) else {
            return Some(usage(format!("`{text}` is not a date in the local format")));
        };
        self.input_format = Some("local".into());
        Some(Instant { ns: i128::from(secs) * NS, zone: Zone::Local }.in_range())
    }

    /// A written date followed by date math (`2026/12/04 - 12d`), or by one format, the target:
    /// how to read the date is guessed; the date math (and a target, if any) follows.
    fn guessed_first(&mut self, rest: &'a str, words: &[&'a str]) -> Option<Result<Instant, TimeError>> {
        if let Some(i) = written_date_then_math(rest) {
            let text = rest[..i].trim();
            let reading = read_date(text, None, Order::of_locale()).expect("checked by written_date_then_math");
            self.pos += i;
            let format = reading.pattern.name();
            return Some(self.date_read(text, &reading, Some(format)));
        }
        let reading = guessed(words)?;
        let text = words[..words.len() - 1].join(" ");
        self.take_words(words.len() - 1);
        Some(self.date_read(&text, &reading, None))
    }

    /// A written date on its own, with a GMT zone (`2026/12/04 GMT+2`) or a month name
    /// (`12 Sept 2026`).
    fn written_alone(&mut self, rest: &'a str) -> Option<Result<Instant, TimeError>> {
        let text = rest.trim();
        let reading = read_date(text, None, Order::of_locale()).filter(|r| self.zone_given() || r.month_name.is_some())?;
        self.pos = self.s.len();
        let format = reading.pattern.name();
        Some(self.date_read(text, &reading, Some(format)))
    }

    /// A later date of a difference, written (`2026-12-25 - 12 Sept 2026 d`), or a first one with
    /// a month name (`12 Dec 2026 - 12 Sept 2026 wd`): the longest run of words (8 at most) that
    /// reads as a date.
    fn longest_date(&mut self, words: &[&'a str], first: bool) -> Option<Result<Instant, TimeError>> {
        let words = &words[..words.len().min(8)];
        let (n, reading) = (1..=words.len()).rev().find_map(|n| {
            let reading = read_date(&words[..n].join(" "), None, Order::of_locale()).filter(|r| !first || r.month_name.is_some())?;
            Some((n, reading))
        })?;
        if first {
            self.input_format = Some(reading.pattern.name());
        }
        let text = words[..n].join(" ");
        self.take_words(n);
        Some(self.date_read(&text, &reading, None))
    }

    /// What is wrong with words that are no date: a month name with no such day (`31 Feb 2026`),
    /// else the first word.
    fn not_a_date(&self, words: &[&str]) -> Result<Instant, TimeError> {
        // The year after a day and a month is the date's (`31 Feb 2026`, not `31 Feb`).
        let year = |w: &str| (w.len() == 2 || w.len() == 4) && w.bytes().all(|b| b.is_ascii_digit());
        for n in 1..=words.len() {
            let text = words[..n].join(" ");
            if numeric_from_named(&text).is_some() && !words.get(n).is_some_and(|w| year(w)) {
                return usage(format!("`{text}` is not a date (no such day in that month)"));
            }
        }
        usage(format!("`{}` is not a date", self.word()))
    }

    /// A guessed date with more than one valid reading is not guessed: the operator chooses. Each
    /// choice is the request with the pattern written in after the date (it replaces `date`).
    pub(super) fn unambiguous(&self, text: &str, reading: &Reading) -> Result<(), TimeError> {
        if reading.readings.len() < 2 {
            return Ok(());
        }
        let rest = self.rest().trim();
        let rest = rest.strip_prefix("date").map_or(rest, str::trim_start);
        // With a city, the city is the target: no `iso`.
        let rest = if rest.is_empty() && self.cities.is_none() { "iso" } else { rest };
        let after = [self.zone_word.as_deref(), self.cities.as_ref().map(|c| c.text.as_str())];
        let choices = reading
            .readings
            .iter()
            .map(|(pattern, y, m, d)| {
                let name = pattern.name();
                let parts = [Some(text), Some(name.as_str()), Some(rest), after[0], after[1]];
                Choice {
                    pattern: pattern.name(),
                    date: format!("{}-{m:02}-{d:02}", year(*y)),
                    request: parts.iter().flatten().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join(" "),
                }
            })
            .collect();
        Err(TimeError::Ambiguous(Ambiguity { text: text.to_string(), choices, kind: AmbiguityKind::Date, note: String::new() }))
    }

    /// How a guessed date was read: `read 12/04/26 3:20PM as DD/MM/YY-12h (the locale's order)`.
    pub(super) fn note(&mut self, text: &str, reading: &Reading) {
        if let Some(m) = &reading.month_name {
            let c = &reading.civil;
            let name = ENGLISH_MONTHS[m.month as usize - 1];
            let typo = if m.typo { ", read as a misspelling" } else { "" };
            self.notes.push(format!("read `{text}` as {}-{:02}-{:02} (`{}`: {name}{typo})", year(c.year), c.month, c.day, m.written));
            return;
        }
        let why = match (reading.by_locale, reading.pattern.fields) {
            (_, Fields::Ymd4) => "year first",
            (true, _) => "the locale's order",
            (false, _) => "the only valid order",
        };
        self.notes.push(format!("read `{text}` as {} ({why})", reading.pattern.name()));
    }

    /// `<number>[ ]<time unit>` at the cursor: (amount, unit symbol), consumed; `None` if it isn't one.
    pub(super) fn duration(&mut self, config: &Config) -> Option<(&'a str, &'static str)> {
        self.skip_ws();
        let rest = self.rest();
        let n = rest.bytes().take_while(|b| b.is_ascii_digit() || *b == b'.').count();
        if n == 0 {
            return None;
        }
        let after = &rest[n..];
        let unit_text = after.trim_start();
        let len = unit_text.find(|c: char| c.is_whitespace() || c == '+' || c == '-').unwrap_or(unit_text.len());
        let symbol = time_unit(config, &unit_text[..len])?;
        self.pos += n + (after.len() - unit_text.len()) + len;
        Some((&rest[..n], symbol))
    }
}

/// The symbol of a time unit name (symbol or alias, case-insensitive), if it is one.
pub(super) fn time_unit(config: &Config, name: &str) -> Option<&'static str> {
    if WORKDAY_NAMES.iter().any(|w| w.eq_ignore_ascii_case(name)) {
        return Some(WORKDAY);
    }
    let (category, unit) = convert::find_unit(config, name)?;
    (category.name == crate::config::TIME).then_some(unit.symbol)
}

/// Evaluates a date request: the result and notes on how guessed dates were read. `None` if
/// `input` is not one (then it is a numeric request). `precision` formats a difference given in a
/// time unit.
/// `verbose`: the ISO side too (`<iso> iso = <value> <format>`), not only the result. A request
/// that fails with a misspelt city (`10:00 Tokio`, within 2 letters) is an `Ambiguous` error with
/// the cities it may be, each with the corrected request.
pub fn request(config: &Config, input: &str, precision: usize, verbose: bool) -> Option<Result<(String, Vec<String>), TimeError>> {
    request_with(config, input, precision, verbose, true)
}

/// `request`; with `typos` false, a request that fails is not checked for misspelt places (the
/// check itself tries its corrections this way).
pub(super) fn request_with(config: &Config, input: &str, precision: usize, verbose: bool, typos: bool) -> Option<Result<(String, Vec<String>), TimeError>> {
    // A country: its city (`Italy` → Rome), or which one when it has several zones (`USA`).
    match countries_in(config, input) {
        Some(Err(a)) => return Some(Err(TimeError::Ambiguous(a))),
        Some(Ok((rewritten, note))) => {
            return request_with(config, &rewritten, precision, verbose, typos).map(|r| {
                r.map(|(out, mut notes)| {
                    notes.insert(0, note);
                    (out, notes)
                })
            })
        }
        None => {}
    }
    match exact_request(config, input, precision, verbose) {
        result @ (Some(Ok(_)) | Some(Err(TimeError::Ambiguous(_)))) => result,
        other if typos => place_typos(config, input, precision, verbose).map(Err).or(other),
        other => other,
    }
}

pub(super) fn exact_request(config: &Config, input: &str, precision: usize, verbose: bool) -> Option<Result<(String, Vec<String>), TimeError>> {
    if !is_date_request(input) {
        return None;
    }
    // A `GMT±N` word is the zone of the dates written without one (`GMT+2` alone: now there).
    let (rest, zone_word) = match take_zone(input) {
        Ok(found) => found,
        Err(msg) => return Some(Err(TimeError::Usage(msg))),
    };
    // `now Rome Tokyo`: the time now in each city, one line each.
    if zone_word.is_none() {
        if let Some(found) = now_in_cities(&rest) {
            return Some(found.and_then(|cities| {
                let lines = cities.into_iter().map(|target| {
                    let alone = Cities { source: None, target, text: target.name.to_string() };
                    city_result(&alone, now(), false, "", true, verbose, false)
                });
                lines.collect::<Result<Vec<_>, _>>().map(|lines| (lines.join("\n"), Vec::new()))
            }));
        }
    }
    // Cities: `Paris` (now there), `10:00AM Paris`, `10:00AM New York to Paris`.
    let (rest, cities) = match take_cities(&rest) {
        Ok(found) => found,
        Err(e) => return Some(Err(e)),
    };
    let source = cities.as_ref().and_then(|c| c.source);
    if let (Some(source), Some((word, _))) = (source, &zone_word) {
        return Some(Err(TimeError::Usage(format!("two time zones: `{word}` and {}", source.name))));
    }
    let rest = if rest.trim().is_empty() { "now".to_string() } else { rest };
    let zone = match (source, &zone_word) {
        (Some(city), _) => Zone::Named(city.tz),
        (None, Some((_, offset))) => Zone::Fixed(*offset),
        (None, None) => Zone::Local,
    };
    let zone_word = zone_word.map(|(word, _)| word);
    let mut p = Parser { s: rest.trim(), pos: 0, notes: Vec::new(), input_format: None, zone, zone_word, cities, verbose };
    Some(evaluate(config, &mut p, precision).map(|out| (out, p.notes)))
}

pub(super) fn evaluate(config: &Config, p: &mut Parser, precision: usize) -> Result<String, TimeError> {
    // ISO results show seconds when the request wrote some (`10:00:30`), or they are not zero.
    let seconds = writes_seconds(p.s);
    let first = p.value(true)?;
    let Math { date, difference, stepped } = date_math(config, p, first)?;
    let target = p.take_word();
    if !p.rest().trim().is_empty() {
        return usage(format!("unexpected `{}`", p.rest().trim()));
    }
    if let Some(cities) = p.cities.clone() {
        return city_result(&cities, date, difference.is_some(), target, p.s.trim() == "now", p.verbose, seconds);
    }
    match difference {
        Some((a, b)) => difference_in(config, a, b, target, precision),
        None => date_result(p, &date, target, stepped, seconds),
    }
}

/// What the date math after a date gives.
struct Math {
    date: Instant,
    /// The difference of two dates: (the first, the second).
    difference: Option<(Instant, Instant)>,
    /// Date math was done (`+ 5wd`): without a target, only the result is shown.
    stepped: bool,
}

/// The date math after `date` (`+ 5wd`, `- 12d`), or the second date of a difference
/// (`- 2026-12-25`).
fn date_math(config: &Config, p: &mut Parser, mut date: Instant) -> Result<Math, TimeError> {
    let mut difference: Option<(Instant, Instant)> = None;
    let mut stepped = false;
    loop {
        p.skip_ws();
        let sign = match p.rest().chars().next() {
            Some('+') => 1,
            Some('-') => -1,
            _ => break,
        };
        p.pos += 1;
        if let Some((amount, symbol)) = p.duration(config) {
            if difference.is_some() {
                return usage("a difference is a duration: nothing can be added to it");
            }
            date = add(date, amount, symbol, sign)?;
            stepped = true;
        } else if sign == -1 && difference.is_none() {
            let other = p.value(false)?;
            difference = Some((date, other));
        } else {
            return usage(format!("expected a duration (like 3d or 90min) after `{}`", if sign > 0 { '+' } else { '-' }));
        }
    }
    Ok(Math { date, difference, stepped })
}

/// The difference of two dates, `a - b`: broken down (no unit), or in the time unit `unit`
/// (`wd`: the working days between their days).
fn difference_in(config: &Config, a: Instant, b: Instant, unit: &str, precision: usize) -> Result<String, TimeError> {
    if unit.is_empty() {
        return Ok(breakdown(a.ns - b.ns));
    }
    let symbol =
        time_unit(config, unit).ok_or_else(|| TimeError::Usage(format!("`{unit}`: a difference of dates is a duration; give a time unit (d, h, s…)")))?;
    if symbol == WORKDAY {
        // Working days between the two days (each in its own zone), times of day aside.
        let day = |i: &Instant| {
            let c = i.civil();
            days_from_civil(c.year, c.month, c.day)
        };
        return Ok(format!("{} {WORKDAY}", workdays_between(day(&b), day(&a))));
    }
    let seconds = (a.ns - b.ns) as f64 / NS as f64;
    let (value, unit) = convert::convert(config, seconds, "s", symbol).map_err(|_| TimeError::Range)?;
    Ok(format!("{} {}", convert::format_value(value, precision), unit.symbol))
}

/// A date (no difference) in the format `target`, or without one as written.
fn date_result(p: &mut Parser, date: &Instant, target: &str, stepped: bool, seconds: bool) -> Result<String, TimeError> {
    match target {
        // No target: the format the date was written in. After date math only the result
        // (`2026/12/04 + 5wd` → `2026/12/11 YYYY/MM/DD`); a date alone is read into ISO too, as
        // its own format would only repeat it (`1791644400 unix` → `… iso = 1791644400 unix`).
        // `now` and plain `iso`: ISO.
        // A date alone (no math): its ISO reading (with --verbose, both sides).
        // Not verbose, the date for people (`12 Sept 26`, `12 Sept 26 20:00 GMT+2`) wherever it would
        // be ISO: a date alone, `now`, an ISO date after date math.
        "" => match p.input_format.take() {
            Some(word) if word != "iso" && stepped && (p.verbose || !word.starts_with("iso")) => Ok(format!("{} {word}", format_as(date, &word, seconds)?)),
            Some(word) if word != "iso" && p.verbose => both_sides(date, &word, seconds),
            _ if p.verbose => Ok(format!("{} iso", format_as(date, "iso", seconds)?)),
            _ => Ok(human(date, date_only(date, p.s))),
        },
        "iso" => Ok(format!("{} iso", format_as(date, "iso", seconds)?)),
        // A target: only the converted value (with --verbose, the ISO side too).
        word if is_format(word) && p.verbose => both_sides(date, word, seconds),
        word if is_format(word) => Ok(format!("{} {word}", format_as(date, word, seconds)?)),
        // A time being typed after the date (`2026-12-04 10`, `10:`): not a format.
        word if word.starts_with(|c: char| c.is_ascii_digit()) && word.bytes().all(|b| b.is_ascii_digit() || b == b':') => {
            usage(format!("`{word}`: write the time as H:MM, like 10:00 (then AM / PM if 12-hour)"))
        }
        word => usage(format!("unknown date format `{word}` (formats: {})", DATE_WORDS[3..].join(", "))),
    }
}

/// A date shown in a city's zone: `<date> iso = <date there> Paris`, or with a source city
/// `<date> New York = <date there> Paris`, or for the city alone (now) only the time there. To the
/// minute, unless the request wrote seconds.
/// `alone`: the city alone (now there). Not `verbose`: the time there for people (`11 Oct 26 16:00
/// GMT+2 Paris`).
pub(super) fn city_result(
    cities: &Cities,
    date: Instant,
    difference: bool,
    target: &str,
    alone: bool,
    verbose: bool,
    seconds: bool,
) -> Result<String, TimeError> {
    let to = cities.target;
    if difference {
        return usage(format!("a difference of dates is a duration: it has no time in {}", to.name));
    }
    if !target.is_empty() {
        return usage(format!("`{target}`: the city is the target (`10:00 {}`)", to.name));
    }
    let date = if seconds { date } else { Instant { ns: date.ns - date.ns.rem_euclid(60 * NS), ..date } };
    let iso = |zone: Zone| format_as(&Instant { zone, ..date }, "iso", seconds);
    if !verbose {
        return Ok(format!("{} {}", human(&Instant { zone: Zone::Named(to.tz), ..date }, false), to.name));
    }
    let there = format!("{} {}", iso(Zone::Named(to.tz))?, to.name);
    Ok(match cities.source {
        _ if alone => there,
        Some(from) => format!("{} {} = {there}", iso(Zone::Named(from.tz))?, from.name),
        None => format!("{} iso = {there}", iso(date.zone)?),
    })
}

/// Whether a date is shown without its time: midnight, and the request wrote no time (`12 Sept 2026`,
/// `2026-12-04`, a Unix timestamp at midnight), and not `now`.
pub(super) fn date_only(date: &Instant, request: &str) -> bool {
    let c = date.civil();
    let midnight = c.hour == 0 && c.minute == 0 && c.second == 0 && c.nano == 0;
    let b = request.as_bytes();
    let writes_time = (1..b.len()).any(|i| b[i] == b':' && b[i - 1].is_ascii_digit());
    midnight && !writes_time && !request.contains("now")
}

/// The words at the start of `words` that are a time of day (`10:00AM`, `10 pm`), if any.
pub(super) fn leading_time(words: &[&str]) -> Option<usize> {
    (1..=words.len().min(3)).rev().find(|&n| read_time_only(&words[..n].join(" "), (2000, 1, 1)).is_some())
}

/// The countries of a request, each written as its city: `Ok` with the request rewritten
/// (`10:00 Italy to Japan` → `10:00 Rome to Tokyo`) and a note, or the choice of a country with
/// several time zones (the first such one), each with the request using that zone's city.
/// `None`: no country. A city's name (`Mexico City`, `Singapore`) or a unit (`us`) is not one.
/// The first country named in `words` from word `from` on: at each word its longest name, unless a
/// city's name is longer or as long (`Mexico City`); never a unit, format or currency word. Its
/// first word, its number of words, the words as written, and the country.
pub(super) fn next_country(config: &Config, words: &[&str], from: usize) -> Option<(usize, usize, String, &'static tz::Country)> {
    (from..words.len()).find_map(|i| {
        let longest = tz::longest_country().max(tz::longest_name()).min(words.len() - i);
        for n in (1..=longest).rev() {
            let span = words[i..i + n].join(" ");
            if tz::city(&span).is_some() {
                return None;
            }
            if let Some(c) = tz::country(&span).filter(|_| !reserved_word(config, &span)) {
                return Some((i, n, span, c));
            }
        }
        None
    })
}

pub(super) fn countries_in(config: &Config, input: &str) -> Option<Result<(String, String), Ambiguity>> {
    let words: Vec<&str> = input.split_whitespace().collect();
    let mut out: Vec<String> = Vec::new();
    let mut named: Vec<String> = Vec::new();
    let mut i = 0;
    while let Some((at, n, span, country)) = next_country(config, &words, i) {
        out.extend(words[i..at].iter().map(ToString::to_string));
        i = at;
        if let [one] = country.cities {
            named.push(format!("{} is {one}", country.name));
            out.push(one.to_string());
            i += n;
            continue;
        }
        // Several zones: one choice per zone, with its most relevant city and its offset now.
        let now = unix_now();
        let choices = country
            .cities
            .iter()
            .filter_map(|city| {
                let tz = tz::city(city)?.ok()?.tz;
                let offset = tz.offset_at(now);
                let gmt = match (offset.abs() / 3600, offset.abs() / 60 % 60) {
                    (0, 0) => "GMT".to_string(),
                    (h, 0) => format!("GMT{}{h}", if offset < 0 { '-' } else { '+' }),
                    (h, m) => format!("GMT{}{h}:{m:02}", if offset < 0 { '-' } else { '+' }),
                };
                let request = [&out.iter().map(String::as_str).collect::<Vec<_>>()[..], &[*city][..], &words[i + n..]].concat().join(" ");
                Some(Choice { pattern: city.to_string(), date: format!("{gmt} · {}", tz.name), request })
            })
            .collect::<Vec<_>>();
        // None of its zones installed: its first city, which then says which zone is missing,
        // as `now Tokyo` does; a question needs choices (bug 011).
        if choices.is_empty() {
            named.push(format!("{} is {}", country.name, country.cities[0]));
            out.push(country.cities[0].to_string());
            i += n;
            continue;
        }
        return Some(Err(Ambiguity { text: span, choices, kind: AmbiguityKind::Country, note: String::new() }));
    }
    out.extend(words[i..].iter().map(ToString::to_string));
    (!named.is_empty()).then(|| Ok((out.join(" "), named.join(", "))))
}

/// Words that are not misspelt cities: units, formats, months, keywords.
pub(super) fn known_word(config: &Config, text: &str) -> bool {
    tz::city(text).is_some() || tz::country(text).is_some() || reserved_word(config, text)
}

/// Words that are neither places nor misspelt ones: units, formats, months, keywords, currencies.
pub(super) fn reserved_word(config: &Config, text: &str) -> bool {
    convert::find_unit(config, text).is_some() || date_word(text) || crate::currency::exact(text).is_some_and(|_| tz::country(text).is_none())
}

/// Whether a word is something else than a place to aequalis: a unit, a currency (not a country), a
/// format, a month, a keyword (`geo.rs`: such a word is not a city unless qualified).
pub fn reserved(config: &Config, text: &str) -> bool {
    convert::find_unit(config, text).is_some() || date_word(text) || crate::currency::exact(text).is_some_and(|_| tz::country(text).is_none())
}

/// Whether a word means a date or a place to aequalis: a format, a month, a keyword, a number word, a
/// time zone, a city or a country (`currency.rs` checks its names against it).
#[cfg(test)]
pub fn date_or_place_word(config: &Config, text: &str) -> bool {
    let _ = config;
    date_word(text) || tz::city(text).is_some() || tz::country(text).is_some()
}

/// Formats, months, keywords, number words, time zones.
pub(super) fn date_word(text: &str) -> bool {
    is_format(text)
        || DATE_WORDS.contains(&text)
        || month_named(text).is_some()
        || gmt_zone(text).is_some()
        || crate::config::is_number_word(text)
        || ["to", "of", "de", "del", "the", "am", "pm", "a.m.", "p.m."].contains(&fold(text).as_str())
}

/// A place a misspelt word may be: its name, its zone (or "several time zones"), and whether it is
/// a country.
pub(super) struct NearPlace {
    name: String,
    zone: String,
    country: bool,
}

/// The cities and countries within 2 letters of `text` (`Tokio`: Tokyo; `jappan`: Japan).
pub(super) fn near_places(text: &str) -> Vec<NearPlace> {
    let cities = tz::near(text).into_iter().map(|c| NearPlace { name: c.name.to_string(), zone: c.tz.name.to_string(), country: false });
    let countries = tz::near_countries(text).into_iter().map(|c| {
        let zone = match c.cities {
            [one] => tz::city(one).and_then(Result::ok).map_or_else(|| one.to_string(), |city| city.tz.name.to_string()),
            _ => "several time zones".to_string(),
        };
        NearPlace { name: c.name.to_string(), zone, country: true }
    });
    // Only the nearest, cities and countries together: `Tokio` is Tokyo (1 letter), not Togo (2).
    let all: Vec<(usize, NearPlace)> = cities
        .chain(countries)
        .map(|p| {
            let names: Vec<&str> = match tz::country(&p.name) {
                Some(c) if p.country => std::iter::once(c.name).chain(c.aliases.iter().copied()).collect(),
                _ => vec![p.name.as_str()],
            };
            (tz::distance(text, &names), p)
        })
        .collect();
    let best = all.iter().map(|(d, _)| *d).min().unwrap_or(0);
    all.into_iter().filter(|(d, _)| *d == best).map(|(_, p)| p).collect()
}

/// A request that failed because of misspelt places (`10:00 Tokio`, `New Yrok to Paris`,
/// `10:00am jappan romme`): every run of up to 3 words, left to right, that is not a unit, format,
/// month or keyword and is within **2 letters** of a city or a country (case and spaces aside) is
/// corrected; the corrections, together, whose request works are an `Ambiguous` error to choose
/// from (at most 8). `None`: no such places.
pub(super) fn place_typos(config: &Config, input: &str, precision: usize, verbose: bool) -> Option<TimeError> {
    // A calculation is no misspelt place (`pi-pi`, not Dili).
    if crate::calc::parse_prefix(input).is_ok_and(|(_, rest)| rest.trim().is_empty()) {
        return None;
    }
    let words: Vec<&str> = input.split_whitespace().collect();
    let spans = misspelt_spans(config, &words);
    if spans.is_empty() {
        return None;
    }
    let choices = corrections(config, &words, &spans, precision, verbose);
    if choices.is_empty() {
        return None;
    }
    let misspelt: Vec<String> = spans.iter().map(|(i, n, _)| words[*i..i + n].join(" ")).collect();
    let note = misspelt_note(&spans, &misspelt, &choices);
    Some(TimeError::Ambiguous(Ambiguity { text: misspelt.join(", "), choices, kind: AmbiguityKind::City, note }))
}

/// A misspelt run of words: (first word, word count, what they may be).
type Span = (usize, usize, Vec<NearPlace>);

/// The misspelt runs of `words`, left to right, the longest first at each word.
fn misspelt_spans(config: &Config, words: &[&str]) -> Vec<Span> {
    // A word, not an operator (`pi - pi`) or a connector (`42 mpg in km/l`).
    let wordlike =
        |w: &&str| w.chars().any(char::is_alphabetic) && w.chars().all(|c| c.is_alphabetic() || "'’-.".contains(c)) && !crate::currency::connector(w);
    let mut spans: Vec<Span> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let found = (1..=(words.len() - i).min(3)).rev().find_map(|n| {
            let span = &words[i..i + n];
            if !span.iter().all(wordlike) {
                return None;
            }
            let text = span.join(" ");
            if known_word(config, &text) {
                return None;
            }
            let near = near_places(&text);
            (!near.is_empty()).then_some((n, near))
        });
        match found {
            Some((n, near)) => {
                spans.push((i, n, near));
                i += n;
            }
            None => i += 1,
        }
    }
    spans
}

/// Every combination of corrections (at most 8), each tried as a request: those that work.
fn corrections(config: &Config, words: &[&str], spans: &[Span], precision: usize, verbose: bool) -> Vec<Choice> {
    let mut combos: Vec<Vec<&NearPlace>> = vec![Vec::new()];
    for (_, _, near) in spans {
        combos = combos.iter().flat_map(|c| near.iter().map(move |p| [c.as_slice(), &[p]].concat())).take(8).collect();
    }
    // Several corrections: each named with what it is (`Japan (country), Rome (city)`).
    let named = |p: &&NearPlace| match (spans.len(), p.country) {
        (1, _) => p.name.clone(),
        (_, true) => format!("{} (country)", p.name),
        (_, false) => format!("{} (city)", p.name),
    };
    combos
        .into_iter()
        .filter_map(|combo| {
            let mut out: Vec<&str> = Vec::new();
            let mut at = 0;
            for ((start, n, _), place) in spans.iter().zip(&combo) {
                out.extend(&words[at..*start]);
                out.push(&place.name);
                at = start + n;
            }
            out.extend(&words[at..]);
            let request = out.join(" ");
            let works = matches!(request_with(config, &request, precision, verbose, false), Some(Ok(_)) | Some(Err(TimeError::Ambiguous(_))));
            works.then(|| Choice {
                pattern: combo.iter().map(named).collect::<Vec<_>>().join(", "),
                date: combo.iter().map(|p| p.zone.as_str()).collect::<Vec<_>>().join(", "),
                request,
            })
        })
        .collect()
}

/// What the misspelt words are not: for a single one, what the places offered are (`Tokio`: a
/// city, not Togo).
fn misspelt_note(spans: &[Span], misspelt: &[String], choices: &[Choice]) -> String {
    let offered = |p: &&NearPlace| choices.iter().any(|c| c.pattern.split(", ").any(|name| name == p.name || name.starts_with(&format!("{} (", p.name))));
    match spans {
        [(_, _, near)] => {
            let near: Vec<&NearPlace> = near.iter().filter(offered).collect();
            let what = match (near.iter().all(|p| p.country), near.iter().any(|p| p.country)) {
                (true, _) => "a country",
                (false, false) => "a city",
                _ => "a known place",
            };
            format!("`{}` is not {what}", misspelt[0])
        }
        _ => format!("{} are not known places", misspelt.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(", ")),
    }
}

/// Whether a request names a city (`Paris`, `10:00AM New York to Paris`), for the stdin rule.
pub fn names_city(input: &str) -> bool {
    let words: Vec<&str> = input.split_whitespace().collect();
    take_zone(input).is_ok_and(|(rest, _)| !matches!(take_cities(&rest), Ok((_, None)))) || next_country(Config::shared(), &words, 0).is_some()
}
