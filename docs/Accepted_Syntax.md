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
