package io.lucapf.aequalis

import android.content.Context
import android.icu.util.LocaleData
import android.icu.util.ULocale
import android.os.Build
import org.json.JSONObject
import java.io.File
import java.text.DecimalFormatSymbols
import java.util.Locale
import java.util.TimeZone

/** A choice of an ambiguous request, as the core words it: its button's label, more about it (a
 *  place's population and map; may be empty), and the request that says it exactly. */
data class Choice(val label: String, val detail: String, val request: String)

/** A word offered while typing: what is written, and how it is shown (`km - kilometre`). */
data class Suggestion(val write: String, val label: String)

/** The words offered for the word being typed, which starts at `start` (an index in the text). */
data class Suggestions(val start: Int, val items: List<Suggestion>)

/** A unit of a class of the Measures panel: its symbol (what is written), name and system. */
data class MeasureUnit(val symbol: String, val name: String, val system: String)

/** A class of units (`Length`): its two systems' names (`Metric`, `Imperial`) and its units. */
data class Measure(val name: String, val systems: List<String>, val units: List<MeasureUnit>)

/** A currency: its code (what is written), name and symbols; the countries that use it, and their
 *  other names (`USA`), for the search. */
data class Currency(
    val code: String,
    val name: String,
    val symbols: List<String>,
    val countries: List<String> = emptyList(),
    val aliases: List<String> = emptyList(),
)

/** A line of a Trivia value's details (`Current distance`, `25,800,920,358 km - 172.4685 au`), and
 *  its measures: (as shown, as written into the request), one per unit. */
data class TriviaLine(val label: String, val text: String, val measures: List<Pair<String, String>>)

/** A Trivia value: its symbol (`♃`), name, a few words, its value now, what it writes into the
 *  request, and its details, as the TUI's popup shows them: a description, then lines. */
data class TriviaEntry(
    val symbol: String,
    val name: String,
    val short: String,
    val value: String,
    val written: String,
    val description: String,
    val details: List<TriviaLine>,
)

data class TriviaSection(val name: String, val entries: List<TriviaEntry>)

/** A setting: its key, value and default; `kind` is `bool`, `choice` (one of `options`) or `text`. */
data class Setting(val key: String, val value: String, val default: String, val kind: String, val options: List<String>, val about: String)

/** What the screen shows of a reply: the core decides it (`json::view`, src/json.rs). */
enum class Show { Result, Hint, Question, Error, None }

/** The core's answer to a request, ready to show: nothing in it is worked out here. */
data class Reply(
    val show: Show = Show.None,
    /** With `Show.Result`: the result, and its value for Copy and Share (`7.4565`). */
    val result: String? = null,
    val value: String? = null,
    /** The hint, the question or the error. */
    val message: String = "",
    val choices: List<Choice> = emptyList(),
    /** The request ⏎ picks, on a question. */
    val default: String? = null,
    /** With a currency result: `rates 2d old`. */
    val rates: String? = null,
    /** Notes and warnings, under the result. */
    val details: List<String> = emptyList(),
    /** A calculation's grouping, every operation in brackets (`(12 + (4 / 2))`). */
    val grouping: String? = null,
    /** The Measures class of the request's unit (`Length`): the Measures open at it. */
    val unitClass: String? = null,
)

/** About: the app's name, tagline and core version, the facts (online data, last sync) and the
 *  data's sources, each a (label, text). */
data class About(val name: String, val tagline: String, val version: String, val facts: List<Pair<String, String>>, val sources: List<Pair<String, String>>)

/**
 * The bridge to aequalis's Rust core: libaequalis.so (built by scripts/build-android.sh), whose
 * functions src/jni.rs exports. Text crosses as UTF-8 bytes; replies are JSON, as `aequalis --json`.
 */
object Core {
    init {
        // Loads lib/<abi>/libaequalis.so from the app: the x86_64 one on the emulator, the arm64 or the
        // 32-bit ARM one on phones.
        System.loadLibrary("aequalis")
    }

    // Implemented in Rust (src/jni.rs): `Java_io_lucapf_aequalis_Core_init`, `…_eval`, `…_suggest`,
    // `…_lists`, `…_set`, `…_seed`.
    private external fun init(config: ByteArray): ByteArray
    private external fun eval(request: ByteArray, submitted: Boolean): ByteArray
    private external fun suggest(before: ByteArray): ByteArray
    private external fun lists(which: ByteArray): ByteArray
    private external fun set(key: ByteArray, value: ByteArray): ByteArray
    private external fun seed(rates: ByteArray, cities: ByteArray): ByteArray

    private val launched = java.util.concurrent.atomic.AtomicBoolean(false)

    /** The longest request, in characters (code points): the core's limit, from its start; a long
     *  paste is cut to it. Until the core answers, its usual value. */
    @Volatile
    var maxInput = 512
        private set

    /** Open once `start` has run: every call into the core waits for it. */
    private val started = java.util.concurrent.CountDownLatch(1)

    /**
     * Starts the core in a thread of its own, so that the first frame doesn't wait for it: loading
     * libaequalis.so and asking Android about the device (about 10 ms on a Redmi 9A, more when the
     * phone is busy), while the first request comes later. Then, at a low priority so as not to slow
     * the screen, the data shipped with the app (seconds, after an install). Once per process: the
     * activity can start several times (a rotation, a theme change).
     */
    fun startInBackground(context: Context) {
        if (!launched.compareAndSet(false, true)) return
        val app = context.applicationContext
        Thread {
            start(app)
            android.os.Process.setThreadPriority(android.os.Process.THREAD_PRIORITY_BACKGROUND)
            seedShipped(app)
        }.start()
    }

