# To do

Open work, in order. Details and evidence: `docs/ANDROID_UX_REVIEW.md` (the `#` numbers below).
Tick an item when it is done, and note the commit.

## TUI redesign (Android-like, keyboard first, mouse as a rescue)

Branch `tui-redesign`. Layout: toolbar, history as a scrollback, request line, band (keypad or
panel), status; no modes.

- [x] 0. The TUI leaves the library (a module of the `aequalis` binary).
- [x] 1. Input groundwork: F-keys, Alt, word moves and mouse reports decoded; keys and clicks
      share `Action`s (`src/tui/action.rs`).
- [x] 2. No modes: toolbar (F2 Measures, F3 Currencies, F4 Trivia, F5 History, F10 Settings, Alt
      letters), Ctrl+K unit search, shell-like ↑ ↓, Esc / Ctrl+C steps, REPL layout (Standard).
- [x] 1b. Windows: console raw mode, VT input and output, a timed read of the console (`src/tui/term.rs`),
      the clipboard through PowerShell. Type-checked for both Windows targets; its first run in a real
      console is the CI job `windows-tui` (`scripts/windows-tui-smoke.py`).
- [x] 3. The band under the request line: the keypad on request (F9: Basic, Sci, hidden; `tui.keyboard`
      now defaults to off) and the panels (Measures, unit keys, Currencies, Ctrl+K search, Trivia),
      which take the rows they need. The suggestion chips, the result beside the request, buttons for
      the ambiguous date and a History panel are left to the mouse phase.
- [x] 3b. Shortcuts at the right of the keypad: five units (Alt+1…5) and five currencies (Alt+6…0),
      kept with their hits in `tui.favourite_units` / `tui.favourite_currencies` (`src/favourites.rs`):
      a new one replaces the listed one with the fewest hits (the last of them), in its place; every
      pick (Ctrl+K, Currencies, unit keys, shortcuts) and every unit or currency of a request that
      worked counts. `d` in Settings clears them (the defaults: USD EUR JPY CNY AED, units by system).
- [ ] 4. Mouse: hit map, `tui.mouse`, terminal mouse modes.
- [ ] 5. `src/find.rs` in the core and the Ctrl+K palette (units, currencies, cities, trivia,
      settings, history).
- [ ] 6. Compact (< 60 columns) and Wide (≥ 100) layouts.
- [ ] 7. Docs, help, dead-code cleanup.

## Android app

### 1. Keep the user's work, and keys that work for everyone (done, 15 Oct 2026)

- [x] The request survives rotation, dark mode, font and language changes, and Android stopping
      the app (#2): `rememberSaveable` for the request, Enter, the open panel, Settings.
- [x] Keys are buttons for accessibility services, with names for ⌫ ⏎ × ÷ √ π… (#1); the request
      line and the panel buttons have names too.
- [x] Keys show they are pressed: a ripple (#4).
- [x] ⌫ held repeats, as on every keyboard; clearing gets its own key (#5).
- [x] Results are announced to screen readers (#11, cheap with #1).

### 2. Landscape, and copy / share / paste (done, 15 Oct 2026)

- [x] Landscape prototype (#3): digits and operators on the right, in the calculator's grid (the
      functions beside it in Sci); the toolbar (C, 123, Measures…, ⚙) on the left, vertical; the
      letters under the request, at about their portrait size; a panel takes the right side.
- [x] Tap or long-press the result: copy; Copy and Share beside it (#7). Share sends
      `request = result`.
- [x] Long-press the request line: paste, on one line (#7); for screen readers, a Paste action.

### 3. Responsiveness (done, 15 Oct 2026)

- [x] The core's calls (`evaluate`, `suggestions`) off the main thread, the previous answer shown
      until the new one arrives (#6). With Compose's `produceState` on `Dispatchers.Default`, not a
      `ViewModel`: the request already survives rotation (`rememberSaveable`), and an answer takes
      milliseconds to compute again. The panels' lists (Measures, Currencies, Trivia) still load
      on the main thread, once per opening.

### 4. Asked on 15 Oct 2026 (next session)

- [x] Trivia: the length of a day and of a year for every planet.
- [x] Recheck the whole code: performance, cleanup, refactoring (15 Oct 2026; `docs/CORE_REVIEW.md`
      §9: what was measured, fixed, and left open).
- [x] Ship the city archive and the exchange rates valid at build time inside the app, so the
      features work before any download; with currencies, say how old the rates are (1h, 2d, 3w,
      1m, 1y…). `scripts/build-android.sh` downloads them into the app's assets; the app installs
      them at its first start.
- [x] Check on screen: the Trivia measure chips (a chip writes `0.3871 au `) and the Wikipedia page
      inside the app (with Open in browser).

### From the recheck (`docs/CORE_REVIEW.md` §9, to decide)

- [x] A `rustfmt.toml` (`max_width = 160`, `use_small_heuristics = "Max"`) and one `cargo fmt`
      (142 changes).
- [x] A written date alone (`12/04/2026`): a hint in every UI ("now the format to convert to…");
      `12/04/2026 num` is the division.

### Later (medium issues)

- [x] while writing math expression, in the preview keep the grouping via brackets as the TUI
- 
- [x] the about function is missing (Settings › About)
- [x] add in the about the link to install Aequalis on mac and Linux
- [x] add share feature to share the mac, Linux and Android version (when available) installation link (github repo / official play store app)
      (Android: `PLAY_STORE` in AboutScreen.kt, once published)
- [ ] add localization
- [x] Icons instead of text characters, with names (#8): ⌫ ⏎ ⚙ ← ✕ ⌨, the panels' keys and titles,
      the unit classes and the Trivia sections (Material Icons, as paths in `Icons.kt`).
- [x] A tap on "Aequalis": the memory the app uses (Android's figures by kind, the Rust core's own as
      its allocator counts it, `src/alloc.rs`), the phone's free memory.
- [ ] Settings for people: titles, groups, choices for intervals, a top app bar (#9).
- [x] Search in the Currencies panel (#10): by code, name or symbol, with the phone's keyboard.
- [x] A fast start, even on old phones: the release build installable (R8, a Baseline Profile
      compiled at install), the core started in the background, the window in the app's colours.
      `scripts/install-android.sh` installs it, `scripts/android-startup.sh` measures it.
- [ ] Errors and hints told apart by more than colour (#12).
- [ ] Toolbar and panel buttons at least 48 dp (#13).
- [x] History (#14): a History panel (a tap: the request back in the line, to change; ⧉ copies the
      result; ⋮ Share, Remove; Clear the history), the three latest on the empty screen, kept in
      `files/history.txt`. A request goes in once done with: ⏎, Copy, Share, C, the app left.
- [ ] First-launch examples (#14).
- [x] After a unit is chosen (`10 cm `), the units of its kind offered at once; Measures opens at
      its class.
- [ ] Tablets and foldables: a maximum keyboard width or two panes (#15).

## Elsewhere

- [ ] Downloads on the phone: exchange rates and the city archive (the app's downloader).
- [ ] Push the local commits (`git status` says how many).

