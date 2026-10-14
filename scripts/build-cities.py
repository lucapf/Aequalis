#!/usr/bin/env python3
"""Builds Tally's city archive from GeoNames (geonames.org, CC BY 4.0).

    scripts/build-cities.py [<output dir>]        (default: dist)

Downloads cities5000 (every city of 5,000 inhabitants or more, and seats of first-order
divisions), the capitals below that from cities1000, the region names (admin1CodesASCII) and the
country names (countryInfo), and writes:

    cities.json     {"format":1, "source", "license", "built_at", "count", "cities":[…]}: what
                    Tally downloads, as it is (no compression: any downloader saves it as a file)
    cities.version  "<built_at> <sha256 of cities.json>", to check for a new archive cheaply

Each city: id (GeoNames id), name, aliases (every alternate name, any language, but not the name of
a more populous city), lat, lon,
population, country, country_code, region, updated (GeoNames' modification date), and
`"capital": true` for a country's capital (GeoNames' PPLC). The Google Maps link is not stored:
Tally builds it from the coordinates. Standard library only.
"""

import hashlib
import io
import json
import os
import sys
import time
import urllib.request
import zipfile

BASE = "https://download.geonames.org/export/dump/"


def fetch(name):
    with urllib.request.urlopen(BASE + name, timeout=300) as r:
        return r.read()


def rows(text):
    for line in text.splitlines():
        if line and not line.startswith("#"):
            yield line.split("\t")


def zipped(name):
    data = fetch(name)
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        return z.read(name.replace(".zip", ".txt")).decode("utf-8")


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "dist"
    os.makedirs(out, exist_ok=True)

    countries = {r[0]: r[4] for r in rows(fetch("countryInfo.txt").decode("utf-8")) if len(r) > 4}
    regions = {r[0]: r[1] for r in rows(fetch("admin1CodesASCII.txt").decode("utf-8")) if len(r) > 1}

    def city(r):
        name = r[1]
        seen = {name.casefold()}
        aliases = []
        for a in [r[2]] + r[3].split(","):
            a = a.strip()
            if a and a.casefold() not in seen:
                seen.add(a.casefold())
                aliases.append(a)
        c = {
            "id": int(r[0]),
            "name": name,
            "aliases": aliases,
            "lat": round(float(r[4]), 5),
            "lon": round(float(r[5]), 5),
            "population": int(r[14] or 0),
            "country": countries.get(r[8], r[8]),
            "country_code": r[8],
            "region": regions.get(f"{r[8]}.{r[10]}", ""),
            "updated": r[18],
        }
        # The capital: offered first among the cities of its name (`Rome`: Rome, Italy).
        if r[7] == "PPLC":
            c["capital"] = True
        return c

    cities = {}
    for r in rows(zipped("cities5000.zip")):
        cities[r[0]] = city(r)
    # Capitals smaller than 5,000 inhabitants (Vaduz, Ngerulmud…).
    for r in rows(zipped("cities1000.zip")):
        if r[7] == "PPLC" and r[0] not in cities:
            cities[r[0]] = city(r)

    listed = sorted(cities.values(), key=lambda c: (-c["population"], c["name"]))
    # An alias that is the name of a more populous city is dropped: Lomé is not "Rome", Willard
    # (Ohio) not "Chicago", Parys not "Paris" (historic or foreign names that would shadow them).
    biggest = {}
    for c in listed:
        biggest.setdefault(c["name"].casefold(), c["population"])
    for c in listed:
        c["aliases"] = [a for a in c["aliases"] if biggest.get(a.casefold(), -1) <= c["population"]]
    doc = {
        "format": 1,
        "source": "GeoNames (https://www.geonames.org)",
        "license": "CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/)",
        "built_at": int(time.time()),
        "count": len(listed),
        "cities": listed,
    }
    # One city per line: readable, and diffs stay small.
    body = ",\n".join(json.dumps(c, ensure_ascii=False, separators=(",", ":")) for c in listed)
    head = json.dumps({k: v for k, v in doc.items() if k != "cities"}, ensure_ascii=False)[:-1]
    text = f'{head}, "cities": [\n{body}\n]}}\n'.encode("utf-8")
    json.loads(text)  # valid

    with open(os.path.join(out, "cities.json"), "wb") as f:
        f.write(text)
    with open(os.path.join(out, "cities.version"), "w") as f:
        f.write(f"{doc['built_at']} {hashlib.sha256(text).hexdigest()}\n")
    print(f"{len(listed)} cities, {len(text) / 1e6:.1f} MB → {out}")


if __name__ == "__main__":
    main()
