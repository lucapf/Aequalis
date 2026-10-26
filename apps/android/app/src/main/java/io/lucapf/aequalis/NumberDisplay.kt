package io.lucapf.aequalis

import java.text.DecimalFormatSymbols

/** The device's separators (the ones the core is told at its start, `Core.start`): `.` and `,` in
 *  English, `,` and `.` in Italian. */
object NumberSeparators {
    val decimal: String by lazy { DecimalFormatSymbols.getInstance().decimalSeparator.toString() }
    val thousands: String by lazy { DecimalFormatSymbols.getInstance().groupingSeparator.toString() }
}

/**
 * The request as it is shown: its whole numbers of four digits or more grouped by the device's
 * thousands separator, its decimal point written as the device's (`1000+2500.5` is shown `1,000+2,500.5`).
 * What is written stays as typed (the core reads `1000`); the cursor and a tap are mapped between
 * the two.
 */
class Shown(val text: String, private val toShown: IntArray, private val toText: IntArray) {
    /** Where the cursor at `cursor` (an index in the request) is drawn in `text`. */
    fun at(cursor: Int): Int = toShown[cursor.coerceIn(0, toShown.size - 1)]

    /** The index in the request of a place in `text` (a tap): the separator inserted belongs to the digit before it. */
    fun offset(shown: Int): Int = toText[shown.coerceIn(0, toText.size - 1)]
}

/** What is not a number to group: a date (`2026-12-04`, `25-10-2025`, `12/04/2026`, `12.04.2026`) or a
 *  time (`10:00`). */
private val NOT_NUMBERS = listOf(
    Regex("""\d{4}-\d{1,2}-\d{1,2}"""),
    Regex("""\d{1,2}-\d{1,2}-\d{1,4}"""),
    Regex("""\d{1,4}/\d{1,2}/\d{1,4}"""),
    Regex("""\d{1,4}\.\d{1,2}\.\d{1,4}"""),
    Regex("""\d+(?::\d+)+"""),
)

/** A year after a month's name, a day between or not: `Jan 2025`, `30 Jan 2025`, `Jan 30, 2025`,
 *  `30/Jan/2025`. The name is the first group, the year the second. */
private val AFTER_A_MONTH = Regex("""(\p{L}{3,})\.?[\s/-]+(?:\d{1,2}(?:st|nd|rd|th)?,?[\s/-]+)?(\d{4})(?!\d)""")

/** Month names as the core reads them (src/time/written.rs `MONTH_NAMES`), lowercase without accents;
 *  a word of 3 letters or more that begins one is a month (`Sep`, `janv`, `giu`). */
private val MONTHS = listOf(
    "january", "enero", "janvier", "gennaio", "januar", "janner", "janeiro",
    "february", "febrero", "fevrier", "febbraio", "februar", "feber", "fevereiro",
    "march", "marzo", "mars", "marz", "marco", "mrz",
    "april", "abril", "avril", "aprile",
    "may", "mayo", "mai", "maggio", "maio",
    "june", "junio", "juin", "giugno", "juni", "junho",
    "july", "julio", "juillet", "luglio", "juli", "julho",
    "august", "agosto", "aout",
    "september", "septiembre", "setiembre", "septembre", "settembre", "setembro",
    "october", "octubre", "octobre", "ottobre", "oktober", "outubro",
    "november", "noviembre", "novembre", "novembro",
    "december", "diciembre", "decembre", "dicembre", "dezember", "dezembro",
)

private fun isMonth(word: String): Boolean {
    val folded = java.text.Normalizer.normalize(word.lowercase(), java.text.Normalizer.Form.NFD).filter { it in 'a'..'z' }
    return folded.length >= 3 && MONTHS.any { it.startsWith(folded) }
}

fun showNumbers(text: String, decimal: String = NumberSeparators.decimal, thousands: String = NumberSeparators.thousands): Shown {
    val kept = BooleanArray(text.length)
    NOT_NUMBERS.forEach { re -> re.findAll(text).forEach { m -> for (i in m.range) kept[i] = true } }
    // A year is no number to group either.
    AFTER_A_MONTH.findAll(text).forEach { m ->
        val year = m.groups[2]!!.range
        if (isMonth(m.groupValues[1])) for (i in year) kept[i] = true
    }
    val out = StringBuilder()
    // `boundary[p]`: the length shown for the first `p` characters of the request.
    val boundary = IntArray(text.length + 1)
    var i = 0
    while (i < text.length) {
        val c = text[i]
        val before = if (i > 0) text[i - 1] else ' '
        when {
            c.isAsciiDigit() && !kept[i] -> {
                var end = i
                while (end < text.length && text[end].isAsciiDigit() && !kept[end]) end++
                // The digits of a fraction (`.5000`), an exponent (`1E5000`), a word's (`h1000`) stay as they are.
                val whole = !(before == '.' || before.isLetter() || (before in "+-" && i >= 3 && text[i - 2] in "eE" && text[i - 3].isAsciiDigit()))
                val group = whole && end - i >= 4 && thousands.isNotEmpty()
                for (k in i until end) {
                    if (group && k > i && (end - k) % 3 == 0) out.append(thousands)
                    out.append(text[k])
                    boundary[k + 1] = out.length
                }
                i = end
                continue
            }
            // A decimal point of a number (not a letter's, as in `St.`, nor a date's).
            c == '.' && !kept[i] && !before.isLetter() && (before.isAsciiDigit() || text.getOrNull(i + 1)?.isAsciiDigit() == true) -> out.append(decimal)
            else -> out.append(c)
        }
        boundary[i + 1] = out.length
        i++
    }
    // Back: the request's place of each place shown.
    val toText = IntArray(out.length + 1)
    var p = 0
    for (s in 0..out.length) {
        while (p < text.length && boundary[p + 1] <= s) p++
        toText[s] = p
    }
    return Shown(out.toString(), boundary, toText)
}

private fun Char.isAsciiDigit() = this in '0'..'9'
