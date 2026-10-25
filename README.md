# Aequalis

**Type it. Get it.** Aequalis answers maths, units, currencies and time as you type.

Write your request the way you would say it. If it can be read in more than one way, Aequalis asks you which one to get, no guessing.

```console
$ aequalis 10 km mi
6.2137 mi
$ aequalis 12 Sept 2026 + 5wd
18/09/2026 DD/MM/YYYY
$ aequalis Rome to Paris
1,107 km
$ aequalis 10:00AM Paris to New York
11 Oct 26 4:00 AM GMT-4 New York
```

Installation guides: [Windows](./docs/install_windows.md) · [macOS](./docs/install_mac.md)

## What Aequalis can do

Aequalis accepts a loose syntax, so the learning curve is flat (almost :P).

### Calculator

Normal precedence, powers (`^` or `**`), scientific notation (`2E5` is 2 × 10^5), percentages and scientific functions:
`sqrt`, `cbrt`, `root(27, 3)`, `log`, `ln`, `sin`, `cos`, `tan`, `sinh`, `cosh`, `tanh`, the factorial `5!`, plus `e` and `pi`.

| You type | Aequalis answers |
|---|---|
| `12+4/2` | `14` (the TUI preview shows the grouping: `(12 + (4 / 2)) = 14`) |
| `2^10` | `1024` |
| `sqrt 16` | `4` |
| `root(27, 3)` | `3` |
| `5!` | `120` |
| $\sin(30^\circ)$ | `0.5` |
| `150 + 20%` | `180` |
| `20% of 150` | `30` |

### A prompt

`aequalis --shell` is a plain prompt: each line is a request, answered as the whole expression
(`12 AED = 3.27 USD`), as Copy gives it in the apps. `exit`, `quit` or Ctrl-D leave. At a terminal an
ambiguous request is asked about; with the input piped it prints the session as a transcript, which is
how the examples of the website are made:

```
$ printf '2400 Thailand usd\n100 km/h mph\n' | aequalis --shell -s
aequalis> 2400 Thailand usd
2400 THB = 71.09 USD
aequalis> 100 km/h mph
100 km/h = 62.1371 mph
```

### Units

Metric and imperial: length, area, volume, mass, temperature, time, data, speed, pressure, fuel
economy, energy, power and angles.

| You type | Aequalis answers |
|---|---|
| `10 km mi` | `6.2137 mi` |
| `100 km/h mph` | `62.1371 mph` |
| `1 bar psi` | `14.5038 psi` |
| `30 deg rad` | `0.5236 rad` |
| `1 ly km` | `9460730472580.8 km` |

### Currencies

Currencies by code, name, symbol or country, in any case: `12 usd euro`, `12 USA Europe`,
`12€ gbp`; the amount can be a calculation (`12+3 Italy UAE` is `15 Italy UAE`). With one
currency, to your local one. Shared names (`dollar`) and misspellings are
asked about. Rates from [ExchangeRate-API](https://www.exchangerate-api.com), synced once a day
in the background. A date at the end gives a past day's rates (saved daily, up
to 750 days back, from when the history began): `100 usd eur 2025-01-30`, `100 usd eur yesterday`, `100 usd eur 3 months ago`.
The `rates.history` setting (on by default) turns the history off and shows how many days are held.

| You type | Aequalis answers |
|---|---|
| `12 usd euro` | `10.74 EUR` |
| `12 USD UAE` | `44.07 AED` |
| `12 China Euro` | `1.60 EUR` |
| `12+3 Italy UAE` | `61.59 AED` |
| `12 dollar euro` | asks which dollar |

### Dates

Calendar-aware date math, working days (`wd`) and dates written in many forms.

| You type | Aequalis answers |
|---|---|
| `12 Sept 2026 + 5wd` | `18/09/2026 DD/MM/YYYY` (5 working days later) |
| `2026/10/05 + 5wd` | `2026/10/12 YYYY/MM/DD` |
| `12/04/2026 unix` | asks: April 12 or December 4? |

### Time zones

Every capital and major city, and countries too.

| You type | Aequalis answers |
|---|---|
| `now new york` | the time in New York now |
| `10:00AM Paris to New York` | `11 Oct 26 4:00 AM GMT-4 New York` |
| `Italy` | the time in Rome now |
| `now new york tokyo` | the time now in New York and in Tokyo, a line each (with `now` every city is a target; any number) |
| `now usa japan` | asks which US time zone first, then shows the time now in it and in Tokyo |

### Distances

Between two cities, countries or coordinates, along the Earth's surface; a third word is the
unit. With the city archive (`aequalis --sync`, about 70,000 cities from
[GeoNames](https://www.geonames.org)), any city of 5,000 inhabitants or more, by any of its names
in any language. When several cities share a name, Aequalis asks which one: the country's capital
first, selected by default (Rome, Italy; Paris, France), then the others, those in your country
first.

| You type | Aequalis answers |
|---|---|
| `Rome Paris` | `1,107 km` (or miles, where your device is imperial); with the city archive, asks which Rome and which Paris, the capitals selected |
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

You can run Aequalis in several ways:

- **Aequalis CLI**: the `aequalis` command. When you are already in your shell and for scripts.
- **Aequalis widget**: for [Omarchy](https://omarchy.org/). Quick access from the bar when you need it.
- **Aequalis on Windows**: its icon in the notification area, or **Win+Alt+A**, opens the input box
  ([install](./docs/install_windows.md)).
- **Aequalis on macOS**: its icon in the menu bar, or **⌥⌘A**, opens the input box; also as a
  Service on any selected text ([install](./docs/install_mac.md)).
- **Aequalis TUI**: for something more complex, when you prefer to be guided through the options. Open it by:
  - typing `aequalis` in your terminal
  - the TUI button in the widget
  - the app launcher, once you add a launcher entry for it

## Settings

Everything you can configure is in one place, `~/.config/aequalis/settings.json`: offline mode,
whether and how the online data is refreshed (city archive, exchange rates, the Trivia's live
values: in the background, with a notice, or only when you ask), your local currency, the
distance unit, the decimal places and the date formats. Change them with the **⚙ Settings**
section: in the TUI (`F10` or `Alt+S`), in the widget (the gear button), or with
`aequalis --set key=value`; `aequalis --settings` shows them all. Both also offer the downloads, and
say when the city archive is not downloaded yet.

## Why Aequalis
Because to look for a tool every time you need something is really annoying any tool looks complete enough and AI is very slow!

### Why the name Aequalis

*Aequalis* is Latin for equal: what a converter tells you, that one thing equals another.
Aequalis is built around the idea if a tool that is always with you.

Aequalis runs in all your devices your laptop even the oldest you have without problems and Phone version (iOS and Android are underway).

Aequalis operate completely offline and only sync some data (for example exchange rates) if appear to be online. This means you'll never face slowness regardless the device condition!


## License

[MIT](LICENSE)
