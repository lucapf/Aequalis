package io.lucapf.tally

import org.junit.Assert.assertEquals
import org.junit.Test

/** The Measures panel opens at the class of the request's unit. */
class ClassOfTest {
    private val measures = listOf(
        Measure("Data", listOf("Bytes", "Bits"), listOf(MeasureUnit("GB", "gigabyte", "metric"), MeasureUnit("Gb", "gigabit", "metric"))),
        Measure("Length", listOf("Metric", "Imperial"), listOf(MeasureUnit("cm", "centimetre", "metric"), MeasureUnit("in", "inch", "imperial"))),
        Measure("Mass", listOf("Metric", "Imperial"), listOf(MeasureUnit("g", "gram", "metric"))),
    )

    @Test
    fun theUnitAfterTheValue() {
        assertEquals("Length", classOf("10 cm ", measures))
        assertEquals("Length", classOf("10 cm in", measures))
        assertEquals("Data", classOf("1 GB", measures))
        // A case no unit has: any case.
        assertEquals("Data", classOf("1 gb", measures))
        assertEquals("Length", classOf("(1+2) CM", measures))
    }

    @Test
    fun noValueOrNoUnit() {
        assertEquals(null, classOf("", measures))
        assertEquals(null, classOf("10 ", measures))
        assertEquals(null, classOf("12 EUR in", measures))
        assertEquals(null, classOf("Rome Paris", measures))
    }
}
