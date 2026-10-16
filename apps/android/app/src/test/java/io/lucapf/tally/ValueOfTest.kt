package io.lucapf.tally

import org.junit.Assert.assertEquals
import org.junit.Test

/** What Copy and Share give of a result: the value only. */
class ValueOfTest {
    @Test
    fun theNumberOfAMeasure() {
        assertEquals("7.4565", valueOf("7.4565 mi"))
        assertEquals("8.94", valueOf("8.94 EUR"))
        assertEquals("-40", valueOf("-40 F"))
        assertEquals("1,107", valueOf("1,107 km"))
        assertEquals("14", valueOf("14"))
    }

    @Test
    fun anythingElseAsShown() {
        assertEquals("16 Oct 26 4:00 AM GMT+9 Tokyo", valueOf("16 Oct 26 4:00 AM GMT+9 Tokyo"))
        // A date in a pattern, a timestamp: without the pattern's name, as a measure without its unit.
        assertEquals("2026/12/11", valueOf("2026/12/11 YYYY/MM/DD"))
        assertEquals("1796338800", valueOf("1796338800 unix"))
    }
}
