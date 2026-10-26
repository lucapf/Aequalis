#!/usr/bin/env python3
"""Builds Aequalis's public holidays: every country the `holidays` library knows (python-holidays,
https://github.com/vacanza/holidays), the national public holidays of last year, this year and
next year, with their names in each language the library has for the country.

    scripts/build-holidays.py [<output dir>]        (default: dist)

Writes `holidays.json`, as the app reads it (src/holidays.rs):

    {"format":1,"built":"2026-10-26","years":[2025,2026,2027],"countries":{
      "IT":{"default":"it","days":["2025-01-01",…],"names":{"en":["New Year's Day",…],"it":["Capodanno",…]}},…}}

`days` and each list of `names` go together, a name per day (a day with two holidays is there
twice). A language is its first part (`en_US` → `en`; `en_US` first where a country has several
English ones); `default` is the country's own. Islamic and other lunar holidays are the library's
estimates, and say so in their name (`Eid al-Fitr (estimated)`). Needs `pip install holidays`.
"""

import datetime
import json
import os
import sys

import holidays


def languages(country):
    """The country's languages: short name → the library's name (`en` → `en_US`)."""
    found = {}
    for lang in country.supported_languages or ():
        short = lang.split("_")[0]
        if short not in found or lang == "en_US":
            found[short] = lang
    return found


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "dist"
    os.makedirs(out, exist_ok=True)
    today = datetime.date.today()
    years = [today.year - 1, today.year, today.year + 1]
    countries = {}
    # Two-letter codes only: the library lists the three-letter ones too, as the same countries.
    for code in sorted(c for c in holidays.list_supported_countries() if len(c) == 2):
        country = holidays.country_holidays(code)
        langs = languages(country)
        default = (country.default_language or "en").split("_")[0]
        if not langs:
            langs = {default: None}
        days, names = None, {}
        for short, lang in langs.items():
            items = sorted(holidays.country_holidays(code, years=years, language=lang).items())
            if days is None:
                days = [day.isoformat() for day, _ in items]
            # A day with several holidays: `A; B` in the library, a name each here.
            names[short] = [name for _, name in items]
        if not days:
            continue
        # Several on one day: the days repeated, a name each.
        split_days, split_names = [], {short: [] for short in names}
        for i, day in enumerate(days):
            parts = {short: names[short][i].split("; ") for short in names}
            count = max(len(p) for p in parts.values())
            for k in range(count):
                split_days.append(day)
                for short, p in parts.items():
                    split_names[short].append(p[min(k, len(p) - 1)])
        countries[code] = {"default": default if default in names else next(iter(names)), "days": split_days, "names": split_names}
    archive = {"format": 1, "built": today.isoformat(), "years": years, "countries": countries}
    path = os.path.join(out, "holidays.json")
    with open(path, "w", encoding="utf-8") as f:
        json.dump(archive, f, ensure_ascii=False, separators=(",", ":"))
        f.write("\n")
    print(path, len(countries), "countries", years, os.path.getsize(path), "bytes")


if __name__ == "__main__":
    main()
