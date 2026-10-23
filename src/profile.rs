//! Profiling (the `profiling` setting, off by default): how long each keystroke's answer and
//! suggestions take, measured with two clocks: inside the library (`core`, `timed`) and around the
//! call in the app (`app`, its round trip: JNI, the thread, the JSON). About shows the slowest and
//! the median of both: the difference is what isn't the library's. Kept for the process, since
//! profiling was turned on or `reset`.

use std::sync::Mutex;
use std::time::Instant;

/// What a call answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The answer to the request (`reply::view`).
    Answer,
    /// The suggestions for the word being typed.
    Suggestions,
}

impl Kind {
    /// As the app names it (`Core.profile`): `answer`, `suggestions`.
    pub fn of(name: &str) -> Option<Kind> {
        match name {
            "answer" => Some(Kind::Answer),
            "suggestions" => Some(Kind::Suggestions),
            _ => None,
        }
    }
}

/// One call: what was asked, and its times in µs.
#[derive(Debug, Clone, PartialEq)]
struct Call {
    request: String,
    core_us: u64,
    app_us: Option<u64>,
}

impl Call {
    /// The time it is judged by: the app's when it said it, else the core's.
    fn felt(&self) -> u64 {
        self.app_us.unwrap_or(self.core_us)
    }
}

/// The calls of one kind: their times (for the median), the latest and the slowest.
#[derive(Debug, Default)]
struct Calls {
    count: u64,
    core: Vec<u64>,
    app: Vec<u64>,
    last: Option<Call>,
    slowest: Option<Call>,
}

/// The samples kept for a median: the latest, at most this many.
const SAMPLES: usize = 10_000;

fn push(samples: &mut Vec<u64>, v: u64) {
    if samples.len() >= SAMPLES {
        samples.drain(..SAMPLES / 2);
    }
    samples.push(v);
}

impl Calls {
    const EMPTY: Calls = Calls { count: 0, core: Vec::new(), app: Vec::new(), last: None, slowest: None };

    fn core(&mut self, request: &str, us: u64) {
        self.count += 1;
        push(&mut self.core, us);
        let call = Call { request: request.to_string(), core_us: us, app_us: None };
        if self.slowest.as_ref().is_none_or(|s| s.app_us.is_none() && us > s.core_us) {
            self.slowest = Some(call.clone());
        }
        self.last = Some(call);
    }

    /// The app's round trip of `request`: the latest call's, when it is that request's (a key
    /// pressed meanwhile made a newer one).
    fn app(&mut self, request: &str, us: u64) {
        let Some(last) = self.last.as_mut().filter(|l| l.app_us.is_none() && l.request == request) else { return };
        last.app_us = Some(us);
        push(&mut self.app, us);
        if self.slowest.as_ref().is_none_or(|s| s.app_us.is_none() || us > s.felt()) {
            self.slowest = Some(last.clone());
        }
    }
}

static RECORD: Mutex<[Calls; 2]> = Mutex::new([Calls::EMPTY, Calls::EMPTY]);

fn with<T>(kind: Kind, f: impl FnOnce(&mut Calls) -> T) -> T {
    let mut record = RECORD.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut record[kind as usize])
}

/// Whether the `profiling` setting is on.
pub fn on() -> bool {
    crate::settings::current().get("profiling") == "true"
}

/// `reply()` timed when profiling is on, its time added to it as `"core_us":N` (the app reports its
/// round trip with `app`); as it is when off.
pub fn timed(kind: Kind, request: &str, reply: impl FnOnce() -> String) -> String {
    if !on() {
        return reply();
    }
    let started = Instant::now();
    let out = reply();
    let us = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    with(kind, |c| c.core(request, us));
    match out.strip_suffix('}') {
        Some(head) if head.len() > 1 => format!("{head},\"core_us\":{us}}}"),
        Some(head) => format!("{head}\"core_us\":{us}}}"),
        None => out,
    }
}

/// The app's round trip of its call of `kind` for `request`, in µs: around the call, the thread hops
/// and the JSON read too.
pub fn app(kind: Kind, request: &str, us: u64) {
    if on() {
        with(kind, |c| c.app(request, us));
    }
}

