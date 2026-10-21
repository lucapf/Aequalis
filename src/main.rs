//! aequalis: an ultra lightweight command-line unit converter.

use aequalis::config::Config;
use aequalis::{convert, convert_request, notify, online, parse_options, reply, run, settings, time, CliError, Output, Parsed};
use std::io::{IsTerminal, Read};
use std::process::ExitCode;

mod device;
#[cfg(feature = "tui")]
mod tui;

fn main() -> ExitCode {
    // The tests' simulated machine (`AEQUALIS_MACHINE`, debug builds only): the only source of the
    // device's settings for the whole run.
    let _machine = aequalis::machine::from_env();
    // The device's time zone, read here and given to the library (which does not look for it).
    device::start();
    let config = Config::shared();
    let mut all: Vec<String> = std::env::args().skip(1).collect();
    if let Err(code) = freeze_now(&mut all) {
        return code;
    }
    // `--json` changes only how results are printed (and that nothing is asked).
    let json = all.iter().any(|a| a == "--json");
    let args: Vec<String> = all.into_iter().filter(|a| a != "--json").collect();
    if let Some(code) = as_typed(&args, json).or_else(|| settings_command(&args, json)) {
        return code;
    }
    if let Some(e) = settings::error() {
        return fail(format!("{e} (fix it with aequalis --set, or edit the file)"));
    }
    if let Some(code) = sync(&args, json) {
        return code;
    }
    // Any run: when the online data has expired, refresh it in the background (a separate
    // process: nothing here waits for it).
    let _sync = online::ensure_fresh();
    #[cfg(feature = "tui")]
    if let Some(code) = interactive(config, &args, json) {
        return code;
    }
    let code = requests(config, &args, json);
    notices(json);
    ExitCode::from(code)
}

/// An error of the command line (stderr), with exit code 2: a usage or settings error.
fn fail(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("aequalis: error: {message}");
    ExitCode::from(2)
}

/// `--now <unix seconds>` (not in the help): "now" frozen, for tests whose data has a date; taken
/// out of the arguments.
fn freeze_now(all: &mut Vec<String>) -> Result<(), ExitCode> {
    let Some(i) = all.iter().position(|a| a == "--now" || a.starts_with("--now=")) else { return Ok(()) };
    let value = match all[i].strip_prefix("--now=") {
        Some(v) => Some(v.to_string()),
        None => all.get(i + 1).cloned(),
    };
    let Some(seconds) = value.as_deref().and_then(|v| v.parse::<i64>().ok()) else {
        return Err(fail("--now requires Unix seconds (--now 1791800000)"));
    };
    time::set_now(seconds);
    all.drain(i..if all[i].starts_with("--now=") { i + 1 } else { i + 2 });
    Ok(())
}

/// `--as-typed [--submitted] "<request>"` (with `--json`, for a UI such as the Omarchy widget): the
/// request as typed, a trailing space included, and what the UI shows of its reply
/// (`reply::view`). `--submitted`: Enter was pressed on it. The reply says what went wrong.
fn as_typed(args: &[String], json: bool) -> Option<ExitCode> {
    if !json || !args.iter().any(|a| a == "--as-typed") {
        return None;
    }
    let submitted = args.iter().any(|a| a == "--submitted");
    let words: Vec<&str> = args.iter().map(String::as_str).filter(|a| !["--as-typed", "--submitted"].contains(a)).collect();
    println!("{}", reply::view(&words.join(" "), submitted));
    Some(ExitCode::SUCCESS)
}

