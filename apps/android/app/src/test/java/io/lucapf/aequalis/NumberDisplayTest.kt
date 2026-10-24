package io.lucapf.aequalis

import org.junit.Assert.assertEquals
import org.junit.Test

/** Numbers grouped as the device writes them while the request is typed. */
class NumberDisplayTest {
    private fun shown(text: String, decimal: String = ".", thousands: String = ",") = showNumbers(text, decimal, thousands).text

    @Test
    fun wholeNumbersAreGrouped() {
        assertEquals("1,000", shown("1000"))
        assertEquals("999", shown("999"))
        assertEquals("1,234,567", shown("1234567"))
        assertEquals("1,000+2,500", shown("1000+2500"))
        assertEquals("1,000km mi", shown("1000km mi"))
        assertEquals("(1,000 + 20,000) * 3", shown("(1000 + 20000) * 3"))
    }

    @Test
    fun theDevicesSeparators() {
        assertEquals("1.234,5678", shown("1234.5678", decimal = ",", thousands = "."))
        assertEquals("1,234.5678", shown("1234.5678"))
        assertEquals("1 234", shown("1234", thousands = " "))
        assertEquals("1234", shown("1234", thousands = ""))
        assertEquals("5,", shown("5.", decimal = ","))
        assertEquals(",5", shown(".5", decimal = ","))
    }

    @Test
    fun nothingElseIsTouched() {
        // Fractions, exponents, dates, times, words.
        assertEquals("1.23456", shown("1.23456"))
        assertEquals("0.12345", shown("0.12345"))
        assertEquals("1E5000", shown("1E5000"))
        assertEquals("1e-5000", shown("1e-5000"))
        assertEquals("2026-12-04", shown("2026-12-04"))
        assertEquals("12/04/2026", shown("12/04/2026"))
        assertEquals("12.04.2026", shown("12.04.2026", decimal = ","))
        assertEquals("10:30", shown("10:30"))
        assertEquals("10:00:0000", shown("10:00:0000"))
        assertEquals("St. Louis", shown("St. Louis", decimal = ","))
        assertEquals("h1000", shown("h1000"))
        assertEquals("18E6 unix", shown("18E6 unix"))
        assertEquals("1,791,644,400 unix", shown("1791644400 unix"))
    }

    @Test
    fun theCursorAndTheTapFollow() {
        val s = showNumbers("1000+2", ".", ",")
        assertEquals("1,000+2", s.text)
        // The cursor before the `+` (4 in the request) is drawn after `1,000`.
        assertEquals(5, s.at(4))
        assertEquals(0, s.at(0))
        assertEquals(7, s.at(6))
        // A tap after `1,000` is the request's 4; before the separator, 1; on the separator, 1.
        assertEquals(4, s.offset(5))
        assertEquals(1, s.offset(1))
        assertEquals(1, s.offset(2))
        assertEquals(2, s.offset(3))
        assertEquals(6, s.offset(7))
    }

    @Test
    fun emptyAndSmall() {
        val s = showNumbers("", ".", ",")
        assertEquals("", s.text)
        assertEquals(0, s.at(0))
        assertEquals(0, s.offset(0))
    }
}
