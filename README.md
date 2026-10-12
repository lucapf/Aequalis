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

One amount and two currencies, as ISO codes in any case (`usd`, `USD`), with `to` or `in` if you
like. The rates are the European Central Bank's reference rates, downloaded once a day in the
background like the other online data: a conversion itself never waits for the network.
`tally --list` lists the 30 currencies.

| You type | Tally answers |
|---|---|
| `100 usd eur` | `89.44 EUR` |
| `2500 JPY in GBP` | `11.91 GBP` |
| `100 usd` | asks for the currency to convert to |

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

Between cities, countries or coordinates, along the Earth's surface.

| You type | Tally answers |
|---|---|
| `Rome to Paris` | `1,107 km` |
| `Rome Doha mi` | `2,497 mi` |

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
- **Tally menu bar app**: for macOS. The same quick access from the menu bar, or ⌃⌥T anywhere.
- **Tally for Raycast**: for macOS, in Raycast's search bar.
- **Tally TUI**: for something more complex, when you prefer to be guided through the options. Open it by:
  - typing `tally` in your terminal
  - the TUI button in the widget
  - the app launcher, once you add a launcher entry for it


On a Mac, see [Install Tally on macOS](docs/install_mac.md).

## License

[MIT](LICENSE)
