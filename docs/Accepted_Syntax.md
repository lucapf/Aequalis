# Accepted Syntax

One of the values `Aequalis` aims to bring is understanding what you write the way you would write it.
That is evident in the command line, the TUI, the widget and the apps.

Aequalis parses the input as you write it and shows the result.

For example:
typing: `2026/12/04` shows: `42.2083`
Adding `+3d`  (`2026/12/04 +3d`) Aequalis 'understands' you want to add 3 days to the date and shows  `2026/12/07`

There are no strict rules:

- **Case does not matter**: `usd`, `USD` and `Usd` are the same, so are `new york` and `NEW YORK`, `sqrt` and `SQRT`,
  `to` and `TO`. The one exception is where the case tells two things apart: the data units (`MB` is megabytes,
  `Mb` megabits). Written in a case that fits both (`mb`), Aequalis asks which one you mean.
- **The unit may touch the number**: `12usd`, `5KM`, `100km/h`, `1e3km`, `(2+3)km`, `18E6unix`. The arrow too: `12USD->EUR`.
- **`to`, `in` and `->`** may stand between the two units, places or currencies, or be left out.
- **Numbers as the device writes them** are read back: where the thousands separator is `,`, `1,000km mi` is `1000 km`.

Some examples of accepted syntax (each row: the same request written in other ways gives the same result):

