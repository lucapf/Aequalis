"""What counts as a finding, besides a crash or a hang (the supervisor sees those): replies that
break what the Android app reads (`apps/android/.../Core.kt`; a missing key there is an exception,
so a crash of the app), the 512-character limit, and the values Python can check. Each check
returns a list of `(kind, key, detail)`; the key deduplicates.
"""

import json
import math

TOO_LONG = "the request is too long: at most 512 characters"
MAX_INPUT = 512


def parse(reply):
    """The reply as JSON, or the finding that it isn't."""
    try:
        value = json.loads(reply)
    except ValueError as e:
        return None, [("contract", "not-json", f"{e}: {reply[:200]!r}")]
    if not isinstance(value, dict):
        return None, [("contract", "not-an-object", reply[:200])]
    return value, []


def _string(obj, key, where, optional=False):
    if key not in obj:
        return [] if optional else [("contract", f"{where}.{key}:missing", json.dumps(obj)[:300])]
    if not isinstance(obj[key], str):
        return [("contract", f"{where}.{key}:type", json.dumps(obj)[:300])]
    return []


def eval_reply(request, reply):
    """`Core.reply` (Kotlin): `ok`; then `result` (getString); or `incomplete` with `error`; or
    `ambiguous.choices[]` with `pattern`, `date`, `request` (getString each)."""
    doc, found = parse(reply)
    if doc is None:
        return None, found
    if not isinstance(doc.get("ok"), bool):
        found.append(("contract", "eval.ok", reply[:300]))
        return doc, found
    for key in ("rates_age", "grouping"):
        found += _string(doc, key, "eval", optional=True)
    if doc["ok"]:
        found += _string(doc, "result", "eval")
    else:
        found += _string(doc, "error", "eval", optional=not ("incomplete" in doc))
        amb = doc.get("ambiguous")
        if amb is not None:
            choices = amb.get("choices") if isinstance(amb, dict) else None
            if not isinstance(choices, list):
                found.append(("contract", "eval.ambiguous.choices", reply[:300]))
            else:
                if not choices:
                    found.append(("contract", "eval.ambiguous.no-choices", reply[:300]))
                for c in choices:
                    if not isinstance(c, dict):
                        found.append(("contract", "eval.choice:type", reply[:300]))
                        continue
                    for key in ("pattern", "date", "request"):
                        found += _string(c, key, "eval.choice")
    # The other results (`42 eu us`: the shoe sizes): `extra[]` with `type` and `results[]`, strings.
    extra = doc.get("extra")
    if extra is not None:
        if not isinstance(extra, list) or not extra:
            found.append(("contract", "eval.extra", reply[:300]))
        else:
            for e in extra:
                ok = isinstance(e, dict) and isinstance(e.get("type"), str) and isinstance(e.get("results"), list)
                if not ok or not all(isinstance(r, str) for r in e["results"]):
                    found.append(("contract", "eval.extra:item", reply[:300]))
    # The length limit, counted in characters (code points), spaces too.
    long = len(request) > MAX_INPUT
    refused = doc.get("error") == TOO_LONG
    if long and not refused:
        found.append(("length", "accepted-too-long", f"{len(request)} characters: {reply[:200]}"))
    if refused and not long:
        found.append(("length", "refused-short", f"{len(request)} characters"))
    return doc, found


def suggest_reply(before, reply):
    """`Core.suggestions`: `start` (getInt) then `before.offsetByCodePoints(0, start)`, which throws
    past the end; `items[]` with `write` and `label` (getString)."""
    doc, found = parse(reply)
    if doc is None:
        return None, found
    start = doc.get("start")
    if not isinstance(start, int) or isinstance(start, bool):
        return doc, found + [("contract", "suggest.start:type", reply[:300])]
    if not 0 <= start <= len(before):
        found.append(("contract", "suggest.start:range", f"start {start}, {len(before)} characters"))
    items = doc.get("items")
    if not isinstance(items, list):
        return doc, found + [("contract", "suggest.items", reply[:300])]
    for item in items:
        if not isinstance(item, dict):
            found.append(("contract", "suggest.item:type", reply[:300]))
            continue
        for key in ("write", "label"):
            found += _string(item, key, "suggest.item")
    if len(before) > MAX_INPUT and items:
        found.append(("length", "suggested-too-long", f"{len(before)} characters"))
    return doc, found


# The lists the app reads (`Core.measures`, `currencies`, `trivia`, `settings`): key → its shape.
LIST_SHAPES = {
    "measures": ("classes", {"name": str, "systems": list, "units": [{"symbol": str, "name": str, "system": str}]}),
    "currencies": ("currencies", {"code": str, "name": str, "symbols": list, "countries": list, "aliases": list}),
    "trivia": ("sections", {"name": str, "entries": [{"symbol": str, "name": str, "short": str, "value": str, "written": str, "description": str, "popup": list}]}),
    "settings": ("settings", {"key": str, "value": str, "default": str, "kind": str, "options": list, "about": str}),
}


def list_reply(which, reply):
    doc, found = parse(reply)
    if doc is None:
        return found
    top, shape = LIST_SHAPES[which]
    if not isinstance(doc.get(top), list):
        return found + [("contract", f"{which}.{top}", reply[:300])]

    def check(obj, shape, where):
        out = []
        if not isinstance(obj, dict):
            return [("contract", f"{where}:type", json.dumps(obj)[:200])]
        for key, kind in shape.items():
            if key not in obj:
                out.append(("contract", f"{where}.{key}:missing", json.dumps(obj)[:200]))
            elif isinstance(kind, list):
                if not isinstance(obj[key], list):
                    out.append(("contract", f"{where}.{key}:type", json.dumps(obj)[:200]))
                else:
                    for item in obj[key]:
                        out += check(item, kind[0], f"{where}.{key}")
            elif not isinstance(obj[key], kind):
                out.append(("contract", f"{where}.{key}:type", json.dumps(obj)[:200]))
        return out

    for item in doc[top]:
        found += check(item, shape, which)
    if which == "trivia":
        for section in doc[top]:
            for entry in section.get("entries", []):
                for line in entry.get("popup", []):
                    if not (isinstance(line, list) and len(line) >= 2 and all(isinstance(x, str) for x in line[:2])):
                        found.append(("contract", "trivia.popup-line", json.dumps(line)[:200]))
    return found


def number_of(result):
    """Aequalis's plain number (`0.333333333333333`, `1.6022e-19`), or None."""
    try:
        return float(result)
    except (TypeError, ValueError):
        return None


def differential(text, expected, scale, doc):
    """The calculator against Python: the same value within 1e-9 of the largest magnitude met, or
    an error in both."""
    if doc is None:
        return []
    got = number_of(doc.get("result")) if doc.get("ok") else None
    if expected is None and got is None:
        return []
    if expected is None:
        return [("differential", "aequalis-answers-python-fails", f"{text!r} → Aequalis {doc.get('result')!r}, Python: error")]
    if got is None:
        return [("differential", "aequalis-fails-python-answers", f"{text!r} → Aequalis {doc.get('error') or doc.get('result')!r}, Python {expected!r}")]
    tolerance = 1e-9 * max(1.0, scale if math.isfinite(scale) else abs(expected))
    if abs(got - expected) > tolerance:
        return [("differential", "values-differ", f"{text!r} → Aequalis {got!r}, Python {expected!r}")]
    return []


def strip(reply):
    """A reply without what changes between two identical calls (the memory figure)."""
    doc, _ = parse(reply)
    if doc is None:
        return reply
    doc.pop("peak_kb", None)
    return json.dumps(doc, sort_keys=True)
