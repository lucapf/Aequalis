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
    fun notOtherAnswers() {
        // One person, a currency with the sizes beside it, a unit.
        assertNull(peopleOf(Reply(show = Show.Result, result = "US 8.5 (Nike), 9 (Clarks)")))
        assertNull(peopleOf(Reply(show = Show.Result, result = "47.18 USD", extra = listOf(Extra("shoes", listOf("US 8.5 men", "US 10 women"))))))
        assertNull(peopleOf(Reply(show = Show.Result, result = "6.2137 mi")))
    }
}
