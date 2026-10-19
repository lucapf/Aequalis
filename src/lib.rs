//! aequalis: an ultra lightweight unit converter, calculator, date and distance tool. The library
//! is everything but the terminal: the requests (`convert_request`) and the command line's options
//! (`run`); `main.rs` is the `aequalis` command around it, with the interactive mode (`tui`).

pub mod about;
pub mod alloc;
pub mod calc;
pub mod cities;
pub mod completion;
pub mod config;
pub mod convert;
pub mod currency;
#[cfg(feature = "ffi")]
pub mod ffi;
pub mod files;
pub mod geo;
pub mod http;
#[cfg(all(feature = "ffi", target_os = "android"))]
pub mod jni;
pub mod json;
pub mod locale;
pub mod network;
pub mod notify;
pub mod numbers;
pub mod online;
pub mod planets;
pub mod settings;
pub mod suggest;
pub mod time;
pub mod trivia;
#[cfg(windows)]
pub mod windows;

use config::Config;

const USAGE: &str = "\
Usage:
  aequalis <value> <from> <to> [--precision N] [--silent]
  aequalis <value><from> <to> [--precision N] [--silent]
  aequalis <expression> [<from> <to>] [--precision N] [--silent]
  <input> | aequalis [<from>] [<to>] [options]
  aequalis                 (no arguments, in a terminal) interactive mode
  aequalis --list          units and currencies
  aequalis --trivia        useful values (Voyagers' distance live when online)
  aequalis --online        online, offline or offline (forced): the last download of the online data
  aequalis --about         the logo, the version, memory, the online data and their sources
  aequalis --sync          download all the online data now (the city archive, the exchange
                           rates, the Trivia's live values); otherwise as the refresh
                           settings say
  aequalis --settings      every setting, its value and what it does
  aequalis --set key=value change a setting (`aequalis --set key`: back to its default)
  aequalis --autocompletion <bash|zsh>
  aequalis --help | --version

Examples:
  aequalis 10 km mi          6.2137 mi
  aequalis 100 C F           212 F
  aequalis 1 gal-uk l        4.5461 l
  aequalis 100 Mb MB         12.5 MB
  aequalis 100Gb Kb          100000000 kb
  aequalis -40C F            -40 F
  aequalis 1 kg lb --precision 2
  echo \"100F C\" | aequalis   37.7778 C
  echo 100 | aequalis F C    37.7778 C
  echo 100F | aequalis C     37.7778 C
  aequalis 10^2              100
  aequalis [100+20]/120      1
  aequalis 1/3               0.3333
  aequalis 100 + 20 + 50 / 10 sqft m2
                                 11.6129 m2  (with -v, stderr: evaluated as (100 + 20) + (50 / 10))
  aequalis 90 min h          1.5 h
  aequalis 100 km/h mph      62.1371 mph
  aequalis 1 bar psi         14.5038 psi
  aequalis 5 l/100km mpg     47.0429 mpg
  aequalis 1 kWh MJ          3.6 MJ
  aequalis 100 hp kW         74.57 kW
  aequalis 12 usd euro       10.73 EUR   (rates synced daily from ExchangeRate-API)
  aequalis 12 USA Europe     10.73 EUR   (countries: their currency; any case, plurals,
  aequalis 12usd dhs         44.07 AED    symbols, abbreviations, amount attached)
  aequalis 12 usd            10.73 EUR   (one currency: to the local one, currency.local)
  aequalis 9.81 m/s^2 ft/s2  32.185 ft/s2
  aequalis 100 Mb/s MB/s     12.5 MB/s
  aequalis sqrt 16           4
  aequalis sin(30°)          0.5
  aequalis 30 deg rad        0.5236 rad
  aequalis 1 ly km           9460730472580.8 km
  aequalis 150 + 20%         180
  aequalis 20% of 150        30
  aequalis Rome Paris        1,107 km    (distance.unit: km, or mi where the device is imperial)
  aequalis Springfield, Illinois Chicago mi
                                 176 mi      (any city once downloaded: aequalis --sync)
  aequalis Italy             11 Oct 26 12:38 PM GMT+2 Rome
  aequalis 10:00 USA to Japan
                                 (asks which US time zone: New York, Chicago, …)
  aequalis 'log(1000) + ln(e)'
                                 4
  aequalis now + 3d          13 Oct 26 5:00 PM GMT+2
  aequalis 12 Sept 2026      12 Sept 26
  aequalis 2026/04/12 date unix
                                 1775944800 unix
  aequalis -v 2026/04/12 date unix
                                 2026-04-12T00:00+02:00 iso = 1775944800 unix
  aequalis 1791644400 unix iso-week
                                 2026-W41-6 iso-week
  aequalis 2026-12-25 - now d
                                 75.3381 d

When no value is given as an argument, each non-empty line of stdin is one
request; its words come before the argument words.

The value can be an arithmetic expression: + - * / ** with ( ), standard
precedence. Shell-safe spellings that need no quotes: x for *, ^ for **,
[ ] or { } for ( ). With two or more operations, the grouping used is shown
on stderr (turned off by --silent). Without units, only the result is printed.
Functions: sqrt, log (log10), ln (loge), sin, cos, tan, sinh, cosh, tanh, with
the argument in brackets (sqrt(16)) or after a space (sqrt 16). Constants: e,
pi. Angles are radians; in a function's argument 30° or 30deg is in degrees
(sin(30°)); elsewhere deg, rad, grad, arcmin, arcsec, turn are angle units.
Percentages: 20% is 0.2; 150 + 20% = 180, 150 - 20% = 120; 20% of 150 = 30.
Results that would round to 0, or have more than 15 significant digits, are
shown in scientific notation (1 eV J → 1.6022e-19 J).

Distances on the Earth: [distance] <place> [to] <place> [length unit]: two places,
then the unit (default: the distance.unit setting). A place: a city (with the city
archive, any city of 5,000 inhabitants or more, by any of its names: `Springfield,
Illinois` tells one apart; several with that name are asked about), a country, or
coordinates (41.9,12.5).
or just <place> to <place> [unit] when no time is written (Rome to Doha; with a
time, 10:00 Rome to Doha or now Rome to Doha, it is the time there),
along the WGS84 ellipsoid; a place is a capital, a major city or coordinates
(41.9,12.5 or 41.9N 12.5E).

Countries work like cities in time requests (Italy, 10:00 Japan to Brazil); a
country with several time zones (USA, Russia, Canada, …) asks which one, each
shown with its most relevant city and its offset now.

Compound units: known units joined by / and *, with powers (^2, 2, ²) and a
number before a unit: km/h, m/s^2, kg/m3, Mb/s, mi/gal, l/100km, 1/min.
Fuel economy also converts to its reciprocal (mpg <-> l/100km).

Dates: ISO 8601 (2026-10-10, 2026-W41-6, 2026-283T15:00, 20261010T150405Z,
offsets Z / +02:00; no zone = local time, or the zone of a GMT+2 / UTC-05:30 word:
2026/12/04 15:30 GMT+2 unix), now, today, `<n> unix|unix-ms|unix-us|
unix-ns`, `<text> local` (the locale's format), `<text> date` (2026/04/12,
2026.4.12 15:30, 12/04/2026 in the locale's day/month order), also with the month's
name or abbreviation in English, Spanish, French, Italian, German or Portuguese
(12 Sept 2026, September 12, 2026, 12 de septiembre de 2026, 1er janvier 2026). A date result is the
converted value only (with -v, the ISO side too: `<date> iso = <value> <format>`).
Without a target, dates are shown for people (12 Sept 26, 12 Sept 26 8:00 PM GMT+2;
see the format.date / format.datetime settings), and after date math on a written
pattern in that pattern (2026/12/04 + 5wd →
2026/12/11 YYYY/MM/DD). A written date with a time is a date on its own
(2026/12/04 10:00 PM, 2026-12-04 22:00). ISO results leave out seconds the input
did not write when they are zero (2026-12-04T22:00+01:00). Patterns, for output and (before the
target) input: YYYY/MM/DD YY/MM/DD DD/MM/YY DD/MM/YYYY MM/DD/YYYY MM/DD/YY, each
also with -24h, -24h-tz, -12h, -12h-tz. A written date with only a target is
guessed (04/13/26 3:20PM unix); with -v, stderr notes how it was read. Numbers: an ISO
shape followed by / * ^ x is arithmetic (2026-12-02/12); the num target (n,
number) forces a number (2026-12-02 num → 2012). An ambiguous
one (12/04/2026: April or December?) is asked at a terminal, an error elsewhere. Output formats: iso (default,
local zone), iso-utc, iso-basic, iso-date, iso-time, iso-ordinal, iso-week, unix,
unix-ms, unix-us, unix-ns, local. Date math: `+`/`-` durations (ns us ms s min h
are exact; d wk mo yr follow the calendar; wd = working days, Monday to Friday:
2026-10-05 + 5wd = 2026-10-12). date - date is a duration: in a time unit, in wd
(working days between), or `75d 9h`. Durations convert like other units (1 yr = 365.25 d).

Cities: every country's capital and major cities (or a zone name: Europe/Paris), from
the system's time zone database. A city at the end is the target, one before `to`
the source; a time alone is today (24-hour unless AM/PM). A city misspelt by up to
2 letters (Tokio) is suggested: asked at a terminal, an error with the fix elsewhere:
  aequalis Paris                        the time in Paris now
  aequalis 10:00AM Paris                my 10:00 today, in Paris
  aequalis 10:00AM New York to Paris    New York's 10:00, in Paris
  aequalis Rome to Doha                 the distance: 4,019 km (no time)
  aequalis 2026/12/04 10:00 PM Tokyo    any date, in Tokyo

Interactive mode (no arguments, in a terminal), Vim style. Input mode (the
start): type an expression, Enter evaluates, Tab completes units, Up/Down
recall the history, Esc switches to command mode. Command mode: M lists the
unit classes, a class its units (h j k l or arrows move, Tab switches tab,
Enter writes the unit, Esc goes back), N writes `now`, C shows the keyboard
(clicked with the mouse), I (or i) returns to input mode, E shows all the
shortcuts, q quits.

Options:
  --precision N             Decimal places, 0-15 (default: the precision setting, 4); trailing
                            zeros are trimmed
  -s, --silent              Don't warn about mixing bits/bytes or 1000/1024-based units
  -v, --verbose             Also show the notes (how a date was read, how an expression
                            was grouped) and dates in ISO beside the result
                            (`<iso> iso = <value> <format>`); also in the interactive mode
  --json                    One JSON object per request on stdout, for programs (never asks)
  --list                    List all supported units
  --autocompletion SHELL    Print a bash or zsh completion script to stdout. It also
                            hooks Enter to quote expressions, so `aequalis (1+2)*3`
                            works without quotes in an interactive shell
  --quote-line LINE         Print LINE with its expression quoted (used by the hook)
  -h, --help                Show this help
  -V, --version             Show version

Settings (~/.config/aequalis/settings.json; `aequalis --settings` shows them, `aequalis --set
key=value` changes one, the interactive mode's Settings window too; nothing is read
from the environment):
  offline                   true: no network at all (online data stays as last
                            downloaded); default false
  cities.enabled            distances between any city (the city archive); default true
  cities.refresh            silent (downloaded in the background), notify (a notice
  rates.refresh             when there is something new) or manual (aequalis --sync):
  live.refresh              cities manual, rates (exchange rates) silent, live (the
                            Trivia's live values) manual
  cities.interval           how often each is checked or refreshed: 30d, 24h, 24h
  rates.interval / live.interval
  cities.url                where the city archive comes from
  currency.local            the local currency for `12 usd` (default auto: the time
                            zone's country)
  distance.unit             auto (miles where the device is imperial), km or mi
  precision                 decimal places of numbers, 0-15 (default 4; currencies 2)
  format.date               how dates are shown (default `DD MMM YY`: 12 Sept 26)
  format.datetime           with a time (default `DD MMM YY h:mm AM tz`). Tokens:
                            YYYY YY MMMM MMM MM M DD D dddd ddd H24/HH H H12/hh h
                            mm ss AM/A tz (GMT+2) ZZ (+02:00); [text] is literal";

/// The longest request, in characters: far more than anyone types, and a bound for what each
/// stage reads (a paste of a page is an error, not a long wait). The apps stop the request line
/// there too.
pub const MAX_INPUT: usize = 512;

/// A request longer than `MAX_INPUT` characters, every character counted (spaces too), is refused
/// before anything else reads it: the first thing each entry point does (`convert_request`,
/// `grouping`, `suggest::suggest`, `ffi::eval`, `ffi::suggest`, the JNI bridge, the command line's
/// lines). At most `MAX_INPUT + 1` characters are looked at, however long the text.
pub fn check_length(text: &str) -> Result<(), CliError> {
    // A character is 1 to 4 bytes: up to `MAX_INPUT` bytes, never too many characters.
    match text.len() > MAX_INPUT && text.chars().nth(MAX_INPUT).is_some() {
        true => Err(too_long()),
        false => Ok(()),
    }
}

/// `check_length` for a request already split into words, which are joined by one space.
fn check_words(words: &[&str]) -> Result<(), CliError> {
    let mut left = MAX_INPUT + 1;
    for (i, word) in words.iter().enumerate() {
        let spent = usize::from(i > 0) + word.chars().take(left).count();
        left = left.saturating_sub(spent);
        if left == 0 {
            return Err(too_long());
        }
    }
    Ok(())
}

/// A word is being typed: Enter not pressed, and no space after the last word. Every UI then shows
/// only a result, not an error or a question, which would call a request on its way wrong (`Rie` →
/// `Rieti to Rome`).
pub fn typing(request: &str, submitted: bool) -> bool {
    !submitted && !request.ends_with(char::is_whitespace)
}

pub(crate) fn too_long() -> CliError {
    CliError::Usage(format!("the request is too long: at most {MAX_INPUT} characters"))
}

const EXPECTED: &str = "expected <value> <from> <to>, <value><from> <to> or an expression without units";

#[derive(Debug, PartialEq)]
pub enum CliError {
    /// Bad invocation: exit code 2.
    Usage(String),
    /// Valid invocation that cannot be converted: exit code 1.
    Convert(convert::ConvertError),
    /// A written date with more than one reading (`12/04/2026`): the operator chooses at a
    /// terminal; elsewhere exit code 2.
    Ambiguous(time::Ambiguity),
    /// Not finished yet, no mistake: what comes next (`10 km`: the unit to convert to; `Rieti`:
    /// the place to go to). A UI shows it as a hint; exit code 2.
    Incomplete(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CliError::Usage(msg) | CliError::Incomplete(msg) => f.write_str(msg),
            CliError::Convert(e) => write!(f, "{e}"),
            CliError::Ambiguous(a) => f.write_str(&a.message()),
        }
    }
}

impl CliError {
    /// Where the command line's help says more (`see aequalis --list`), printed after the message at a
    /// terminal only: the message itself is for every UI (`--json`, the widget, the apps).
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            CliError::Convert(convert::ConvertError::UnknownUnit(_)) => Some("see `aequalis --list`"),
            CliError::Usage(msg) if msg == EXPECTED => Some("see `aequalis --help`"),
            _ => None,
        }
    }

    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::Convert(_) => 1,
            CliError::Usage(_) | CliError::Ambiguous(_) | CliError::Incomplete(_) => 2,
        }
    }
}

