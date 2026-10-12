// Runs the tally CLI (`tally --json …`) and reads its reply: the same contract as the Omarchy
// widget (omarchy-widget/Panel.qml) and the macOS menu bar app (macos/Sources/TallyCore).
import { execFile } from "node:child_process";
import { accessSync, constants } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

export interface Choice {
  pattern?: string;
  date?: string;
  request: string;
}

export interface Ambiguity {
  kind?: string;
  text?: string;
  note?: string;
  choices: Choice[];
}

export interface Reply {
  ok: boolean;
  result?: string;
  error?: string;
  notes?: string[];
  warnings?: string[];
  ambiguous?: Ambiguity;
  peak_kb?: number;
}

export interface Entry {
  input: string;
  result: string;
}

/// Where the tally CLI is: the preference, else the usual places (Raycast doesn't get the
/// shell's PATH).
export function locateTally(setting: string | undefined, isExecutable = executable): string | undefined {
  const candidates: string[] = [];
  const s = setting?.trim();
  if (s) candidates.push(s.startsWith("~/") ? join(homedir(), s.slice(2)) : s);
  candidates.push(join(homedir(), ".local/bin/tally"), "/opt/homebrew/bin/tally", "/usr/local/bin/tally");
  return candidates.find(isExecutable);
}

function executable(path: string): boolean {
  try {
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

/// The request's words, as the command line gets them.
export function words(query: string): string[] {
  return query.split(/\s+/).filter((w) => w !== "");
}

/// `tally --json <args>`; a failed run (exit 1 or 2) still prints its reply.
export function run(path: string, args: string[], offline: boolean): Promise<Reply> {
  const env = { ...process.env, ...(offline ? { TALLY_MODE: "offline" } : {}) };
  return new Promise((resolve) => {
    execFile(path, ["--json", ...args], { env, timeout: 10_000 }, (error, stdout) => {
      try {
        resolve(JSON.parse(stdout) as Reply);
      } catch {
        resolve({ ok: false, error: error ? `tally did not answer: ${error.message}` : "tally did not answer" });
      }
    });
  });
}

/// The line under the field: `= 6.2137 mi`, or what is wrong, or the question for the choices.
export function headline(reply: Reply): string {
  if (reply.ok) return `= ${reply.result ?? ""}`;
  const a = reply.ambiguous;
  if (a) {
    if (a.kind === "city") return `${a.note ?? `\`${a.text}\` is not a city`}: did you mean…`;
    if (a.kind === "country") return `\`${a.text}\` has several time zones: which one?`;
    return `Which date is ${a.text}?`;
  }
  return reply.error ?? "no answer";
}

/// The value to copy: the result without the format's name (`2026/11/22 YYYY/MM/DD` →
/// `2026/11/22`), the ISO time of a city's time; `6.2137 mi` stays as it is.
export function valueOf(result: string): string {
  const side =
    result.includes(" = ") && /T\d\d:\d\d/.test(result) ? result.slice(result.lastIndexOf(" = ") + 3) : result;
  const iso = side.match(/^(\d{4}-\d\d-\d\dT\S+) /);
  if (iso) return iso[1];
  const cut = side.lastIndexOf(" ");
  const last = cut >= 0 ? side.slice(cut + 1) : "";
  const format = /^(iso|iso-[a-z]+|unix|unix-[a-z]+|local|[YMD]{2,4}\/[YMD]{2,4}\/[YMD]{2,4}(-(12|24)h(-tz)?)?)$/;
  return format.test(last) ? side.slice(0, cut) : side;
}

/// Recent with `entry` first: no duplicate request, at most `limit`.
export function remember(entry: Entry, recent: Entry[], limit = 10): Entry[] {
  return [entry, ...recent.filter((e) => e.input !== entry.input)].slice(0, limit);
}
