# Accepted Syntax

One of the value `Tally` is aimed to bring is the possibility of parsing the user input in various format. 
That characteristic is evident while dealing the command line / widget.

Tally parses the input string as you write to verify the result.

For example:
typing: `2026/12/04` shows: `42.2083`
Adding `+3d`  (`2026/12/04 +3d`) Tally 'understand' you want to add 3 days to the date and shows  `2026/12/07`

Some examples of accepts syntax:

| Type                        | Result                |
|-----------------------------|-----------------------|
| `10 km mi`                    | `6.2137 mi`             |
| `12+12/3`                     | `12 + (12/3) = 16`      |
| `100sqft mq`                  | `9.2903 m2`             |
| `100 km/h mph`                | `62.1371 mph`           |
| `9.81 m/s^2 ft/s2`            | `32.185 ft/s2` (compound units) |
| `1 bar psi`                   | `14.5038 psi`           |
| `5 l/100km mpg`               | `47.0429 mpg`           |
| `1 kWh MJ`                    | `3.6 MJ`                |
| `100 hp kW`                   | `74.57 kW`              |
| `sqrt 16`                     | `4`                     |
| `150 + 20%`                   | `180`                   |
| `20% of 150`                  | `30`                    |
| `30 deg rad`                  | `0.5236 rad`            |
| `1 ly km`                     | `9460730472580.8 km`    |
| `Rome to Paris`      | `1,107 km`          |
| `Italy`                       | the current time in Rome |
| `10:00 USA to Japan`          | asks which US time zone (New York, Chicago, …) |
| `sin(30°)`                    | `0.5`                   |
| `log(1000) + ln(e)`           | `4`                     |
| `2026/12/04 - 12d`            | `2026/11/22 YYYY/MM/DD` |
| `Rome `                       | `current time in Rome`  |
| `10:00AM paris to new york`   | `time in New York when in Paris is 10:00AM`|
| `12 Sept 2026 + 5wd`          | `18/09/2026 DD/MM/YYYY` |
| `12 Sept 2026 10:00 PM + 5wd` | `18/09/2026 DD/MM/YYYY` |
| `now unix`                    | the current Unix time |
| `12/04/2026 unix`             | ambiguous: one button per reading (DD/MM/YYYY, MM/DD/YYYY) |

## Currencies

One amount and one or two currencies. A currency can be written as its code (`usd`), its name in
the singular or plural (`euro`, `US dollars`, `yen`), a symbol (`€`, `£`, `¥`), a common
abbreviation (`Dhs`, `RMB`, `kr`), a country (`USA`, `UAE`, `China`) or `Europe` / `EU`. Case
and accents don't matter, and the amount may be attached (`12usd`, `12€`, `US$12`). Rates:
ExchangeRate-API, synced once a day in the background (`tally --list` lists the currencies).

| Type                       | Result |
|----------------------------|--------|
| `12 usd euro`              | `10.74 EUR` |
| `12 USA Europe`            | `10.74 EUR`, noting the currencies used: `United States: US dollar (USD, US$) → Europe: euro (EUR, €)` |
| `12 USD UAE`               | `44.07 AED` |
| `12 China Euro`            | `1.60 EUR` |
| `12€ gbp`                  | `10.14 GBP` |
| `12 usd`                   | `10.74 EUR`: to the local currency (the time zone's country, or `TALLY_CURRENCY`) |
| `12 dollar euro`           | asks which dollar (US, Canadian, Australian, …) |
| `12 dolar eur`             | "`dolar` is not a currency: did you mean US dollar?" |
| `10:00AM Italy Japan`      | a time: the time in Tokyo (a number is money, a time is time zones) |
| `Italy Japan`              | no value: the distance |
| `12 pounds kg`             | `5.4431 kg`: with a unit, `pounds` is the unit |

## Distances

Two places, then optionally the unit: `[distance] <place> [to] <place> [<unit>]`. A place is a
city, a country or coordinates (`41.9,12.5`). With two places the unit is the `distance.unit`
setting (auto: miles where the device is imperial, km elsewhere).

With the city archive (`tally --sync`, from GeoNames) every city of 5,000 inhabitants or
more is known, by any of its names in any language (`Roma`, `Рим`); `Springfield, Illinois` or
`Springfield, US` picks one. Several cities with that name: Tally asks, those in the other
place's country first, then in the device's country, then the others, by population. A word that
is a unit, a currency or a date keyword keeps that meaning (`Bar` is a unit: write
`Bar, Montenegro`).

| Type                                   | Result |
|----------------------------------------|--------|
| `Rome Paris`                           | `1,107 km`, or a choice of Romes and Parises with the archive |
| `Springfield, Illinois Chicago mi`     | `176 mi` |
| `Chicago Paris`                        | asks: Paris, Texas first (same country as Chicago) |
| `distance Sprngfield to Chicago`       | "`Sprngfield` is not a known place: did you mean Springfield, …?" |
| `Italy Japan`                          | countries: their capitals |
| `10:00 Rome Paris`                     | a time: the time in Paris |

