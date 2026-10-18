/*
 * aequalis's C interface (src/ffi.rs), for apps that embed it: the Android app.
 *
 * Every string is UTF-8 and NUL-terminated. Every string returned is JSON and must be given
 * back with aequalis_free(). Call aequalis_init() once, first, then aequalis_set_fetcher().
 */
#ifndef AEQUALIS_H
#define AEQUALIS_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Frees a string returned by any aequalis_* function. */
void aequalis_free(char *s);

/*
 * Starts aequalis with what the app knows about the device:
 * {"dir": "<data folder>", "zone": "Europe/Rome", "zoneinfo": "<zone files folder>",
 *  "decimal": ",", "thousands": ".", "grouping": [3], "imperial": false}
 * Only "dir" is required. Returns {"ok":true} or {"ok":false,"error":"…"}.
 */
char *aequalis_init(const char *config_json);

/*
 * The app's downloader: saves `url` into the file `to` within `seconds` and returns whether it
 * did; with `strict`, an HTTP error is a failure. Called from the thread running aequalis_sync().
 */
typedef bool (*aequalis_fetch_fn)(const char *url, const char *to, uint32_t seconds, bool strict);
void aequalis_set_fetcher(aequalis_fetch_fn fetch);

/* The reply of `aequalis --json <request>`: {"ok":true,"result":…} or the error, the choices. */
char *aequalis_eval(const char *request);

/* The reply as a UI shows it: aequalis_eval's, with "show" (result, hint, question, error, none),
   "message", "value" (what Copy gives), "rates", "details", "choices", "default", "class".
   submitted: Enter was pressed; until then a reply that is no result shows nothing while a word is
   being typed. */
char *aequalis_view(const char *request, bool submitted);

/* Every setting ({"file":…,"settings":[{"key","value","default","kind","options","about"}]}). */
char *aequalis_settings(void);

/* Changes a setting (value NULL: its default): {"ok":true,"key":…,"value":…} or the error. */
char *aequalis_set(const char *key, const char *value);

/* Downloads the online data in this thread: everything, or with `auto` what the settings say. */
char *aequalis_sync(bool automatic);

/*
 * The data shipped with an app: `rates` (ExchangeRate-API's answer) and the path of the city
 * archive's JSON file (NULL or empty: none): {"ok":true,"rates":N,"cities":N} or the error.
 */
char *aequalis_seed(const char *rates, const char *cities);

/*
 * Suggestions for the word being typed at the end of `before` (the text up to the cursor), from its
 * `min`-th character: {"start":N,"items":[{"write":"km","label":"km - kilometre"}]}.
 */
char *aequalis_suggest(const char *before, uint32_t min);

/* The Trivia, the unit classes, the currencies (see src/ffi.rs for their shapes). */
char *aequalis_trivia(void);
char *aequalis_measures(void);
char *aequalis_currencies(void);

#ifdef __cplusplus
}
#endif

#endif
