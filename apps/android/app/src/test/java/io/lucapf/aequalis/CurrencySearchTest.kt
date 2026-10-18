package io.lucapf.aequalis

import org.junit.Assert.assertEquals
import org.junit.Test

/** The Currencies panel's search. */
class CurrencySearchTest {
    private val all = listOf(
        Currency("USD", "US dollar", listOf("$", "US$"), listOf("United States", "Ecuador"), listOf("USA", "America")),
        Currency("EUR", "Euro", listOf("€"), listOf("Italy", "France", "Germany")),
        Currency("JPY", "Japanese yen", listOf("¥")),
        Currency("CHF", "Swiss franc", listOf("Fr.")),
        Currency("INR", "Indian rupee", listOf("₹")),
        Currency("BRL", "Brazilian real", listOf("R$")),
        Currency("XOF", "West African CFA franc", listOf("CFA"), listOf("Senegal", "Côte d'Ivoire")),
        Currency("AED", "UAE dirham", listOf("د.إ"), listOf("United Arab Emirates"), listOf("UAE")),
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
        // CHF by its code; the two francs by a word of their names, then the euro for France.
        assertEquals(listOf("CHF"), codes("ch"))
        assertEquals(listOf("CHF", "XOF", "EUR"), codes("franc"))
        // `us`: USD's code, then "US dollar"… none else begins a word with it.
        assertEquals("USD", codes("us").first())
    }

    @Test
    fun caseAndAccentsIgnored() {
        assertEquals(listOf("BRL"), codes("RÉAL"))
        assertEquals(listOf("SEK"), codes("swed"))
    }

    /** By a country that uses it, as requests do (`15 Italy UAE`), and the country shown. */
    @Test
    fun byCountry() {
        assertEquals(listOf("EUR"), codes("italy"))
        assertEquals(listOf("Italy"), countriesMatched(all[1], "ital"))
        // Every currency of a country beginning with it, the most used first.
        assertEquals(listOf("USD", "AED"), codes("united"))
        assertEquals(listOf("United Arab Emirates"), countriesMatched(all.first { it.code == "AED" }, "united"))
        // Another name of a country: shown as written.
        assertEquals(listOf("USD"), codes("america"))
        assertEquals(listOf("America"), countriesMatched(all[0], "america"))
        // Accents ignored: `cote` finds Côte d'Ivoire.
        assertEquals(listOf("XOF"), codes("cote"))
        // Found by its own name or code: no country to show.
        assertEquals(emptyList<String>(), countriesMatched(all[1], "euro"))
        assertEquals(emptyList<String>(), countriesMatched(all.first { it.code == "AED" }, "uae"))
    }

    @Test
    fun nothingFound() {
        assertEquals(emptyList<String>(), codes("zzz"))
    }
}
