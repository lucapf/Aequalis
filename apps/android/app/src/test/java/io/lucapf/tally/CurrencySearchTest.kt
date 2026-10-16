package io.lucapf.tally

import org.junit.Assert.assertEquals
import org.junit.Test

/** The Currencies panel's search. */
class CurrencySearchTest {
    private val all = listOf(
        Currency("USD", "US dollar", listOf("$", "US$")),
        Currency("EUR", "Euro", listOf("€")),
        Currency("JPY", "Japanese yen", listOf("¥")),
        Currency("CHF", "Swiss franc", listOf("Fr.")),
        Currency("INR", "Indian rupee", listOf("₹")),
        Currency("BRL", "Brazilian real", listOf("R$")),
        Currency("XOF", "West African CFA franc", listOf("CFA")),
        Currency("SEK", "Swedish krona", listOf("kr")),
        Currency("NOK", "Norwegian krone", listOf("kr")),
    )

    private fun codes(query: String) = search(all, query).map { it.code }

    @Test
    fun anEmptySearchShowsThemAll() {
        assertEquals(all, search(all, ""))
        assertEquals(all, search(all, "  "))
    }

    @Test
    fun byCodeNameWordOrSymbol() {
        assertEquals(listOf("JPY"), codes("yen"))
        assertEquals(listOf("EUR"), codes("EUR"))
        assertEquals(listOf("INR"), codes("₹"))
        assertEquals(listOf("BRL"), codes("brazil"))
        assertEquals(listOf("SEK", "NOK"), codes("kr"))
    }

    @Test
    fun theCodeFirstThenANameWordThenAnywhere() {
        // CHF by its code; the two francs by a word of their names.
        assertEquals(listOf("CHF"), codes("ch"))
        assertEquals(listOf("CHF", "XOF"), codes("franc"))
        // `us`: USD's code, then "US dollar"… none else begins a word with it.
        assertEquals("USD", codes("us").first())
    }

    @Test
    fun caseAndAccentsIgnored() {
        assertEquals(listOf("BRL"), codes("RÉAL"))
        assertEquals(listOf("SEK"), codes("swed"))
    }

    @Test
    fun nothingFound() {
        assertEquals(emptyList<String>(), codes("zzz"))
    }
}