fn usage<T>(msg: impl Into<String>) -> Result<T, CliError> {
    Err(CliError::Usage(msg.into()))
}

/// What a successful run prints: `stdout` on stdout, each warning and note on stderr.
#[derive(Debug, Default, PartialEq)]
pub struct Output {
    pub stdout: String,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
    /// Already formatted for people (dates, distances with their separators): the interactive
    /// mode shows it as it is, other numbers get the device's separators there.
    pub formatted: bool,
    /// The currencies countries stood for (also the first note), next to the result in the
    /// interactive mode.
    pub check: Option<String>,
    /// For a currency result: how old the rates are (`2d`), for a UI to say it.
    pub rates_age: Option<String>,
}

impl From<String> for Output {
    fn from(stdout: String) -> Self {
        Output { stdout, ..Output::default() }
    }
}

/// Parses `args` (without the program name) and returns what to print, one result per request.
/// `read_stdin` is only called when the arguments contain no value; it returns `None` if stdin is a terminal.
pub fn run(config: &Config, args: &[String], read_stdin: impl FnOnce() -> Option<String>) -> Vec<Result<Output, CliError>> {
    match parse_options(config, args) {
        Err(e) => vec![Err(e)],
        Ok(Parsed::Info(out)) => vec![Ok(out)],
        Ok(Parsed::Convert(opts)) => {
            let args = &opts.positional;
            if let Err(e) = check_words(args) {
                return vec![Err(e)];
            }
            if !needs_value(config, args) {
                return vec![convert_request(config, &opts, args)];
            }
            let Some(input) = read_stdin() else {
                return vec![convert_request(config, &opts, args)];
            };
            let results: Vec<_> = input
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| {
                    // The line as it is, before its words are split (`convert_request` then checks
                    // the line's words with the arguments').
                    check_length(line)?;
                    let tokens: Vec<&str> = line.split_whitespace().chain(args.iter().copied()).collect();
                    convert_request(config, &opts, &tokens)
                })
                .collect();
            if results.is_empty() {
                return vec![usage("no conversion request on stdin")];
            }
            results
        }
    }
}

