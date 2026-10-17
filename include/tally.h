/*
 * tally's C interface (src/ffi.rs), for apps that embed it: the Android app.
 *
 * Every string is UTF-8 and NUL-terminated. Every string returned is JSON and must be given
 * back with tally_free(). Call tally_init() once, first, then tally_set_fetcher().
 */
#ifndef TALLY_H
#define TALLY_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Frees a string returned by any tally_* function. */
void tally_free(char *s);

/*
 * Starts tally with what the app knows about the device:
 * {"dir": "<data folder>", "zone": "Europe/Rome", "zoneinfo": "<zone files folder>",
 *  "decimal": ",", "thousands": ".", "grouping": [3], "imperial": false}
 * Only "dir" is required. Returns {"ok":true} or {"ok":false,"error":"…"}.
 */
char *tally_init(const char *config_json);

/*
 * The app's downloader: saves `url` into the file `to` within `seconds` and returns whether it
 * did; with `strict`, an HTTP error is a failure. Called from the thread running tally_sync().
 */
typedef bool (*tally_fetch_fn)(const char *url, const char *to, uint32_t seconds, bool strict);
void tally_set_fetcher(tally_fetch_fn fetch);

/* The reply of `tally --json <request>`: {"ok":true,"result":…} or the error, the choices. */
char *tally_eval(const char *request);

/* The reply as a UI shows it: tally_eval's, with "show" (result, hint, question, error, none),
   "message", "value" (what Copy gives), "rates", "details", "choices", "default", "class".
   submitted: Enter was pressed; until then a reply that is no result shows nothing while a word is
   being typed. */
char *tally_view(const char *request, bool submitted);

/* Every setting ({"file":…,"settings":[{"key","value","default","kind","options","about"}]}). */
char *tally_settings(void);

/* Changes a setting (value NULL: its default): {"ok":true,"key":…,"value":…} or the error. */
char *tally_set(const char *key, const char *value);

/* Downloads the online data in this thread: everything, or with `auto` what the settings say. */
char *tally_sync(bool automatic);

/*
 * The data shipped with an app: `rates` (ExchangeRate-API's answer) and the path of the city
 * archive's JSON file (NULL or empty: none): {"ok":true,"rates":N,"cities":N} or the error.
 */
char *tally_seed(const char *rates, const char *cities);

/*
 * Suggestions for the word being typed at the end of `before` (the text up to the cursor), from its
 * `min`-th character: {"start":N,"items":[{"write":"km","label":"km - kilometre"}]}.
 */
char *tally_suggest(const char *before, uint32_t min);

/* The Trivia, the unit classes, the currencies (see src/ffi.rs for their shapes). */
char *tally_trivia(void);
char *tally_measures(void);
char *tally_currencies(void);

#ifdef __cplusplus
}
#endif

#endif
