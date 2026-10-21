# Core review: one zero-dependency library, a UI per platform

Review of the whole application (master at `238c174`, 14 Oct 2026), with one goal: the business
logic in a library with **no dependencies**, and a UI per target on top of it: the TUI on Linux,
macOS and Windows, and apps on Android and iOS. Also checked: inconsistencies, and whether the
algorithms are efficient. The `ios` branch (a C interface and a SwiftUI app) is reviewed too: it is
the first attempt at this separation.

## Summary

- **The core is a good base.** No dependencies, 361 tests, and the algorithms fit the job:
  5–8 ms per command-line request *including process start*, 0.2–0.9 ms per keystroke in process,
  4.1–4.4 MB peak memory. The data files agree with each other (checked by script, below).
- **It is not yet a library that every UI can embed.** It reaches the operating system directly:
  `curl`, `gzip` and itself as child processes, `XDG_CONFIG_HOME`/`HOME`, `/usr/share/zoneinfo`,
  the C library's local time and locale (`setlocale`), `defaults` on macOS, desktop notifications,
  clipboard tools (§4.3). Three Unix-only calls stop a Windows build: `process_group`,
  `FileExt::read_at`, `getrusage`.
- **UI and logic are mixed both ways.** CLI help, argument parsing, TUI shortcuts and layout live in
  the core; the as-you-type rules ("no error while a word is being typed", "now the target unit")
  live in the TUI and again in the Omarchy widget's QML.
- **Correctness:** wall times in a DST gap are read 1 hour apart depending on whether the zone is the
  device's or a named one; halves round to even (`2.5` → `2`, `0.5` → `0`); a decimal comma is
  refused while results can be written with locale separators that Aequalis can't read back; 50,000
  nested brackets crash the process. Details and fixes in §2.
- **Done in this pass:** the city archive is published and downloaded as plain `cities.json` (with a
  completeness check that replaces gzip's checksum); with several cities of a name, Aequalis still
  asks, with the country's capital first and selected by default; the core is a library
  (`src/lib.rs`); clippy is clean again. See §7.

## 1. Method

- Read every core module (12,000 lines) and the TUI's logic paths (`tui/mod.rs`, `suggest.rs`,
  `search.rs`, `currencies.rs`, `measures.rs`), the scripts and workflows, and the `ios` branch
  (`ffi.rs`, `http.rs`, `inflate.rs`, `aequalis.h`, the Swift engine).
- Timings: release build, a fresh process per request, median of 15–40 runs, offline, with the real
  city archive (69,801 cities, 545,955 index lines); in process: `cargo test --release -- --ignored
  keystroke_cost`.
- Edge cases probed through the binary (DST, rounding, separators, very long or nested input).
- A script cross-checked `tz.rs` `CITIES`, `countries.json`, `places.json` and `currencies.json`.
- `cargo check` for `aarch64-apple-ios` and `aarch64-apple-darwin` (installed targets). Windows and
  Android were not compiled (targets not installed): their findings come from reading the code.

## 2. Correctness and consistency

