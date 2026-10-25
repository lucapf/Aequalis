#!/usr/bin/env python3
"""Keeps Aequalis's exchange rates of each day: the latest rates of ExchangeRate-API
(open.er-api.com, the ones the app uses), written as a file named for their day.

    scripts/build-rates.py [<output dir>]        (default: dist)

Writes `currencies-YYMMDD.json`, YYMMDD being the UTC day of `time_last_update_unix`, in
ExchangeRate-API's own answer format, as the app reads it:

    {"result":"success","time_last_update_unix":…,"rates":{"USD":1,"EUR":0.92,…}}

There is no history to download: the archive grows by a file a day (.github/workflows/rates.yml),
from the day it was started. Standard library only.
"""

import datetime
import json
import os
import sys
import urllib.request

URL = "https://open.er-api.com/v6/latest/USD"


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "dist"
    os.makedirs(out, exist_ok=True)
    request = urllib.request.Request(URL, headers={"User-Agent": "aequalis-rates (https://github.com/lucapf/aequalis)"})
    with urllib.request.urlopen(request, timeout=60) as r:
        body = json.load(r)
    assert body.get("result") == "success", body.get("result")
    assert len(body["rates"]) > 100, len(body["rates"])
    day = datetime.datetime.fromtimestamp(body["time_last_update_unix"], datetime.timezone.utc)
    keep = {"result": "success", "provider": body.get("provider", "https://www.exchangerate-api.com"),
            "time_last_update_unix": body["time_last_update_unix"], "rates": body["rates"]}
    name = f"currencies-{day:%y%m%d}.json"
    with open(os.path.join(out, name), "w") as f:
        json.dump(keep, f, separators=(",", ":"))
        f.write("\n")
    print(name, len(keep["rates"]), "rates")


if __name__ == "__main__":
    main()
