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

/** The core's answer to a request: a result, an error, or a question with its choices. */
data class Reply(
    val result: String? = null,
    val error: String? = null,
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

    // Implemented in Rust (src/jni.rs): `Java_io_lucapf_tally_Core_init`, `…_eval`, `…_suggest`.
    private external fun init(config: ByteArray): ByteArray
    private external fun eval(request: ByteArray): ByteArray
    private external fun suggest(before: ByteArray): ByteArray

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
