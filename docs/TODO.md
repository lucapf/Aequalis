# To do

Open work, in order. Details and evidence: `docs/ANDROID_UX_REVIEW.md` (the `#` numbers below).
Tick an item when it is done, and note the commit.

## Android app

### 1. Keep the user's work, and keys that work for everyone (done, 15 Oct 2026)

- [x] The request survives rotation, dark mode, font and language changes, and Android stopping
      the app (#2): `rememberSaveable` for the request, Enter, the open panel, Settings.
- [x] Keys are buttons for accessibility services, with names for ⌫ ⏎ × ÷ √ π… (#1); the request
      line and the panel buttons have names too.
- [x] Keys show they are pressed: a ripple (#4).
- [x] ⌫ held repeats, as on every keyboard; clearing gets its own key (#5).
- [x] Results are announced to screen readers (#11, cheap with #1).

### 2. Landscape, and copy / share / paste

- [ ] Portrait only on phones, for now (#3); a landscape layout later.
- [ ] Tap or long-press the result: copy; a Share action (#7).
- [ ] Long-press the request line: paste (#7).

### 3. Responsiveness

- [ ] The core's calls (`evaluate`, `suggestions`) off the main thread, in a `ViewModel`, the
      previous answer shown until the new one arrives (#6).

### Later (medium issues)

- [ ] Icons instead of text characters, with names (#8).
- [ ] Settings for people: titles, groups, choices for intervals, a top app bar (#9).
- [ ] Search in the Currencies panel (#10).
- [ ] Errors and hints told apart by more than colour (#12).
- [ ] Toolbar and panel buttons at least 48 dp (#13).
- [ ] History and first-launch examples (#14).
- [ ] Tablets and foldables: a maximum keyboard width or two panes (#15).

## Elsewhere

- [ ] Downloads on the phone: exchange rates and the city archive (the app's downloader).
- [ ] Push the local commits (`git status` says how many).
