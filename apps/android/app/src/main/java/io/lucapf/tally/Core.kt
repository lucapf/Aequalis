package io.lucapf.tally

import android.content.Context
import android.icu.util.LocaleData
import android.icu.util.ULocale
import android.os.Build
import org.json.JSONObject
import java.io.File
import java.text.DecimalFormatSymbols
import java.util.Locale
import java.util.TimeZone

/** A choice of an ambiguous request: what it means, and the request that says it exactly. */
data class Choice(val label: String, val detail: String, val request: String)

/** A word offered while typing: what is written, and how it is shown (`km - kilometre`). */
data class Suggestion(val write: String, val label: String)

/** The words offered for the word being typed, which starts at `start` (an index in the text). */
data class Suggestions(val start: Int, val items: List<Suggestion>)

/** A unit of a class of the Measures panel: its symbol (what is written), name and system. */
data class MeasureUnit(val symbol: String, val name: String, val system: String)

/** A class of units (`Length`): its two systems' names (`Metric`, `Imperial`) and its units. */
data class Measure(val name: String, val systems: List<String>, val units: List<MeasureUnit>)

/** A currency: its code (what is written), name and symbols. */
data class Currency(val code: String, val name: String, val symbols: List<String>)

/** A Trivia value: its name, a few words, its value now, and what tapping it writes. */
data class TriviaEntry(val name: String, val short: String, val value: String, val written: String)

data class TriviaSection(val name: String, val entries: List<TriviaEntry>)

/** A setting: its key, value and default; `kind` is `bool`, `choice` (one of `options`) or `text`. */
data class Setting(val key: String, val value: String, val default: String, val kind: String, val options: List<String>, val about: String)

/** The core's answer to a request: a result, an error, a hint (not finished yet: what comes next),
 *  or a question with its choices. */
data class Reply(
    val result: String? = null,
    val error: String? = null,
    val hint: String? = null,
    val question: String? = null,
    val choices: List<Choice> = emptyList(),
)

/**
 * The bridge to tally's Rust core: libtally.so (built by scripts/build-android.sh), whose
 * functions src/jni.rs exports. Text crosses as UTF-8 bytes; replies are JSON, as `tally --json`.
 */
object Core {
    init {
        // Loads lib/<abi>/libtally.so from the app: the x86_64 one on the emulator, arm64 on phones.
        System.loadLibrary("tally")
    }

    // Implemented in Rust (src/jni.rs): `Java_io_lucapf_tally_Core_init`, `…_eval`, `…_suggest`,
    // `…_lists`, `…_set`.
    private external fun init(config: ByteArray): ByteArray
    private external fun eval(request: ByteArray): ByteArray
    private external fun suggest(before: ByteArray): ByteArray
    private external fun lists(which: ByteArray): ByteArray
    private external fun set(key: ByteArray, value: ByteArray): ByteArray

    /** Tells the core what it can't find on a phone: where to keep its files, the time zone, the
     *  zone database, the number format and the measurement system. Once, at start. */
    fun start(context: Context) {
        val symbols = DecimalFormatSymbols.getInstance()
        val config = JSONObject()
            .put("dir", context.filesDir.absolutePath)
            .put("zone", TimeZone.getDefault().id)
            .put("decimal", symbols.decimalSeparator.toString())
            .put("thousands", symbols.groupingSeparator.toString())
            .put("imperial", imperial())
        zoneDatabase()?.let { config.put("zoneinfo", it) }
        val reply = JSONObject(init(config.toString().encodeToByteArray()).decodeToString())
        check(reply.optBoolean("ok")) { "tally: ${reply.optString("error")}" }
    }