pub struct Options<'a> {
    pub positional: Vec<&'a str>,
    pub precision: usize,
    pub silent: bool,
    /// `-v`: the notes (how a date was read, the grouping used) and the ISO side of date results.
    pub verbose: bool,
}

pub enum Parsed<'a> {
    /// `--help`, `--list`, ...: print and exit.
    Info(Output),
    Convert(Options<'a>),
}

pub fn parse_options<'a>(config: &Config, args: &'a [String]) -> Result<Parsed<'a>, CliError> {
    let mut positional: Vec<&str> = Vec::new();
    // Decimal places: `--precision`, else the `precision` setting (4).
    let mut precision = settings::current().precision().min(config.max_precision);
    let mut silent = false;
    let mut verbose = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let arg = arg.as_str();
        // Negative values (`-40`, `-40C`, `-.5`, `-(1+2)`), a lone `-` and the `->` connector are
        // values, not flags.
        if !arg.starts_with('-') || arg.len() == 1 || arg == "->" || calc::starts_expression(arg) {
            positional.push(arg);
            continue;
        }
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) => (f, Some(v)),
            None => (arg, None),
        };
        let mut value = |name: &str| match inline {
            Some(v) => Ok(v.to_string()),
            None => it.next().cloned().ok_or_else(|| CliError::Usage(format!("{name} requires a value"))),
        };
        match flag {
            "-h" | "--help" => return Ok(Parsed::Info(USAGE.to_string().into())),
            "-V" | "--version" => return Ok(Parsed::Info(format!("aequalis {}", env!("CARGO_PKG_VERSION")).into())),
            "--list" => return Ok(Parsed::Info(list(config).into())),
            "--trivia" => return Ok(Parsed::Info(trivia::list(config, precision).into())),
            "--about" => return Ok(Parsed::Info(about::text().into())),
            // For the widget and scripts: `online`, `offline` or `offline (forced)`.
            "--online" => return Ok(Parsed::Info(network::status().label().to_string().into())),
            "-s" | "--silent" => silent = true,
            "-v" | "--verbose" => verbose = true,
            "--autocompletion" => {
                let shell = value("--autocompletion")?;
                return completion::script(config, &shell)
                    .map(|s| Parsed::Info(s.into()))
                    .ok_or_else(|| CliError::Usage(format!("unsupported shell `{shell}` (supported: {})", completion::SHELLS.join(", "))));
            }
            // Used by the Enter hook in the completion scripts; prints the line, quoted if needed.
            "--quote-line" => {
                let line = value("--quote-line")?;
                return Ok(Parsed::Info(completion::quote_line(&line).unwrap_or(line).into()));
            }
            "--precision" => {
                let v = value("--precision")?;
                precision = match v.parse::<usize>() {
                    Ok(p) if p <= config.max_precision => p,
                    _ => return usage(format!("--precision must be an integer from 0 to {}", config.max_precision)),
                };
            }
            _ => return usage(format!("unknown option `{arg}`")),
        }
    }
    Ok(Parsed::Convert(Options { positional, precision, silent, verbose }))
}