/// Everything recorded forgotten: About starts over.
pub fn reset() {
    let mut record = RECORD.lock().unwrap_or_else(|e| e.into_inner());
    *record = Default::default();
}

/// `84 ms`, `3.2 ms`, `0.4 ms`.
fn ms(us: u64) -> String {
    let ms = us as f64 / 1000.0;
    match ms {
        m if m >= 10.0 => format!("{m:.0} ms"),
        m => format!("{m:.1} ms"),
    }
}

fn median(samples: &[u64]) -> Option<u64> {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted.get(sorted.len() / 2).copied()
}

/// The two clocks of a time: `84 ms in the app, 3 ms in the core`, or the core's alone.
fn both(app: Option<u64>, core: u64) -> String {
    match app {
        Some(app) => format!("{} in the app, {} in the core", ms(app), ms(core)),
        None => format!("{} in the core", ms(core)),
    }
}

/// About's lines when profiling is on: the slowest keystroke, then how many and their median,
/// for the answers and the suggestions.
pub fn facts() -> Vec<(&'static str, String)> {
    if !on() {
        return Vec::new();
    }
    let record = RECORD.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = Vec::new();
    for (kind, slowest, count) in [(Kind::Answer, "Slowest answer", "Answers"), (Kind::Suggestions, "Slowest suggestions", "Suggestions")] {
        let c = &record[kind as usize];
        let Some(s) = &c.slowest else {
            if kind == Kind::Answer {
                out.push(("Profiling", "on: nothing typed yet".to_string()));
            }
            continue;
        };
        out.push((slowest, format!("{} (`{}`)", both(s.app_us, s.core_us), s.request)));
        let medians = both(median(&c.app), median(&c.core).unwrap_or(0));
        out.push((count, format!("{}, median {medians}", c.count)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slowest_by_the_app_clock_and_the_medians() {
        let mut c = Calls::default();
        c.core("10 km mi", 300);
        c.app("10 km mi", 2_000);
        c.core("10:00 Rome to Tokyo", 3_000);
        c.app("10:00 Rome to Tokyo", 84_000);
        c.core("12+4", 100);
        c.app("12+4", 1_000);
        let s = c.slowest.clone().unwrap();
        assert_eq!((s.request.as_str(), s.core_us, s.app_us), ("10:00 Rome to Tokyo", 3_000, Some(84_000)));
        assert_eq!((c.count, median(&c.core), median(&c.app)), (3, Some(300), Some(2_000)));
        // A round trip comes once per call, and only for the latest request.
        c.app("12+4", 999_000);
        c.core("12+45", 100);
        c.app("12+4", 999_000);
        assert_eq!(c.slowest.unwrap().app_us, Some(84_000));
    }

    #[test]
    fn without_the_app_the_core_clock() {
        let mut c = Calls::default();
        c.core("a", 500);
        c.core("b", 900);
        c.core("c", 200);
        assert_eq!(c.slowest.unwrap().request, "b");
    }

    #[test]
    fn times_for_people() {
        assert_eq!((ms(84_000), ms(3_200), ms(400), ms(12)), ("84 ms".to_string(), "3.2 ms".to_string(), "0.4 ms".to_string(), "0.0 ms".to_string()));
        assert_eq!(both(Some(84_000), 3_000), "84 ms in the app, 3.0 ms in the core");
    }

    #[test]
    fn off_nothing_is_timed() {
        let _m = crate::machine::Mock::italy().install();
        assert!(!on());
        assert_eq!(timed(Kind::Answer, "x", || "{\"ok\":true}".into()), "{\"ok\":true}");
        assert!(facts().is_empty());
    }

    #[test]
    fn on_the_reply_says_its_time() {
        let _m = crate::machine::Mock::italy().install();
        crate::settings::change("profiling", Some("true")).unwrap();
        let reply = timed(Kind::Suggestions, "10 k", || "{\"start\":3,\"items\":[]}".into());
        assert!(reply.starts_with("{\"start\":3,\"items\":[],\"core_us\":") && reply.ends_with('}'), "{reply}");
        assert!(facts().iter().any(|(label, _)| *label == "Slowest suggestions"), "{:?}", facts());
        reset();
        assert_eq!(facts(), [("Profiling", "on: nothing typed yet".to_string())]);
    }
}
