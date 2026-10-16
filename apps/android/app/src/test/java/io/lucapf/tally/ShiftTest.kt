package io.lucapf.tally

import org.junit.Assert.assertEquals
import org.junit.Test

/** The keyboard's ⇧: one capital, or locked by two quick taps. */
class ShiftTest {
    @Test
    fun oneCapital() {
        val capitals = Capitals()
        assertEquals("g", capitals.letter('g'))
        capitals.tap(1_000)
        assertEquals(Shift.Once, capitals.shift)
        assertEquals("G", capitals.letter('g'))
        // Off again after the letter: `Gb`.
        assertEquals("b", capitals.letter('b'))
        assertEquals(Shift.Off, capitals.shift)
    }

    @Test
    fun lockedByTwoQuickTaps() {
        val capitals = Capitals()
        capitals.tap(1_000)
        capitals.tap(1_300)
        assertEquals(Shift.Locked, capitals.shift)
        assertEquals("GB", capitals.letter('g') + capitals.letter('b'))
        // Another tap: off.
        capitals.tap(5_000)
        assertEquals(Shift.Off, capitals.shift)
        assertEquals("b", capitals.letter('b'))
    }

    @Test
    fun twoSlowTapsTurnItOff() {
        val capitals = Capitals()
        capitals.tap(1_000)
        capitals.tap(2_000)
        assertEquals(Shift.Off, capitals.shift)
    }
}