    /** Tells the core what it can't find on a phone: where to keep its files, the time zone, the
     *  zone database, the number format and the measurement system. */
    private fun start(context: Context) {
        try {
            val symbols = DecimalFormatSymbols.getInstance()
            val config = JSONObject()
                .put("dir", context.filesDir.absolutePath)
                .put("zone", TimeZone.getDefault().id)
                .put("decimal", symbols.decimalSeparator.toString())
                .put("thousands", symbols.groupingSeparator.toString())
                .put("imperial", imperial())
            zoneDatabase()?.let { config.put("zoneinfo", it) }
            val reply = JSONObject(init(config.toString().encodeToByteArray()).decodeToString())
            check(reply.optBoolean("ok")) { "aequalis: ${reply.optString("error")}" }
            maxInput = reply.optInt("max_input", maxInput)
        } finally {
            // Even when it fails (the app then stops, as before), nothing waits forever.
            started.countDown()
        }
    }

    /** Installs the online data shipped in the app (assets/, from scripts/build-android.sh): the
     *  exchange rates, kept only if newer than the ones there; the city archive, only if there is
     *  none yet (the core reads a file: the asset copied to the cache, then removed). Off the main
     *  thread: converting the archive takes a few seconds. */
    private fun seedShipped(context: Context) {
        val assets = context.assets
        val shipped = assets.list("").orEmpty()
        val rates = if ("rates.json" in shipped) assets.open("rates.json").use { it.readBytes() } else ByteArray(0)
        // The core's own file (files/aequalis/cities.json, src/files.rs) says the archive is installed:
        // no copy needed.
        val copy = File(context.cacheDir, "cities.json")
        val cities = if ("cities.json" in shipped && !File(context.filesDir, "aequalis/cities.json").exists()) {
            assets.open("cities.json").use { input -> copy.outputStream().use { input.copyTo(it) } }
            copy.absolutePath
        } else ""
        try {
            val reply = JSONObject(seed(rates, cities.encodeToByteArray()).decodeToString())
            if (!reply.optBoolean("ok")) android.util.Log.w("aequalis", "shipped data: ${reply.optString("error")}")
        } finally {
            copy.delete()
        }
    }

    /** The core's answer to `request`, as typed; `submitted`: ⏎ was pressed on it. */
    fun evaluate(request: String, submitted: Boolean): Reply {
        started.await()
        return reply(JSONObject(eval(request.encodeToByteArray(), submitted).decodeToString()))
    }

    /** The reply's fields, as the core sends them. */
    private fun reply(json: JSONObject): Reply {
        val text = { key: String -> json.optString(key).ifEmpty { null } }
        return Reply(
            show = when (json.optString("show")) {
                "result" -> Show.Result
                "hint" -> Show.Hint
                "question" -> Show.Question
                "error" -> Show.Error
                else -> Show.None
            },
            result = text("result"),
            value = text("value"),
            message = json.optString("message"),
            choices = json.optJSONArray("choices")?.objects { Choice(it.getString("label"), it.getString("detail"), it.getString("request")) }.orEmpty(),
            default = text("default"),
            rates = text("rates"),
            details = json.optJSONArray("details")?.strings().orEmpty(),
            grouping = text("grouping"),
            unitClass = text("class"),
        )
    }

    /** The words to offer for the word being typed at the end of `before` (the text up to the
     *  cursor), from its third letter: units, currencies, places, formats. */
    fun suggestions(before: String): Suggestions {
        started.await()
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

    private fun list(which: String): JSONObject {
        started.await()
        return JSONObject(lists(which.encodeToByteArray()).decodeToString())
    }

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
        Currency(c.getString("code"), c.getString("name"), c.getJSONArray("symbols").strings(), c.getJSONArray("countries").strings(), c.getJSONArray("aliases").strings())
    }

    /** The Trivia, by section. */
    fun trivia(): List<TriviaSection> = list("trivia").getJSONArray("sections").objects { s ->
        TriviaSection(s.getString("name"), s.getJSONArray("entries").objects { e ->
            val popup = e.getJSONArray("popup")
            TriviaEntry(
                e.getString("symbol"), e.getString("name"), e.getString("short"), e.getString("value"), e.getString("written"), e.getString("description"),
                (0 until popup.length()).map { i ->
                    val line = popup.getJSONArray(i)
                    val measures = line.optJSONArray(2)
                    TriviaLine(line.getString(0), line.getString(1), (0 until (measures?.length() ?: 0)).map { m ->
                        measures!!.getJSONArray(m).let { it.getString(0) to it.getString(1) }
                    })
                },
            )
        })
    }

    /** About Aequalis: the version, the online data, the sources. */
    fun about(): About = list("about").let { a ->
        // `[[label, text], …]`
        val pairs = { key: String ->
            val list = a.getJSONArray(key)
            (0 until list.length()).map { i -> list.getJSONArray(i).let { it.getString(0) to it.getString(1) } }
        }
        About(a.getString("name"), a.getString("tagline"), a.getString("version"), pairs("facts"), pairs("sources"))
    }

    /** The bytes the Rust core holds now and the most it has held (its allocator counts them,
     *  src/alloc.rs); null if they aren't counted. */
    fun memory(): Pair<Long, Long>? = list("memory").let { if (it.has("held")) it.getLong("held") to it.getLong("peak") else null }

    /** Every setting, in order. */
    fun settings(): List<Setting> = list("settings").getJSONArray("settings").objects { s ->
        Setting(s.getString("key"), s.getString("value"), s.getString("default"), s.getString("kind"), s.getJSONArray("options").strings(), s.getString("about"))
    }

    /** Changes a setting and saves it (`""`: its default); the error, or null. */
    fun change(key: String, value: String): String? {
        started.await()
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
