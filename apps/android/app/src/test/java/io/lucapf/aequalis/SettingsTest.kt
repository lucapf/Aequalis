package io.lucapf.aequalis

import org.junit.Assert.assertEquals
import org.junit.Test

/** Settings as the Omarchy widget shows them: `auto (AED)`, the lists, `online (2h old)`. */
class SettingsTest {
    private val currency = Setting(
        "currency.local", "auto", "auto", "text", emptyList(), "",
        auto = "AED",
        currencies = listOf("USD" to "US dollar", "EUR" to "euro", "AED" to "UAE dirham"),
    )

    @Test
    fun autoSaysWhatItStandsFor() {
        assertEquals("auto (AED)", shownValue("auto", "AED"))
        assertEquals("auto", shownValue("auto", null))
        assertEquals("EUR", shownValue("EUR", "AED"))
    }

    @Test
    fun theCurrencyListStartsWithAutoAndFilters() {
        assertEquals(listOf("auto", "USD", "EUR", "AED"), pickChoices(currency, "").map { it.first })
        assertEquals(listOf("EUR" to "EUR  euro"), pickChoices(currency, " Eur"))
        assertEquals(listOf("auto", "AED"), pickChoices(currency, "aed").map { it.first })
    }

    @Test
    fun aChoiceListsItsOptions() {
        val unit = Setting("distance.unit", "km", "auto", "choice", listOf("auto", "km", "mi"), "", auto = "km")
        assertEquals(listOf("auto (km)", "km", "mi"), pickChoices(unit, "").map { it.second })
    }

    @Test
    fun atMostEight() {
        val many = currency.copy(currencies = (1..20).map { "C$it" to "currency $it" })
        assertEquals(8, pickChoices(many, "").size)
    }

    @Test
    fun theStatusAndTheAgeOfTheLastSync() {
        assertEquals("online (2h old)", statusText(Online("online", "2h")))
        assertEquals("offline (forced, 2d old)", statusText(Online("offline (forced)", "2d")))
        assertEquals("online (just synced)", statusText(Online("online", "now")))
        assertEquals("never synced", statusText(Online("not synced", null)))
        assertEquals("offline (never synced)", statusText(Online("offline", null)))
    }
}