| # | Severity | Where | Finding | Fix |
|---|---|---|---|---|
| 1 | High | `time/tz.rs:44`, `time/sys.rs:80` | **Two time-zone engines disagree in a DST gap.** The device's zone goes through the C library (`mktime`), a named zone through `tz.rs`. With `TZ=Europe/Rome`: `2026-03-29 02:30 unix` → 01:30Z, but `2026-03-29 02:30 Rome to London` → 00:30Z. `to_utc`'s doc says "the offset in force just before"; the code uses the offset after, in gaps and overlaps. | One engine: the device's zone by name through `tz.rs` too (also the fix for iOS, Android, Windows: §4). Pick one rule and document it, e.g. JavaScript Temporal's "compatible": a gap moves forward, an overlap takes the earlier time. |
| 2 | High | `convert.rs:212`, `currency.rs:485` | **Halves round to even** (Rust's formatting): `2.5 --precision 0` → `2`, `1.25 --precision 1` → `1.2`, `0.125 --precision 2` → `0.12`, and `0.5 --precision 0` → `0`, which breaks `format_value`'s own rule that a non-zero value never shows as 0. Calculators round halves up. | Round half away from zero on the shortest decimal form (`{}`), so the digits people typed decide (`2.675` → `2.68`). Same function for currencies. **Fixed** 15 Oct (`b69595e`, §9). |
| 3 | High (mobile) | `calc.rs` number grammar, `geo.rs:394` | **Separators break round trips.** Input only accepts `.` (`2,5 + 1` → a generic usage error), so the Italian keypad on a phone can't be used. Output is sometimes localized: `Rome Tokyo` → `6,136 mi` (en_US), and `6,136 mi km` is refused; an Italian device would show `1.107 km`, and `1.107 km mi` reads it as 1.107 (`0.6879 mi`). | The core returns a plain result (`6136 mi`) plus a display text in the device's format, and reads the locale's decimal separator in input (comma when the locale says so; coordinates `41,9 12,5` need a rule). |
| 4 | Medium | `geo.rs:394`, `currency.rs:485`, `convert.rs:212` | **Each kind of result is formatted its own way:** distances grouped by the device locale, in the CLI and in `--json` too (`6,136 mi`); money always `{:.2}` (`895156.00 EUR`, `1902.53 JPY`: the yen has no cents), ignoring `--precision`; units plain (`100000000 kb`). | One reply type (§4.4): value, unit, plain text, display text. Currencies get their minor units (ISO 4217) from `currencies.json`. |
| 5 | Medium | README, `docs/Accepted_Syntax.md`, `docs/install_mac.md` | **The docs say otherwise:** "`Chicago Paris` asks Paris, Texas first (same country as Chicago)", but `cities::rank` (`cities.rs:201`) orders by the *device's* country, so Paris, France comes first. `12 Sept 2026 10:00 PM + 5wd` is documented as `18/09/2026 DD/MM/YYYY` and answers `18/09/2026 10:00:00 PM DD/MM/YYYY-12h`. `AEQUALIS_CURRENCY` is gone (it is `currency.local`). The README's distance example, `Rome to Paris` → `1,107 km`, asks which Rome and which Paris once the archive is installed. The Mac guide installs v0.1.1 (the latest tag is v0.2.4). `docs/SPEC.md` is ignored by git. | Fixed in this pass: the order of the cities (now the capital first, §7), the `10:00 PM` example, `AEQUALIS_CURRENCY`. Left: the Mac guide's version; track `docs/SPEC.md`. |
| 6 | Medium | `calc.rs:324` (recursion) | **Crash on nested input:** 50,000 nested brackets overflow the stack (exit 134). In an app, a paste can do it. | A depth limit (a few hundred) with a usage error. **Fixed** 15 Oct (`e0b1105`, §9). |
| 7 | Medium | `online.rs:293` | **One failing group stops the sync:** `sync_cities(…)?` returns early, so a failed archive download skips the exchange rates and the live values after it. | Sync each group on its own, and report a line per group (error or count). **Fixed** 15 Oct (`03d33ca`, §9). |
| 8 | Low | `calc.rs:158`, `lib.rs:424` | `(-2)^0.5` says "result is out of range": NaN is reported as an overflow. | NaN is a domain error ("not a real number"). **Fixed** 15 Oct (`b69595e`, §9). |
| 9 | Low | `time.rs:2283`, `time.rs:1425` | `names_city` builds a new `Config` on every call, and `starts_with_duration` keeps a second static one; with a country in the request, the unit-name table is rebuilt. | One `Config` for the process (it is constant data). **Fixed** 15 Oct (`b797e47`, §9). |
| 10 | Low | `numbers.rs:104`, `time/sys.rs:68` | On macOS the number format comes from the region setting (`AppleLocale`) but the date order from `LANG` (read from the code, not run: no Mac here): a Mac set to Italy with `LANG=en_US.UTF-8` gets `1.234,5` and month-first dates. | One locale source per platform (§4.3). |
| 11 | Low | `time.rs:1172` | Dates for people are English only (`MMMM`, `MMM`, `dddd`), while input reads 6 languages. | Month and weekday names from the locale given by the UI. |
| 12 | Low | `notify.rs:58`, error texts | Notices and errors name CLI commands (`aequalis --sync`, `see aequalis --list`), shown as they are in every UI. | Notices as data (`CitiesNotDownloaded`, `NewCities`, `Stale(group)`); each UI words them. |
| 13 | Info | `time/tz.rs:960` and others | Time-zone tests return early when the zone database is missing, so they pass on Windows without testing anything. | Fail when the database is missing in CI; a bundled copy makes it moot. |
| 14 | Info | `.github/workflows/release.yml` | macOS tests run with `continue-on-error`: macOS paths are not verified. | Make them blocking once green. |
| 15 | Info | `.gitignore` (`*.lock`) | `Cargo.lock` is not committed. | Commit it with the workspace (recommended for binaries). |

**`Rome Paris` with the archive (decided).** Aequalis keeps asking ("no guessing"), and a country's
capital is offered first, so it is the choice selected by default, wherever the device is: Rome,
Italy → Paris, France. Done in this pass (§7); the README says so.

**Data:** the cross-check passed. 281 built-in cities, 281 with coordinates, every country's
cities have a zone, every country's currency exists, no name shared by two countries. The cities
are kept in two places (`tz.rs` `CITIES` for the zone, `places.json` for the coordinates). One
file (`name, zone, lat, lon`) would prevent drift.

## 3. Performance: is the algorithm optimized?

| Request | Median (fresh process) | Peak memory |
|---|---|---|
| `--version` (process start only) | 1.6 ms | — |
| `10 km mi`, `12+4/2`, `150 + 20%`, `100 km/h mph` | 4.7–5.9 ms | 4.2–4.3 MB |
| `12 usd euro`, `12 USA Europe` | 5.4–5.7 ms | 4.1 MB |
| `now Tokyo`, `10:00AM Paris to New York`, `12 Sept 2026 + 5wd` | 6.1–6.8 ms | 4.1–4.2 MB |
| `Springfield, Illinois Chicago`, `distance Milano to Napoli` (archive) | 7.1–8.1 ms | 4.4 MB |
| misspelt city in the archive (`distance Sanf Fransisco to Paris`) | 17–22 ms | 4.1 MB |
| a keystroke in the TUI, in process (handle + render) | 0.23–0.85 ms | — |
| installing the archive (from `docs/REVIEW.md`) | 1.2 s | 6.1 MB |

**Verdict: yes.** The heavy work uses the right structures: a binary search by byte position on
the on-disk index (no database, nothing loaded), an external merge sort to build it, hash tables
for names, zones cached once loaded, Vincenty with a great-circle fallback. Three hot spots remain:

1. **macOS: `defaults` on every request** (`geo.rs:175`; **fixed** 15 Oct, `21949c0`: asked for only once a request has two places). `geo::request` works out the default
   unit (`locale::device_system`, `locale.rs:13`: up to 3 `defaults` processes, not cached), the
   number format (`numbers.rs:163`: 1 more) and opens the archive *before* it knows the request is a
   distance. So `aequalis 10 km mi` runs `defaults` on a Mac. Not measured here (no Mac). A
   process launch costs milliseconds, which is likely the biggest cost there. Fix: compute these
   only for a distance request, and read the locale through CoreFoundation
   (`CFLocaleCopyCurrent`, `CFPreferencesCopyAppValue`). It is a system framework, so there is still
   no dependency, and the same code serves iOS.
2. **Misspellings in the archive: +12–17 ms** (`cities.rs:341`; **fixed** 15 Oct, `8a59cc7`: bounded, no copy per line). Every index line sharing the
   first two letters is read (12,284 for `sa`), copied without spaces (`cities.rs:353`) and compared
   with a full Levenshtein. A bounded Levenshtein (stop once a row's minimum passes the limit) and no
   copy per line should cut it several times. It only runs for a name that is not found.
3. **Process start: ~3 ms of the ~5 ms per request** go to parsing the embedded JSON and building
   the name tables. In process (TUI, apps) this happens once. For the per-keystroke widget, a
   `build.rs` could pre-bake the tables. Optional.

Long input scales acceptably: 20,000 terms in 115 ms; 120 words of place names in 128 ms
(the distance parser tries each split).

## 4. Separating the business logic from the UI

### 4.1 UI inside the core today

| Where | What | Goes to |
|---|---|---|
| `config.rs:111` and fields | the TUI's menu, shortcut letters, button columns, history size, tab labels | TUI |
| `lib.rs:29` and `run`/`parse_options` | CLI help text and argument parsing | CLI |
| `json.rs` | the CLI's `--json` reply and the process's memory (`getrusage`) | reply → core API; memory → CLI |
| `about.rs` | ASCII logo, the process's memory and binary size | facts → core; the rest → each UI |
| `completion.rs` | bash/zsh completion scripts and the Enter hook | CLI |
| `notify.rs` | desktop notifications through `notify-send`/`osascript`; notice texts with CLI commands | data → core; showing → UI |

### 4.2 Business logic inside the TUI today (the apps need it too)

| Where | What |
|---|---|
| `tui/mod.rs:706` `preview()` | the as-you-type rules: no error while a word is being typed (also in `omarchy-widget/Panel.qml:37`), "now the target unit", "now the target format", the summary of a question |
| `tui/mod.rs` `grouping()` | the bracketed expression (`(12 + (12 / 3))`), not for dates |
| `tui/suggest.rs` | completion of units and archive cities at the cursor |
| `tui/search.rs`, `tui/currencies.rs` | ranking units and currencies for a query |
| `tui/measures.rs`, `tui/trivia.rs` | the unit classes with the Earth distance; the Trivia by section |

These become core functions (`preview`, `suggest`, `search_units`, `search_currencies`, `catalog`)
so every UI shows the same thing.

### 4.3 Operating-system couplings in the core

| Coupling | Where | Breaks on | Replacement |
|---|---|---|---|
| downloads through `curl` processes | `cities.rs`, `currency.rs`, `trivia.rs` | iOS (no child processes), Android | **the core does no network I/O:** it lists what to download and reads the files; each UI downloads (curl in the CLI, `URLSession`, Kotlin) |
| `gzip -dc` | `cities.rs` | Windows, iOS, Android | **removed in this pass** (plain `cities.json`) |
| starting itself for the background sync | `online.rs:202`, `tui/settings.rs:131` (`process_group`, Unix only) | Windows (compile), iOS, Android | CLI only; apps schedule the sync themselves (background tasks) |
| data folder from `XDG_CONFIG_HOME`/`HOME` | `files.rs:10` | Windows (no `HOME`), Android, iOS | given by the UI (CLI: XDG on Unix, `%APPDATA%` on Windows) |
| zone database `/usr/share/zoneinfo` | `time/tz.rs:281` | Windows (none), Android (a packed file) | a zone source given by the UI: the system folder, Android's `tzdata`, or a copy in the app |
| local time through the C library; the zone's name from `TZ` or `/etc/localtime` | `time/sys.rs`, `currency.rs:556` | iOS, Android, Windows: **local time is UTC** (`sys.rs` handles Linux and macOS only) | the device's zone by name from the UI, computed by `tz.rs` (fixes §2.1) |
| locale: `setlocale`, `localeconv`, `nl_langinfo`, `strptime`; `defaults` on macOS | `numbers.rs`, `locale.rs`, `time/sys.rs` | everything but Linux/macOS; `setlocale` is process-wide, unsafe in an app's threads | a `Locale` given by the UI: decimal and grouping, date order and pattern, measurement system, language |
| `getrusage` declared unconditionally | `json.rs:131` | Windows (link error) | CLI only, per platform |
| `FileExt::read_at` (Unix) | `cities.rs:23` | Windows (compile) | a small helper: `read_at` on Unix, `seek_read` on Windows |
| desktop notifications, clipboard tools | `notify.rs`, `tui/clipboard.rs:14` | Windows, apps | UI. The TUI can also copy with OSC 52, an escape sequence most terminals understand, Windows Terminal included |

**The `ios` branch already found most of these** (`http.rs`, `aequalis_init`). Three issues remain
there:

- `aequalis_init` configures the core through `std::env::set_var` (`XDG_CONFIG_HOME`, `TZ`, `TZDIR`).
  That races with any other thread reading the environment.
- `time/sys.rs` is unchanged, so on an iPhone local time is UTC whatever zone the app passes.
- `inflate.rs` (478 lines) exists only to unpack the gzip archive, and the plain archive makes it
  unnecessary.

### 4.4 The library's API (proposal)

```rust
/// What the UI knows about the device, given once at start (no environment variables).
pub struct Platform {
    pub data_dir: PathBuf,
    pub zone: Option<String>,        // "Europe/Rome"
    pub zoneinfo: ZoneSource,        // Dir(path) | AndroidTzdata(path) | Bundled(path)
    pub locale: Locale,              // decimal, grouping, date order/pattern, metric, language
}

pub struct Engine { /* config, settings, caches */ }

impl Engine {
    pub fn new(platform: Platform) -> Engine;
    pub fn evaluate(&self, input: &str, options: &Options) -> Reply;   // Enter
    pub fn preview(&self, input: &str, options: &Options) -> Preview;  // as you type
    pub fn suggest(&self, input: &str, cursor: usize) -> Vec<Suggestion>;
    pub fn catalog(&self) -> Catalog;                                  // modes: units, currencies, trivia
    pub fn settings(&self) -> Vec<SettingView>;
    pub fn set(&self, key: &str, value: Option<&str>) -> Result<(), String>;
    pub fn notices(&self) -> Vec<Notice>;
    pub fn sync_plan(&self, auto: bool) -> Vec<Download>;              // url, file to save, timeout
    pub fn ingest(&self, download: &Download) -> Result<String, String>; // read the saved file
}

pub enum Reply {
    Value { value: Value, plain: String, display: String, notes: Vec<String>, warnings: Vec<String> },
    Ambiguous(Ambiguity),            // kind, text, note, choices (each a complete request)
    Error { kind: ErrorKind, message: String },
}
```

The network stays outside the core ("sans-IO"): `sync_plan` says what to fetch, the UI saves each
download to the given file and calls `ingest`. The CLI loops over it with `curl`; the apps use
their own downloader in their own background task. No callbacks cross the FFI, and tests can
stay hermetic. Today they block the network with an unreachable proxy (`tests/common/mod.rs`), a
trick that only works with `curl`.

### 4.5 Bindings, still without dependencies

- **C interface** (as on the `ios` branch): `aequalis.h`, UTF-8 JSON strings in and out, `aequalis_free`.
  Swift calls it directly.
- **Android:** JNI functions exported by hand from the same crate. They need three `JNIEnv`
  functions (`GetStringUTFChars`, `ReleaseStringUTFChars`, `NewStringUTF`), declared in a few lines,
  with no `jni` crate. With sans-IO there are no calls back into Java.
- **This revises the UniFFI suggestion made earlier:** with no dependencies allowed and the C
  interface already written, generated bindings don't pay for themselves.

## 5. Per platform

| Target | UI | What it needs |
|---|---|---|
| Linux | TUI (today) | nothing beyond the split; the CLI crate supplies the defaults (XDG, `/usr/share/zoneinfo`, libc locale, curl) |
| macOS | TUI (today) | CoreFoundation for the locale instead of `defaults` (§3.1); blocking tests in CI |
| Windows | TUI (new) | raw console and escape sequences through `kernel32` (`GetConsoleMode`/`SetConsoleMode` with virtual-terminal input and output, `GetConsoleScreenBufferInfo`, UTF-8 code page); `%APPDATA%\aequalis`; a bundled zone database; the device's zone name (ICU's `ucal_getDefaultTimeZone` in `icu.dll`, Windows 10 1903+, or the CLDR Windows-to-IANA table); `GetLocaleInfoEx` for the locale; `seek_read`; `creation_flags` instead of `process_group`; OSC 52 for the clipboard. `curl.exe` ships with Windows 10 since 1803, and gzip is no longer needed. A `windows-latest` job in the release workflow. |
| Android | Kotlin + Compose (new) | `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android` with the NDK linker; the JNI exports; zones from `/apex/com.android.tzdata/etc/tz/tzdata` (one packed file, short to read) or a copy in the app; the locale from Kotlin |
| iOS | SwiftUI (`ios` branch) | rebase; drop `inflate.rs` and the environment variables; local time by zone name (§4.3) |

## 6. Repository layout (proposal)

```text
aequalis/
├── Cargo.toml              [workspace] members = ["core", "cli", "ffi"]; one version for all
├── Cargo.lock
├── core/                   aequalis-core: the library, no dependencies, no network, no processes
│   ├── data/               countries.json, currencies.json, places.json (+ zones), trivia.json
│   └── src/
│       ├── lib.rs          the API (§4.4): Engine, Platform, Reply, Preview, Catalog
│       ├── calc.rs  convert.rs  units.rs (today config.rs, without the TUI's fields)
│       ├── time/           mod.rs, calendar.rs, parse.rs, format.rs, tz.rs
│       ├── geo.rs  cities.rs  currency.rs  trivia.rs  planets.rs
│       ├── sync.rs         today online.rs: plan and ingest, no downloads
│       ├── settings.rs  notices.rs  files.rs  json.rs
│       └── preview.rs  suggest.rs  search.rs   (from the TUI, §4.2)
├── cli/                    aequalis (binary): arguments, --json, completion, the TUI
│   └── src/
│       ├── main.rs  args.rs  completion.rs  reply_json.rs
│       ├── platform/       unix.rs, macos.rs, windows.rs: paths, zones, locale, curl, memory
│       └── tui/            editor, keys, picker, render, term (unix.rs, windows.rs), …
├── ffi/                    aequalis-ffi (staticlib + cdylib): the C interface and the JNI exports
│   ├── include/aequalis.h
│   └── src/  lib.rs  c.rs  jni.rs
├── apps/
│   ├── android/            Gradle project (Kotlin + Compose), loads libaequalis_ffi.so
│   ├── ios/                XcodeGen project (SwiftUI), AequalisCore.xcframework
│   ├── omarchy-widget/     moved from the root
│   └── raycast/            from the macos-app branch, if kept (the untracked raycast/ at the
│                           root holds only node_modules and a generated raycast-env.d.ts)
├── scripts/                build-cities.py, build-ios.sh, build-android.sh, bench.py, coverage.py
├── docs/                   SPEC.md (tracked), REVIEW.md, CORE_REVIEW.md, Accepted_Syntax.md, …
└── .github/workflows/      ci.yml (Linux, macOS, Windows; Android and iOS builds),
                            release.yml (+ Windows), cities.yml
```

The core's dependency rule can be enforced in CI: `cargo tree -p aequalis-core --edges normal`
must list only the crate itself.

## 7. Changes made in this pass

**The core as a library.** `src/lib.rs` holds every module and the request pipeline;
`src/main.rs` is only the `aequalis` command (`main` and the terminal's `ask`). No behavior change;
the release binary is 2 KB larger. *The `ios` branch made the same split*: merging it, its
`lib.rs`/`main.rs` conflict with these; they hold the same code, so keep master's and add the
branch's `ffi`, `http` and its `Cargo.toml` feature.

**The city archive as plain JSON:**

- `cities.rs`: the address is `…/releases/download/cities/cities.json`, saved by `curl -o` as it
  comes. Without gzip's checksum, the header's `count` is checked: an archive cut short is refused
  (`incomplete: 7 of 9 cities`). A gzip file is refused with a message naming the setting. A failed
  conversion now leaves nothing behind: before, its half-built directory (up to 24 MB) stayed.
- `settings.rs`: the settings file saves every key, so existing files hold the old `….json.gz`
  address (yours does too). It is read as today's address and rewritten at the next save. A
  mirror's address is kept.
- `scripts/build-cities.py` writes `cities.json` and `cities.version` (`<built_at> <sha256 of
  cities.json>`), no `.gz`. `.github/workflows/cities.yml` publishes those two files.
- `scripts/bench.py` serves the plain file. Its archive step also ran `--sync cities`, which the
  CLI no longer accepts, so that step was broken: it now runs `--sync` with the network sources
  behind the tests' unreachable proxy.
- Tests: the three that gzipped the sample serve it plain; new tests for an archive cut short or
  compressed (`cities.rs`) and for the old address (`settings.rs`).
- `docs/SPEC.md` (not tracked) updated.
- Clippy: the split made two items public that clippy then flagged (`Object` without `Default`,
  `duration` returning `Result<_, ()>`). Fixed as on the `ios` branch, so the merge stays simple.

**The capital first, selected by default:**

- `scripts/build-cities.py` marks a country's capital (GeoNames' feature code PPLC) with
  `"capital": true`; the Cities workflow checks that Rome, Paris and Tokyo are marked.
- `cities.rs`: `City::capital`, a column of `cities.tsv`; `rank` puts a country's capital first,
  wherever the device is, then the order as before (the exact name in the device's country, that
  country, the exact name elsewhere, the rest; by population). A territory's capital (Hamilton,
  Bermuda: not a country of `countries.json`, `tz::country_by_code`) does not go first. Aequalis still
  asks: `Rome Paris` offers Rome, Italy → Paris, France first, also on a device in the US.
