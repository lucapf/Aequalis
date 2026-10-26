package io.lucapf.aequalis

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CalendarTest {
    private val december = CalendarMonth("December 2026", 2026, 12, listOf(CalendarWeek(49, listOf(0, 1, 2, 3, 4, 5, 6))))
    private val calendar = Calendar(
        title = "December 2026",
        year = false,
        today = "2026-12-02",
        weekdays = listOf("Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"),
        weekend = setOf(5, 6),
        countries = listOf("AE" to "United Arab Emirates", "US" to "United States", "IT" to "Italy"),
        months = listOf(december),
        holidays = listOf(
            CalendarHoliday("2026-12-02", 2, 12, 0, "National Day", "2026-12-02", 2, "2-3 Dec"),
            CalendarHoliday("2026-12-03", 3, 12, 0, "National Day", "2026-12-02", 2, "2-3 Dec"),
            CalendarHoliday("2026-12-25", 25, 12, 1, "Christmas Day"),
            CalendarHoliday("2026-12-25", 25, 12, 2, "Natale"),
        ),
    )

    @Test
    fun aDaysHolidaysByCountry() {
        assertEquals(listOf(0), calendar.holidayOf(12, 2))
        assertEquals(listOf(1, 2), calendar.holidayOf(12, 25))
        assertTrue(calendar.holidayOf(12, 4).isEmpty())
        assertTrue(calendar.holidayOf(11, 25).isEmpty())
    }

    @Test
    fun aTappedDaysHolidaysAndHowLong() {
        val christmas = calendar.holidaysOn(12, 25)
        assertEquals(listOf("Christmas Day", "Natale"), christmas.map { it.name })
        assertEquals(listOf(1, 2), christmas.map { it.country })
        assertEquals("1 day", lasting(christmas[0]))
        assertEquals("2 days · 2-3 Dec", lasting(calendar.holidaysOn(12, 3).single()))
        // The list: a holiday once, its days together.
        assertEquals(listOf("National Day", "Christmas Day", "Natale"), calendar.listed().map { it.name })
    }

    @Test
    fun aSwipeAcrossNotDown() {
        assertEquals(1, swipeStep(-200f, 10f, 100f))
        assertEquals(-1, swipeStep(200f, -10f, 100f))
        assertEquals(0, swipeStep(-60f, 0f, 100f))
        assertEquals(0, swipeStep(-200f, 150f, 100f))
    }

    @Test
    fun today() {
        assertTrue(calendar.isToday(december, 2))
        assertFalse(calendar.isToday(december, 3))
    }

    @Test
    fun aColourEachCountry() {
        val colours = (0 until 5).map { countryColour(it, dark = false) }
        assertEquals(5, colours.toSet().size)
        assertEquals(countryColour(0, false), countryColour(5, false))
        assertNotEquals(countryColour(0, false), countryColour(0, true))
    }
}
