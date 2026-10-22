# To do

Three lists: what we are on now, the backlog by priority, and what is done.
Every item has an id of its own (`size-4`, `android-27`): its feature and a
number, never reused. Details and evidence: `docs/ANDROID_UX_REVIEW.md` (the `#`
numbers), `docs/SPEC_SIZES.md` (the `§` numbers in size items). When an item is
done, move it to DONE and note the commit.

## WORKING / planned

- [ ] **size-3** Check Nike's women's and kids' foot lengths on nike.com, by eye
      (§6.1 ¹ ²).

## TODO

By priority, most urgent first.

- [ ] **size-9** Phase 1, TUI: the Sizes class in Measures (its letter, the
      system tabs, the Shoes group writing `shoes`), the shoes line beside a
      currency (the list, §4.1), a two-chart result at the right edge and in the
      history, the board.
- [ ] **size-10** Phase 1, Omarchy widget: the Sizes class in Measures, the
      shoes line beside a currency (the list, from `extra`), a two-chart result
      and its Copy.
- [ ] **size-11** Phase 1, Android: the Sizes class in the Measures panel (the
      system tabs, the Shoes group, its icon), the shoes line beside a currency
      (the list, from `extra`), Copy and Share of a two-chart result, the quick
      window (QuickActivity) answering size requests; landscape and two panes.
- [ ] **size-19** Phase 1, TUI: the Sizes menu item, `F4 👕👟 Sizes` and Alt+Z,
      opening the groups and system keys (§7); Trivia moves from F4 to F6. The
      help, `--help`, README and `scripts/windows-tui-smoke.py` (it clicks the
      toolbar) follow the new keys.
- [ ] **size-20** Phase 1, Android: the Sizes item in the toolbar after
      Currencies, with the Material Symbols `apparel` and `steps` icons in
      `Icons.kt` (§7).
- [ ] **size-21** Phase 1: a Shoes section in Trivia: Nike's Men, Women and Kids
      tables (US, UK, EU, JP, foot length in cm), in the core's trivia data and
      in the TUI's and Android's Trivia (§7).
- [ ] **size-12** Phase 1: tried on the phone, released as a `minor`.
- [ ] **size-13** Phase 2, core: women's and men's clothing charts (dresses,
      tops and trousers, suits, collar shirts, letters, the CN `160/84A` labels)
      in `sizes.json`; a person word alone means clothing (`12 US woman EU`;
      men's asks `which?`); the §9 phase 2 tests.
- [ ] **size-14** Phase 2, UIs: the Women and Men groups in Measures, in the
      TUI, the Omarchy widget and Android; the `which?` question's choices.
- [ ] **size-15** Phase 2: tried on the phone, released as a `minor`.
- [ ] **size-16** Phase 3, core: kids' clothing (height, age), bras (band and
      cup), hats (fractions: `7⅛`); the §9 phase 3 tests.
- [ ] **size-17** Phase 3, UIs: the Kids, Bras and Hats groups in Measures, in
      the TUI, the Omarchy widget and Android.
