# Changelog

## 1.5.0

- **Android: the nightly download.** Once a day, about 02:30, on Wi-Fi and with the battery not
  low, the app downloads the online data without being opened (WorkManager; Android may delay it).
  Days missed are added by the next download, rates of past days included. The Android-only setting
  `android.nightly_sync` (on by default, under `offline`) switches it.
- Android, landscape: the Sizes keys' hint ("tap Sizes again to go back") is over the Sizes button
  on the toolbar, no longer inside the keyboard.

## 1.4.0

- **Exchange rates of past days.** A date at the end or the start of a currency request gives that
  day's rates: `100 usd eur 2025-01-30`, `on 30 Jan 2025`, `yesterday`, `3 months ago`. An
  ambiguous date (`03/04/25`) asks day or month; no date is the latest rates, never asked.
- The history is saved once a day (`.github/workflows/rates.yml`, from ExchangeRate-API) and kept
  for 750 days, from the day it began; the app downloads each new day with its rates sync, and
  fetches a missing day when a request asks for it.
- New setting `rates.history` (on by default) in the Settings window, with the number of days held
  (`42 days, since 14 Sep 26`), also in About, `aequalis --online` and the Android settings. New
  setting `rates.history_url` for mirrors.