/// Converts one request: `<value> <from> <to>`, `<value><from> <to>`, or `<value>` alone (a calculation);
/// `to`, `in` or `->` may stand between the units (`1000 ly to yd`).
/// The value is an expression (`100 + 20 sqft m2`, `[1+2]km mi`); a plain number is the simplest one.
pub fn convert_request(config: &Config, opts: &Options, tokens: &[&str]) -> Result<Output, CliError> {
    check_words(tokens)?;
    let input = tokens.join(" ");
    if input.trim().is_empty() {
        return usage(EXPECTED);
    }
    // Currencies: `12 usd euro`, `12 USA Europe`; one amount (a plain number) and currencies or
    // countries, nothing else (a time with countries is a time request, countries alone a distance).
    // The rates and the local currency are read only for a currency request.
    let context = || currency::Context { rates: currency::load(), now: time::unix_now(), local: currency::local().ok().flatten() };
    if let Some(result) = currency::request(config, &input, context) {
        return match result {
            // The currencies countries stand for are always noted, to check the result.
            Ok(c) => Ok(Output {
                stdout: c.result,
                warnings: if opts.silent { Vec::new() } else { c.warnings },
                notes: match (opts.silent, opts.verbose) {
                    (true, _) => Vec::new(),
                    (false, verbose) => c.check.iter().cloned().chain(if verbose { c.notes } else { Vec::new() }).collect(),
                },
                check: c.check.filter(|_| !opts.silent),
                formatted: false,
                rates_age: Some(c.age),
            }),
            Err(currency::Error::Usage(msg)) => Err(CliError::Usage(msg)),
            Err(currency::Error::Ambiguous(a)) => Err(CliError::Ambiguous(a)),
            Err(currency::Error::NoRates(msg)) => Err(CliError::Convert(convert::ConvertError::Domain(msg))),
            Err(currency::Error::Incomplete(msg)) => Err(CliError::Incomplete(msg)),
        };
    }
    // Distances on the Earth: `distance Rome to Paris km`.
    if let Some(result) = geo::request(config, &input, opts.precision) {
        let notes = |notes: Vec<String>| if opts.verbose && !opts.silent { notes } else { Vec::new() };
        return result.map(|(stdout, n)| Output { stdout, notes: notes(n), formatted: true, ..Output::default() }).map_err(|e| match e {
            geo::Error::Usage(msg) => CliError::Usage(msg),
            geo::Error::Ambiguous(a) => CliError::Ambiguous(a),
            geo::Error::Incomplete(msg) => CliError::Incomplete(msg),
        });
    }
    // Dates: `now + 3d`, `2026-10-10T15:00Z unix`, `1791644400 unix iso-week`. Not a lone number
    // expression (`1-10-10`, `2026-12-02`): that is arithmetic, whatever its shape.
    let arithmetic = time::is_number_expression(&input) && calc::parse_prefix(&input).is_ok_and(|(_, rest)| rest.trim().is_empty());
    if let Some(result) = Some(()).filter(|_| !arithmetic).and_then(|_| time::request(config, &input, opts.precision, opts.verbose)) {
        let notes = |notes: Vec<String>| if opts.verbose && !opts.silent { notes } else { Vec::new() };
        return result.map(|(stdout, n)| Output { stdout, notes: notes(n), formatted: true, ..Output::default() }).map_err(|e| match e {
            time::TimeError::Usage(msg) => CliError::Usage(msg),
            time::TimeError::Range => CliError::Convert(convert::ConvertError::NotFinite),
            time::TimeError::Ambiguous(a) => CliError::Ambiguous(a),
            time::TimeError::Incomplete(msg) => CliError::Incomplete(msg),
        });
    }
    // A written date with no format yet (`12/04/2026`): a date on its way, not
    // the division it reads as (`12/04/2026 num` asks for that).
    if !arithmetic && time::is_date_value(&input) {
        return Err(CliError::Incomplete("now the format to convert to: iso, unix, local…".into()));
    }
    // Not a number: one place so far (`Rieti`, `Rieti to`) is a distance on its way.
    let (expr, rest) = calc::parse_prefix(&input).map_err(|e| match geo::being_typed(config, &input) {
        Some(next) => CliError::Incomplete(next),
        None => calc_error(e),
    })?;
    let value = expr.eval().map_err(calc_error)?;
    if !value.is_finite() {
        return Err(CliError::Convert(convert::ConvertError::NotFinite));
    }
    let notes = match !opts.verbose || opts.silent || expr.operations() < 2 {
        true => Vec::new(),
        false => vec![format!("evaluated as {}", expr.grouped())],
    };
    let words: Vec<&str> = rest.split_whitespace().collect();
    // `to`, `in` or `->` between the units (`1000 ly to yd`); a last `in` is the inch (`10 ft in`).
    let waiting = |w: &str| currency::connector(w) && !w.eq_ignore_ascii_case("in");
    let (from, to) = match words[..] {
        // No unit, or the `num` target (`2026-12-02 num`): the number.
        [] => return Ok(Output { stdout: convert::format_value(value, opts.precision), notes, ..Output::default() }),
        [to] if config::is_number_word(to) => return Ok(Output { stdout: convert::format_value(value, opts.precision), notes, ..Output::default() }),
        // A unit so far (`10 km`, `10 km to`): the target comes next; for a compound one (`m/s^2`) no
        // kind's units to list.
        [from] | [from, _] if words.len() == 1 || waiting(words[1]) => {
            return Err(match convert::find_unit(config, from) {
                Some((category, unit)) => CliError::Incomplete(next_unit(category, unit)),
                None if convert::resolve(config, from).is_ok() => CliError::Incomplete("now the unit to convert to".into()),
                None => CliError::Usage(EXPECTED.into()),
            })
        }
        [from, to] => (from, to),
        [from, c, to] if currency::connector(c) => (from, to),
        _ => return usage(EXPECTED),
    };
    let (result, target) = convert::convert(config, value, from, to).map_err(CliError::Convert)?;
    let warnings = match (opts.silent, convert::find_unit(config, from), target.unit) {
        (false, Some((_, from_unit)), Some(to_unit)) => convert::warnings(from_unit, to_unit),
        _ => Vec::new(),
    };
    Ok(Output { stdout: format!("{} {}", convert::format_value(result, opts.precision), target.symbol), warnings, notes, ..Output::default() })
}

