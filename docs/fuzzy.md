# Fuzzing the engine

The fuzzer (`fuzz/`, Python standard library only) runs the engine for hours on generated
requests, the way the Android app calls it, and keeps what goes wrong. It complements the short
fuzz test in every `cargo test` (`tests/fuzz.rs`).

## Run it

```sh
fuzz/fuzz.py run --minutes 10          # a quick check
fuzz/fuzz.py run --hours 8 --workers 6 # overnight (8 cores: leave 2 free)
```

- `run`, `minimize` and `replay` always make a new fuzz build of the engine and its vocabulary
  first (`fuzz/fuzz.py build` does only that).
- `run` puts itself in a network namespace (`unshare -rn`): the engine reaches no network, its
  downloads come from the fuzzer. Ctrl-C stops it cleanly, with the summary.
- `--seed N` repeats a run; `--rotate M` minutes before a worker moves to the next configuration
  (with 6 workers and the default 20, a run under 40 min never reaches the last configurations).
- Every run starts clean: `fuzz/findings/` and `fuzz/work/` (all but the vocabulary) are deleted,
  and a worker that restarts starts from an empty folder: no settings or downloads of the one before.
- The fuzz build (`[profile.fuzz]` in `Cargo.toml`) checks every integer overflow: a silent wrong
  answer in the release build is a crash here, so it gets found.

What it does: each worker loads `target/fuzz/libaequalis.so` and, in one configuration (time zone,
zone files missing or damaged, Android's zone bundle, number format, imperial, city archive,
settings), types requests key by key: the answer and the suggestions for every prefix. It also
changes settings, reads the panels' lists, and syncs with damaged downloads.

## Read the results

Every 30 s a status line: calls, calls per second, crashes, hangs, findings by kind. A new finding
is printed once (`NEW crash: …`, `new contract: …`). At the end, the summary: per finding, how many
times it came up and its input. New crashes are then minimized automatically, and the whole run
is written to **`fuzz/findings/report.md`**: the run, the findings by kind and by configuration,
each crash with the commands that reproduce it and its backtrace, each hang with the calls before
it and the stuck worker's stack.

Findings are in `fuzz/findings/<kind>/<key>/` (not in git):

| File | |
|---|---|
| `report.json` | what happened, the configuration and seed, the count, first and last seen; for a crash the panic's location and message, the last 32 calls (`history`) and stderr |
| `input.txt` | the input as found |
| `minimal.txt` | the smallest input that still crashes the same way (crashes only) |

| Kind | Means | Priority |
|---|---|---|
| `crash` | the engine panicked: **in the app, Aequalis closes**. Key: the source location | fix first |
| `hang` | a call over 2 s, seen on two reads in a row. Key: the function and its input. The report says whether the worker was busy (CPU time ≈ wall time: a loop, a slow path) or waiting (disk, a lock, an overloaded machine), with its stack (`eu-stack`) | fix |
| `contract` | a reply the app can't read (bad JSON, a missing key, a suggestion's `start` out of range): the Kotlin side would crash | fix |
| `length` | the 512-character limit not respected | fix |
| `choices` | a question's choice asks the same question again: a loop in the app | fix |
| `differential` | the calculator disagrees with Python on the same expression | check: a bug, or a rule Python doesn't model |
| `inconsistency` | two spellings of the same request answer differently (spaces, brackets, case, `to`/`in`/`->`, round trips) | check |
| `semantics` | a design question, not a bug (a huge intermediate value turned into an answer) | decide |
| `slow` | a call over 50 ms, timed three times (a phone is 5–10× slower) | look |
| `memory` | a worker over 300 MB | look |
| `harness` | the fuzzer itself failed | fix the fuzzer |

Known and expected now: `inconsistency: connector:to/in/->` (`10 km to mi` is refused,
`docs/localization.md` §1.1).

## Check one finding

```sh
fuzz/fuzz.py minimize fuzz/findings/crash/<key>   # shrink it again
fuzz/fuzz.py replay                               # every fixed finding, in every configuration
```

To try an input by hand: `echo 'GMT+2 €' | aequalis --json` (the CLI shares the engine), or
`aequalis_eval` through `fuzz/worker.py --replay` (see `fuzz.py`'s `replay_calls`).

## From finding to fix

1. Reproduce with `minimal.txt`.
2. Fix, and add a regression suite `tests/bug_NNN_<name>.rs` (the next number, `docs/SPEC.md` §9).
3. Add the input to `fuzz/corpus/regressions.txt` (one JSON per line: `input`, optional
   `config`): `cargo test` and `fuzz.py replay` run it forever.
4. Run again (it builds the fixed code).

The engine is solid enough when a night (8 h, 6 workers) ends with no crash, hang, contract or
choices finding.
