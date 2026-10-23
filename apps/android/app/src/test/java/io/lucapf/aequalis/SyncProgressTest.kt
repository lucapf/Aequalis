package io.lucapf.aequalis

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream

/** A sync's progress and speed, as Settings shows them. */
class SyncProgressTest {
    @Test
    fun speedsAsBrowsersWriteThem() {
        assertEquals("800 B/s", speedText(800))
        assertEquals("400 kB/s", speedText(400_123))
        assertEquals("1.2 MB/s", speedText(1_234_567))
    }

    @Test
    fun aBigDownloadSaysHowFarItIs() {
        val data = ByteArray(300_000) { it.toByte() }
        val out = ByteArrayOutputStream()
        SyncProgress.start()
        SyncProgress.copy(ByteArrayInputStream(data), out, data.size.toLong())
        SyncProgress.stop()
        assertEquals(data.size, out.size())
        assertEquals(1f, SyncProgress.fraction ?: 0f, 0.3f)
    }

    @Test
    fun aSmallOneLeavesTheBar() {
        SyncProgress.start()
        SyncProgress.copy(ByteArrayInputStream(ByteArray(10_000)), ByteArrayOutputStream(), 10_000)
        SyncProgress.stop()
        assertNull(SyncProgress.fraction)
    }
}