| Type                        | Result                |
|-----------------------------|-----------------------|
| `44 it us`, `44 IT US`            | the shoes and the clothes of that size: `shoes: 44 it = US 10 men, US 11.5 women`, `clothes: 44 it = US 8 women, …` |
| `shoe 42 eu us`, `SHOES 42 EU US` | `EU 42 = US 8.5 men` (sizes vary by brand: check the brand's chart) |
| `women shoes 38 eu us`, `38 eu women shoes us` | `EU 38 = US 7 (Nike), 7.5 (Clarks)` |
| `dress 44 it us`, `DRESS 44 IT US` | `IT 44 = US 8` |
| `size 42 eu us`              | the shoes, then the clothes: `EU 42 = US 8.5 men, US 10 women` … |
| `10 km mi`, `10 KM MI`, `10km mi`   | `6.2137 mi`             |
| `1000 ly -> yd`, `1000 ly in yd`, `1000 ly to yd`, `1000 LY TO YD` | `1.0346e19 yd` (`->`, `in` or `to` between the units) |
| `12+12/3`                     | `12 + (12 / 3) = 16` (the order the operations are done in) |
| `100sqft mq`, `100 SQFT MQ`   | `9.2903 m2`             |
| `100 km/h mph`, `100km/h MPH` | `62.1371 mph`           |
| `9.81 m/s^2 ft/s2`            | `32.185 ft/s2` (compound units) |
| `1 bar psi`, `1 BAR PSI`      | `14.5038 psi`           |
| `1 kWh MJ`, `1 KWH MJ`        | `3.6 MJ`                |
| `100 hp kW`, `100hp KW`       | `74.57 kW`              |
| `sqrt 16`, `SQRT 16`, `sqrt(16)` | `4`                  |
| `5!`, `(2+3)!`                | `120` (whole numbers 0 to 170) |
| `2E5`, `1.5e-3 * 4`           | `200000`, `0.006` (x E y is x × 10^y, in any number) |
| `18E6 unix`, `18000000unix`, `18E6UNIX` | `28 Jul 70 12:00 PM GMT+4` (the number may touch its format, and be written xEy) |
| `12USD->EUR`, `5km->mi`, `5KM->MI`  | as with spaces: the arrow may touch its words |
| `root(27, 3)`, `ROOT(27, 3)`, `cbrt(27)` | `3` (the number, then the index) |
| `150 + 20%`                   | `180`                   |
| `20% of 150`, `20% OF 150`    | `30`                    |
| `30 deg rad`, `30DEG RAD`     | `0.5236 rad`            |
| `1 ly km`, `1 LY KM`          | `9460730472580.8 km`    |
| `Rome to Paris`, `ROME PARIS`, `Rome -> Paris` | `1,107 km` (capitals need no city archive) |
| `Italy`, `ITALY`              | the current time in Rome |
| `10:00 USA to Japan`, `10:00 usa TO japan` | asks which US time zone (New York, Chicago, …) |
| `sin(30°)`, `SIN(30°)`        | `0.5`                   |
| `log(1000) + ln(e)`, `LOG(1000) + LN(e)` | `4`          |
| `pi * 2`, `PI * 2`            | `6.2832`                |
| `2026/12/04 - 12d`, `2026/12/04 - 12D` | `2026/11/22 YYYY/MM/DD` |
| `Rome `, `ROME`               | `current time in Rome`  |
| `now Rome Tokyo`, `NOW ROME TOKYO` | one line for each city: `24 Oct 26 8:17 PM GMT+2 Rome`, `25 Oct 26 3:17 AM GMT+9 Tokyo` |
| `10:00AM paris to new york`, `10:00AM PARIS TO NEW YORK`, `10am Paris -> New York` | `time in New York when in Paris is 10:00AM`|
| `12 Sept 2026 + 5wd`, `12 SEPT 2026 + 5WD` | `18/09/2026 DD/MM/YYYY` |
| `12 Sept 2026 10:00 PM + 5wd` | `18/09/2026 10:00:00 PM DD/MM/YYYY-12h` |
| `now unix`, `NOW UNIX`        | the current Unix time |
| `12/04/2026 unix`             | ambiguous: one button per reading (DD/MM/YYYY, MM/DD/YYYY) |
| `100F C`, `100 f c`, `100 °F °C` | `37.7778 C` (temperature) |
| `300 K C`, `0 c f`            | `26.85 C`, `32 F` |
| `8 l/100km mpg`, `8 L/100KM MPG`, `8l/100km mpg` | `29.4018 mpg` (fuel consumption: litres per 100 km, miles per gallon, km per litre) |
| `12.5 km/l l/100km`           | `8 l/100km` |
| `1 MB kB`, `1 Mb kb`          | `1000 kB`, `1000 kb` (the case tells megabytes from megabits) |
| `1 GiB MB`                    | `1073.7418 MB`, noting that decimal and binary units are mixed |
| `8 bit B`                     | `1 B`, noting that bits and bytes are mixed |
| `1 mb kb`                     | asks: megabyte or megabit? (`mb`, `kib`, `gb` fit both) |

## Currencies

One amount and one or two currencies. A currency can be written as its code (`usd`), its name in
the singular or plural (`euro`, `US dollars`, `yen`), a symbol (`€`, `£`, `¥`), a common
abbreviation (`Dhs`, `RMB`, `kr`), a country (`USA`, `UAE`, `China`) or `Europe` / `EU`. Case
and accents don't matter, and the amount may be attached (`12usd`, `12€`, `US$12`, `12€gbp`); `to`, `in` or `->`
may stand between the currencies (`12 usd -> eur`). Rates:
ExchangeRate-API, synced once a day in the background (`aequalis --list` lists the currencies); a date
gives the rates of a past day (below).
The amounts below change with the rates.

| Type                       | Result |
|----------------------------|--------|
| `12 usd euro`, `12 USD EUR`, `12usd EUR` | `10.71 EUR` |
| `12 USA Europe`            | `10.71 EUR`, noting the currencies used: `United States: US dollar (USD, US$) → Europe: euro (EUR, €)` |
| `12 USD UAE`               | `44.07 AED` |
| `12 China Euro`            | `1.60 EUR` |
| `12€ gbp`, `12€gbp`        | `10.05 GBP` |
| `12 usd`                   | `44.07 AED`: to the local currency (the time zone's country, or the `currency.local` setting) |
| `12 dollar euro`           | asks which dollar: US, Canadian, Australian, New Zealand, Hong Kong, Singapore, New Taiwan, Barbadian |
| `12 dolar eur`             | "`dolar` is not a currency", asks which one: US dollar, Canadian dollar, Australian dollar, New Zealand dollar, Hong Kong dollar, Singapore dollar, New Taiwan dollar, Barbadian dollar |
| `10:00AM Italy Japan`      | a time: the time in Tokyo (a number is money, a time is time zones) |
| `Italy Japan`              | no value: the distance |
| `12 pounds kg`             | `5.4431 kg`: with a unit, `pounds` is the unit |

### Rates of a past day

A date at the end of the request (or at its start) gives the rates of that day, up to 750 days
back: `100 usd eur 2025-01-30`, `100 usd to eur on 30 Jan 2025`, `100 usd eur yesterday`,
`100 usd eur 3 months ago`. Without a date it is always the latest rates, and nothing is asked.
Rates of past days are ExchangeRate-API's, the same as the latest ones, saved once a day. The
history starts the day it was set up and grows from then on (there are no rates of earlier days),
and the note says which day they are of. A day is downloaded the first time it is asked for and
kept on the device, so it works offline afterwards.

| Type                                     | Result |
|------------------------------------------|--------|
| `100 usd eur 2025-01-30`                 | `96.13 EUR`, noting `rates of 30 Jan 25` |
| `100 usd to eur on 30 Jan 2025`          | the same |
| `on 2025-01-30 100 usd eur`              | the same: the date may come first |
| `100 usd eur yesterday`                  | the rates of the day before |
| `100 usd eur 3 months ago`, `a week ago`, `2 days ago`, `1 year ago` | the rates of that day |
| `100 usd eur today`, `100 usd eur`       | the latest rates |
| `100 usd eur 03/04/25`                   | asks: 4 March or 3 April 2025 (a written month, `3 Apr 2025`, is not asked) |
| `100 usd eur 2025-01-30`, before the history began | error: no rates of that day |
| `100 usd eur 2020-01-01`                 | error: older than 750 days |
| `100 usd eur tomorrow`                   | error: that day has not come yet |

The `rates.history` setting (on by default; the first of the Settings window) turns it on and
off, and shows how many days are held (`on · 42 days, since 14 Sep 26`; also in `aequalis
--online`, About and the apps' settings). On, each rates sync (once a day, in the background)
downloads the days not held yet, up to 750; off, nothing is downloaded and a dated request says
the history is off (the days already held stay). Offline (the `offline` setting), only the days
already held are available. The files come from `rates.history_url`, a setting for mirrors.

## Distances

Two places, then optionally the unit: `[distance] <place> [to] <place> [<unit>]`. A place is a
city, a country or coordinates (`41.9,12.5`). With two places the unit is the `distance.unit`
setting (auto: miles where the device is imperial, km elsewhere). `to`, `->` or nothing between
the places, in any case.

The world's capitals and the countries are built in: `Rome to Paris`, `ROME PARIS` and `Rome -> Paris` give `1,107 km`
with no download. With the city archive (`aequalis --sync`, from GeoNames) every city of 5,000 inhabitants or
more is known, by any of its names in any language (`Roma`, `Рим`); `Springfield, Illinois` or
`Springfield, US` picks one. Several cities with that name: Aequalis asks, the country's capital
first and selected by default, wherever the device is (`Rome`: Rome, Italy), then those named
exactly so in the device's country, the others there, then the rest, by population. A word that
is a unit, a currency or a date keyword keeps that meaning (`Bar` is a unit: write
`Bar, Montenegro`).

| Type                                   | Result |
|----------------------------------------|--------|
| `Rome Paris`, `ROME PARIS`, `rome -> paris` | `1,107 km`, or with the archive a choice of Romes and Parises, Rome, Italy and Paris, France selected |
| `Rome to Paris mi`                     | `688 mi` |
| `Springfield, Illinois Chicago mi`     | `176 mi` |
| `Chicago Paris`                        | asks: Paris, France first (the capital), then Paris, Texas… |
| `distance Sprngfield to Chicago`       | "`Sprngfield` is not a known place: did you mean Springfield, …?" |
| `Italy Japan`, `ITALY -> JAPAN`        | countries: their capitals: `9,875 km` |
| `10:00 Rome Paris`                     | a time: the time in Paris |