/// `--set key[=value]`, `--reset-settings`, `--settings`: the settings (`settings.rs`). They work
/// even when the file has a problem, to fix it.
fn settings_command(args: &[String], json: bool) -> Option<ExitCode> {
    if let Some(i) = args.iter().position(|a| a == "--set" || a.starts_with("--set=")) {
        return Some(set(args, i));
    }
    let done = |result: Result<String, String>| match result {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    };
    if args.iter().any(|a| a == "--reset-settings") {
        let reset = settings::path().ok_or_else(|| "no settings folder".to_string()).and_then(|p| settings::reset_in(&p));
        return Some(done(reset.map(|_| "every setting is back to its default".to_string())));
    }
    if args.iter().any(|a| a == "--settings") {
        let loaded = settings::path().as_deref().map(settings::load_from).unwrap_or(Ok(settings::Settings::default()));
        return Some(done(loaded.map(|s| if json { settings::json(&s) } else { settings::text(&s) })));
    }
    None
}

/// `--set key=value`, the argument at `i` (`--set key`: its default).
fn set(args: &[String], i: usize) -> ExitCode {
    let spec = match args[i].strip_prefix("--set=") {
        Some(s) => Some(s.to_string()),
        None => args.get(i + 1).cloned(),
    };
    let Some(spec) = spec else {
        return fail("--set requires key=value (or key, for its default; see aequalis --settings)");
    };
    let (key, value) = match spec.split_once('=') {
        Some((k, v)) => (k.trim(), Some(v.trim())),
        None => (spec.trim(), None),
    };
    match settings::change(key, value) {
        Ok(s) => {
            println!("{key} = {}", s.get(key));
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// `--sync`: download all the online data now; `--sync --auto`: the background sync, what the
/// refresh policies say. A group that failed is reported (stderr; `--json`: its warnings) and
/// makes the exit code 1, after the others had their turn.
fn sync(args: &[String], json: bool) -> Option<ExitCode> {
    let i = args.iter().position(|a| a == "--sync")?;
    let auto = args.iter().any(|a| a == "--auto");
    if let Some(extra) = args[i + 1..].iter().find(|a| !a.starts_with("--")) {
        return Some(fail(format!("`--sync` downloads everything: no `{extra}` after it")));
    }
    let code = |report: &online::Report| if report.failed.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(1) };
    Some(match (online::sync(auto), json) {
        // `--json`: the usual reply, the lines as the result (with the peak memory).
        (Ok(report), true) => {
            let output = Output { stdout: report.done.join("\n"), warnings: report.failed.clone(), ..Output::default() };
            println!("{}", reply::result(&Ok(output), &[]));
            code(&report)
        }
        (Ok(report), false) => {
            for line in &report.done {
                println!("{line}");
            }
            for line in &report.failed {
                eprintln!("aequalis: {line}");
            }
            code(&report)
        }
        (Err(e), true) => {
            println!("{}", reply::result(&Err(CliError::Convert(convert::ConvertError::Domain(e))), &[]));
            ExitCode::from(1)
        }
        (Err(e), false) => {
            eprintln!("aequalis: {e}");
            ExitCode::from(1)
        }
    })
}

/// No value and no stdin to read, at a terminal: the interactive mode.
#[cfg(feature = "tui")]
fn interactive(config: &Config, args: &[String], json: bool) -> Option<ExitCode> {
    let Ok(Parsed::Convert(opts)) = parse_options(config, args) else { return None };
    if json || !opts.positional.is_empty() || !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return None;
    }
    // It runs for hours: the device's zone is read again every hour.
    std::thread::spawn(|| loop {
        std::thread::sleep(device::CHECK_EVERY);
        device::check();
    });
    Some(tui::run(config, &opts))
}

/// Every request, reported; the exit code is the most severe one seen.
fn requests(config: &Config, args: &[String], json: bool) -> u8 {
    let read_stdin = || {
        let mut stdin = std::io::stdin();
        let mut input = String::new();
        (!stdin.is_terminal() && stdin.read_to_string(&mut input).is_ok()).then_some(input)
    };
    // An ambiguous date is asked about when someone can answer: requests from the arguments, at a
    // terminal. Requests piped on stdin, or output to a script, get the error.
    let can_ask = !json && std::io::stdin().is_terminal() && std::io::stderr().is_terminal();
    let mut code = 0;
    for result in run(config, args, read_stdin) {
        let result = match result {
            Err(CliError::Ambiguous(a)) if can_ask => answered(config, args, a),
            other => other,
        };
        if let Err(err) = &result {
            code = code.max(err.exit_code());
        }
        print(&result, json);
    }
    code
}

/// The request of the reading chosen at the terminal (`ask`), evaluated; the question again when
/// none is chosen.
fn answered(config: &Config, args: &[String], a: time::Ambiguity) -> Result<Output, CliError> {
    match (ask(&a), parse_options(config, args)) {
        (Some(choice), Ok(Parsed::Convert(opts))) => {
            let tokens: Vec<&str> = choice.request.split_whitespace().collect();
            convert_request(config, &opts, &tokens)
        }
        _ => Err(CliError::Ambiguous(a)),
    }
}

/// A request's outcome: `--json`, its reply (with the notices); else the result on stdout, the
/// warnings, notes and errors on stderr.
fn print(result: &Result<Output, CliError>, json: bool) {
    if json {
        println!("{}", reply::result(result, &notify::current()));
        return;
    }
    match result {
        Ok(out) => {
            for w in &out.warnings {
                eprintln!("aequalis: warning: {w}");
            }
            for n in &out.notes {
                eprintln!("aequalis: note: {n}");
            }
            println!("{}", out.stdout);
        }
        Err(err) => match err.hint() {
            Some(hint) => eprintln!("aequalis: error: {err} ({hint})"),
            None => eprintln!("aequalis: error: {err}"),
        },
    }
}

/// Notices (the `notify` policies, the city archive missing): on stderr at a terminal, once a day
/// at most; never in scripts.
fn notices(json: bool) {
    if json || !std::io::stderr().is_terminal() {
        return;
    }
    let notices = notify::current();
    let mut state = notify::load();
    let now = time::unix_now();
    if !notices.is_empty() && now - state.shown_at >= 86_400 {
        // The notices say what; here, how: the command that downloads.
        for n in &notices {
            eprintln!("aequalis: note: {n}: aequalis --sync");
        }
        state.shown_at = now;
        notify::save(&state);
    }
}

/// Asks on the terminal which reading of an ambiguous date is meant; Enter takes the first (the
/// locale's order). `None` after three answers that are not a choice, or at the end of input.
fn ask(a: &time::Ambiguity) -> Option<&time::Choice> {
    let question = match a.kind {
        time::AmbiguityKind::City => {
            eprintln!("aequalis: {}. Did you mean:", a.note);
            if a.note.ends_with("is not a city") {
                "Which city?"
            } else {
                "Which one?"
            }
        }
        time::AmbiguityKind::Country => {
            eprintln!("aequalis: `{}` has several time zones:", a.text);
            "Which one?"
        }
        time::AmbiguityKind::Date => {
            eprintln!("aequalis: `{}` is ambiguous:", a.text);
            "Which one?"
        }
        time::AmbiguityKind::Currency => {
            eprintln!("aequalis: {}. Did you mean:", a.note);
            "Which currency?"
        }
        time::AmbiguityKind::Place => {
            eprintln!("aequalis: {}. Did you mean:", a.note);
            "Which one?"
        }
    };
    let column = a.choices.iter().map(|c| c.pattern.chars().count()).max().unwrap_or(0).max(12);
    for (i, c) in a.choices.iter().enumerate() {
        eprintln!("  {}) {:<column$} {}", i + 1, c.pattern, c.date);
    }
    for _ in 0..3 {
        eprint!("{question} [1-{}, Enter = 1]: ", a.choices.len());
        let mut answer = String::new();
        if std::io::stdin().read_line(&mut answer).ok()? == 0 {
            return None;
        }
        match answer.trim() {
            "" => return a.choices.first(),
            n => {
                if let Some(c) = n.parse::<usize>().ok().and_then(|i| i.checked_sub(1)).and_then(|i| a.choices.get(i)) {
                    return Some(c);
                }
            }
        }
    }
    None
}