- An archive installed before has no capitals: `aequalis --sync` once the new archive is published.
- Tests: the sample archive marks Rome and Paris; new `the_capital_first`; the questions on a US
  device expect the capital first.
- Docs: README, `docs/Accepted_Syntax.md` (and the two wrong lines found there), `docs/SPEC.md`.

Checks: 362 tests pass (3 new); clippy reports 0 warnings, with and without the TUI; the build
without the TUI is clean; `cargo check` passes for iOS.

**On pushing it** (backward compatibility not needed, as decided):

- Every push to master is a release (`release.yml`), and a change to `build-cities.py` on master
  runs `cities.yml`. Both start with the push. Until the Cities workflow has published
  `cities.json`, `aequalis --sync` can't find it.
- `gh release upload --clobber` only replaces files of the same name, so the old `cities.json.gz`
  stays in the release, frozen. Installed v0.2.x binaries keep downloading it: they work, but with
  the last compressed archive, and every new `cities.version` looks to them like a new archive
  (a notice under `notify`, the same file again under `silent` or `aequalis --sync`). Deleting it
  (`gh release delete-asset cities cities.json.gz`) makes their downloads fail instead and finishes
  the switch.
- The plain archive is about 3 times the download (the sample compresses 2.6×; the real size is
  printed by the build script).