/// How a calculation is grouped, every operation in brackets (`12+4/2` → `(12 + (4 / 2))`), for a
/// UI to show beside the result while it is typed; `None` without operations, or for a date
/// (`2026-12-25` is not 2026 − 12 − 25).
pub fn grouping(input: &str) -> Option<String> {
    check_length(input).ok()?;
    if time::is_date_request(input) {
        return None;
    }
    let (expr, _) = calc::parse_prefix(input).ok()?;
    (expr.operations() > 0).then(|| expr.bracketed())
}

/// Whether the arguments wait for values on stdin: none, or one or two units, currencies or
/// formats and nothing else (`F C`, `usd eur`, `unix`): `echo 100 | aequalis F C`. Anything else is a
/// request of its own, evaluated once.
fn needs_value(config: &Config, args: &[&str]) -> bool {
    let target = |a: &&str| convert::resolve(config, a).is_ok() || time::is_format(a) || (currency::exact(a).is_some() && time::tz::country(a).is_none());
    let value_first = args.first().is_some_and(|t| calc::starts_expression(t) || time::starts_date(t));
    !value_first && args.len() <= 2 && args.iter().all(target)
}

/// After `10 km`, what comes next, without guessing which: `now the length unit to convert to: in,
/// ft, yd…`, the other system's units first, in the table's order.
fn next_unit(category: &config::Category, from: &config::Unit) -> String {
    let others = |same: bool| category.units.iter().filter(move |u| u.symbol != from.symbol && (u.system == from.system) == same);
    let shown: Vec<&str> = others(false).chain(others(true)).take(3).map(|u| u.symbol).collect();
    let more = if category.units.len() - 1 > shown.len() { "…" } else { "" };
    format!("now the {} unit to convert to: {}{more}", category.name.to_lowercase(), shown.join(", "))
}

