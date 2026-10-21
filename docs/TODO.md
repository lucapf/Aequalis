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
      F9 opens the keyboard on Sci when it fits with the shortcuts (else Basic) and gives it the focus:
      Tab / Shift+Tab go round Basic, Sci, Units, Money; ↑↓ ←→ Enter in the columns; letters and digits
      still go to the line; Esc gives the focus back. No Tab completion while the keyboard is shown.
- [x] 4. Mouse: a hit map filled by what draws the screen (`render::Hit`, `action::Target`); a left click
      does what the key does: toolbar, keyboard keys, Basic / Sci / Units / Money, shortcut buttons,
      history (request back into the line, result copied), the live answer (copied), the line
      (cursor), suggestions, items and tabs of every panel, the settings rows, the readings of an
      ambiguous date; a click outside a window closes it; the wheel moves through the window open.
      `tui.mouse` (default on) and Alt+O; Shift+drag still selects text. On Windows Quick Edit is
      turned off while the mouse is on (untested on a real console: see the CI job `windows-tui`).
      The suggestions are buttons (`[ mi ]`) and the answer is at the right edge (`= 6.2137 mi ⧉`, a click
      copies it). F5 is a History panel (all requests, newest first, filtered by typing; ↩ ⧉ ✕ or
      Enter / Alt+Y / Del) in place of the old history focus mode.
- [x] 5. `src/find.rs` in the core and the Ctrl+K palette: units, currencies, settings, trivia, cities
      (from the archive, two letters at least, the device's country first) and earlier requests, each
      with its kind. Enter writes a unit, currency or city into the request, a trivia entry's value,
      reopens an earlier request, or opens the Settings on a setting.
- [x] 6. Compact and Wide: below 60 columns the toolbar has short names (keys only under 52) and the
      suggestions are plain words; from 120 columns (and 14 rows) the keyboard and the panels open in
      a pane at the right (40% of the width, 52 to 90) while the history and the request keep the
      left; under that they are in the band under the line.
- [x] 6b. The board (Wide, from 120 columns): the pane at the right always holds clocks, every unit
      class with its five most used units (boxes three rows high, used ones lit; one row per class
      when the screen is short), and the five most used currencies (the most traded ones to start:
      USD EUR JPY GBP CNY) valued in the local currency (`currency.local`). From 150
      columns the big keyboard (keys three rows high, with C ← → to ␣ for a pad without a keyboard)
      sits at its left; `tui.keyboard` is auto / true / false. The usage is counted in `usage.json`.
- [x] 6c. Simpler settings: the Settings window opens on four settings (offline, currency.local,
      distance.unit, precision); `a` / a row shows the advanced ones; Reset to the standard settings
      (Enter twice, `R`); `tui.base_currency` is gone (the board uses `currency.local`), old files load.
      Android's settings screen uses them too (6d).
- [x] 6d. Android's Settings laid out as the Omarchy widget's: key, value (a tap for yes/no; a list
      under the row for a choice and the currency, the currency's filtered as you type; `auto (AED)`),
      `default` when changed; Advanced (N changed), Reset (twice), Sync; `online (2h old)` under the
      title. The app downloads at last: `Core.sync` (src/jni.rs) has the core call `Core.fetch`
      (Kotlin, `HttpURLConnection`) for each download; what is due is synced at start.
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