    /** The core's answer to `request`, as typed. */
    fun evaluate(request: String): Reply {
        val json = JSONObject(eval(request.encodeToByteArray()).decodeToString())
        if (json.optBoolean("ok")) return Reply(result = json.getString("result"))
        // Not finished yet (`10 km`): what comes next, no mistake.
        if (json.optBoolean("incomplete")) return Reply(hint = json.optString("error"))
        val ambiguous = json.optJSONObject("ambiguous") ?: return Reply(error = json.optString("error"))
        val choices = ambiguous.getJSONArray("choices")
        return Reply(
            error = json.optString("error"),
            question = ambiguous.optString("note").ifEmpty { ambiguous.optString("text") },
            choices = (0 until choices.length()).map { i ->
                val c = choices.getJSONObject(i)
                Choice(c.getString("pattern"), c.getString("date"), c.getString("request"))
            },
        )
    }

    /** The words to offer for the word being typed at the end of `before` (the text up to the
     *  cursor), from its third letter: units, currencies, places, formats. */
    fun suggestions(before: String): Suggestions {
        val json = JSONObject(suggest(before.encodeToByteArray()).decodeToString())
        val items = json.getJSONArray("items")
        // The core counts characters (code points); a Kotlin string counts UTF-16 units, two for an
        // emoji: the same position, in Kotlin's terms.
        val start = before.offsetByCodePoints(0, json.getInt("start"))
        return Suggestions(start, (0 until items.length()).map { i ->
            val item = items.getJSONObject(i)
            Suggestion(item.getString("write"), item.getString("label"))
        })
    }

    private fun list(which: String): JSONObject = JSONObject(lists(which.encodeToByteArray()).decodeToString())

    private fun org.json.JSONArray.strings(): List<String> = (0 until length()).map { getString(it) }

    private fun <T> org.json.JSONArray.objects(read: (JSONObject) -> T): List<T> = (0 until length()).map { read(getJSONObject(it)) }

    /** The unit classes, in the TUI's order. */
    fun measures(): List<Measure> = list("measures").getJSONArray("classes").objects { c ->
        Measure(c.getString("name"), c.getJSONArray("systems").strings(), c.getJSONArray("units").objects { u ->
            MeasureUnit(u.getString("symbol"), u.getString("name"), u.getString("system"))
        })
    }

    /** Every currency, the most used first. */
    fun currencies(): List<Currency> = list("currencies").getJSONArray("currencies").objects { c ->
        Currency(c.getString("code"), c.getString("name"), c.getJSONArray("symbols").strings())
    }

    /** The Trivia, by section. */
    fun trivia(): List<TriviaSection> = list("trivia").getJSONArray("sections").objects { s ->
        TriviaSection(s.getString("name"), s.getJSONArray("entries").objects { e ->
            TriviaEntry(e.getString("name"), e.getString("short"), e.getString("value"), e.getString("written"))
        })
    }

    /** Every setting, in order. */
    fun settings(): List<Setting> = list("settings").getJSONArray("settings").objects { s ->
        Setting(s.getString("key"), s.getString("value"), s.getString("default"), s.getString("kind"), s.getJSONArray("options").strings(), s.getString("about"))
    }

    /** Changes a setting and saves it (`""`: its default); the error, or null. */
    fun change(key: String, value: String): String? {
        val reply = JSONObject(set(key.encodeToByteArray(), value.encodeToByteArray()).decodeToString())
        return if (reply.optBoolean("ok")) null else reply.optString("error")
    }

    /** Android's time zones, all in one file (since Android 10 in its own module, before in the
     *  system); the core reads it as it is. */
    private fun zoneDatabase(): String? =
        listOf("/apex/com.android.tzdata/etc/tz/tzdata", "/system/usr/share/zoneinfo/tzdata")
            .firstOrNull { File(it).canRead() }

    /** Miles and pounds: the region's measurement system (Android 9+), else the US, Liberia, Myanmar. */
    private fun imperial(): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            LocaleData.getMeasurementSystem(ULocale.getDefault()) == LocaleData.MeasurementSystem.US
        } else {
            Locale.getDefault().country in setOf("US", "LR", "MM")
        }
}
