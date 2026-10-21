package io.lucapf.aequalis

import org.junit.Assert.assertTrue
import org.junit.Test

/** What Share sends from About. */
class AboutTest {
    @Test
    fun theTaglineThenWhatItDoesThenTheLinks() {
        val text = installText("Type it. Get it.", "Maths, units, currencies and time, answered as you type.")
        assertTrue(text, text.startsWith("Aequalis — Type it. Get it.\nMaths, units, currencies and time, answered as you type.\n\n"))
        assertTrue(text, text.contains("https://"))
    }
}