fn calc_error(err: calc::CalcError) -> CliError {
    match err {
        calc::CalcError::Syntax(msg) => CliError::Usage(msg),
        calc::CalcError::DivisionByZero => CliError::Convert(convert::ConvertError::DivisionByZero),
        calc::CalcError::Domain(msg) => CliError::Convert(convert::ConvertError::Domain(msg)),
    }
}

fn list(config: &Config) -> String {
    let mut out = String::new();
    for cat in &config.categories {
        out.push_str(&format!("{} (base: {})\n", cat.name, cat.base));
        for u in &cat.units {
            out.push_str(format!("  {:<8} {}", u.symbol, u.aliases.join(", ")).trim_end());
            out.push('\n');
        }
        out.push('\n');
    }
    out.push_str("Compound units: units joined by / and *, with powers: km/h, m/s^2, Mb/s, mi/gal\n");
    out.push('\n');
    out.push_str(&currency::list());
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piped(input: Option<&str>, args: &str) -> Vec<Result<Output, CliError>> {
        let args: Vec<String> = args.split_whitespace().map(String::from).collect();
        run(&Config::default(), &args, || input.map(String::from))
    }

    fn output(args: &str) -> Result<Output, CliError> {
        let mut results = piped(None, args);
        assert_eq!(results.len(), 1);
        results.pop().unwrap()
    }

    fn stdin(input: &str, args: &str) -> Vec<String> {
        piped(Some(input), args).into_iter().map(|r| r.map(|o| o.stdout).unwrap_or_else(|e| format!("exit {}", e.exit_code()))).collect()
    }

    fn cli(args: &str) -> Result<String, CliError> {
        output(args).map(|o| o.stdout)
    }

    fn code(args: &str) -> u8 {
        cli(args).unwrap_err().exit_code()
    }

    #[test]
    fn conversions() {
        assert_eq!(cli("10 km mi").unwrap(), "6.2137 mi");
        assert_eq!(cli("-40 C F").unwrap(), "-40 F");
        assert_eq!(cli("1 kg lb --precision 2").unwrap(), "2.2 lb");
        assert_eq!(cli("--precision 2 1 kg lb").unwrap(), "2.2 lb");
        assert_eq!(cli("1 kg lb --precision=0").unwrap(), "2 lb");
        assert_eq!(cli("1e3 m km").unwrap(), "1 km");
        assert_eq!(cli("3 m m").unwrap(), "3 m");
    }

    #[test]
    fn info_flags() {
        assert!(cli("--help").unwrap().starts_with("Usage:"));
        assert_eq!(cli("-V").unwrap(), format!("aequalis {}", env!("CARGO_PKG_VERSION")));
        let list = cli("--list").unwrap();
        assert!(list.contains("Length (base: m)") && list.contains("gal-uk") && list.contains("TiB"));
        assert!(cli("--autocompletion bash").unwrap().contains("complete -F _aequalis aequalis"));
        assert!(cli("--autocompletion=zsh").unwrap().starts_with("#compdef aequalis"));
    }

    #[test]
    fn warnings_and_silent() {
        assert!(output("15739208 kB GB").unwrap().warnings.is_empty());
        let out = output("15739208 kB Gb").unwrap();
        assert_eq!(out.stdout, "125.9137 Gb");
        assert_eq!(out.warnings.len(), 1);
        assert_eq!(output("1 GB GiB").unwrap().warnings.len(), 1);
        for flag in ["-s", "--silent"] {
            let out = output(&format!("15739208 kB Gb {flag}")).unwrap();
            assert_eq!(out.stdout, "125.9137 Gb");
            assert!(out.warnings.is_empty());
        }
        assert_eq!(cli("-s 1 GB GiB").unwrap(), "0.9313 GiB");
    }

    #[test]
    fn attached_units() {
        assert_eq!(cli("100Gb Kb").unwrap(), "100000000 kb");
        assert_eq!(cli("100C F").unwrap(), "212 F");
        assert_eq!(cli("-40C F").unwrap(), "-40 F");
        assert_eq!(cli("1.5km m").unwrap(), "1500 m");
        assert_eq!(cli(".5kg g").unwrap(), "500 g");
        assert_eq!(cli("1e3m km").unwrap(), "1 km");
        assert_eq!(cli("10gal-uk l --precision 1").unwrap(), "45.5 l");
        assert_eq!(cli("2kilometers miles").unwrap(), "1.2427 mi");
        assert_eq!(cli("20°C F").unwrap(), "68 F");
        assert_eq!(cli("-s 1GB GiB").unwrap(), "0.9313 GiB");
        assert_eq!(output("15739208kB Gb").unwrap().warnings.len(), 1);
        assert_eq!(code("100 Gb"), 2);
        assert_eq!(code("abc Gb"), 2);
        assert_eq!(code("100foo Gb"), 1);
        assert_eq!(code("100km kg"), 1);
    }

    #[test]
    fn stdin_requests() {
        assert_eq!(stdin("100F C\n", ""), ["37.7778 C"]);
        assert_eq!(stdin("100\n", "F C"), ["37.7778 C"]);
        assert_eq!(stdin("100F\n", "C"), ["37.7778 C"]);
        assert_eq!(stdin("100 F C", ""), ["37.7778 C"]);
        assert_eq!(stdin("100 F\n", "C --precision 1"), ["37.8 C"]);
        // One request per line; blank lines are skipped and bad lines don't stop the rest.
        assert_eq!(stdin("32\n\n212\n  \nabc\n-40\n", "F C"), ["0 C", "100 C", "exit 2", "-40 C"]);
        assert_eq!(stdin("1 km kg\n", ""), ["exit 1"]);
        assert_eq!(stdin("", "F C"), ["exit 2"]);
        assert_eq!(stdin("\n\n", ""), ["exit 2"]);
        // A value in the arguments means stdin is not used.
        assert_eq!(stdin("999\n", "100 F C"), ["37.7778 C"]);
        // Not piped (terminal): the arguments alone are used.
        assert_eq!(code("F C"), 2);
        let warned = piped(Some("1GB\n"), "GiB");
        assert_eq!(warned[0].as_ref().unwrap().warnings.len(), 1);
        assert!(piped(Some("1GB\n"), "GiB -s")[0].as_ref().unwrap().warnings.is_empty());
    }

    /// The arguments wait for values on stdin only when they are units, currencies or formats.
    #[test]
    fn needs_value_from_stdin() {
        let config = Config::default();
        for args in [&[][..], &["F", "C"], &["km"], &["usd", "eur"], &["unix"], &["GiB"], &["km/h", "mph"]] {
            assert!(needs_value(&config, args), "{args:?}");
        }
        for args in [&["100", "F", "C"][..], &["100F", "C"], &["Italy"], &["Rome", "Paris"], &["now"], &["km", "mi", "m"], &["abc"], &["2026-12-04", "unix"]] {
            assert!(!needs_value(&config, args), "{args:?}");
        }
    }

    #[test]
    fn groupings() {
        assert_eq!(grouping("12+4/2").as_deref(), Some("(12 + (4 / 2))"));
        assert_eq!(grouping("100 + 20 + 50 / 10 sqft m2").as_deref(), Some("((100 + 20) + (50 / 10))"));
        assert_eq!(grouping("10 km mi"), None, "no operation");
        assert_eq!(grouping("2026-12-25"), None, "a date");
    }

    /// Every character counts, spaces too; characters, not bytes.
    #[test]
    fn the_longest_request() {
        for c in ["1", " ", "é", "€", "🙂"] {
            assert!(check_length(&c.repeat(MAX_INPUT)).is_ok(), "{c} × 512");
            assert_eq!(check_length(&c.repeat(MAX_INPUT + 1)), Err(too_long()), "{c} × 513");
        }
        assert!(check_length("").is_ok());
        // Words joined by one space: 256 words of 1 character and 255 spaces fit, 257 don't.
        assert!(check_words(&["1"; 256]).is_ok());
        assert!(check_words(&["1"; 257]).is_err());
        assert!(check_words(&["é".repeat(511).as_str(), "x"]).is_err(), "511 + 1 + 1");
        assert!(check_words(&["é".repeat(510).as_str(), "x"]).is_ok());
        assert!(check_words(&[]).is_ok());
        // Refused before anything reads it: no grouping either.
        let long = vec!["1"; 300].join("+");
        assert_eq!(grouping(&long), None);
        assert_eq!(grouping(&vec!["1"; 256].join("+")).map(|g| g.len() > 500), Some(true), "511 characters: grouped");
    }

    #[test]
    fn calculator() {
        assert_eq!(cli("10**2").unwrap(), "100");
        assert_eq!(cli("10^2").unwrap(), "100");
        assert_eq!(cli("(100+20)/120").unwrap(), "1");
        assert_eq!(cli("[100+20]/120").unwrap(), "1");
        assert_eq!(cli("1/3").unwrap(), "0.3333");
        assert_eq!(cli("1/3 --precision 2").unwrap(), "0.33");
        assert_eq!(cli("10 - 5").unwrap(), "5");
        assert_eq!(cli("-(2+3)").unwrap(), "-5");
        assert_eq!(cli("100").unwrap(), "100");
        assert_eq!(cli("3 x 4").unwrap(), "12");
    }

    #[test]
    fn expressions_with_units() {
        assert_eq!(cli("100 + 20 + 50 / 10 sqft m2").unwrap(), "11.6129 m2");
        assert_eq!(cli("100+20sqft m2").unwrap(), "11.1484 m2");
        assert_eq!(cli("[1+1]km m").unwrap(), "2000 m");
        assert_eq!(cli("2x5 km m").unwrap(), "10000 m");
        assert_eq!(cli("100 - 40 C F").unwrap(), "140 F");
        assert_eq!(cli("2e3+1 m km").unwrap(), "2.001 km");
        assert_eq!(code("1+2 km"), 2);
        assert_eq!(code("1+2 km kg"), 1);
    }

    #[test]
    fn grouping_note() {
        // Only with --verbose.
        assert!(output("100 + 20 + 50 / 10 sqft m2").unwrap().notes.is_empty());
        let out = output("-v 100 + 20 + 50 / 10 sqft m2").unwrap();
        assert_eq!(out.notes, ["evaluated as (100 + 20) + (50 / 10)"]);
        assert_eq!(output("--verbose (100+20)/120").unwrap().notes, ["evaluated as (100 + 20) / 120"]);
        // A single operation needs no grouping.
        assert!(output("1/3").unwrap().notes.is_empty());
        assert!(output("10 km mi").unwrap().notes.is_empty());
        assert!(output("-v -s 100 + 20 + 50 / 10").unwrap().notes.is_empty(), "-s wins");
    }

    #[test]
    fn calculator_errors() {
        assert_eq!(code("1/0"), 1);
        assert_eq!(code("10**400"), 1);
        assert_eq!(code("(1+2"), 2);
        assert_eq!(code("1+2)"), 2);
        assert_eq!(code("1 +"), 2);
        assert_eq!(code("1 2"), 2);
    }

    #[test]
    fn calculator_on_stdin() {
        assert_eq!(stdin("1+2\n10^2\n", ""), ["3", "100"]);
        assert_eq!(stdin("100 + 20\n", "sqft m2"), ["11.1484 m2"]);
        assert_eq!(stdin("[1+1]km\n", "m"), ["2000 m"]);
        // An expression in the arguments means stdin is not used.
        assert_eq!(stdin("999\n", "1+1"), ["2"]);
    }

    #[test]
    fn exit_codes() {
        assert_eq!(code("10 km kg"), 1);
        assert_eq!(code("10 foo m"), 1);
        assert_eq!(code("abc km m"), 2);
        assert_eq!(code("inf km m"), 2);
        assert_eq!(code("10 km"), 2);
        // Not finished yet: what comes next, also after a compound unit.
        assert_eq!(cli("10 km").unwrap_err(), CliError::Incomplete("now the length unit to convert to: in, ft, yd…".into()));
        assert_eq!(cli("100 m/s^2").unwrap_err(), CliError::Incomplete("now the unit to convert to".into()));
        assert_eq!(cli("10 foo").unwrap_err(), CliError::Usage(EXPECTED.into()));
        // A number expression alone is arithmetic, whatever its shape; a date needs a word beside it.
        for (expr, value) in [("12/04/2026", "0.0015"), ("2026/12/04", "42.2083"), ("1-10-10", "-19"), ("2026-12-02-2026-12-01", "-27")] {
            assert_eq!(cli(expr).unwrap(), value, "{expr}");
        }
        assert_eq!(cli("12/4").unwrap(), "3");
        // Past the longest request: an error, whatever it is.
        assert_eq!(cli(&vec!["1"; 256].join("+")).unwrap(), "256", "511 characters");
        assert_eq!(cli(&format!("{}+1", vec!["1"; 256].join("+"))).unwrap_err(), CliError::Usage("the request is too long: at most 512 characters".into()));
        assert_eq!(cli("12/04/2026 num").unwrap(), "0.0015");
        assert_eq!(code("1 2 3 4"), 2);
        assert_eq!(code(""), 2);
        assert_eq!(code("1 km m --precision 16"), 2);
        assert_eq!(code("1 km m --precision"), 2);
        assert_eq!(code("--bogus"), 2);
        assert_eq!(code("--autocompletion fish"), 2);
    }
}
