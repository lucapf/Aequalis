# Tally for Omarchy

A bar widget for [Omarchy](https://omarchy.org/): one icon in the bar opens
[Tally](https://github.com/lucapf/tally), the ultralight calculator and converter made for
humans. Type `10 km mi`, `12+4/2`, `12 Sept 2026 + 5wd` or `10:00AM Paris to New York` and copy
the answer.

The widget runs the `tally` command, so install that first.

## Install

1. The `tally` command, from the AUR:

   ```sh
   yay -S tally-calc-bin
   ```

   Or download the archive for your machine from the
   [releases](https://github.com/lucapf/tally/releases) and put `tally` on your `PATH`
   (for example in `~/.local/bin`).

2. The widget:

   ```sh
   omarchy plugin add https://github.com/lucapf/omarchy-tally.git --enable
   ```

   It goes in the right section of the bar by default.

## Update

```sh
yay -Syu tally-calc-bin
omarchy plugin update lucapf.tally
```

## Settings

- `command`: the tally binary, when it is not on the shell's `PATH` (default `tally`).
- `TALLY_MODE=offline` in the shell's environment keeps Tally off the network.

## Remove

```sh
omarchy plugin remove lucapf.tally
yay -R tally-calc-bin
```

This repository is published from [lucapf/tally](https://github.com/lucapf/tally)
(`omarchy-widget/`) on every release: change the widget there.

## License

[MIT](LICENSE)
