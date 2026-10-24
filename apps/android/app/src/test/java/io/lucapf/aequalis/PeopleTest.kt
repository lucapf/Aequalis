package io.lucapf.aequalis

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The answers that name no person, shown as a list of people, all alike. */
class PeopleTest {
    @Test
    fun theSizesBesideTheFirst() {
        // `42 eu shoes us`: the first person's size the result, the others beside it.
        val reply = Reply(show = Show.Result, result = "US 8.5 men", extra = listOf(Extra("shoes", listOf("US 10 women", "US 15.5 kids (Clarks)"))))
        assertEquals(listOf("Men" to "US 8.5", "Women" to "US 10", "Kids" to "US 15.5 (Clarks)"), peopleOf(reply))
    }

    @Test
    fun theTablesRows() {
        // `shoes 42`: a row per person.
        val reply = Reply(
            show = Show.Result,
            result = "         US    UK\nmen     8.5   7.5\nwomen  10     7.5",
            extra = listOf(Extra("shoes men", listOf("US 8.5", "UK 7.5")), Extra("shoes women", listOf("US 10", "UK 7.5"))),
        )
        assertEquals(listOf("Men" to "US 8.5 · UK 7.5", "Women" to "US 10 · UK 7.5"), peopleOf(reply))
    }

    @Test
    fun theClothesCharts() {
        // `clothes M it`: the first chart's size the result, the others' by their chart.
        val reply = Reply(
            show = Show.Result,
            result = "IT 44 women",
            extra = listOf(Extra("clothes men's suit", listOf("IT 48")), Extra("clothes men's trousers", listOf("IT 48–49"))),
        )
        assertEquals(listOf("Women" to "IT 44", "Men's suit" to "IT 48", "Men's trousers" to "IT 48–49"), peopleOf(reply))
        // `clothes 44 it`: a line per chart, every system.
        val table = Reply(
            show = Show.Result,
            result = "women       US 8 · IT 44\nmen's suit  US 34 · EU 44",
            extra = listOf(Extra("clothes women", listOf("US 8", "IT 44")), Extra("clothes men's suit", listOf("US 34", "EU 44"))),
        )
        assertEquals(listOf("Women" to "US 8 · IT 44", "Men's suit" to "US 34 · EU 44"), peopleOf(table))
    }

    @Test
    fun anotherReadingIsNoPerson() {
        // `42 eu women us`: the clothes size, the women's shoes a line of another reading.
        val reply = Reply(show = Show.Result, result = "US 12", extra = listOf(Extra("shoes", listOf("US 10 women"), from = "EU 42")))
        assertNull(peopleOf(reply))
    }

    @Test
    fun notOtherAnswers() {
        // One person, a currency with the sizes beside it, a unit.
        assertNull(peopleOf(Reply(show = Show.Result, result = "US 8.5 (Nike), 9 (Clarks)")))
        assertNull(peopleOf(Reply(show = Show.Result, result = "47.18 USD", extra = listOf(Extra("shoes", listOf("US 8.5 men", "US 10 women"))))))
        assertNull(peopleOf(Reply(show = Show.Result, result = "6.2137 mi")))
    }
}

/** `now New York Tokyo`: a line per city, the city beside its time. */
class ZonesTest {
    @Test
    fun aLinePerCity() {
        val reply = Reply(show = Show.Result, result = "24 Oct 26 2:41 AM GMT-4 New York\n24 Oct 26 3:41 PM GMT+9 Tokyo\n24 Oct 26 7:41 AM GMT London")
        assertEquals(listOf("New York" to "24 Oct 26 2:41 AM GMT-4", "Tokyo" to "24 Oct 26 3:41 PM GMT+9", "London" to "24 Oct 26 7:41 AM GMT"), zonesOf(reply))
    }

    @Test
    fun anythingElseIsNone() {
        assertNull(zonesOf(Reply(show = Show.Result, result = "24 Oct 26 2:41 AM GMT-4 New York")))
        assertNull(zonesOf(Reply(show = Show.Result, result = "         US    UK\nmen     8.5   7.5")))
    }
}
