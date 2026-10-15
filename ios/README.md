# Tally for iPhone

A SwiftUI app with tally's Rust core inside: no `tally` program to run (iOS doesn't allow it). The
core is a static library (`src/ffi.rs`, `include/tally.h`) packed as `TallyCore.xcframework`; the
app calls it in-process and gets the same JSON as `tally --json`.

- **Tally** tab: the request (operators above the keyboard), the result with Copy, the choices of
  an ambiguous request, the notices, Recent. No error while a word is being typed.
- **Measures** (the unit classes, then their units), **Currencies** (search), **Trivia** (with
  the details), **Settings** (every setting, Download everything, About).
- The online data is refreshed at launch and in the background, as the refresh settings say.

## Build and run (on a Mac with Xcode)

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
scripts/build-ios.sh              # ios/TallyCore.xcframework
brew install xcodegen
cd ios && xcodegen generate       # ios/Tally.xcodeproj
open Tally.xcodeproj
```

In Xcode: target Tally › Signing & Capabilities › Team (or set `DEVELOPMENT_TEAM` in
`project.yml`), choose your iPhone, Run. Run `scripts/build-ios.sh` again after changing the
Rust code.

Tests: `xcodebuild test -project ios/Tally.xcodeproj -scheme Tally -destination 'platform=iOS
Simulator,name=iPhone 16'` (CI does it on every push: `.github/workflows/ci.yml`, job `ios`).

## How it fits

| Piece | Where |
|---|---|
| C interface: init, eval, settings, sync, lists | `src/ffi.rs`, `include/tally.h` |
| Downloads through the app (`URLSession`) | `http.rs` (`Fetcher`), `ios/Tally/Engine.swift` |
| The city archive: plain JSON, saved by the app's downloader | `cities.rs`, `http.rs` |
| Device: data folder, time zone, zone files, number format, units | `tally_init`, `Engine.started` |
| The screens | `ios/Tally/*View.swift`, state in `AppModel.swift` |
