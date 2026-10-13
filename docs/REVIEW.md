# Code review: speed, memory, clean code, tests

Review of every algorithm in Tally (the `cities` branch: the app with currencies, the city
archive and the settings), with three priorities in this order: **fast responses**, **memory
below 10 MB**, **clean code**; then how complete the test suite is. Everything found was fixed
on the branch `review`; this report says what was found, what changed, and what is left.

## Summary

| | before | after |
|---|---|---|
| A request (`10 km mi`, `now Tokyo`, `Rome Paris`…) | 15–60 ms | 3.2–7.0 ms |
| A keystroke in the interactive mode (key + screen + preview) | 30–42 ms | 0.26–0.84 ms |
| `--trivia` | 72.9 ms | 14.8 ms |
| Installing the city archive (69,801 cities, 546k names) | 1.4 s, **47 MB** peak | 1.2 s, **6.1 MB** peak |
| Peak memory of a request | 3.6–4.2 MB | 3.8–4.3 MB |
| The test suite (`cargo test`) | 343 tests, 55.4 s | 355 tests, 14.4 s |
| Line / function coverage | 94.5% / 94.2% | 95.3% / 94.8% |
| `cargo clippy --all-targets` (with and without the TUI) | 19 warnings | 0 |

Every run of tally now stays below 10 MB, enforced by a test (`tests/memory.rs`).

## Method