## 8. Suggested order

1. Merge the `ios` branch (C interface, Swift app); keep its `http.rs` seam for now, drop
   `inflate.rs`.
2. The platform seams (§4.3): `Platform` given at start, the device's zone by name through
   `tz.rs`, the locale as data, downloads planned and ingested. This fixes §2.1, iOS local time,
   and most of Windows.
3. Correctness: rounding (§2.2), plain and display results (§2.3, §2.4), depth limit (§2.6), sync
   per group (§2.7), the docs (§2.5).
4. Move the TUI's logic into the core (§4.2), so the TUI, the widget and the apps share it.
5. The workspace (§6).
6. The new UIs: Windows TUI, Android app; finish the iOS app.

## 9. Recheck, 15 Oct 2026: performance, cleanup, refactoring

Master at `44d6a5b` (the Android app's landscape layout), rechecked against the goals above:
what the code costs per request and per keystroke (the TUI's, the app's), what it repeats, and
which of §2's findings were still open. 12 commits, `659b6ab`..`58ce13f`; 377 tests pass (4
new), clippy clean in the four feature sets.

### Measured

In process, on this laptop (a phone is several times slower; the app calls the core on a
background thread since step 3 of `docs/TODO.md`):

| What | Before | After | Why |
|---|---|---|---|
| The app's Trivia panel (`ffi::trivia`) | 17.0 ms | 3.5 ms | a planet's mean distance from the Earth |
| `--trivia` (fresh process) | 20.6 ms | 6.1 ms | the same |
| A planet's distances from the Earth (`planets::from_earth`) | 1.0–2.2 ms | 0.06–0.17 ms | the same |
| TUI suggestions for `no` (two letters, the archive) | 20.9 ms | 3.7 ms | each city read once, ranked from its first fields |
| TUI suggestions for `12+4/2*s` (a calculation) | 1.4 ms | 0.08 ms | no misspelt places in a calculation |
| Archive prefix `rom` (the app's suggestions) | 0.71 ms | 0.21 ms | each city read once |
| Archive misspelling `Tokio` | 1.3 ms | 0.49 ms | bounded distance, no copy per line |
| `is_date_request("12 USA UAE")` (3–4 times per keystroke) | 141 µs | 3 µs | one shared `Config` |
| `convert_request`: `10 km mi` / `now Tokyo` / `Springfield, Illinois Chicago` | 177 / 114 / 848 µs | 66 / 21 / 160 µs | the same, and the currency stage |
| The currency stage on a request with no amount, per keystroke | 75–120 µs | 1 µs | an amount has digits, no currency name has any |
| A TUI keystroke (`keystroke_cost`) | 0.5–1.2 ms | 0.13–0.29 ms | all of the above |

A request in a fresh process stays 2–6 ms (the process's start and the tables built once
dominate); installing the archive stays at 6.1 MB.

### Fixed

- **Performance:** a planet's mean distance from the Earth is the average over every pair of
  positions of the two orbits (32 × 32), not over the next century: exact to 6 digits, within
  0.03% of an average over 2,000 years, and a true long-run mean (a century's moved by up to 0.9%
  with the century chosen) (`659b6ab`). One `Config` for the process; `names_city` only detects a
  country, without building its question; the currency stage steps aside without exactly one word
  with digits (`b797e47`). The archive: each city read once, ranked from its first fields, the
  misspelling scan bounded (`8a59cc7`). Suggestions: the cheap test first (`48bf228`). The device's
  unit, number format and country asked for only for a distance (`21949c0`). The app's panels load
  off the main thread (`1f2c9e6`).
- **Correctness:** nesting past 100 levels or 1,000 operations is an error, not a stack overflow
  (a paste in the app could crash it); the JSON reader too, past 64 (`e0b1105`). Halves away from
  zero, currencies too; `(-2)^0.5` is no real number, not "out of range" (`b69595e`). A failed
  group no longer stops the sync (`03d33ca`). The archive's prefix order: a city whose own name
  starts so before one only an old alias does (`new` offered San Diego before New Orleans)
  (`8a59cc7`). A compound unit alone (`100 m/s^2`) is a hint, not an error (`58ce13f`).
- **Cleanup:** `Value::strings` and `Object::list` replace the same code in six places
  (`bf387db`); the TUI shows the core's hint for a target unit instead of its own rule
  (`58ce13f`); `keystroke_cost` warms up and keeps the best of 5 runs (`4023f75`); `bench.py`
  reads `--sync`'s memory from the process itself (watching `/proc` once read 17.6 MB for 6.1).

### Left open (proposals)

| What | Why not now |
|---|---|
| A `rustfmt.toml` (`max_width = 160`, `use_small_heuristics = "Max"`): `cargo fmt` would then change 142 places, instead of 1,154 with the defaults | a style choice across every file |
| A written date alone (`12/04/2026`): the app shows `0.0015` (a division, as the command line does by design: a date needs `date` or a format after it); the TUI says "now the target format" | a choice for every UI: the core could give that hint, then `aequalis 12/04/2026` would be one too |
| The tables built once per process: ~1–1.5 ms of a 2–6 ms request (JSON allocations, ~0.6 ms; the name indexes) | felt only by the widget's process per keystroke, and well below a frame |
| Two-letter prefixes in the TUI read ~1,000 records (3.5 ms) | the population in `cities.idx` would avoid it: an archive format change |
| A misspelling scans every index line with its first two letters (`Sanf Fransisco`: 12,000 lines, 5.2 ms) | rare, and off the main thread in the app |
| `ffi::trivia` computes each popup twice (shown and in plain digits) | 3.5 ms in all, once per panel opening |
| §2.3–2.4 (plain and display results, currencies' minor units: `1902.53 JPY`), §2.10–2.12 | features, not fixes |
| `time.rs` is 2,900 lines (parse, format, calendar, cities, requests) | a split as §6 proposes is best done with the workspace |

## 10. Review, 21 Oct 2026: dead code, long functions, speed and memory, tests

`tui-redesign` at `2fec936`, the whole repository: the library, the command line and the TUI, the
Windows tray app, the Omarchy widget, the Android app's Kotlin, the scripts. Not committed yet. No
behaviour changes, except for speed and memory (below).

### Method

- **Dead code:** clippy with every feature set and on every installed target (Windows MSVC and
  GNU; Android arm64, armv7 and x86-64; macOS; iOS): the Linux CI never compiles the Windows and
  Android code. Then what no compiler sees in a library whose items are public: a script listing
  the public items that no other file names (by module, for a name defined twice), the fields
  never read, the widget's QML functions and properties, the Android app's declarations, the
  scripts nothing refers to.
- **Long functions:** a scanner pairing braces outside strings, characters and comments, over
  every `.rs` file, tests included.
- **Speed and memory:** a harness linking the library with an allocator that counts: the heap each
  table keeps once built, the allocations and the time of a request in process (best of 3; the same
  harness built from `2fec936` for the figures before); `scripts/bench.py` (a fresh process per
  request, 2 × 40 runs, the best median); `keystroke_cost` (best of 3).
- **Tests:** CI's runs (`cargo test --workspace` in debug, the `ffi` tests), the release build,
  the ignored tests; `scripts/coverage.py`, and the functions it says no test runs.

### Dead code and warnings

| Where | Found | Done |
|---|---|---|
| `src/jni.rs` | 28 clippy errors on Android (`not_unsafe_ptr_arg_deref`): the six JNI exports dereference the JVM's pointers, but were safe functions | `pub unsafe extern "system"`, each with its `# Safety` |
| `src/device/windows.rs:311` | two `transmute`s without types (Windows) | annotated, with `ToWindows` and `FromWindows` |
| `src/windows.rs:135` | `repeat(c).take(run)` (Windows) | `repeat_n` |
| `apps/windows/res.rs` | `% 4 != 0`, a complex type (every target: `build.rs` includes it) | `is_multiple_of`, `IconImage` |
| `src/tui/render.rs`, `action.rs`, `mod.rs` | the six panels listed four times | `Overlay::is_panel` |
| `src/find.rs` | every unit's names copied to be scored | `score` takes `&str` |
| `apps/android/…/ExampleUnitTest.kt` | Android Studio's template (`2 + 2 == 4`) | removed |
| `omarchy-widget/Panel.qml` | an `id` nothing uses (added that day) | removed |

Nothing else is dead: no public item without a use (two are for tests only, both
`#[cfg(test)]` already), no field never read (those left are C layouts: `termios`, `winsize`),
no unused QML function or Kotlin declaration, every script referenced. Clippy: 0 warnings in the
4 feature sets and on the 7 targets (before: 2 on Linux, 3 more on Windows, the 28 errors on
Android).

### Functions over 60 lines: 32 → 0

The same code, moved into functions named for what they do; the tests pass unchanged.

| Function | Lines | Now |
|---|---|---|
| `Config::default` | 312 | the table is `const CATEGORIES` (data, not a function); `default` is one line |
| `main` | 205 | `freeze_now`, `as_typed`, `settings_command`, `set`, `sync`, `interactive`, `requests`, `answered`, `print`, `notices`, `fail` |
| `time::request` `Parser::value` | 194 | a method per form of date (`with_input_word`, `keyword`, `time_of_day`, `timed`, `iso_word`, `stamp`, `local`, `guessed_first`, `written_alone`, `longest_date`, `not_a_date`); `date_read` for the five copies of "ask, note, format, zone" |
| `tui::render::board_view` | 137 | `board_clocks`, `jump_buttons`, `fitting`, `board_class`, `board_currencies` |
| `tui::action` `App::apply` | 123 | `edit_line`, `toggle_mouse`, `open_panel`, `toggle_keypad`, `use_shortcut`, `write_now`, `cancel`, `interrupt` |
| `time::written::read_numeric_date` | 115 | `three_numbers`, `date_orders`, `read_clock`, `meridiem` |
| Windows `popup::layout` | 110 | `header`, `about`, `reply`, `below`, `footer` |
| `time::tz` `Rule::parse` | 96 | `skip_name`, `read_clock`, `read_number`, `read_change` |
| `tui::render::unit_keys` | 95 | `panel_tabs` (the Trivia's too), `format_table`, `unit_buttons` |
| `convert_request` | 93 | `money`, `calculation` |
| `tui::run` | 88 | `session`, `Screen::update`, `Input::read` |
| `tui::render::render` | 87 | `wide`, `window` |
| `tui` `App::handle` | 87 | `settings_key`, `choice_key`, `help_key` |
| `time::request::place_typos` | 84 | `misspelt_spans`, `corrections`, `misspelt_note` |
| `tui::action` `App::click` | 81 | `remove_entry`, `copy_entry`, `click_board` |
| Windows `app::run` | 81 | `already_running`, `main_window` |
| `tui::settings::key` | 80 | `edit_key`, `list_key` |
| `tui::render::settings_window` | 79 | `settings_row`, `settings_list` |
| `tui::render::trivia` | 79 | `trivia_entries`, `trivia_about`, `trivia_live` |
| `time::request::evaluate` | 77 | `date_math` (and `Math`), `difference_in`, `date_result` |
| `trivia` test `popups` | 74 | five tests, one per subject |
| Windows `popup::proc_` | 73 | `edited`, `edit_colors`, `rescaled` |
| `tui::render::toolbar` | 72 | `online_indicator` |
| Windows `app::proc_` | 70 | `handled` |
| `tui::render::expression` | 68 | `suggestions` |
| Windows `popup::create` | 65 | `register`, `edit_control` |
| `json` `Reader::one_value` | 64 | `object`, `array`, `number` |
| `find::search` | 64 | a function per source |
| `currency::index` | 63 | `index` and `catalog` (below) |
| `tui::action` `App::mouse` | 62 | `press`, `drag` |
| `trivia::parse` | 62 | `entry`, `source` |
| `geo::question` | 62 | `offered`, `unclear_places` |

### Speed and memory (the library)

1. **Misspellings were checked by allocating.** Most requests ask whether a word is a misspelt
   place or currency (`10 km mi` asks it of `km mi`, `sqrt 16` of `sqrt`), against about 600
   names, and the distance allocated two vectors per name. `tz::Distance` now reads the word's
   letters once and keeps its row from one name to the next; the archive's scan reads each line's
   letters as they are, where it copied the line without its spaces.
2. `place_typos` joined a run of words before checking that they were words: checked first.
3. **The tables kept copies.** The currency index (192 KB, built by any request with a word) also
   held lower-case copies, prefixes and each currency's countries, which only searches and
   suggestions read: a `Catalog` built when one of them asks. The place names kept each name in a
   vector and again as a hash map's key: a sorted index and a binary search over the vector now
   (the first of equal names wins, as before).
4. **The data files' strings were copied twice.** The JSON reader built each string byte by byte
   (8 bytes at least: `EUR` too), and the tables copied it again; collected through `filter_map`,
   the tables kept up to twice their size. A string without escapes is now copied once, at its
   size; the tables take their strings from the document (`Value::take`, `into_string`,
   `into_strings`, `into_members`) and keep no spare room.
5. The unit table (`Config::default`) built 12 vectors at start: a `const`, nothing is built.
6. `find::score` (Ctrl+K, every keystroke) lowercased each name up to four times: once.
7. Installing the archive: `City::from_json` copied every string of its 70,000 lines: taken.

In process, warm, best of 3:

| Request | Allocations before | After | Time before | After |
|---|---|---|---|---|
| `10 km mi` | 877 | 317 | 58 µs | 45 µs |
| `sqrt 16` | 704 | 179 | 45 µs | 33 µs |
| `12/04/2026 unix` | 689 | 161 | 44 µs | 32 µs |
| `10:00AM Paris to New York` | 2,116 | 376 | 139 µs | 95 µs |
| `10:00am jappan romme` | 5,758 | 674 | 396 µs | 259 µs |
| `12+4/2`, `now Tokyo`, `Rome Paris`, `12 usd eur`… | unchanged | | unchanged (±1 µs) | |

The tables, built once per process:

| Table | Before | After |
|---|---|---|
| units (`Config::shared`) | 9.9 KB, 14 allocations | nothing |
| currencies | 64 KB, 2,827 allocations | 34 KB, 2,125 |
| countries, place names, currency index | 353 KB, 10,234 allocations, 764 µs | 204 KB, 6,563, 709 µs |
| trivia | 42 KB, 1,517 allocations | 31 KB, 1,137 |
| everything, after the requests above | 548 KB held (peak 604 KB) | 338 KB (peak 380 KB) |

A fresh process per request (`bench.py`): 1.63–1.99 ms → 1.51–1.83 ms, peak memory 4.2–4.8 MB →
4.0–4.5 MB; `now Tokyo` and `--trivia` unchanged. A TUI keystroke: `10:00am jappan romme` 163 →
130 µs, `Rome Paris` 87 → 74 µs, `now Tokyo` 77 → 67 µs; the others unchanged (drawing dominates).

### Tests

- **Added:** 12 tests, and checks in 6 others. The Android zone bundle (`zones_from_an_android_bundle`:
  only the app reads it, nothing tested it); POSIX rules with `Jn`, `n` and no dates (the US
  rule); the distance kept from one name to the next, against a plain Levenshtein; the names'
  binary search (the first of equal names); taking a document's values, a string's size; the
  countries of a currency (the app's currency list); a country as a place; points nearly opposite
  (Vincenty does not converge: the great circle); the TUI's list of a setting's values on screen;
  the C interface's lists; `--reset-settings`, `auto` and the currencies of `--settings --json`;
  format names (`iso-week`); `Default` and `is_empty`, which nothing called.
- `scripts/coverage.py` runs the `ffi` tests too: `src/ffi.rs` (514 lines) was not measured.
- CI has a `clippy` job: Linux (3 feature sets), Windows (the command line and the tray app) and
  the Android library for its 3 ABIs, any warning an error. Checking a target needs only its
  standard library, so one Linux job does them all.

| Run | Passed | Failed | Ignored |
|---|---|---|---|
| `cargo test --workspace` (debug, as CI) | 495 (483 before) | 0 | 3 |
| `cargo test --features ffi --lib ffi` | 10 (9) | 0 | 0 |
| `cargo test --release` | 495 | 0 | 3 |
| ignored `random_requests_never_panic_long` (75,000 requests) | 1 | 0 | |
| ignored `keystroke_cost` | 1 | 0 | |
| ignored `real_archive` | not run: it needs `scripts/build-cities.py target` (the GeoNames download) | | |

Coverage (`scripts/coverage.py`):

| | Before | After |
|---|---|---|
| lines | 95.9% of 16,162 | 95.9% of 16,765 (`ffi.rs` measured now) |
| functions | 95.5% of 2,385 | 95.8% of 2,658 |
| like for like (without `ffi.rs`, `config.rs`) | 95.7% of lines, 95.5% of functions | 96.4%, 96.4% |
| `src/ffi.rs` | not measured | 83.5%, 114 of 131 functions |
| `src/time/tz.rs` | 90.6% | 95.6% |
| `src/main.rs` | 84.4% | 90.8% |
| `src/json.rs` | 93.8% | 95.6% |
| `src/config.rs` | 100% of 426 lines | 83.8% of 117: its `const fn`s run in the compiler now |

### Left open (proposals)

| What | Why not now |
|---|---|
| The tables built at compile time (`build.rs`: the names normalised by the build) | about 1.1 ms of the 1.5–1.8 ms of a request in a fresh process (the Omarchy widget starts one per keystroke); a change in how the data is built |
| The date detection still allocates 160–380 times per request (each reading tried in turn) | microseconds; a rewrite of the detection |
| Code no test runs: the clipboard, desktop notifications, the process-wide setters (`files::set_dir`, `tz::set_source`, `set_zoneinfo`, `http::register`, `time::fixed_now`: the first call wins, so a test would set them for every other one), the C entry points that start the app (`aequalis_init`, `aequalis_seed`, `aequalis_sync`) | a process per test, or the platforms' own test runs |
| The Android app: Gradle not run; `ExampleInstrumentedTest` is a template too, and needs a device | |
| `Cargo.lock` is still ignored (§2.15) | |