- [ ] **size-18** Phase 3: tried on the phone, released as a `minor`.
- [ ] **git-1** Push the local commits (`git status` says how many).
- [ ] **tui-7** Docs, help, dead-code cleanup.
- [ ] **android-26** Errors and hints told apart by more than colour (#12).
- [ ] **android-27** Toolbar and panel buttons at least 48 dp (#13).
- [ ] **android-28** Settings for people: titles, groups, choices for intervals,
      a top app bar (#9).
- [ ] **android-29** First-launch examples (#14).
- [ ] **android-30** Localization.
- [ ] **android-31** Tablets and foldables, a maximum keyboard width or two
      panes (#15). Two panes are done (tui-6e): check what is left.
- [ ] **android-32** The tabletop posture (`androidx.window`): the result above
      the fold, the keys below.
- [ ] **android-33** Downloads on the phone, exchange rates and the city archive
      (the app's downloader). Probably done by tui-6d (`Core.sync`): check, then
      move to DONE.

## DONE

- [x] **size-8** Phase 1, the core for the UIs: the Sizes class in
      `ffi::measures` (a new shape: the systems as tabs, the Shoes group, its
      item words), suggestions while typing in `suggest.rs` (item, person and
      body words; the systems after a size), Copy of a two-chart result
      (`reply::value_of`: the first chart's value).
- [x] **size-7** Phase 1: `tests/sizes.rs` on the `italy` and `us` machines
      (§9), and the size requests in the fuzzer (`fuzz/`).
- [x] **size-6** Phase 1: currency stays the default, with the shoes line beside
      it (§4.1): `42 eu us` answers with a list (the currency, then
      `shoes: 42 eu = US 8.5 men, US 10 women`), on the CLI's stdout too; the
      extra results in `Output`, as `extra` in `--json` and in `reply::view`
      (the FFI/JNI answer); `42 eu` gets the line under its hint, in the
      region's system.
- [x] **size-5** Phase 1, rules in `src/sizes.rs`: the request grammar (§3), the
      nearest row, the two charts merged into one result (§6.1), what is missing
      shown instead of asked (no person, no target, the table: §4.1), the
      region's system (§4.2), the notes; `sizes::request` first in
      `convert_request`; later items answered with the error
      `only shoes for now`.
- [x] **size-4** Phase 1, data: the shoe charts (Nike's and Clarks's, men,
      women, kids) and every item, body and person word of the 10 languages in
      `src/data/sizes.json`, compiled by `build/tables.rs`.
- [x] **size-22** Decided: with no person, `result` the first person's and
      `extra` the others, Copy the first value; the table, `result` the text and
      `extra` a row per person, Copy the text. Trivia's shoe tables as entries
      with a table popup, a row writing its request; answers of several lines
      shown below the request (spec §4.1, §7).
- [x] **size-2** The last phase 1 decisions in the spec: beside a currency that
      could be a shoe size, a list with a shoes line (`extra` in `--json` and
      the UIs' answer); the "between" note named by chart; clothing in phase 1
      is `only shoes for now`; kids' shoes get a region column. Branch `sizes`,
      commit 2b3eb36.
- [x] **size-1** The spec written and agreed (`docs/SPEC_SIZES.md`, 22 Oct
      2026): sizes only when asked (an item, body or person word), currency the
      default; shoes from Nike's and Clarks's own charts, both named in the
      result where they differ (Adidas and Brannock not named); the foot
      measured by the charts' foot length, not JP.

### TUI redesign (Android-like, keyboard first, mouse as a rescue)

Branch `tui-redesign`, merged to master. Layout: toolbar, history as a
scrollback, request line, band (keypad or panel), status; no modes.

- [x] **tui-0** The TUI leaves the library (a module of the `aequalis` binary).
- [x] **tui-1** Input groundwork: F-keys, Alt, word moves and mouse reports
      decoded; keys and clicks share `Action`s (`src/tui/action.rs`).
- [x] **tui-2** No modes: toolbar (F2 Measures, F3 Currencies, F4 Trivia, F5
      History, F10 Settings, Alt letters), Ctrl+K unit search, shell-like ↑ ↓,
      Esc / Ctrl+C steps, REPL layout (Standard).
- [x] **tui-1b** Windows: console raw mode, VT input and output, a timed read of
      the console (`src/tui/term.rs`), the clipboard through PowerShell.
      Type-checked for both Windows targets; its first run in a real console is
      the CI job `windows-tui` (`scripts/windows-tui-smoke.py`).
- [x] **tui-3** The band under the request line: the keypad on request (F9:
      Basic, Sci, hidden; `tui.keyboard` now defaults to off) and the panels
      (Measures, unit keys, Currencies, Ctrl+K search, Trivia), which take the
      rows they need. The suggestion chips, the result beside the request,
      buttons for the ambiguous date and a History panel are left to the mouse
      phase.
- [x] **tui-3b** Shortcuts at the right of the keypad: five units (Alt+1…5) and
      five currencies (Alt+6…0), kept with their hits in `tui.favourite_units` /
      `tui.favourite_currencies` (`src/favourites.rs`): a new one replaces the
      listed one with the fewest hits (the last of them), in its place; every
      pick (Ctrl+K, Currencies, unit keys, shortcuts) and every unit or currency
      of a request that worked counts. `d` in Settings clears them (the
      defaults: USD EUR JPY CNY AED, units by system). F9 opens the keyboard on
      Sci when it fits with the shortcuts (else Basic) and gives it the focus:
      Tab / Shift+Tab go round Basic, Sci, Units, Money; ↑↓ ←→ Enter in the
      columns; letters and digits still go to the line; Esc gives the focus
      back. No Tab completion while the keyboard is shown.
- [x] **tui-4** Mouse: a hit map filled by what draws the screen (`render::Hit`,
      `action::Target`); a left click does what the key does: toolbar, keyboard
      keys, Basic / Sci / Units / Money, shortcut buttons, history (request back
      into the line, result copied), the live answer (copied), the line
      (cursor), suggestions, items and tabs of every panel, the settings rows,
      the readings of an ambiguous date; a click outside a window closes it; the
      wheel moves through the window open. `tui.mouse` (default on) and Alt+O;
      Shift+drag still selects text. On Windows Quick Edit is turned off while
      the mouse is on (untested on a real console: see the CI job
      `windows-tui`). The suggestions are buttons (`[ mi ]`) and the answer is
      at the right edge (`= 6.2137 mi ⧉`, a click copies it). F5 is a History
      panel (all requests, newest first, filtered by typing; ↩ ⧉ ✕ or Enter /
      Alt+Y / Del) in place of the old history focus mode.
- [x] **tui-5** `src/find.rs` in the core and the Ctrl+K palette: units,
      currencies, settings, trivia, cities (from the archive, two letters at
      least, the device's country first) and earlier requests, each with its
      kind. Enter writes a unit, currency or city into the request, a trivia
      entry's value, reopens an earlier request, or opens the Settings on a
      setting.
- [x] **tui-6** Compact and Wide: below 60 columns the toolbar has short names
      (keys only under 52) and the suggestions are plain words; from 120 columns
      (and 14 rows) the keyboard and the panels open in a pane at the right (40%
      of the width, 52 to 90) while the history and the request keep the left;
      under that they are in the band under the line.
- [x] **tui-6b** The board (Wide, from 120 columns): the pane at the right
      always holds clocks, every unit class with its five most used units (boxes
      three rows high, used ones lit; one row per class when the screen is
      short), and the five most used currencies (the most traded ones to start:
      USD EUR JPY GBP CNY) valued in the local currency (`currency.local`). From
      150 columns the big keyboard (keys three rows high, with C ← → to ␣ for a
      pad without a keyboard) sits at its left; `tui.keyboard` is auto / true /
      false. The usage is counted in `usage.json`.
- [x] **tui-6c** Simpler settings: the Settings window opens on four settings
      (offline, currency.local, distance.unit, precision); `a` / a row shows the
      advanced ones; Reset to the standard settings (Enter twice, `R`);
      `tui.base_currency` is gone (the board uses `currency.local`), old files
      load. Android's settings screen uses them too (tui-6d).
- [x] **tui-6d** Android's Settings laid out as the Omarchy widget's: key, value
      (a tap for yes/no; a list under the row for a choice and the currency, the
      currency's filtered as you type; `auto (AED)`), `default` when changed;
      Advanced (N changed), Reset (twice), Sync; `online (2h old)` under the
      title. The app downloads at last: `Core.sync` (src/jni.rs) has the core
      call `Core.fetch` (Kotlin, `HttpURLConnection`) for each download; what is
      due is synced at start.
- [x] **tui-6e** Large screens (a foldable open, tablets; Material's window
      classes: at least 600 dp wide and 480 dp high): the supporting-pane
      layout. The request, the result and the keyboard (at a phone's width, at
      most 480 dp) on the left; a panel always open on the right, its tabs
      History · Measures · Currencies · Trivia, what it writes leaving it open;
      the panes meet at the fold. Phones, portrait and landscape, unchanged.
- [x] **tui-6f** Aequalis from anywhere: a small window over the app in use
      (`QuickActivity`), the phone's keyboard, the answer as you type, Copy
      (value or expression), Open in Aequalis. Opened from Share, from the Quick
      Settings tile (`QuickTile`) and from a long press on the icon
      (res/xml/shortcuts.xml). Not in the text-selection menu: too invasive (and
      MIUI didn't show it). Commits 4c66c77, 570eb31.
- [x] **tui-6g** A unit alone converts to the device's measurement system
      (`28 in` → cm on a metric phone, `10 km` → mi on an imperial one), as
      `12 usd` to the local currency (`convert::counterpart`: the closest in
      size, or the usual one: km/h ↔ mph, °F ↔ °C, hp → kW). A unit of the
      device's own system still asks for its target.
- [x] **test-1** A simulated machine (`machine::Mock`), the same results on
      every laptop (16aed28).

### Android app

Keep the user's work, and keys that work for everyone (15 Oct 2026):

- [x] **android-1** The request survives rotation, dark mode, font and language
      changes, and Android stopping the app (#2): `rememberSaveable` for the
      request, Enter, the open panel, Settings.
- [x] **android-2** Keys are buttons for accessibility services, with names for
      ⌫ ⏎ × ÷ √ π… (#1); the request line and the panel buttons have names too.
- [x] **android-3** Keys show they are pressed: a ripple (#4).
- [x] **android-4** ⌫ held repeats, as on every keyboard; clearing gets its own
      key (#5).
- [x] **android-5** Results are announced to screen readers (#11, cheap with
      #1).

Landscape, and copy / share / paste (15 Oct 2026):

- [x] **android-6** Landscape prototype (#3): digits and operators on the right,
      in the calculator's grid (the functions beside it in Sci); the toolbar (C,
      123, Measures…, ⚙) on the left, vertical; the letters under the request,
      at about their portrait size; a panel takes the right side.
- [x] **android-7** Tap or long-press the result: copy; Copy and Share beside it
      (#7). Share sends `request = result`.
- [x] **android-8** Long-press the request line: paste, on one line (#7); for
      screen readers, a Paste action.

Responsiveness (15 Oct 2026):

- [x] **android-9** The core's calls (`evaluate`, `suggestions`) off the main
      thread, the previous answer shown until the new one arrives (#6). With
      Compose's `produceState` on `Dispatchers.Default`, not a `ViewModel`: the
      request already survives rotation (`rememberSaveable`), and an answer
      takes milliseconds to compute again. The panels' lists (Measures,
      Currencies, Trivia) still load on the main thread, once per opening.

Asked on 15 Oct 2026:

- [x] **android-10** Trivia: the length of a day and of a year for every planet.
- [x] **android-11** Recheck the whole code: performance, cleanup, refactoring
      (15 Oct 2026; `docs/CORE_REVIEW.md` §9: what was measured, fixed, and left
      open).
- [x] **android-12** Ship the city archive and the exchange rates valid at build
      time inside the app, so the features work before any download; with
      currencies, say how old the rates are (1h, 2d, 3w, 1m, 1y…).
      `scripts/build-android.sh` downloads them into the app's assets; the app
      installs them at its first start.
- [x] **android-13** Check on screen: the Trivia measure chips (a chip writes
      `0.3871 au` and a space) and the Wikipedia page inside the app (with Open
      in browser).

From the recheck (`docs/CORE_REVIEW.md` §9):

- [x] **android-14** A `rustfmt.toml` (`max_width = 160`,
      `use_small_heuristics = "Max"`) and one `cargo fmt` (142 changes).
- [x] **android-15** A written date alone (`12/04/2026`): a hint in every UI
      ("now the format to convert to…"); `12/04/2026 num` is the division.

Later (medium issues):

- [x] **android-16** While writing a math expression, the preview keeps the
      grouping in brackets, as the TUI does.
- [x] **android-17** The about function (Settings › About).
- [x] **android-18** In the about, the link to install Aequalis on mac and
      Linux.
- [x] **android-19** Share the mac, Linux and Android installation links (GitHub
      repo / official Play Store app) (Android: `PLAY_STORE` in AboutScreen.kt,
      once published).
- [x] **android-20** Icons instead of text characters, with names (#8): ⌫ ⏎ ⚙ ←
      ✕ ⌨, the panels' keys and titles, the unit classes and the Trivia sections
      (Material Icons, as paths in `Icons.kt`).
- [x] **android-21** A tap on "Aequalis": the memory the app uses (Android's
      figures by kind, the Rust core's own as its allocator counts it,
      `src/alloc.rs`), the phone's free memory.
- [x] **android-22** Search in the Currencies panel (#10): by code, name or
      symbol, with the phone's keyboard.
- [x] **android-23** A fast start, even on old phones: the release build
      installable (R8, a Baseline Profile compiled at install), the core started
      in the background, the window in the app's colours.
      `scripts/install-android.sh` installs it, `scripts/android-startup.sh`
      measures it.
- [x] **android-24** History (#14): a History panel (a tap: the request back in
      the line, to change; ⧉ copies the result; ⋮ Share, Remove; Clear the
      history), the three latest on the empty screen, kept in
      `files/history.txt`. A request goes in once done with: ⏎, Copy, Share, C,
      the app left.
- [x] **android-25** After a unit is chosen (`10 cm` and a space), the units of
      its kind offered at once; Measures opens at its class.
