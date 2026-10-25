package io.lucapf.aequalis

import androidx.work.NetworkType
import java.time.Duration
import java.time.ZoneId
import java.time.ZonedDateTime
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The night's download: when it first runs, and what it needs. */
class NightlySyncTest {
    private fun at(date: String) = ZonedDateTime.parse("${date}[Europe/Rome]")

    @Test
    fun theNextNightIsTodaysWhenItIsAhead() {
        assertEquals(Duration.ofMinutes(150), NightlySync.untilNight(at("2026-10-20T00:00:00+02:00")))
        assertEquals(Duration.ofHours(1), NightlySync.untilNight(at("2026-10-20T01:30:00+02:00")))
    }

    @Test
    fun otherwiseTomorrows() {
        assertEquals(Duration.ofHours(24), NightlySync.untilNight(at("2026-10-20T02:30:00+02:00")))
        assertEquals(Duration.ofMinutes(60 * 23 + 30), NightlySync.untilNight(at("2026-10-20T03:00:00+02:00")))
        assertEquals(Duration.ofHours(7).plusMinutes(30), NightlySync.untilNight(at("2026-10-20T19:00:00+02:00")))
    }

    @Test
    fun theClockChangeIsTheClocks() {
        // 25 Oct 2026 03:00 → 02:00 in Italy: the night after is 25 hours away from 02:30.
        val now = ZonedDateTime.of(2026, 10, 25, 2, 30, 0, 0, ZoneId.of("Europe/Rome"))
        assertTrue(NightlySync.untilNight(now) >= Duration.ofHours(24))
    }

    @Test
    fun itNeedsWifiAndABatteryNotLow() {
        val constraints = NightlySync.constraints()
        assertEquals(NetworkType.UNMETERED, constraints.requiredNetworkType)
        assertTrue(constraints.requiresBatteryNotLow())
        assertTrue(!constraints.requiresCharging())
    }
}
