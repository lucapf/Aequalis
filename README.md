# Tally

**Tally is an ultralight calculator and converter made for humans, answering as you type.**

Write your request the way you would say it. If it can be read in more than one way, Tally asks you which one to get, no guessing.

```console
$ tally 10 km mi
6.2137 mi
$ tally 12 Sept 2026 + 5wd
18/09/2026 DD/MM/YYYY
$ tally Rome to Paris
1,107 km
$ tally 10:00AM Paris to New York
11 Oct 26 4:00 AM GMT-4 New York
```

Installation guides [Mac Os](./docs/install_mac.md)

## What Tally can do

Tally accepts a loose syntax, so the learning curve is flat (almost :P).

### Calculator

Normal precedence, powers (`^` or `**`), percentages and scientific functions:
`sqrt`, `log`, `ln`, `sin`, `cos`, `tan`, `sinh`, `cosh`, `tanh`, plus `e` and `pi`.

| You type | Tally answers |
|---|---|
| `12+4/2` | `14` (the TUI preview shows the grouping: `(12 + (4 / 2)) = 14`) |
| `2^10` | `1024` |
| `sqrt 16` | `4` |
| $\sin(30^\circ)$ | `0.5` |
| `150 + 20%` | `180` |
| `20% of 150` | `30` |

### Units

Metric and imperial: length, area, volume, mass, temperature, time, data, speed, pressure, fuel
economy, energy, power and angles.

| You type | Tally answers |
|---|---|
| `10 km mi` | `6.2137 mi` |
| `100 km/h mph` | `62.1371 mph` |
| `1 bar psi` | `14.5038 psi` |
| `30 deg rad` | `0.5236 rad` |
| `1 ly km` | `9460730472580.8 km` |

### Currencies

Currencies by code, name, symbol or country, in any case: `12 usd euro`, `12 USA Europe`,
`12€ gbp`. With one currency, to your local one. Shared names (`dollar`) and misspellings are
asked about. Rates from [ExchangeRate-API](https://www.exchangerate-api.com), synced once a day
in the background.

| You type | Tally answers |
|---|---|
| `12 usd euro` | `10.74 EUR` |
| `12 USD UAE` | `44.07 AED` |
| `12 China Euro` | `1.60 EUR` |
| `12 dollar euro` | asks which dollar |

### Dates

Calendar-aware date math, working days (`wd`) and dates written in many forms.

| You type | Tally answers |
|---|---|
| `12 Sept 2026 + 5wd` | `18/09/2026 DD/MM/YYYY` (5 working days later) |
| `2026/10/05 + 5wd` | `2026/10/12 YYYY/MM/DD` |
| `12/04/2026 unix` | asks: April 12 or December 4? |

### Time zones

Every capital and major city, and countries too.

| You type | Tally answers |
|---|---|
| `now new york` | the time in New York now |
| `10:00AM Paris to New York` | `11 Oct 26 4:00 AM GMT-4 New York` |
| `Italy` | the time in Rome now |
| `now usa japan` | asks which US time zone first |

### Distances

Between two cities, countries or coordinates, along the Earth's surface; a third word is the
unit. With the city archive (`tally --sync`, about 70,000 cities from
[GeoNames](https://www.geonames.org)), any city of 5,000 inhabitants or more, by any of its names
in any language. When several cities share a name, Tally asks which one: those in the other
city's country first, then in yours.

| You type | Tally answers |
|---|---|
| `Rome Paris` | `1,107 km` (or miles, where your device is imperial) |
| `Rome Doha mi` | `2,497 mi` |
| `Springfield, Illinois Chicago` | `283 km` |
| `Springfield Chicago` | asks which Springfield (Missouri, Massachusetts, Illinois…) |

### Trivia

If you are curious, it is worth a try.
Examples of info you can find:
- live distance of the Voyager probes 
- distance between the planets in the solar system 
- how many satellites in orbit right now?
- math: $\pi$, $e$ and more.

## How to use it

You can run Tally in several ways:

- **Tally CLI**: the `tally` command. When you are already in your shell and for scripts.
- **Tally widget**: for [Omarchy](https://omarchy.org/) only. Quick access from the bar when you need it.
- **Tally TUI**: for something more complex, when you prefer to be guided through the options. Open it by:
  - typing `tally` in your terminal
  - the TUI button in the widget
  - the app launcher, once you add a launcher entry for it

## Settings

Everything you can configure is in one place, `~/.config/tally/settings.json`: offline mode,
whether and how the online data is refreshed (city archive, exchange rates, the Trivia's live
values: in the background, with a notice, or only when you ask), your local currency, the
distance unit, the decimal places and the date formats. Change them with the **⚙ Settings**
section: in the TUI (`O`, or the top bar), in the widget (the gear button), or with
`tally --set key=value`; `tally --settings` shows them all. Both also offer the downloads, and
say when the city archive is not downloaded yet.

## License

[MIT](LICENSE)
