# Changelog

## 1.5.0

- **Short dates for past rates**: `200 eur yesterday`, `12 eur -1d`, `12 eur 3 d ago` (`3d ago`,
  `3 dago`), in days `d`, working days `wd` (Monday to Friday: `-1wd` on a Monday is Friday), weeks
  `w`, months `m` or `mo`, years `y`: `2d ago`, `2m ago`, `2y ago`. The rates of past days are kept
  for 750 days by default (since 1.4.0).
- Android: a past day not downloaded yet is fetched when a request asks for it, as on the
  command line (it was left to the next sync).
- A past day that cannot be downloaded says why: no connection, or the archive has none.
- Android: a year in the request is no longer shown with a thousands separator (`25-10-2025`,
  `30 Jan 2025`, not `2.025`).
- **The calendar**: `cal` (this month), `cal march 2027`, `cal 2027` (the year: three months a row
  in the interactive mode, two on Android, four in landscape),
  with ISO week numbers, today marked and the week starting as the region does
  (`calendar.week_start`). The public holidays of up to 5 countries, a colour each:
  `cal march UAE,USA,Italy`, `cal 2027 de fr`; none named, `holidays.countries` (the time zone's
  country). In the interactive mode a window (Enter, or `C` in the Date & Time keys); on Android in
  the result's place, and a Calendar button in Date & Time. A month's name first needs no `cal`
  (`oct egypt`). A holiday of several days is listed once with its days (`2-3 Dec`, `(2 days)`);
  on Android a tap on a holiday's day shows each country's holiday and how many days it lasts, and
  the weekdays and the weekend's columns are on a band of the theme's grey. A swipe left or right
  (← → in the interactive mode) goes to the month or year after or before; touching the calendar
  folds the keyboard away. A month and its year however written: `oct 2026`, `oct 26`, `OCT26`,
  `oct2026`, `0ct 26`.
- The calendar reads only its countries' holidays from the archive (a calendar's peak memory
  11.4 MB → 5.8 MB, the first one 3× faster); on Android the year's months are drawn at once
  (a canvas each) and an answer still on its way shows a veil and a spinner over the last one.
- A date with a month's name needs no year: `26 oct` is this year's, `26 oct + 3wd`, `Oct 26th - 2d`.
- The holidays: every country's (over 200), last year to next year, from python-holidays, named in
  the device's language when there is one (`.github/workflows/holidays.yml`, the release
  `holidays`). They need a sync to download them (about 1 MB): until then the calendar shows none;
  checked once a day, offline afterwards. New settings `holidays` (on by
  default), `holidays.countries`, `holidays.refresh`, `holidays.interval`, `holidays.url`,
  `calendar.week_start`.

## 1.4.0

- **Android: the nightly download.** Once a day, about 02:30, on Wi-Fi and with the battery not
  low, the app downloads the online data without being opened (WorkManager; Android may delay it).
  Days missed are added by the next download, rates of past days included. The Android-only setting
  `night.sync` (on by default, under `offline`) switches it.
- Android, landscape: the Sizes keys' hint ("tap Sizes again to go back") is over the Sizes button
  on the toolbar, no longer inside the keyboard.

- **Exchange rates of past days.** A date at the end or the start of a currency request gives that
  day's rates: `100 usd eur 2025-01-30`, `on 30 Jan 2025`, `yesterday`, `3 months ago`. An
  ambiguous date (`03/04/25`) asks day or month; no date is the latest rates, never asked.
- The history is saved once a day (`.github/workflows/rates.yml`, from ExchangeRate-API) and kept
  for 750 days, from the day it began; the app downloads each new day with its rates sync, and
  fetches a missing day when a request asks for it.
- New setting `rates.history` (on by default) in the Settings window, with the number of days held
  (`42 days, since 14 Sep 26`), also in About, `aequalis --online` and the Android settings. New
  setting `rates.history_url` for mirrors.
