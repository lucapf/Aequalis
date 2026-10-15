# Android app: user experience review

The Android app (`apps/android`, commit `7247c80`) checked against Android's guidance:
Material 3, Android's accessibility guidance, its guidance on keeping state across configuration
changes, and its large-screen guidance. Checked on the Pixel 7 emulator (Android 17, 1080 × 2400,
411 × 914 dp): the accessibility tree (`uiautomator dump`), dark mode, text at 150%, landscape, and
a configuration change while typing.

## Summary

The core flow works well: type → chips → panels → result, a theme that follows the system (dark
mode works), a keyboard that scales with the font size, and haptics. **The four major problems:**

1. **The keyboard is invisible to accessibility services.** TalkBack, Switch Access and Voice Access
   can't press a single key.
2. **The request is lost on any configuration change** (rotation, dark mode, font size, language)
   and when Android stops the app in the background.
3. **Landscape is unusable:** the keyboard covers the whole screen, with the status bar drawn over
   it.
4. **Keys give no visual feedback,** and a long press on ⌫ wipes the request with no undo.

## Major issues

| # | Issue | Evidence | Why it matters (guideline) | Fix |
|---|---|---|---|---|
| 1 | **Keys can't be activated by accessibility services.** Each key is a `Surface` with `pointerInput { detectTapGestures }`, which declares no click action. | The accessibility tree has 121 elements, and **only the gear is clickable**. TalkBack reads "q", "w"… but can't press them. ⌫ and ⏎ are read as their symbols. | Every interactive element must expose its action and a name (Android accessibility: "make apps more accessible"; WCAG 2.1, 4.1.2). | Use `Modifier.combinedClickable(onClick, onLongClick, role = Role.Button)` (it also gives the ripple, see #4), and `semantics { contentDescription = "Backspace" }` for ⌫, ⏎, ×, ÷, √, π. The same for the request line (`semantics { text; editableText }`) and the panel buttons ⌨ ← ✕. |
| 2 | **The request is lost on configuration changes and when Android stops the app.** `edit`, `submitted`, `panel` and `settings` use `remember`; only `mode` uses `rememberSaveable`. | Typed `10`, switched to dark mode: the request was empty afterwards. | Android recreates the activity on rotation, theme, font or locale changes, and may stop a backgrounded app; the user must find the screen as they left it ("Save UI states"). | `rememberSaveable` for all four, with a `Saver` for `Edit`. Better still, a `ViewModel` with `SavedStateHandle`, which also gives one place for the core's calls (#6). |
| 3 | **Landscape is unusable.** The keyboard has a fixed 380 dp height, and the phone in landscape is 411 dp tall. | In landscape the keyboard fills the screen: no request, no result, and the status bar is drawn over the toolbar. | Apps should adapt to the window, or declare an orientation ("Large screens", "Support different screen sizes"). | Quick: lock portrait on phones (`android:screenOrientation="portrait"`; Android ignores it on large screens since Android 16). Later: in landscape, the request on the left and the keyboard on the right. |
| 4 | **Keys give no visual feedback.** `detectTapGestures` draws no pressed state or ripple, and there's no key preview. | Seen while typing: nothing changes on the key, only the haptic tick. | A control must show that it is pressed (Material 3: "states"); with a finger over the key, a keyboard shows what was hit. | `combinedClickable` (#1) brings the ripple. Also a pressed colour (`interactionSource` → `surfaceContainerHigh`), and optionally a small key preview above letters. |
| 5 | **A long press on ⌫ clears the whole request, with no undo.** | `Key.Clear` on a long press. | On every Android keyboard a held ⌫ repeats; here the same gesture destroys input with no way back (Material 3: confirm or undo destructive actions). | A held ⌫ repeats (every ~50 ms after ~400 ms). Clearing gets its own `C` key on the toolbar or in the calculator grid, or a Snackbar with *Undo*. |
| 6 | **The core runs on the main thread, on every keystroke.** `Core.evaluate` and `Core.suggestions` are called during composition. | Fine today (well under 1 ms). A misspelling searched in the city archive takes 12–17 ms (`docs/CORE_REVIEW.md`), the budget of a whole frame. | No work that can take milliseconds on the UI thread ("Keep your app responsive"). | Compute in a `ViewModel` on `Dispatchers.Default` (`LaunchedEffect(input)` or a flow), and show the previous answer until the new one arrives. |
| 7 | **No way to copy or share the result, nor to paste into the request.** | Nothing reacts to a tap or long press on the result; nothing pastes. | Copy and paste are expected on any text a user wants to reuse. Every calculator app copies its result. | Tap or long-press the result to copy it (Android 13+ shows its own "copied" confirmation: no extra toast), plus a Share action. Long-press the request line to paste. |

## Medium issues

| # | Issue | Fix |
|---|---|---|
| 8 | The gear, ←, ⌨ and ✕ are text characters, not icons. Their look depends on the font (the gear rendered as an emoji), and they have no accessible names. | Material Symbols as vector drawables in `res/drawable` (no library needed), each with a `contentDescription`. |
| 9 | Settings shows the internal keys (`cities.refresh`, `rates.interval`) and free text for durations (`30d`, `24h`). | Titles for people ("City archive: update"), groups (Downloads, Units, Formats), choices instead of free text for intervals (1 day, 1 week, 30 days), and a Material top app bar. |
| 10 | The Currencies panel is a list of 167 items with no search. | A search field at the top, typed with Tally's keyboard, and the most used or recently used currencies first. |
| 11 | Results, hints and errors aren't announced to screen readers. | `Modifier.semantics { liveRegion = LiveRegionMode.Polite }` on the result area. |
| 12 | Error and hint differ mostly by colour (red / grey). | An icon or label too ("Not finished:", "Error:"), so the meaning doesn't depend on colour alone (WCAG 1.4.1). |
| 13 | Letter keys are 36.7 dp wide (10 per row on a 411 dp screen; their height is 49.7 dp). | That's normal for keyboards (Gboard is similar), but below Material's 48 dp target. Keep 4 dp gaps, and make the panel buttons and toolbar keys at least 48 dp. |
| 14 | No empty state beyond the placeholder: no history, no examples. | A short history of recent requests (tap to reuse), and examples on first launch. |
| 15 | On tablets and foldables the keyboard stretches to the full width (very wide keys). | Limit its width (for example 600 dp, centred), or a two-pane layout when the window is wide. |

## What already works well

- **The theme follows the system**, so dark mode is correct, and the system bars are drawn edge to
  edge with insets in portrait.
- **Key labels scale with the font size** (150% tested), with no clipping.
- **Back works as expected:** from a panel's second level to its first, then to the keyboard; from
  Settings to the main screen.
- **Haptics on every key,** a stronger one on a long press.
- **The phone's own keyboard never covers the request,** and suggestions sit right above the keys.

## Suggested order

1. #2 (state), #1 + #4 + #5 (one change to how keys are built): the app keeps your work and works
   for everyone.
2. #3 (lock portrait for now), #7 (copy, share, paste).
3. #6 (the core's calls off the main thread, with the `ViewModel` from #2).
4. The medium issues, Settings (#9) first.