- **Timings and memory:** `scripts/bench.py` runs each request 30 times in a fresh process
  (release build, offline, its own config directory) and reports the median, and the process's
  peak memory (VmHWM: from the `--json` reply's `peak_kb`, or watched in `/proc`). With
  `--archive`, it installs the real city archive from a local copy and measures requests on it.
- **The interactive mode:** `cargo test --release -- --ignored keystroke_cost --nocapture`
  types requests key by key into the app and draws the screen after each (40 × 120).
- **Coverage:** `scripts/coverage.py` builds with rustc's instrumentation
  (`-C instrument-coverage`, the rustup component `llvm-tools`), runs the whole suite (the tally
  binary started by the integration tests included) and reports lines and functions per file;
  `--uncovered FILE…` lists the regions no test runs.
- **Dead code:** the compiler's warnings (`dead_code`, with and without the TUI), a script that
  lists every function not referenced outside tests, and clippy.
- The numbers above are medians on the author's laptop (CPU frequency varies, ±20%); the "before"
  column is the `cities` branch measured the same way.

## Findings and changes

### 1. Speed

**The cause: every name was looked up by scanning whole tables, normalising each entry again.**
`tz::key` (lower case, no accents, punctuation as spaces) allocated five strings per call, and was
called tens of thousands of times per request:

- the built-in cities (`tz::city`, `place_name`, `zones_of`: 281 entries), the countries
  (`tz::country`: 196 and their aliases), the currencies (`currency::exact`: 167 and their names,
  plurals, symbols), and the units on a miss (`convert::find_unit` lower-cased both strings for
  each of ~600 names);
- the number of words of the longest city and country names was recomputed for every word of a
  request;
- the local currency's and the local country's lookups (the device's time zone → its country)
  each scanned countries × cities × the city table, on **every** request — on a machine whose zone
  matches no country, the whole table twice;
- every request goes through the currency, distance and time parsers before the calculator, so
  even `10 km mi` paid for all of it (40 ms).

**Changes:**

- Name tables built once per process (`OnceLock`, `HashMap`): cities and countries by normalised
  name, time zone → country (`tz::zone_country`, used by currencies and distances), currency
  forms with plurals, abbreviations and symbols (`currency::index`), units exact and in lower
  case (`Config::unit_names`), the longest names. A lookup normalises only its own text.
- `tz::key` in one pass with one allocation, ASCII without the Unicode tables; the tables
  normalise each name once (they used to do it twice).
- One generic `tz::nearest` for misspellings (best Levenshtein distance within a limit, lengths
  out of reach skipped), used by the cities, the countries and the currencies.
- The JSON reader copies string runs at once instead of byte by byte (the embedded data files).
- **Each request is evaluated once.** The command line decided whether to read values on stdin by
  running five parsers (`names_city`, the misspelt-place search, the currency and distance
  parsers) and then evaluated the request again; it now asks `needs_value` (are the arguments only
  units, currencies or formats?). The interactive mode evaluated each keystroke three or four
  times (preview, ambiguity check, distance check, currencies line); it now keeps the last
  evaluation by input (`App::evaluated`), and `Output` says what is already formatted
  (`formatted`) and carries the currencies line (`check`).
- The exchange rates are read only for a currency request, and once per change of the file
  (cached by modification time, so the interactive mode still sees a background sync).
- `settings::current()` shares the settings (`Arc`) instead of copying every value per call.
- The city archive is read by position (`FileExt::read_at`): no seek, no 8 KB buffer per probe.
- `--trivia`: each planet's distance from the Earth sampled 7,305 times for its century mean and
  day by day for its extremes (≈60,000 orbit computations per listing). Now the mean takes 1,588
  samples (every 23 days; the mean moves by 0.03% at most, Venus, well within the orbital
  elements' accuracy) and the extremes are sampled every 4 days, then found to the minute by
  golden-section search (more precise than the hourly search before).

Where the remaining time goes (`10 km mi`, ~4.8 ms): starting the process (~1 ms), parsing the two
embedded JSON files (countries, currencies: ~0.4 ms each), building the name tables (~1 ms), the
request itself (< 0.4 ms for every kind).

### 2. Memory

Requests and the interactive mode were already small (about 4 MB; the city archive is searched
on disk, never loaded). The exception was installing the archive: its 546k names were sorted in
memory (47 MB peak). It now uses an **external merge sort**: names in sorted runs of 32,768
(about 2 MB) written next to the index, then merged with a heap. Measured on the real archive:
1.12 s and 46.6 MB in memory, 1.18 s and 6.1 MB with the merge — 5% slower, well within the agreed
rule (below 10 MB unless more than 2× slower). `tally --sync … --json` reports its peak memory like
any reply, and `tests/memory.rs` checks that representative requests and the archive install stay
below 10 MB.

### 3. Clean code

- **Dead code removed:** `currency::claims`, `currency::describe`, `time::suggests_city`,
  `tz::zones_of`, `convert::names`, `cities::key` (a wrapper), `online::dir`/`online::now` (now one
  each in `files` and `time`), and `scripts/build-macos.sh` (a zig cross-build: releases build on
  GitHub's macOS runner). The two functions only tests use were already `#[cfg(test)]`; nothing
  else was unused.
- **Duplicates merged:** the atomic file write (4 copies) is `files::write_atomic`; the
  hand-written JSON (5 writers) uses `json::Object`; "Unix seconds → a date for people" (5 copies)
  is `time::human_day` / `human_moment`; "now in Unix seconds" (6 copies) is `time::unix_now`.
- A misplaced doc comment (the `grouping()` doc on `currencies()` in `tui/mod.rs`) fixed.
- All 19 clippy warnings fixed, with and without the TUI; one justified `allow`
  (`DayFirst`/`MonthFirst`/`YearFirst` are the domain's own names).

### 4. Tests

| file | lines | covered before | covered after | functions before → after |
|---|---|---|---|---|
| `src/about.rs` | 58 | 88% | 88% | 100% → 100% |
| `src/calc.rs` | 447 | 99% | 99% | 100% → 100% |
| `src/cities.rs` | 498 | 86% | 88% | 72% → 77% |
| `src/completion.rs` | 121 | 99% | 99% | 94% → 94% |
| `src/config.rs` | 445 | 100% | 100% | 100% → 100% |
| `src/convert.rs` | 325 | 100% | 100% | 100% → 100% |
| `src/currency.rs` | 649 | 96% | 96% | 96% → 95% |
| `src/files.rs` (new) | 32 | — | 88% | — → 62% |
| `src/geo.rs` | 457 | 94% | 94% | 96% → 95% |
| `src/json.rs` | 312 | 90% | 91% | 89% → 91% |
| `src/locale.rs` | 44 | 100% | 100% | 100% → 100% |
| `src/main.rs` | 493 | 96% | 95% | 100% → 100% |
| `src/network.rs` | 31 | 100% | 100% | 100% → 100% |
| `src/notify.rs` | 95 | 84% | 86% | 81% → 84% |
| `src/numbers.rs` | 113 | 99% | 98% | 100% → 100% |
| `src/online.rs` | 388 | 89% | 93% | 90% → 91% |
| `src/planets.rs` | 123 | 100% | 100% | 100% → 100% |
| `src/settings.rs` | 247 | 97% | 97% | 94% → 94% |
| `src/time.rs` | 2080 | 98% | 98% | 98% → 98% |
| `src/time/sys.rs` | 74 | 73% | 73% | 75% → 75% |
| `src/time/tz.rs` | 525 | 93% | 95% | 90% → 92% |
| `src/trivia.rs` | 468 | 81% | 84% | 83% → 85% |
| `src/tui/clipboard.rs` | 36 | 0% | 56% | 0% → 43% |
| `src/tui/editor.rs` | 223 | 100% | 100% | 100% → 100% |
| `src/tui/keys.rs` | 116 | 93% | 93% | 100% → 100% |
| `src/tui/mod.rs` | 1119 | 93% | 93% | 97% → 98% |
| `src/tui/picker.rs` | 384 | 99% | 99% | 100% → 100% |
| `src/tui/render.rs` | 1300 | 98% | 98% | 99% → 99% |
| `src/tui/search.rs` | 47 | 100% | 100% | 100% → 100% |
| `src/tui/settings.rs` | 159 | 40% | 89% | 44% → 95% |
| `src/tui/suggest.rs` | 134 | 98% | 98% | 100% → 100% |
| `src/tui/term.rs` | 49 | 96% | 96% | 100% → 100% |
| `src/tui/trivia.rs` | 31 | 100% | 100% | 100% → 100% |
| **total** | 11623 | 94.5% | 95.3% | 94.2% → 94.8% |

**Tests added (12):**

- the Settings window: moving, yes/no and choices changed in place, typed values checked (a bad
  value is refused, Esc cancels), defaults, no download offline — on a settings file of its own
  (the window now takes its file and keeps its values, instead of reading the file at each redraw);
- the refresh policies (`online::due_when`): silent, notify, manual × out of date or fresh, the
  cities off;
- the city archive's sync by policy, from a local copy of the sample archive: under `notify`
  one desktop notification per new archive (the notifier is injected: no real notification in
  tests), under `silent` a download when newer, by hand always, off;
- the notices' state saved and read back; the atomic write;
- the index sorted in runs gives the same file whatever the run size (1, 3, 10,000);
- the clipboard's tools by desktop; the satellite kinds; the stdin rule (`needs_value`);
- memory: every representative request and the archive install below 10 MB (`tests/memory.rs`).

**What stays uncovered, and why:**

- `time/sys.rs` (73%): the macOS paths and the C library's error returns; covered on macOS only.
- `tui/clipboard.rs`, `notify::desktop`: they start the desktop's programs (`wl-copy`,
  `notify-send`); only the choice of program is tested, so tests never touch the clipboard or the
  desktop.
- `trivia.rs`, `currency::fetch`, `cities::remote_version` over HTTP: the network; their parsers
  are tested with saved answers, and the downloads run from local `file://` copies.
- The remaining lines are mostly error returns (`?` on a read that fails): a file that
  disappears between two calls, a full disk.

## Left as is, and recommendations

- **Startup (~3 ms of the ~5 ms per request) is now parsing the embedded JSON and building the
  name tables.** If it ever matters, a `build.rs` could write the tables pre-normalised as Rust
  constants (no JSON parse, no hashing at run time). Not done: it adds a build step for ~2 ms.
- **`Rome Paris` asks which Rome and which Paris once the archive is installed** (several real
  cities share each name), as specified. A future option: answer with the most populous pair and
  offer the others, instead of asking.
- **The test suite's 14 s are mostly the integration tests' processes and a few deliberate
  waits** (300 ms to check that no background sync starts). Fine for now.
- **`docs/SPEC.md` is ignored by git** (`.gitignore`): the specification, updated on every change,
  is not in the repository.
- **On merging `macos-app`:** its Mac app and Raycast "Offline" switches still set `TALLY_MODE`,
  which Tally no longer reads; they should call `tally --set offline=…`.

## Reproduce

```sh
cargo build --release
python3 scripts/bench.py --archive cities.json      # cities.json from scripts/build-cities.py
cargo test --release -- --ignored keystroke_cost --nocapture
rustup component add llvm-tools && python3 scripts/coverage.py --uncovered src/online.rs
cargo clippy --all-targets && cargo clippy --all-targets --no-default-features
```
