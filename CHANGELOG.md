# Changelog

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
