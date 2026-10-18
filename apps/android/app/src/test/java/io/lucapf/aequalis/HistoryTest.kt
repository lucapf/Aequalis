package io.lucapf.aequalis

import org.junit.Assert.assertEquals
import org.junit.Test

/** The history: the newest first, a request once, at most `cap` entries, and its file. */
class HistoryTest {
    private fun e(request: String, result: String = "1", time: Long = 0) = HistoryEntry(request, result, time)

    @Test
    fun theNewestFirstAndARequestOnce() {
        var h = listOf<HistoryEntry>()
        h = added(h, e("12+3", "15"))
        h = added(h, e("10 km mi", "6.2137 mi"))
        assertEquals(listOf("10 km mi", "12+3"), h.map { it.request })
        // Done with again: at the top, with its new result.
        h = added(h, e("12+3", "15", time = 9))
        assertEquals(listOf("12+3", "10 km mi"), h.map { it.request })
        assertEquals(9L, h.first().time)
    }

    @Test
    fun theOldestDroppedPastTheCap() {
        val h = listOf("a", "b", "c").fold(listOf<HistoryEntry>()) { list, r -> added(list, e(r), cap = 2) }
        assertEquals(listOf("c", "b"), h.map { it.request })
    }

    @Test
    fun theFileReadBackAsWritten() {
        val h = listOf(
            HistoryEntry("15 Italy UAE", "61.59 AED", 1_792_000_000_000),
            // Tabs, line breaks and backslashes don't break the lines.
            HistoryEntry("a\tb\\n", "line\none\r", 5),
        )
        assertEquals(h, historyEntries(historyText(h)))
        // The result's value, for Copy, kept with it.
        val valued = listOf(HistoryEntry("10 km mi", "6.2137 mi", 3, "6.2137"))
        assertEquals(valued, historyEntries(historyText(valued)))
    }

    @Test
    fun anOlderFileWithoutValues() {
        // Three fields: the result stands for the value.
        assertEquals(listOf(HistoryEntry("10 km mi", "6.2137 mi", 7, "6.2137 mi")), historyEntries("7\t10 km mi\t6.2137 mi"))
    }

    @Test
    fun aLineNotUnderstoodLeftOut() {
        val text = "7\t12+3\t15\nnot a line\n8\t\t1\n9\t2*3\t6"
        assertEquals(listOf(HistoryEntry("12+3", "15", 7), HistoryEntry("2*3", "6", 9)), historyEntries(text))
        assertEquals(emptyList<HistoryEntry>(), historyEntries(""))
    }
}
